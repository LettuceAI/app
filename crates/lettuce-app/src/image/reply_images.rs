//! The scene tag a chat reply carries: the reply's text is cleaned when it is
//! finalized, and the scene prompt, with the cast the tag names, is handed
//! back for the caller to run or ask about.

use lettuce_conversations::{MessagePart, SceneCastSelection, extract_scene_tag};
use lettuce_models::ModelCatalog;
use lettuce_settings::SceneGenerationMode;
use lettuce_types::CharacterId;

use crate::image::scene_cast::{CastSubjectKind, SceneCastSources};
use crate::{ImageFeature, image_feature_model};

/// A name a scene tag may use for a subject of the chat, lowercased, and the
/// character it names (`None` for the persona).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastName {
    pub name: String,
    pub character_id: Option<CharacterId>,
}

/// The scene mode of a chat's replies, set when scene generation is on and
/// its model resolves, and the names its scene tags may use.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReplyImageFacts {
    pub scene_mode: Option<SceneGenerationMode>,
    pub cast_names: Vec<CastName>,
}

/// A scene image a finalized reply asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneImageFollowUp {
    pub prompt: String,
    /// The user approves the prompt before the image is generated.
    pub ask_first: bool,
    /// The subjects the tag named; `None` when it named none the chat has.
    pub cast: Option<SceneCastSelection>,
}

/// The facts for a conversation's replies. A cast that cannot be read leaves
/// the tags' names unresolved, so the message's own cast is used.
pub(crate) fn reply_image_facts<R: ModelCatalog + SceneCastSources + ?Sized>(
    repository: &R,
    settings: &lettuce_settings::GlobalSettings,
    conversation: &lettuce_conversations::Conversation,
) -> Option<ReplyImageFacts> {
    let images = &settings.image_generation;
    let scene_mode = (images.scene_enabled
        && images.scene_automatic_enabled
        && image_feature_model(repository, settings, ImageFeature::Scene).is_ok())
    .then_some(images.scene_mode);
    let mut cast_names = Vec::new();
    if scene_mode.is_some() {
        match crate::image::scene_cast::chat_cast(repository, conversation) {
            Ok(subjects) => {
                for subject in subjects {
                    let character_id = match subject.kind {
                        CastSubjectKind::Character(id) => Some(id),
                        CastSubjectKind::Persona(_) => None,
                    };
                    cast_names.push(CastName {
                        name: subject.name.trim().to_lowercase(),
                        character_id,
                    });
                    if let Some(id) = character_id {
                        cast_names.extend(
                            conversation
                                .participants
                                .iter()
                                .filter(|participant| {
                                    participant.source
                                        == lettuce_conversations::ParticipantSource::Character(id)
                                })
                                .map(|participant| CastName {
                                    name: participant.display_name.trim().to_lowercase(),
                                    character_id,
                                }),
                        );
                    }
                }
            }
            Err(error) => tracing::warn!(?error, "the scene tag names could not be read"),
        }
    }
    Some(ReplyImageFacts {
        scene_mode,
        cast_names,
    })
}

/// The cast `names` mean, in the order they were named; `None` when none of
/// them is a subject of the chat.
fn named_cast(facts: &ReplyImageFacts, names: &[String]) -> Option<SceneCastSelection> {
    let mut cast = SceneCastSelection::default();
    let mut matched = false;
    for name in names {
        let name = name.trim().to_lowercase();
        let Some(subject) = facts
            .cast_names
            .iter()
            .find(|candidate| !candidate.name.is_empty() && candidate.name == name)
        else {
            continue;
        };
        matched = true;
        match subject.character_id {
            Some(id) if !cast.character_ids.contains(&id) => cast.character_ids.push(id),
            Some(_) => {}
            None => cast.include_persona = true,
        }
    }
    matched.then_some(cast)
}

/// Removes the scene tag from a reply's text, always, and keeps its prompt
/// and cast unless the scene mode is manual or scenes cannot run.
pub(crate) fn take_scene_image(
    parts: &mut [MessagePart],
    facts: &ReplyImageFacts,
) -> Option<SceneImageFollowUp> {
    let mut tag = None;
    for part in parts.iter_mut() {
        if let MessagePart::Text { text } = part {
            let (clean, found) = extract_scene_tag(text);
            *text = clean;
            if tag.is_none() {
                tag = found;
            }
        }
    }
    let ask_first = match facts.scene_mode? {
        SceneGenerationMode::Auto => false,
        SceneGenerationMode::AskFirst => true,
        SceneGenerationMode::Manual => return None,
    };
    let tag = tag?;
    Some(SceneImageFollowUp {
        cast: named_cast(facts, &tag.subjects),
        prompt: tag.prompt,
        ask_first,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(scene_mode: Option<SceneGenerationMode>) -> ReplyImageFacts {
        ReplyImageFacts {
            scene_mode,
            cast_names: Vec::new(),
        }
    }

    #[test]
    fn replies_lose_the_scene_tag_and_keep_its_prompt_by_mode() {
        let run = |scene_mode| {
            let mut parts = vec![MessagePart::Text {
                text: " Hi <img>harbor</img> ".into(),
            }];
            let follow_up = take_scene_image(&mut parts, &facts(scene_mode));
            let MessagePart::Text { text } = &parts[0] else {
                unreachable!()
            };
            assert_eq!(text, "Hi");
            follow_up
        };
        assert_eq!(run(None), None);
        assert_eq!(run(Some(SceneGenerationMode::Manual)), None);
        assert_eq!(
            run(Some(SceneGenerationMode::Auto)),
            Some(SceneImageFollowUp {
                prompt: "harbor".into(),
                ask_first: false,
                cast: None,
            })
        );
        assert!(run(Some(SceneGenerationMode::AskFirst)).is_some_and(|scene| scene.ask_first));
    }

    #[test]
    fn tag_subjects_resolve_to_the_chat_cast() {
        let (mira, tom) = (CharacterId::new(), CharacterId::new());
        let facts = ReplyImageFacts {
            scene_mode: Some(SceneGenerationMode::Auto),
            cast_names: vec![
                CastName { name: "mira".into(), character_id: Some(mira) },
                CastName { name: "captain mira".into(), character_id: Some(mira) },
                CastName { name: "tom".into(), character_id: Some(tom) },
                CastName { name: "sol".into(), character_id: None },
            ],
        };
        let take = |text: &str| {
            let mut parts = vec![MessagePart::Text { text: text.into() }];
            take_scene_image(&mut parts, &facts).and_then(|scene| scene.cast)
        };
        assert_eq!(
            take("<img subjects=\"Tom, captain Mira, MIRA, Sol, Nobody\">deck</img>"),
            Some(SceneCastSelection {
                character_ids: vec![tom, mira],
                include_persona: true,
            })
        );
        assert_eq!(take("<img subjects=\"Nobody\">deck</img>"), None);
        assert_eq!(take("<img>deck</img>"), None);
    }
}
