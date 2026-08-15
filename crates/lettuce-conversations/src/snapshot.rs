//! Closed, versioned launch documents.  These are deliberately copies of the
//! authored values needed to replay a conversation; they are not live domain
//! pointers and never contain adapter-specific maps or provider requests.

use lettuce_types::{
    CharacterId, ContentHash, ConversationStarterId, GroupId, LorebookId, MemoryRevisionId,
    ModelProfileId, PersonaId, PromptDocumentId, Revision, SceneId, SnapshotArtifactId,
    TimestampMillis, VoiceProfileId,
};
use serde::{Deserialize, Serialize};

use crate::ValidationError;
use crate::validation::{
    MAX_AUTHORED_TEXT_BYTES, MAX_DISPLAY_CHARS, MAX_LOREBOOKS, MAX_PARTICIPANTS, MAX_PARTS,
    validate_collection, validate_text, validate_unique,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SnapshotSelection<T> {
    Inherited(T),
    Explicit(T),
    Disabled,
}

impl<T> SnapshotSelection<T> {
    pub const fn is_explicit(&self) -> bool {
        matches!(self, Self::Explicit(_))
    }

    pub const fn is_resolved(&self) -> bool {
        matches!(self, Self::Inherited(_) | Self::Explicit(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtectedSnapshotRef {
    pub source: SnapshotSource,
    pub source_revision: Revision,
    pub artifact_id: SnapshotArtifactId,
    pub digest: ContentHash,
    pub schema_version: u32,
    pub byte_size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "id",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SnapshotSource {
    Character(CharacterId),
    Persona(PersonaId),
    Scene(SceneId),
    Starter(ConversationStarterId),
    Prompt(PromptDocumentId),
    Lorebook(LorebookId),
    Model(ModelProfileId),
    Voice(VoiceProfileId),
    Group(GroupId),
    ProviderAccount(lettuce_types::ProviderAccountId),
}

impl ProtectedSnapshotRef {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.schema_version == 0 || self.source_revision.get() == 0 {
            return Err(ValidationError::ZeroRevision);
        }
        if self.byte_size > 16 * 1024 * 1024 {
            return Err(ValidationError::TooLarge {
                field: "snapshot_ref.byte_size",
            });
        }
        Ok(())
    }
}

fn validate_source_ref(
    reference: &ProtectedSnapshotRef,
    expected: SnapshotSource,
    revision: Revision,
    field: &'static str,
) -> Result<(), ValidationError> {
    reference.validate()?;
    if reference.source != expected || reference.source_revision != revision {
        return Err(ValidationError::InvalidReference { field });
    }
    Ok(())
}

impl<T: ValidateSnapshot> SnapshotSelection<T> {
    pub fn validate(&self, field: &'static str) -> Result<(), ValidationError> {
        if let Self::Inherited(value) | Self::Explicit(value) = self {
            value.validate_snapshot(field)?;
        }
        Ok(())
    }
}

pub trait ValidateSnapshot {
    fn validate_snapshot(&self, field: &'static str) -> Result<(), ValidationError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterLaunchSnapshot {
    pub snapshot_ref: ProtectedSnapshotRef,
    pub source_id: CharacterId,
    pub source_revision: Revision,
    pub name: String,
    pub nickname: Option<String>,
}

impl CharacterLaunchSnapshot {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_snapshot("character_snapshot")
    }
}

impl ValidateSnapshot for CharacterLaunchSnapshot {
    fn validate_snapshot(&self, field: &'static str) -> Result<(), ValidationError> {
        if self.source_revision.get() == 0 {
            return Err(ValidationError::ZeroRevision);
        }
        validate_source_ref(
            &self.snapshot_ref,
            SnapshotSource::Character(self.source_id),
            self.source_revision,
            "character_snapshot.ref",
        )?;
        validate_text(field, &self.name, MAX_DISPLAY_CHARS * 4, false)?;
        for value in [self.nickname.as_deref()].into_iter().flatten() {
            validate_text(field, value, MAX_AUTHORED_TEXT_BYTES, true)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonaLaunchSnapshot {
    pub snapshot_ref: ProtectedSnapshotRef,
    pub source_id: PersonaId,
    pub source_revision: Revision,
    pub title: String,
    pub nickname: Option<String>,
}

impl PersonaLaunchSnapshot {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_snapshot("persona_snapshot")
    }
}

impl ValidateSnapshot for PersonaLaunchSnapshot {
    fn validate_snapshot(&self, field: &'static str) -> Result<(), ValidationError> {
        if self.source_revision.get() == 0 {
            return Err(ValidationError::ZeroRevision);
        }
        validate_source_ref(
            &self.snapshot_ref,
            SnapshotSource::Persona(self.source_id),
            self.source_revision,
            "persona_snapshot.ref",
        )?;
        validate_text(field, &self.title, MAX_DISPLAY_CHARS * 4, false)?;
        if let Some(value) = &self.nickname {
            validate_text(field, value, MAX_AUTHORED_TEXT_BYTES, true)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneLaunchSnapshot {
    pub snapshot_ref: ProtectedSnapshotRef,
    pub source_id: SceneId,
    pub source_revision: Revision,
    pub title: String,
}

impl SceneLaunchSnapshot {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_snapshot("scene_snapshot")
    }
}

impl ValidateSnapshot for SceneLaunchSnapshot {
    fn validate_snapshot(&self, field: &'static str) -> Result<(), ValidationError> {
        if self.source_revision.get() == 0 {
            return Err(ValidationError::ZeroRevision);
        }
        validate_source_ref(
            &self.snapshot_ref,
            SnapshotSource::Scene(self.source_id),
            self.source_revision,
            "scene_snapshot.ref",
        )?;
        validate_text(field, &self.title, MAX_DISPLAY_CHARS * 4, false)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StarterLaunchSnapshot {
    pub snapshot_ref: ProtectedSnapshotRef,
    pub source_id: ConversationStarterId,
    pub source_revision: Revision,
    pub title: String,
}

impl StarterLaunchSnapshot {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_snapshot("starter_snapshot")
    }
}

impl ValidateSnapshot for StarterLaunchSnapshot {
    fn validate_snapshot(&self, field: &'static str) -> Result<(), ValidationError> {
        if self.source_revision.get() == 0 {
            return Err(ValidationError::ZeroRevision);
        }
        validate_source_ref(
            &self.snapshot_ref,
            SnapshotSource::Starter(self.source_id),
            self.source_revision,
            "starter_snapshot.ref",
        )?;
        validate_text(field, &self.title, MAX_DISPLAY_CHARS * 4, false)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptLaunchSnapshot {
    pub snapshot_ref: ProtectedSnapshotRef,
    pub source_id: PromptDocumentId,
    pub source_revision: Revision,
    pub title: String,
    pub purpose: PromptPurposeSnapshot,
}

impl PromptLaunchSnapshot {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_snapshot("prompt_snapshot")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptPurposeSnapshot {
    Direct,
    GroupConversational,
    GroupRoleplay,
    Other,
}

impl ValidateSnapshot for PromptLaunchSnapshot {
    fn validate_snapshot(&self, field: &'static str) -> Result<(), ValidationError> {
        if self.source_revision.get() == 0 {
            return Err(ValidationError::ZeroRevision);
        }
        validate_source_ref(
            &self.snapshot_ref,
            SnapshotSource::Prompt(self.source_id),
            self.source_revision,
            "prompt_snapshot.ref",
        )?;
        validate_text(field, &self.title, MAX_DISPLAY_CHARS * 4, false)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LorebookLaunchSnapshot {
    pub snapshot_ref: ProtectedSnapshotRef,
    pub source_id: LorebookId,
    pub source_revision: Revision,
    pub name: String,
}

impl LorebookLaunchSnapshot {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_snapshot("lorebook_snapshot")
    }
}

impl ValidateSnapshot for LorebookLaunchSnapshot {
    fn validate_snapshot(&self, field: &'static str) -> Result<(), ValidationError> {
        if self.source_revision.get() == 0 {
            return Err(ValidationError::ZeroRevision);
        }
        validate_source_ref(
            &self.snapshot_ref,
            SnapshotSource::Lorebook(self.source_id),
            self.source_revision,
            "lorebook_snapshot.ref",
        )?;
        validate_text(field, &self.name, MAX_DISPLAY_CHARS * 4, false)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSelectionSnapshot {
    pub snapshot_ref: ProtectedSnapshotRef,
    pub source_id: ModelProfileId,
    pub source_revision: Revision,
    pub provider_kind: ModelProviderKind,
    pub external_model_id: String,
    pub display_name: String,
    pub context_length: Option<u32>,
    pub max_output_tokens: Option<u32>,
}

impl ModelSelectionSnapshot {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_snapshot("model_snapshot")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelProviderKind {
    OpenAiCompatible,
    Anthropic,
    Gemini,
    Ollama,
    LlamaCpp,
    Other,
}

impl ValidateSnapshot for ModelSelectionSnapshot {
    fn validate_snapshot(&self, field: &'static str) -> Result<(), ValidationError> {
        if self.source_revision.get() == 0 {
            return Err(ValidationError::ZeroRevision);
        }
        validate_source_ref(
            &self.snapshot_ref,
            SnapshotSource::Model(self.source_id),
            self.source_revision,
            "model_snapshot.ref",
        )?;
        validate_text(field, &self.external_model_id, MAX_DISPLAY_CHARS * 4, false)?;
        validate_text(field, &self.display_name, MAX_DISPLAY_CHARS * 4, false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemorySettingsSnapshot {
    pub policy_ref: Option<ProtectedSnapshotRef>,
    pub mode: MemoryModeSnapshot,
    pub selected_revision_ids: Vec<MemoryRevisionId>,
}

impl MemorySettingsSnapshot {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_snapshot("memory_snapshot")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryModeSnapshot {
    Manual,
    Dynamic,
    Disabled,
}

impl ValidateSnapshot for MemorySettingsSnapshot {
    fn validate_snapshot(&self, field: &'static str) -> Result<(), ValidationError> {
        if self.mode == MemoryModeSnapshot::Disabled
            && (!self.selected_revision_ids.is_empty() || self.policy_ref.is_some())
        {
            return Err(ValidationError::InvalidReference { field });
        }
        validate_collection(field, &self.selected_revision_ids, MAX_PARTS)?;
        validate_unique(field, self.selected_revision_ids.iter().copied())?;
        if let Some(policy_ref) = &self.policy_ref {
            policy_ref.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceSettingsSnapshot {
    pub snapshot_ref: ProtectedSnapshotRef,
    pub source_id: VoiceProfileId,
    pub source_revision: Revision,
    pub display_name: String,
    pub autoplay: bool,
}

impl VoiceSettingsSnapshot {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_snapshot("voice_snapshot")
    }
}

impl ValidateSnapshot for VoiceSettingsSnapshot {
    fn validate_snapshot(&self, field: &'static str) -> Result<(), ValidationError> {
        if self.source_revision.get() == 0 {
            return Err(ValidationError::ZeroRevision);
        }
        validate_source_ref(
            &self.snapshot_ref,
            SnapshotSource::Voice(self.source_id),
            self.source_revision,
            "voice_snapshot.ref",
        )?;
        validate_text(field, &self.display_name, MAX_DISPLAY_CHARS * 4, false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectConversationDetails {
    pub format_version: u32,
    pub character: CharacterLaunchSnapshot,
    pub persona: SnapshotSelection<PersonaLaunchSnapshot>,
    pub scene: SnapshotSelection<SceneLaunchSnapshot>,
    pub starter: SnapshotSelection<StarterLaunchSnapshot>,
    pub prompt: SnapshotSelection<PromptLaunchSnapshot>,
    pub lorebooks: SnapshotSelection<Vec<LorebookLaunchSnapshot>>,
    pub model: SnapshotSelection<ModelSelectionSnapshot>,
    pub memory: SnapshotSelection<MemorySettingsSnapshot>,
    pub voice: SnapshotSelection<VoiceSettingsSnapshot>,
}

impl DirectConversationDetails {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.format_version != 1 {
            return Err(ValidationError::UnsupportedVersion {
                field: "direct_snapshot",
                version: self.format_version,
            });
        }
        self.character.validate_snapshot("direct.character")?;
        self.persona.validate("direct.persona")?;
        self.scene.validate("direct.scene")?;
        self.starter.validate("direct.starter")?;
        self.prompt.validate("direct.prompt")?;
        if let SnapshotSelection::Inherited(prompt) | SnapshotSelection::Explicit(prompt) =
            &self.prompt
        {
            if prompt.purpose != PromptPurposeSnapshot::Direct {
                return Err(ValidationError::InvalidReference {
                    field: "direct.prompt.purpose",
                });
            }
        }
        self.model.validate("direct.model")?;
        self.memory.validate("direct.memory")?;
        self.voice.validate("direct.voice")?;
        validate_lorebooks(&self.lorebooks, "direct.lorebooks")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupMemberLaunchSnapshot {
    pub character: CharacterLaunchSnapshot,
    pub ordinal: u32,
    pub enabled: bool,
    pub muted: bool,
    pub model_override: SnapshotSelection<ModelSelectionSnapshot>,
}

impl GroupMemberLaunchSnapshot {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_snapshot("group_member_snapshot")
    }
}

impl ValidateSnapshot for GroupMemberLaunchSnapshot {
    fn validate_snapshot(&self, field: &'static str) -> Result<(), ValidationError> {
        self.character.validate_snapshot(field)?;
        self.model_override.validate(field)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupLaunchSnapshot {
    pub snapshot_ref: ProtectedSnapshotRef,
    pub source_id: GroupId,
    pub source_revision: Revision,
    pub name: String,
    pub members: Vec<GroupMemberLaunchSnapshot>,
    pub chat_mode: GroupChatModeSnapshot,
    pub speaker_selection: GroupSpeakerSelectionSnapshot,
    pub memory: SnapshotSelection<MemorySettingsSnapshot>,
    pub disable_character_lorebook: bool,
    pub persona: SnapshotSelection<PersonaLaunchSnapshot>,
    pub scene: SnapshotSelection<SceneLaunchSnapshot>,
    pub prompt: SnapshotSelection<PromptLaunchSnapshot>,
    pub lorebooks: SnapshotSelection<Vec<LorebookLaunchSnapshot>>,
    pub model: SnapshotSelection<ModelSelectionSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupChatModeSnapshot {
    Conversation,
    Roleplay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupSpeakerSelectionSnapshot {
    Llm,
    Heuristic,
    RoundRobin,
    Director,
    DirectorAction,
}

impl GroupLaunchSnapshot {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_source_ref(
            &self.snapshot_ref,
            SnapshotSource::Group(self.source_id),
            self.source_revision,
            "group_snapshot.ref",
        )?;
        if self.source_revision.get() == 0 {
            return Err(ValidationError::ZeroRevision);
        }
        validate_text("group.name", &self.name, MAX_DISPLAY_CHARS * 4, false)?;
        validate_collection("group.members", &self.members, MAX_PARTICIPANTS)?;
        if self.members.len() < 2 {
            return Err(ValidationError::Invariant {
                field: "group.members.minimum",
            });
        }
        validate_unique(
            "group.member.characters",
            self.members.iter().map(|m| m.character.source_id),
        )?;
        for (ordinal, member) in self.members.iter().enumerate() {
            if member.ordinal as usize != ordinal {
                return Err(ValidationError::InvalidValue {
                    field: "group.members.ordinal",
                });
            }
            member.validate_snapshot("group.member")?;
        }
        if self.members.iter().all(|member| member.muted) {
            return Err(ValidationError::Invariant {
                field: "group.members.active",
            });
        }
        self.memory.validate("group.memory")?;
        self.persona.validate("group.persona")?;
        self.scene.validate("group.scene")?;
        self.prompt.validate("group.prompt")?;
        self.model.validate("group.model")?;
        validate_lorebooks(&self.lorebooks, "group.lorebooks")
    }
}

fn validate_lorebooks(
    selection: &SnapshotSelection<Vec<LorebookLaunchSnapshot>>,
    field: &'static str,
) -> Result<(), ValidationError> {
    if let SnapshotSelection::Inherited(books) | SnapshotSelection::Explicit(books) = selection {
        validate_collection(field, books, MAX_LOREBOOKS)?;
        validate_unique(field, books.iter().map(|book| book.source_id))?;
        for book in books {
            book.validate_snapshot(field)?;
        }
    }
    Ok(())
}

impl ValidateSnapshot for Vec<LorebookLaunchSnapshot> {
    fn validate_snapshot(&self, field: &'static str) -> Result<(), ValidationError> {
        validate_lorebooks(&SnapshotSelection::Explicit(self.clone()), field)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupParticipantPolicySnapshot {
    pub participant_id: lettuce_types::ConversationParticipantId,
    pub enabled: bool,
    pub muted: bool,
    pub model_override: SnapshotSelection<ModelSelectionSnapshot>,
}

impl GroupParticipantPolicySnapshot {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.model_override
            .validate("group.participant_policy.model")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupParticipantPolicyDocument {
    pub members: Vec<GroupParticipantPolicySnapshot>,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupConversationDetails {
    pub format_version: u32,
    pub group: GroupLaunchSnapshot,
    #[serde(rename = "participant_policy")]
    pub initial_participant_policy: GroupParticipantPolicyDocument,
}

impl GroupConversationDetails {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.format_version != 1 {
            return Err(ValidationError::UnsupportedVersion {
                field: "group_snapshot",
                version: self.format_version,
            });
        }
        self.group.validate()?;
        if let SnapshotSelection::Inherited(prompt) | SnapshotSelection::Explicit(prompt) =
            &self.group.prompt
        {
            let expected = match self.group.chat_mode {
                GroupChatModeSnapshot::Conversation => PromptPurposeSnapshot::GroupConversational,
                GroupChatModeSnapshot::Roleplay => PromptPurposeSnapshot::GroupRoleplay,
            };
            if prompt.purpose != expected {
                return Err(ValidationError::InvalidReference {
                    field: "group.prompt.purpose",
                });
            }
        }
        self.initial_participant_policy.validate()
    }
}

impl GroupParticipantPolicyDocument {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.revision.get() == 0 || self.created_at > self.updated_at {
            return Err(ValidationError::InvalidValue {
                field: "group.participant_policy.metadata",
            });
        }
        validate_collection(
            "group.participant_policy.members",
            &self.members,
            MAX_PARTICIPANTS,
        )?;
        validate_unique(
            "group.participant_policy.participants",
            self.members.iter().map(|member| member.participant_id),
        )?;
        for member in &self.members {
            member.validate()?;
        }
        Ok(())
    }
}
