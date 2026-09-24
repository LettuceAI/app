use lettuce_conversations::{ConversationKind, ParticipantRole};
use lettuce_transfer::{
    ChatImportRepository, ChatImportRepositoryError, ChatJsonlError, ChatJsonlRole, LegacyIdScope,
    LegacyImportRepositoryError,
};
use lettuce_types::{CharacterId, ContentHash, ConversationId, TimestampMillis};
use uuid::Uuid;

use crate::legacy::legacy_direct_conversation_import::{
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
    #[error("Session not found")]
    NotFound,
    #[error("UNRESOLVED_PARTICIPANTS:{0}")]
    UnresolvedParticipants(String),
    #[error("GROUP_CHAT_IMPORT_REQUIRES_CHARACTER_MAPPING")]
    GroupNeedsCharacters,
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
        let lines = imported_lines(&chat, millis);
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

impl<S> ChatFileCoordinator<'_, S>
where
    S: crate::GroupLaunchSources + ChatImportRepository,
{
    /// Imports a transcript with several speakers as a new group of the
    /// characters `participants` maps each speaker name to, and its chat.
    pub fn import_group(
        &self,
        raw: &str,
        participants: &std::collections::BTreeMap<String, CharacterId>,
        now: TimestampMillis,
    ) -> Result<ConversationId, ChatFileError> {
        use lettuce_characters::{CharacterRepository, GroupMember, GroupProfile, GroupRepository};
        let chat = lettuce_transfer::parse_chat_jsonl(raw, now.get())?;
        let mut unresolved = Vec::new();
        let mut speakers = std::collections::BTreeMap::new();
        for speaker in chat.speakers() {
            let character = participants.get(&speaker).copied().filter(|id| {
                CharacterRepository::get(self.sources, *id)
                    .ok()
                    .flatten()
                    .is_some_and(|details| {
                        details.character.status == lettuce_characters::LifecycleStatus::Active
                    })
            });
            match character {
                Some(id) => {
                    speakers.insert(speaker, id);
                }
                None => unresolved.push(speaker),
            }
        }
        if !unresolved.is_empty() {
            return Err(ChatFileError::UnresolvedParticipants(unresolved.join(", ")));
        }
        let mut members = Vec::new();
        for id in speakers.values() {
            if !members.contains(id) {
                members.push(*id);
            }
        }
        match members.as_slice() {
            [] => return Err(ChatFileError::GroupNeedsCharacters),
            [only] => return self.import_direct(raw, None, Some(*only), now),
            _ => {}
        }
        let title = chat
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("character_name"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .unwrap_or("Imported Group Chat")
            .to_owned();
        let group = GroupRepository::create(
            self.sources,
            lettuce_characters::CreateGroupPlan {
                group: GroupProfile::new(
                    lettuce_types::GroupId::new(),
                    title.clone(),
                    members
                        .iter()
                        .enumerate()
                        .map(|(ordinal, character_id)| GroupMember {
                            character_id: *character_id,
                            ordinal: u32::try_from(ordinal).unwrap_or(u32::MAX),
                            muted: false,
                            model_profile_override: None,
                        })
                        .collect(),
                    now,
                )
                .map_err(|_| LegacyImportRepositoryError::InvalidInput)?,
                starting_scene: None,
            },
        )
        .map_err(|_| LegacyImportRepositoryError::InvalidInput)?;
        let source_id = Uuid::new_v4().to_string();
        let scope = LegacyIdScope::new(
            &ContentHash::parse(blake3::hash(source_id.as_bytes()).to_hex().to_string())
                .map_err(|_| LegacyImportRepositoryError::InvalidInput)?,
        );
        let context = ImportContext::fresh(scope);
        let (plan, mut snapshots) = ConversationLaunchPlanner::new(self.sources)
            .prepare_group(
                &crate::GroupConversationLaunchRequest {
                    format_version: crate::GROUP_LAUNCH_REQUEST_FORMAT_V1,
                    title,
                    user: legacy_user(),
                    group_id: group.group.id,
                    persona: persona_selection(false, None, &context)?,
                    operation_key: lettuce_conversations::IdempotencyKey::new(format!(
                        "chat-import.{}",
                        scope.source(&source_id)
                    ))
                    .map_err(|_| LegacyImportRepositoryError::InvalidInput)?,
                },
                now,
            )?
            .into_parts();
        let authors = plan
            .participants
            .iter()
            .filter_map(|participant| match participant.source {
                lettuce_conversations::ParticipantSource::Character(id) => {
                    Some((id, participant.id))
                }
                _ => None,
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let ConversationKind::Group(details) = &plan.kind else {
            return Err(LegacyImportRepositoryError::InvalidInput.into());
        };
        let (settings, settings_snapshots) = session_settings(
            self.sources,
            &source_id,
            &context,
            SessionSettingsSource {
                author_note: None,
                prompt_source_id: None,
                prompt_purposes: &[lettuce_context::PromptPurpose::GroupChatConversational],
                prompt_snapshot_purpose:
                    lettuce_conversations::PromptPurposeSnapshot::GroupConversational,
                lorebook_source_ids: None,
                speaker_selection: None,
                model_settings: &lettuce_models::ModelSettingsLayer::default(),
                background: None,
                companion_clock: None,
            },
        )?;
        snapshots.extend(settings_snapshots);
        let model = selected_model(&details.group.model).or_else(|| {
            details
                .group
                .members
                .iter()
                .find_map(|member| selected_model(&member.model_override))
        });
        let millis = |value: i64| u64::try_from(value).unwrap_or(0);
        let lines = imported_lines(&chat, millis);
        let author = |message: &lettuce_transfer::ChatJsonlMessage| {
            (message.role == ChatJsonlRole::Assistant)
                .then_some(message.name.as_ref())
                .flatten()
                .and_then(|name| speakers.get(name))
                .and_then(|character| authors.get(character))
                .copied()
        };
        let speaking = chat
            .messages
            .iter()
            .filter(|message| message.content.is_some())
            .map(author)
            .collect::<Vec<_>>();
        let record = conversation_record(
            LegacyConversationSource {
                source_id: &source_id,
                title: plan.title.clone(),
                kind: plan.kind.clone(),
                participants: plan.participants.clone(),
                initial_timeline: &plan.initial_timeline.entries,
                snapshots,
                model,
                archived: false,
                created_at: millis(now.get()),
                updated_at: millis(now.get()),
                messages: lines
                    .iter()
                    .zip(&speaking)
                    .map(|(line, author)| TimelineMessage {
                        source_id: &line.source_id,
                        role: line.role,
                        content: &line.content,
                        created_at: line.created_at,
                        effective_at: None,
                        visible_in_chat: true,
                        pinned: false,
                        scene_edited: false,
                        author: *author,
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
                                attachments_json: Some("[]"),
                                author: *author,
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
        let conversation_id = record.history.aggregate.conversation.id;
        self.sources.import_chat(record, None, now)?;
        Ok(conversation_id)
    }
}

/// The transcript's messages that have content, their swipes as variants
/// with `swipe_id`, else the first, selected.
fn imported_lines(
    chat: &lettuce_transfer::ChatJsonl,
    millis: impl Fn(i64) -> u64,
) -> Vec<ImportedLine> {
    chat.messages
        .iter()
        .filter_map(|message| {
            let content = message.content.clone()?;
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
                created_at: millis(message.created_at),
                variants,
                selected,
            })
        })
        .collect()
}

/// A transcript ready to save: its file name and JSONL text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedChatFile {
    pub filename: String,
    pub content: String,
}

impl<S> ChatFileCoordinator<'_, S>
where
    S: lettuce_conversations::ConversationReader
        + lettuce_characters::CharacterRepository
        + lettuce_characters::PersonaRepository
        + ?Sized,
{
    /// The chat's active branch as a SillyTavern JSONL transcript: the
    /// shown content of each message, other candidates as swipes, speakers by
    /// their current names.
    pub fn export(
        &self,
        conversation_id: ConversationId,
        now: TimestampMillis,
    ) -> Result<ExportedChatFile, ChatFileError> {
        use lettuce_conversations::{
            ConversationReader, MessagePart, MessageRenderSource, MessageRole, ParticipantSource,
        };
        let conversation = ConversationReader::get(self.sources, conversation_id)
            .map_err(|_| ChatFileError::NotFound)?
            .conversation;
        let mut items = Vec::new();
        let mut request = lettuce_types::PageRequest {
            cursor: None,
            limit: lettuce_types::PageLimit::new(200),
        };
        loop {
            let page = ConversationReader::timeline_page(
                self.sources,
                conversation_id,
                conversation.active_branch_id,
                &request,
            )
            .map_err(|_| ChatFileError::NotFound)?;
            items.extend(page.items);
            match page.next_cursor {
                Some(cursor) => request.cursor = Some(cursor),
                None => break,
            }
        }
        items.reverse();
        items.retain(|item| {
            item.message.visibility != lettuce_conversations::MessageVisibility::Tombstoned
        });
        let text = |parts: &[MessagePart]| {
            parts
                .iter()
                .filter_map(|part| match part {
                    MessagePart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<String>()
        };
        let character_name = |id: CharacterId, fallback: &str| {
            lettuce_characters::CharacterRepository::get(self.sources, id)
                .ok()
                .flatten()
                .map_or_else(
                    || fallback.to_owned(),
                    |details| details.character.profile.name,
                )
        };
        let names = conversation
            .participants
            .iter()
            .map(|participant| {
                let name = match participant.source {
                    ParticipantSource::Character(id) => {
                        character_name(id, &participant.display_name)
                    }
                    _ => participant.display_name.clone(),
                };
                (participant.id, name)
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let (persona, group, character) = match &conversation.kind {
            ConversationKind::Direct(details) => (
                &details.persona,
                false,
                character_name(details.character.source_id, &details.character.name),
            ),
            ConversationKind::Group(details) => {
                (&details.group.persona, true, conversation.title.clone())
            }
        };
        let user_name = match persona {
            lettuce_conversations::SnapshotSelection::Inherited(persona)
            | lettuce_conversations::SnapshotSelection::Explicit(persona) => {
                lettuce_characters::PersonaRepository::get(self.sources, persona.source_id)
                    .ok()
                    .flatten()
                    .map_or_else(|| persona.title.clone(), |stored| stored.title)
            }
            lettuce_conversations::SnapshotSelection::Disabled => "User".to_owned(),
        };
        let mut lines = Vec::with_capacity(items.len());
        for item in &items {
            let message = &item.message;
            let shown = match (&message.active_render_source, &item.active_candidate) {
                (MessageRenderSource::Candidate(_), Some(candidate)) => text(&candidate.parts),
                _ => item
                    .active_revision
                    .as_ref()
                    .map(|revision| text(&revision.parts))
                    .unwrap_or_default(),
            };
            let candidates = if message.role == MessageRole::Assistant {
                let mut candidates = Vec::new();
                let mut request = lettuce_types::PageRequest {
                    cursor: None,
                    limit: lettuce_types::PageLimit::new(200),
                };
                loop {
                    let page =
                        ConversationReader::page_candidates(self.sources, message.id, &request)
                            .map_err(|_| ChatFileError::NotFound)?;
                    candidates.extend(page.items);
                    match page.next_cursor {
                        Some(cursor) => request.cursor = Some(cursor),
                        None => break,
                    }
                }
                candidates.sort_by_key(|candidate| candidate.ordinal);
                candidates
            } else {
                Vec::new()
            };
            let selected = item
                .active_candidate
                .as_ref()
                .map(|candidate| candidate.id.to_string());
            let (content, swipes) = if candidates.len() < 2 {
                (shown, None)
            } else if group
                && matches!(
                    message.active_render_source,
                    MessageRenderSource::Candidate(_)
                )
            {
                lettuce_transfer::group_chat_content(
                    shown,
                    candidates
                        .iter()
                        .map(|candidate| (candidate.id.to_string(), text(&candidate.parts)))
                        .collect(),
                    selected.as_deref(),
                )
            } else {
                let swipes = lettuce_transfer::direct_chat_swipes(
                    &shown,
                    &candidates
                        .iter()
                        .map(|candidate| (Some(candidate.id.to_string()), text(&candidate.parts)))
                        .collect::<Vec<_>>(),
                    selected.as_deref(),
                );
                (shown, swipes)
            };
            let (name, is_user, is_system) = match message.role {
                MessageRole::User => (user_name.clone(), true, false),
                MessageRole::System => ("System".to_owned(), false, true),
                _ if group => (
                    message
                        .author_participant_id
                        .and_then(|id| names.get(&id).cloned())
                        .unwrap_or_else(|| "Character".to_owned()),
                    false,
                    false,
                ),
                _ => (character.clone(), false, false),
            };
            lines.push(lettuce_transfer::ChatJsonlLine {
                name,
                is_user,
                is_system,
                created_at: message.effective_time.get(),
                content,
                swipes,
            });
        }
        let header = lettuce_transfer::ChatJsonlHeader {
            user_name,
            character_name: character,
            created_at: items
                .first()
                .map_or(now.get(), |item| item.message.effective_time.get()),
            group,
        };
        Ok(ExportedChatFile {
            filename: lettuce_transfer::chat_jsonl_filename(&conversation.title, group, now.get()),
            content: lettuce_transfer::export_chat_jsonl(&header, &lines),
        })
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
        named(backend, "Ada", companion)
    }

    fn named(backend: &crate::AppBackend, name: &str, companion: bool) -> CharacterId {
        let id = CharacterId::new();
        let mut defaults = CharacterDefaults::default();
        if companion {
            defaults.interaction_mode = lettuce_characters::InteractionMode::Companion;
            defaults.companion_soul = Some(lettuce_companions::CompanionSoulConfig::default());
        }
        let character = Character::new(
            id,
            CharacterProfile {
                name: name.into(),
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
        let model = crate::launch::tests::seed_model(
            backend.database(),
            lettuce_models::ProviderProtocol::Anthropic,
            "openai",
        );
        crate::launch::tests::set_application_default_model(backend.database(), model);
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
        let exported = files
            .export(conversation_id, TimestampMillis::new(70))
            .expect("export");
        assert_eq!(
            exported.filename,
            "chat_harbour_night_19700101_000000.jsonl"
        );
        let parsed = lettuce_transfer::parse_chat_jsonl(&exported.content, 0).expect("parse");
        assert_eq!(
            parsed
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.get("character_name"))
                .and_then(serde_json::Value::as_str),
            Some("Ada")
        );
        assert_eq!(parsed.messages.len(), 2);
        assert_eq!(parsed.messages[0].content.as_deref(), Some("Hello"));
        assert_eq!(parsed.messages[1].content.as_deref(), Some("Second"));
        assert_eq!(parsed.messages[1].swipes, vec!["First", "Second"]);
        assert_eq!(parsed.messages[1].swipe_id, Some(1));
        assert_eq!(parsed.messages[1].created_at, 1_767_323_046_000);
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

    #[test]
    fn a_multi_speaker_transcript_becomes_a_new_group_chat() {
        let backend = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let model = crate::launch::tests::seed_model(
            backend.database(),
            lettuce_models::ProviderProtocol::Anthropic,
            "anthropic",
        );
        crate::launch::tests::set_application_default_model(backend.database(), model);
        let ada = character(&backend, false);
        let ben = named(&backend, "Ben", false);
        let raw = [
            r#"{"user_name":"You","character_name":"Harbour crew","chat_metadata":{"group":true}}"#,
            r#"{"name":"You","is_user":true,"mes":"Hello both","send_date":"2026-01-02T03:04:05.000Z"}"#,
            r#"{"name":"Ada","is_user":false,"mes":"Hi","send_date":"2026-01-02T03:04:06.000Z"}"#,
            r#"{"name":"Ben","is_user":false,"mes":"Hey","send_date":"2026-01-02T03:04:07.000Z"}"#,
        ]
        .join("\n");
        let files = backend.chat_files();
        let partial = std::collections::BTreeMap::from([("Ada".to_owned(), ada)]);
        assert!(matches!(
            files.import_group(&raw, &partial, TimestampMillis::new(50)),
            Err(ChatFileError::UnresolvedParticipants(names)) if names == "Ben"
        ));
        let alone =
            std::collections::BTreeMap::from([("Ada".to_owned(), ada), ("Ben".to_owned(), ada)]);
        let direct = files
            .import_group(&raw, &alone, TimestampMillis::new(50))
            .expect("one character");
        assert!(matches!(
            ConversationReader::get(backend.database(), direct)
                .expect("direct")
                .conversation
                .kind,
            ConversationKind::Direct(_)
        ));
        let map =
            std::collections::BTreeMap::from([("Ada".to_owned(), ada), ("Ben".to_owned(), ben)]);
        let conversation_id = files
            .import_group(&raw, &map, TimestampMillis::new(50))
            .expect("import group");
        let conversation = ConversationReader::get(backend.database(), conversation_id)
            .expect("conversation")
            .conversation;
        assert_eq!(conversation.title, "Harbour crew");
        assert!(matches!(conversation.kind, ConversationKind::Group(_)));
        let exported = files
            .export(conversation_id, TimestampMillis::new(70))
            .expect("export");
        let parsed = lettuce_transfer::parse_chat_jsonl(&exported.content, 0).expect("parse");
        let speakers = parsed
            .messages
            .iter()
            .map(|message| (message.name.clone(), message.content.clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            speakers,
            vec![
                (Some("User".to_owned()), Some("Hello both".to_owned())),
                (Some("Ada".to_owned()), Some("Hi".to_owned())),
                (Some("Ben".to_owned()), Some("Hey".to_owned())),
            ]
        );
    }
}
