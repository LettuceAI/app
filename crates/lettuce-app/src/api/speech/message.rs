use crate::api::ApiContext;
use crate::api::error::{IntoApiError, api_error, invalid_field, parse_id};
use lettuce_characters::{CharacterRepository, VoicePreference};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_conversations::{
    ConversationKind, ConversationOverviewReader, ConversationReader, MessageRole,
    MessageVisibility, ParticipantSource,
};
use lettuce_speech::{
    AudioProviderConfig, SynthesisRequest, TtsConfigurationRepository, TtsOutputPolicy,
};
use lettuce_types::{AssetId, RequestId};

fn missing_voice() -> ApiError {
    super::errors::speech_error(
        ApiErrorCode::Unavailable,
        dto::SpeechFailure::VoiceMissing,
        "the message has no available voice",
    )
}

pub async fn message_speak(
    context: &ApiContext,
    request: dto::MessageSpeakRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let id: RequestId = parse_id(&request.request_id, "request_id")?;
    let message_id = parse_id(&request.message_id, "message_id")?;
    let digest = super::operations::digest(&request)?;
    context
        .blocking(move |context| {
            let key = format!("message_speak:{id}");
            if let Some(prior) = super::synthesize::replay(context, &key, &digest)? {
                return Ok(prior);
            }
            let database = context.backend().database();
            let conversation_id = database
                .conversation_of_message(message_id)
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(|| api_error(ApiErrorCode::NotFound, "the message was not found"))?;
            let conversation = ConversationReader::get(database, conversation_id)
                .map_err(IntoApiError::into_api_error)?
                .conversation;
            let item = database
                .timeline_anchor(conversation_id, conversation.active_branch_id, message_id)
                .map_err(IntoApiError::into_api_error)?
                .item;
            if item.message.visibility != MessageVisibility::Visible {
                return Err(api_error(
                    ApiErrorCode::NotFound,
                    "the message is not visible",
                ));
            }
            if !matches!(
                item.message.role,
                MessageRole::Assistant | MessageRole::Scene
            ) {
                return Err(invalid_field(
                    "message_id",
                    "this message has no speech playback",
                ));
            }
            let character_id = item
                .message
                .author_participant_id
                .and_then(|id| {
                    conversation
                        .participants
                        .iter()
                        .find(|participant| participant.id == id)
                })
                .and_then(|participant| {
                    if let ParticipantSource::Character(id) = participant.source {
                        Some(id)
                    } else {
                        None
                    }
                })
                .or(
                    if let ConversationKind::Direct(details) = &conversation.kind {
                        Some(details.character.source_id)
                    } else {
                        None
                    },
                )
                .ok_or_else(missing_voice)?;
            let character = CharacterRepository::get(database, character_id)
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(missing_voice)?
                .character;
            let selection = match request.voice_override {
                Some(selection) => selection,
                None => match character.defaults.voice {
                    Some(VoicePreference::VoiceProfile(id)) => {
                        dto::MessageVoiceOverride::UserVoice {
                            voice_id: id.to_string(),
                        }
                    }
                    Some(VoicePreference::Provider {
                        provider_id,
                        voice_id,
                        model_id,
                        ..
                    }) => dto::MessageVoiceOverride::Provider {
                        provider_id: provider_id.to_string(),
                        voice_id,
                        model_id,
                        prompt: None,
                    },
                    _ => return Err(missing_voice()),
                },
            };
            let (provider_id, voice_id, model_id, prompt) = match selection {
                dto::MessageVoiceOverride::UserVoice { voice_id } => {
                    let id = parse_id(&voice_id, "voice_override.voice_id")?;
                    let voice = database
                        .get_user_voice(id)
                        .map_err(IntoApiError::into_api_error)?
                        .ok_or_else(missing_voice)?;
                    (
                        voice.provider_id,
                        voice.voice_id,
                        Some(voice.model_id),
                        voice.prompt,
                    )
                }
                dto::MessageVoiceOverride::Provider {
                    provider_id,
                    voice_id,
                    model_id,
                    prompt,
                } => (
                    parse_id(&provider_id, "voice_override.provider_id")?,
                    voice_id,
                    model_id,
                    prompt,
                ),
            };
            let provider = database
                .get_audio_provider(provider_id)
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(missing_voice)?;
            let model_id = model_id
                .or_else(|| {
                    if let AudioProviderConfig::Kokoro { variant } = &provider.config {
                        variant.clone()
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    lettuce_speech::tts_catalog_models(provider.config.provider_kind())
                        .first()
                        .map(|model| model.id.to_owned())
                })
                .ok_or_else(|| {
                    api_error(
                        ApiErrorCode::Unavailable,
                        "the provider has no speech models",
                    )
                })?;
            let group = crate::generation::live_sources::live_group(database, &conversation)
                .map_err(IntoApiError::into_api_error)?;
            let persona = crate::generation::live_sources::live_persona(
                database,
                &conversation,
                group.as_ref().and_then(|group| group.profile.as_ref()),
            )
            .map_err(IntoApiError::into_api_error)?;
            let persona_name = persona
                .as_ref()
                .map_or("", |persona| persona.title.as_str());
            let (char_name, persona_name) = if request.swap_places {
                (persona_name, character.profile.name.as_str())
            } else {
                (character.profile.name.as_str(), persona_name)
            };
            let raw_text = crate::api::mapping::shown_text(&item)
                .ok_or_else(|| invalid_field("message_id", "the message has no text"))?;
            let text = display_text(&raw_text, char_name, persona_name);
            let synthesis = SynthesisRequest {
                id,
                provider,
                model_id,
                voice_id,
                prompt,
                text,
                output_asset_id: AssetId::new(),
                output_policy: TtsOutputPolicy::Retained,
                created_at: context.now(),
            };
            super::synthesize::admit_request(context, &key, &digest, synthesis)
        })
        .await
}

fn strip_hidden(text: &str, open: &str, closes: &[&str], insensitive: bool) -> String {
    let mut remaining = text;
    let mut visible = String::new();
    loop {
        let searchable = if insensitive {
            remaining.to_ascii_lowercase()
        } else {
            remaining.to_owned()
        };
        let Some(start) = searchable.find(open) else {
            visible.push_str(remaining);
            break;
        };
        visible.push_str(&remaining[..start]);
        remaining = &remaining[start + open.len()..];
        let searchable = if insensitive {
            remaining.to_ascii_lowercase()
        } else {
            remaining.to_owned()
        };
        let Some((end, close)) = closes
            .iter()
            .filter_map(|close| searchable.find(close).map(|index| (index, *close)))
            .min_by_key(|(index, _)| *index)
        else {
            break;
        };
        remaining = &remaining[end + close.len()..];
    }
    visible
}

fn display_text(text: &str, character: &str, persona: &str) -> String {
    use std::sync::OnceLock;
    static PLACEHOLDERS: OnceLock<regex::Regex> = OnceLock::new();
    let thought_free = strip_hidden(text, "<think>", &["</think>"], false);
    let scene_free = strip_hidden(
        &thought_free,
        "<img>",
        &["</img>", "[continue]", "[/continue]"],
        true,
    );
    let pattern = PLACEHOLDERS.get_or_init(|| {
        regex::Regex::new(r"\{\{\s*(char|persona|user)(?:\.name)?\s*\}\}")
            .expect("placeholder pattern")
    });
    pattern
        .replace_all(scene_free.trim(), |capture: &regex::Captures<'_>| {
            if &capture[1] == "char" {
                character
            } else {
                persona
            }
        })
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::display_text;
    #[test]
    fn message_display_cleaning_keeps_visible_text_and_literal_names() {
        assert_eq!(
            display_text(
                "<think>private</think> **Hi {{ char.name }}!** <IMG>scene[/continue] {{ user }}",
                "Ada $&",
                "Pat"
            ),
            "**Hi Ada $&!**  Pat"
        );
        assert_eq!(
            display_text("Visible <think>unfinished", "Ada", "Pat"),
            "Visible"
        );
        assert_eq!(
            display_text("Visible <img>unfinished", "Ada", "Pat"),
            "Visible"
        );
        assert_eq!(display_text("Visible <im", "Ada", "Pat"), "Visible <im");
        assert_eq!(
            display_text("<THINK>case stays</THINK> {{Char}}", "Ada", "Pat"),
            "<THINK>case stays</THINK> {{Char}}"
        );
    }
}
