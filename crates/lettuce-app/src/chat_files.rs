use lettuce_conversations::{ConversationKind, ParticipantRole};
use lettuce_transfer::{
    ChatImportRepository, ChatImportRepositoryError, ChatJsonlError, ChatJsonlRole, LegacyIdScope,
    LegacyImportRepositoryError,
};
use lettuce_types::{CharacterId, ContentHash, ConversationId, TimestampMillis};
use uuid::Uuid;

use crate::legacy_direct_conversation_import::{
    ImportContext, LegacyConversationSource, SessionSettingsSource, TimelineMessage,
    TimelineVariant, conversation_record, legacy_user, persona_selection, selected_model,
    session_settings,
};
use crate::{
    ConversationLaunchPlanner, DIRECT_LAUNCH_REQUEST_FORMAT_V1, DirectConversationLaunchRequest,
    DirectLaunchSources, LaunchSelection,
};

#[derive(Debug, thiserror::Error)]
pub enum ChatFileError {
    #[error(transparent)]
    Format(#[from] ChatJsonlError),
    #[error("TARGET_CHARACTER_REQUIRED")]
    TargetCharacterRequired,
    #[error(transparent)]
    Launch(#[from] crate::ConversationLaunchError),
    #[error("the chat could not be converted: {0}")]
    Convert(#[from] LegacyImportRepositoryError),
    #[error("chat import storage failed: {0}")]
    Repository(#[from] ChatImportRepositoryError),
}

/// SillyTavern JSONL transcripts read into conversations.
#[derive(Debug)]
pub struct ChatFileCoordinator<'a, S: ?Sized> {
    sources: &'a S,
}

impl<'a, S: ?Sized> ChatFileCoordinator<'a, S> {
    #[must_use]
    pub const fn new(sources: &'a S) -> Self {
        Self { sources }
    }
}

/// The transcript's title, falling back past a blank header name, cut to the
/// longest title a conversation takes.
fn chat_title(chat: &lettuce_transfer::ChatJsonl, file_stem: Option<&str>) -> String {
    const MAX_TITLE_BYTES: usize = 1024;
    let mut title = [
        chat.metadata
            .as_ref()
            .and_then(|metadata| metadata.get("character_name"))
            .and_then(serde_json::Value::as_str),
        file_stem,
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .find(|title| !title.is_empty())
    .unwrap_or("Imported Chat")
    .to_owned();
    if title.len() > MAX_TITLE_BYTES {
        let mut end = MAX_TITLE_BYTES;
        while !title.is_char_boundary(end) {
            end -= 1;
        }
        title.truncate(end);
    }
    title
}

struct ImportedLine {
    source_id: String,
    role: &'static str,
    content: String,
    created_at: u64,
    variants: Vec<(String, String)>,
    selected: Option<String>,
}

impl<S> ChatFileCoordinator<'_, S>
where
    S: DirectLaunchSources + ChatImportRepository,
{
    /// Imports a one-character transcript as a new chat with `character_id`,
    /// titled by the header's character name, else `file_stem`.
    pub fn import_direct(
        &self,
        raw: &str,
        file_stem: Option<&str>,
        character_id: Option<CharacterId>,
        now: TimestampMillis,
    ) -> Result<ConversationId, ChatFileError> {
        let character_id = character_id.ok_or(ChatFileError::TargetCharacterRequired)?;
        let chat = lettuce_transfer::parse_chat_jsonl(raw, now.get())?;
        let source_id = Uuid::new_v4().to_string();
        let scope = LegacyIdScope::new(
            &ContentHash::parse(blake3::hash(source_id.as_bytes()).to_hex().to_string())
                .map_err(|_| LegacyImportRepositoryError::InvalidInput)?,
        );
        let context = ImportContext::fresh(scope);
        let millis = |value: i64| u64::try_from(value).unwrap_or(0);
        let lines = chat
            .messages
            .iter()
            .filter_map(|message| {
                let content = message.content.clone()?;
                let created_at = millis(message.created_at);
                let variants = message
                    .swipes
                    .iter()
                    .map(|swipe| (Uuid::new_v4().to_string(), swipe.clone()))
                    .collect::<Vec<_>>();
                let selected = message
                    .swipe_id
                    .and_then(|index| variants.get(index))
                    .or_else(|| variants.first())
                    .map(|(id, _)| id.clone());
                Some(ImportedLine {
                    source_id: Uuid::new_v4().to_string(),
                    role: match message.role {
                        ChatJsonlRole::User => "user",
                        ChatJsonlRole::Assistant => "assistant",
                        ChatJsonlRole::System => "system",
                    },
                    content,
                    created_at,
                    variants,
                    selected,
                })
            })
            .collect::<Vec<_>>();
        let request = DirectConversationLaunchRequest {
            format_version: DIRECT_LAUNCH_REQUEST_FORMAT_V1,
            title: chat_title(&chat, file_stem),
            user: legacy_user(),
            character_id,
            scene: LaunchSelection::Disabled,
            starter: LaunchSelection::Disabled,
            persona: persona_selection(false, None, &context)?,
            operation_key: lettuce_conversations::IdempotencyKey::new(format!(
                "chat-import.{}",
                scope.source(&source_id)
            ))
            .map_err(|_| LegacyImportRepositoryError::InvalidInput)?,
        };
        let (prepared, launch_companion) =
            ConversationLaunchPlanner::new(self.sources).prepare_direct_parts(&request)?;
        let (plan, mut snapshots) = prepared.into_parts();
        let conversation_id = ConversationId::from_uuid(scope.source(&source_id));
        let character = plan
            .participants
            .iter()
            .find(|participant| participant.role == ParticipantRole::Character)
            .map(|participant| participant.id)
            .ok_or(LegacyImportRepositoryError::InvalidInput)?;
        let ConversationKind::Direct(details) = &plan.kind else {
            return Err(LegacyImportRepositoryError::InvalidInput.into());
        };
        let model_settings = lettuce_models::ModelSettingsLayer::default();
        let (settings, settings_snapshots) = session_settings(
            self.sources,
            &source_id,
            &context,
            SessionSettingsSource {
                author_note: None,
                prompt_source_id: None,
                prompt_purposes: &[
                    lettuce_context::PromptPurpose::DirectChat,
                    lettuce_context::PromptPurpose::CompanionChat,
                ],
                prompt_snapshot_purpose: lettuce_conversations::PromptPurposeSnapshot::Direct,
                lorebook_source_ids: None,
                speaker_selection: None,
                model_settings: &model_settings,
                background: None,
                companion_clock: launch_companion
                    .as_ref()
                    .is_some_and(|(_, _, time_awareness)| *time_awareness)
                    .then(|| lettuce_conversations::CompanionClockSettings {
                        time_awareness_enabled: true,
                        ..Default::default()
                    }),
            },
        )?;
        snapshots.extend(settings_snapshots);
        let record = conversation_record(
            LegacyConversationSource {
                source_id: &source_id,
                title: plan.title.clone(),
                kind: plan.kind.clone(),
                participants: plan.participants.clone(),
                initial_timeline: &plan.initial_timeline.entries,
                snapshots,
                model: selected_model(&details.model),
                archived: false,
                created_at: millis(now.get()),
                updated_at: millis(now.get()),
                messages: lines
                    .iter()
                    .map(|line| TimelineMessage {
                        source_id: &line.source_id,
                        role: line.role,
                        content: &line.content,
                        created_at: line.created_at,
                        effective_at: matches!(line.role, "user" | "assistant")
                            .then_some(line.created_at),
                        visible_in_chat: true,
                        pinned: false,
                        scene_edited: false,
                        author: (line.role == "assistant").then_some(character),
                        model_source_id: None,
                        selected_variant_source_id: line.selected.as_deref(),
                        reasoning: None,
                        attachments_json: "[]",
                        variants: line
                            .variants
                            .iter()
                            .map(|(id, content)| TimelineVariant {
                                source_id: id,
                                content,
                                created_at: line.created_at,
                                prompt_tokens: None,
                                completion_tokens: None,
                                total_tokens: None,
                                reasoning: None,
                                attachments_json: None,
                                author: Some(character),
                            })
                            .collect(),
                    })
                    .collect(),
                memory: None,
                memory_texts: None,
                memory_summary: None,
                memory_summary_token_count: 0,
                memory_tool_events: None,
                settings,
            },
            &context,
        )?;
        let companion = launch_companion.map(|(owner, initial, _)| {
            (
                lettuce_companions::CompanionStateOwner {
                    conversation_id,
                    ..owner
                },
                initial,
            )
        });
        self.sources.import_chat(record, companion, now)?;
        Ok(conversation_id)
    }
}

#[cfg(test)]
mod tests {
    use lettuce_characters::{
        Character, CharacterDefaults, CharacterMedia, CharacterPresentationV1, CharacterProfile,
        CharacterProvenance, CharacterRepository, CreateCharacterPlan,
    };
    use lettuce_conversations::{ConversationReader, MessageRole};

    use super::*;

    fn character(backend: &crate::AppBackend, companion: bool) -> CharacterId {
        let id = CharacterId::new();
        let mut defaults = CharacterDefaults::default();
        if companion {
            defaults.interaction_mode = lettuce_characters::InteractionMode::Companion;
            defaults.companion_soul = Some(lettuce_companions::CompanionSoulConfig::default());
        }
        let character = Character::new(
            id,
            CharacterProfile {
                name: "Ada".into(),
                nickname: None,
                description: Some("Keeper".into()),
                definition: None,
                design_description: None,
                scenario: None,
                rules: Vec::new(),
            },
            CharacterProvenance::default(),
            defaults,
            CharacterPresentationV1::default(),
            None,
            CharacterMedia::default(),
            TimestampMillis::new(1),
        )
        .expect("character");
        CharacterRepository::create(
            backend.database(),
            CreateCharacterPlan {
                character,
                scenes: Vec::new(),
                variants: Vec::new(),
                starters: Vec::new(),
            },
        )
        .expect("create character");
        id
    }

    #[test]
    fn a_transcript_becomes_a_chat_with_the_chosen_character() {
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let character_id = character(&backend, false);
        let blank = r#"{"user_name":"You","character_name":"  ","chat_metadata":{}}"#.to_owned()
            + "\n"
            + r#"{"name":"You","is_user":true,"mes":"Hi"}"#;
        let untitled = backend
            .chat_files()
            .import_direct(
                &blank,
                Some("night"),
                Some(character_id),
                TimestampMillis::new(40),
            )
            .expect("blank header");
        assert_eq!(
            ConversationReader::get(backend.database(), untitled)
                .expect("untitled")
                .conversation
                .title,
            "night"
        );
        let raw = [
            r#"{"user_name":"You","character_name":"Harbour night","create_date":"2026-01-02T03:04:05.000Z","chat_metadata":{}}"#,
            r#"{"name":"You","is_user":true,"is_system":false,"send_date":"2026-01-02T03:04:05.000Z","mes":"Hello"}"#,
            r#"{"name":"Ada","is_user":false,"is_system":false,"send_date":"2026-01-02T03:04:06.000Z","mes":"Second","swipes":["First","Second"],"swipe_id":1}"#,
            r#"{"name":"Ada","is_user":false,"is_system":false,"send_date":"2026-01-02T03:04:07.000Z","mes":"   "}"#,
        ]
        .join("\n");
        let files = backend.chat_files();
        assert!(matches!(
            files.import_direct(&raw, None, None, TimestampMillis::new(50)),
            Err(ChatFileError::TargetCharacterRequired)
        ));
        let conversation_id = files
            .import_direct(
                &raw,
                Some("file"),
                Some(character_id),
                TimestampMillis::new(50),
            )
            .expect("import");
        let stored = ConversationReader::get(backend.database(), conversation_id)
            .expect("conversation")
            .conversation;
        assert_eq!(stored.title, "Harbour night");
        let timeline = ConversationReader::timeline_page(
            backend.database(),
            conversation_id,
            stored.active_branch_id,
            &lettuce_types::PageRequest {
                cursor: None,
                limit: lettuce_types::PageLimit::new(50),
            },
        )
        .expect("timeline");
        let roles = timeline
            .items
            .iter()
            .map(|item| item.message.role)
            .collect::<Vec<_>>();
        assert_eq!(roles, vec![MessageRole::Assistant, MessageRole::User]);
    }

    #[test]
    fn each_companion_transcript_starts_the_next_episode() {
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let character_id = character(&backend, true);
        let raw = r#"{"name":"You","is_user":true,"mes":"Hello"}"#;
        let files = backend.chat_files();
        let first = files
            .import_direct(raw, None, Some(character_id), TimestampMillis::new(50))
            .expect("first");
        let second = files
            .import_direct(raw, None, Some(character_id), TimestampMillis::new(60))
            .expect("second");
        let episodes = [first, second].map(|conversation_id| {
            lettuce_companions::CompanionStateRepository::get_continuity_episode(
                backend.database(),
                conversation_id,
            )
            .expect("episode")
            .map(|episode| (episode.episode_index, episode.previous_conversation_id))
        });
        assert_eq!(episodes, [Some((1, None)), Some((2, Some(first)))]);
    }
}
