use lettuce_characters::{
    CharacterRepository, GroupRepository, LifecycleStatus, PersonaRepository,
};
use lettuce_context::{LorebookRepository, PromptLookupResult, PromptPurpose, PromptRepository};
use lettuce_conversations::{
    Conversation, ConversationBackground, ConversationKind, ConversationReader,
    CurrentConversationSettingsPatch, GroupChatModeSnapshot, GroupSpeakerSelectionSnapshot,
    LorebookLaunchSnapshot, MemoryModeSnapshot, MemorySettingsSnapshot, OperationToken, PatchValue,
    PersonaLaunchSnapshot, PreparedConversationSettingsUpdate, PromptLaunchSnapshot,
    PromptPurposeSnapshot, ProtectedSnapshotRef, SceneLaunchSnapshot, SnapshotArtifactDraft,
    SnapshotSelection, UpdateConversationSettings,
};
use lettuce_database::Database;
use lettuce_media::{MediaAssetRepository, MediaKind};
use lettuce_models::{ModelKind, ModelProfileRepository, ProviderAccountRepository};
use lettuce_types::{
    AssetId, ConversationId, LorebookId, ModelProfileId, PersonaId, PromptDocumentId, Revision,
    SceneId, TimestampMillis,
};

use super::{ConversationEditError, snapshot_artifact_id};
use crate::launch::{documents, planner, policy};

/// A setting that can name a source, turn it off, or return to what the
/// conversation follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice<T> {
    Set(T),
    None,
    Reset,
}

/// A setting that can be set or returned to what the conversation follows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change<T> {
    Set(T),
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundChange {
    Image(AssetId),
    Hidden,
    Reset,
}

/// A conversation settings change in ids and plain values. `None` keeps a
/// field. An empty lorebook selection turns the conversation's lorebooks
/// off; a blank author note removes it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConversationSettingsChange {
    pub persona: Option<Choice<PersonaId>>,
    /// The one-to-one prompt, or a group's conversation-mode prompt.
    pub prompt: Option<Choice<PromptDocumentId>>,
    /// A group's roleplay-mode prompt.
    pub roleplay_prompt: Option<Choice<PromptDocumentId>>,
    pub lorebooks: Option<Change<Vec<LorebookId>>>,
    pub model: Option<Change<ModelProfileId>>,
    pub background: Option<BackgroundChange>,
    pub scene: Option<Change<SceneId>>,
    pub author_note: Option<String>,
    pub speaker_selection: Option<Change<GroupSpeakerSelectionSnapshot>>,
    pub memory: Option<Change<MemoryModeSnapshot>>,
    pub chat_mode: Option<Change<GroupChatModeSnapshot>>,
    pub disable_character_lorebooks: Option<Change<bool>>,
    /// The members and their muted flags follow the group again.
    pub follow_group_members: bool,
    /// The members' models follow the group again.
    pub follow_group_member_models: bool,
}

/// Resolves every id in `change` into the snapshot the settings patch needs
/// and commits the patch under the settings revision the caller read. An
/// unknown or archived source is `InvalidInput` naming its field.
pub fn apply_settings_change(
    database: &Database,
    conversation_id: ConversationId,
    expected_settings_revision: Option<Revision>,
    operation: OperationToken,
    change: &ConversationSettingsChange,
    now: TimestampMillis,
) -> Result<Conversation, ConversationEditError> {
    let conversation = ConversationReader::get(database, conversation_id)?.conversation;
    let mut builder = Builder {
        database,
        conversation: &conversation,
        drafts: Vec::new(),
    };
    let patch = builder.patch(change)?;
    let drafts = builder.drafts;
    let prepared = PreparedConversationSettingsUpdate::new(
        UpdateConversationSettings {
            conversation_id,
            expected_settings_revision,
            operation,
            patch,
        },
        drafts,
    )
    .map_err(|error| match error {
        lettuce_conversations::PreparedConversationSettingsUpdateError::InvalidCommand(error) => {
            ConversationEditError::Conversation(
                lettuce_conversations::ConversationRepositoryError::Invalid(error),
            )
        }
        _ => ConversationEditError::Snapshot,
    })?;
    Ok(
        lettuce_conversations::ConversationRepository::update_settings(database, prepared, now)?
            .value,
    )
}

struct Builder<'a> {
    database: &'a Database,
    conversation: &'a Conversation,
    drafts: Vec<SnapshotArtifactDraft>,
}

fn invalid(field: &'static str) -> ConversationEditError {
    ConversationEditError::InvalidInput { field }
}

fn source<E>(_: E) -> ConversationEditError {
    ConversationEditError::Source
}

impl Builder<'_> {
    fn group(&self) -> bool {
        matches!(self.conversation.kind, ConversationKind::Group(_))
    }

    fn build<T: lettuce_conversations::SnapshotDocumentBody>(
        &self,
        name: String,
        revision: Revision,
        body: T,
    ) -> Result<SnapshotArtifactDraft, ConversationEditError> {
        documents::draft(
            snapshot_artifact_id(self.conversation.id, &name),
            revision,
            body,
        )
        .map_err(|_| ConversationEditError::Snapshot)
    }

    fn stage(&mut self, draft: SnapshotArtifactDraft) -> ProtectedSnapshotRef {
        let reference = draft.reference();
        if !self
            .drafts
            .iter()
            .any(|staged| staged.artifact_id == draft.artifact_id)
        {
            self.drafts.push(draft);
        }
        reference
    }

    fn patch(
        &mut self,
        change: &ConversationSettingsChange,
    ) -> Result<CurrentConversationSettingsPatch, ConversationEditError> {
        let group = self.group();
        let group_only = |field: &'static str, used: bool| {
            if used && !group {
                Err(invalid(field))
            } else {
                Ok(())
            }
        };
        group_only("roleplay_prompt", change.roleplay_prompt.is_some())?;
        group_only("speaker_selection", change.speaker_selection.is_some())?;
        group_only("chat_mode", change.chat_mode.is_some())?;
        group_only(
            "disable_character_lorebooks",
            change.disable_character_lorebooks.is_some(),
        )?;
        group_only("members", change.follow_group_members)?;
        group_only("member_models", change.follow_group_member_models)?;
        let mut patch = CurrentConversationSettingsPatch::default();
        if let Some(choice) = change.persona {
            patch.persona = match choice {
                Choice::Set(id) => PatchValue::Set(self.persona(id)?),
                Choice::None => PatchValue::Clear,
                Choice::Reset => PatchValue::UseLaunchDefault,
            };
        }
        if let Some(choice) = change.prompt {
            let (purposes, slot): (&[PromptPurpose], _) = if group {
                (
                    &[PromptPurpose::GroupChatConversational],
                    PromptPurposeSnapshot::GroupConversational,
                )
            } else {
                (
                    &policy::DIRECT_SELECTION_PURPOSES,
                    PromptPurposeSnapshot::Direct,
                )
            };
            patch.prompt = self.prompt_choice(choice, purposes, slot, "prompt_id")?;
        }
        if let Some(choice) = change.roleplay_prompt {
            patch.roleplay_prompt = self.prompt_choice(
                choice,
                &[PromptPurpose::GroupChatRoleplay],
                PromptPurposeSnapshot::GroupRoleplay,
                "roleplay_prompt_id",
            )?;
        }
        if let Some(change) = &change.lorebooks {
            patch.lorebooks = match change {
                Change::Set(ids) if ids.is_empty() => PatchValue::Clear,
                Change::Set(ids) => PatchValue::Set(self.lorebooks(ids)?),
                Change::Reset => PatchValue::UseLaunchDefault,
            };
        }
        if let Some(change) = &change.model {
            patch.model_override = match change {
                Change::Set(id) => PatchValue::Set(self.model(*id)?),
                Change::Reset => PatchValue::UseLaunchDefault,
            };
        }
        if let Some(change) = change.background {
            patch.background = match change {
                BackgroundChange::Image(asset_id) => {
                    let asset = MediaAssetRepository::get(self.database, asset_id)
                        .map_err(source)?
                        .ok_or(invalid("background_asset_id"))?;
                    if asset.kind.blob_kind() != MediaKind::Image {
                        return Err(invalid("background_asset_id"));
                    }
                    PatchValue::Set(ConversationBackground::Image { asset_id })
                }
                BackgroundChange::Hidden => PatchValue::Set(ConversationBackground::Hidden),
                BackgroundChange::Reset => PatchValue::UseLaunchDefault,
            };
        }
        if let Some(change) = &change.scene {
            patch.scene = match change {
                Change::Set(id) => PatchValue::Set(self.scene(*id)?),
                Change::Reset => PatchValue::UseLaunchDefault,
            };
        }
        if let Some(note) = &change.author_note {
            let note = note.trim();
            patch.author_note = if note.is_empty() {
                PatchValue::UseLaunchDefault
            } else {
                PatchValue::Set(note.to_owned())
            };
        }
        if let Some(change) = &change.speaker_selection {
            patch.speaker_selection = value_patch(change);
        }
        if let Some(change) = &change.memory {
            patch.memory = match change {
                Change::Set(mode) => PatchValue::Set(self.memory(*mode)?),
                Change::Reset => PatchValue::UseLaunchDefault,
            };
        }
        if let Some(change) = &change.chat_mode {
            patch.chat_mode = value_patch(change);
        }
        if let Some(change) = &change.disable_character_lorebooks {
            patch.disable_character_lorebooks = value_patch(change);
        }
        patch.follow_group_members = change.follow_group_members;
        patch.follow_group_member_models = change.follow_group_member_models;
        Ok(patch)
    }

    fn persona(&mut self, id: PersonaId) -> Result<PersonaLaunchSnapshot, ConversationEditError> {
        let persona = PersonaRepository::get(self.database, id)
            .map_err(source)?
            .filter(|persona| persona.status == LifecycleStatus::Active)
            .ok_or(invalid("persona_id"))?;
        let draft = self.build(
            format!("settings:persona:{}:{}", persona.id, persona.revision.get()),
            persona.revision,
            documents::persona_body(&persona),
        )?;
        Ok(PersonaLaunchSnapshot {
            snapshot_ref: self.stage(draft),
            source_id: persona.id,
            source_revision: persona.revision,
            title: persona.title.clone(),
            nickname: persona.nickname.clone(),
            lorebooks: SnapshotSelection::Disabled,
        })
    }

    fn prompt_choice(
        &mut self,
        choice: Choice<PromptDocumentId>,
        purposes: &[PromptPurpose],
        slot: PromptPurposeSnapshot,
        field: &'static str,
    ) -> Result<PatchValue<PromptLaunchSnapshot>, ConversationEditError> {
        Ok(match choice {
            Choice::Set(id) => {
                let mut document = None;
                for purpose in purposes {
                    if let PromptLookupResult::Available { document: found } =
                        PromptRepository::lookup_exact(self.database, id, *purpose)
                            .map_err(source)?
                    {
                        document = Some(found);
                        break;
                    }
                }
                let document = document.ok_or(invalid(field))?;
                let draft = self.build(
                    format!(
                        "settings:prompt:{}:{}",
                        document.id,
                        document.revision.get()
                    ),
                    document.revision,
                    documents::prompt_body(&document),
                )?;
                PatchValue::Set(PromptLaunchSnapshot {
                    snapshot_ref: self.stage(draft),
                    source_id: document.id,
                    source_revision: document.revision,
                    title: document.name.clone(),
                    purpose: slot,
                })
            }
            Choice::None => PatchValue::Clear,
            Choice::Reset => PatchValue::UseLaunchDefault,
        })
    }

    fn lorebooks(
        &mut self,
        ids: &[LorebookId],
    ) -> Result<Vec<LorebookLaunchSnapshot>, ConversationEditError> {
        let mut books = Vec::with_capacity(ids.len());
        for id in ids {
            if books
                .iter()
                .any(|book: &LorebookLaunchSnapshot| book.source_id == *id)
            {
                return Err(invalid("lorebook_ids"));
            }
            let details = LorebookRepository::get(self.database, *id)
                .map_err(source)?
                .filter(|details| details.book.status == lettuce_context::LifecycleStatus::Active)
                .ok_or(invalid("lorebook_ids"))?;
            let draft = self.build(
                format!(
                    "settings:lorebook:{}:{}",
                    details.book.id,
                    details.book.revision.get()
                ),
                details.book.revision,
                documents::lorebook_body(&details),
            )?;
            books.push(LorebookLaunchSnapshot {
                snapshot_ref: self.stage(draft),
                source_id: details.book.id,
                source_revision: details.book.revision,
                name: details.book.name.clone(),
            });
        }
        Ok(books)
    }

    fn model(
        &mut self,
        id: ModelProfileId,
    ) -> Result<lettuce_conversations::ModelSelectionSnapshot, ConversationEditError> {
        let profile = ModelProfileRepository::get(self.database, id)
            .map_err(source)?
            .filter(|profile| profile.kind == ModelKind::Chat)
            .ok_or(invalid("model_profile_id"))?;
        let account = ProviderAccountRepository::get(self.database, profile.provider_account_id)
            .map_err(source)?
            .filter(|account| account.enabled)
            .ok_or(invalid("model_profile_id"))?;
        let draft = self.build(
            format!(
                "settings:model:{}:{}:{}:{}",
                profile.id,
                profile.revision.get(),
                account.id,
                account.revision.get()
            ),
            profile.revision,
            documents::model_body(&profile, &account),
        )?;
        let snapshot = planner::model_snapshot(&profile, &account, &draft);
        self.stage(draft);
        Ok(snapshot)
    }

    fn scene(&mut self, id: SceneId) -> Result<SceneLaunchSnapshot, ConversationEditError> {
        let (scene, variants) = match &self.conversation.kind {
            ConversationKind::Direct(details) => {
                let character =
                    CharacterRepository::get(self.database, details.character.source_id)
                        .map_err(source)?
                        .ok_or(invalid("scene_id"))?;
                let scene = character
                    .scenes
                    .iter()
                    .find(|scene| scene.id == id)
                    .cloned()
                    .ok_or(invalid("scene_id"))?;
                (scene, character.variants)
            }
            ConversationKind::Group(details) => {
                let group = GroupRepository::get(self.database, details.group.source_id)
                    .map_err(source)?
                    .and_then(|group| group.starting_scene)
                    .filter(|starting| starting.scene.id == id)
                    .ok_or(invalid("scene_id"))?;
                (group.scene, group.variants)
            }
        };
        if scene.status != LifecycleStatus::Active {
            return Err(invalid("scene_id"));
        }
        let text = match &self.conversation.kind {
            ConversationKind::Direct(_) => policy::resolve_scene_text(&scene, &variants),
            ConversationKind::Group(_) => policy::resolve_group_scene_text(&scene, &variants),
        };
        let draft = self.build(
            format!("settings:scene:{}:{}", scene.id, scene.revision.get()),
            scene.revision,
            documents::scene_body(&scene, &variants),
        )?;
        Ok(SceneLaunchSnapshot {
            snapshot_ref: self.stage(draft),
            source_id: scene.id,
            source_revision: scene.revision,
            title: policy::scene_title(text.as_deref(), scene.ordinal),
        })
    }

    fn memory(
        &self,
        mode: MemoryModeSnapshot,
    ) -> Result<MemorySettingsSnapshot, ConversationEditError> {
        let settings = lettuce_settings::GlobalSettingsStore::load(self.database)
            .map_err(source)?
            .settings;
        let policy = if self.group() {
            settings.effective_group_dynamic_memory().clone()
        } else {
            settings.dynamic_memory.clone()
        };
        Ok(MemorySettingsSnapshot {
            policy_ref: None,
            mode,
            selected_revision_ids: Vec::new(),
            dynamic_policy: planner::dynamic_memory_policy_snapshot(mode, &policy),
        })
    }
}

fn value_patch<T: Clone>(change: &Change<T>) -> PatchValue<T> {
    match change {
        Change::Set(value) => PatchValue::Set(value.clone()),
        Change::Reset => PatchValue::UseLaunchDefault,
    }
}
