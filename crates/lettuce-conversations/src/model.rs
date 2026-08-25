use lettuce_types::{
    CharacterId, ConversationBranchId, ConversationId, ConversationParticipantId, GroupId,
    Revision, TimestampMillis,
};
use serde::{Deserialize, Serialize};

use crate::ValidationError;
use crate::content::MessageRole;
use crate::snapshot::{
    DirectConversationDetails, GroupConversationDetails, GroupLaunchSnapshot,
    ModelSelectionSnapshot, SnapshotSelection,
};
use crate::validation::{
    MAX_DISPLAY_CHARS, MAX_LOREBOOKS, MAX_PARTICIPANTS, validate_collection,
    validate_revision_timestamps, validate_text, validate_unique,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
#[serde(
    tag = "kind",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ConversationKind {
    Direct(DirectConversationDetails),
    Group(GroupConversationDetails),
}

impl ConversationKind {
    pub fn validate(&self) -> Result<(), ValidationError> {
        match self {
            Self::Direct(details) => details.validate(),
            Self::Group(details) => details.validate(),
        }
    }

    #[must_use]
    pub const fn is_group(&self) -> bool {
        matches!(self, Self::Group(_))
    }
}

/// Conversation-local participants preserve the launch-time display and
/// model values. Persisted participants always have a typed source identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationParticipant {
    pub id: ConversationParticipantId,
    pub role: ParticipantRole,
    pub ordinal: u32,
    pub enabled: bool,
    pub muted: bool,
    pub source: ParticipantSource,
    pub display_name: String,
    pub authored_description: Option<String>,
    pub model_selection: SnapshotSelection<ModelSelectionSnapshot>,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantRole {
    User,
    Character,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "id",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ParticipantSource {
    User,
    Character(CharacterId),
    System,
}

/// The materialized settings record stored with a conversation.  Commands
/// carry [`CurrentConversationSettingsPatch`] values; this type is the
/// resolved, revisioned state returned by reads and mutations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurrentConversationSettings {
    pub revision: Revision,
    pub author_note: Option<String>,
    pub author_note_provenance: crate::commands::SettingProvenance,
    pub memory: Option<crate::snapshot::MemorySettingsSnapshot>,
    pub memory_provenance: crate::commands::SettingProvenance,
    pub model_override: Option<ModelSelectionSnapshot>,
    pub model_provenance: crate::commands::SettingProvenance,
    pub voice: Option<crate::snapshot::VoiceSettingsSnapshot>,
    pub voice_provenance: crate::commands::SettingProvenance,
    pub prompt: Option<crate::snapshot::PromptLaunchSnapshot>,
    pub prompt_provenance: crate::commands::SettingProvenance,
    pub lorebooks: Option<Vec<crate::snapshot::LorebookLaunchSnapshot>>,
    pub lorebooks_provenance: crate::commands::SettingProvenance,
    pub persona: Option<crate::snapshot::PersonaLaunchSnapshot>,
    pub persona_provenance: crate::commands::SettingProvenance,
    pub scene: Option<crate::snapshot::SceneLaunchSnapshot>,
    pub scene_provenance: crate::commands::SettingProvenance,
}

impl CurrentConversationSettings {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.revision.get() == 0 {
            return Err(ValidationError::ZeroRevision);
        }
        if self
            .author_note
            .as_deref()
            .is_some_and(|note| note.trim().is_empty())
        {
            return Err(ValidationError::Blank {
                field: "conversation_settings.author_note",
            });
        }
        if self
            .author_note
            .as_deref()
            .is_some_and(|note| note.len() > crate::validation::MAX_AUTHORED_TEXT_BYTES)
        {
            return Err(ValidationError::TooLarge {
                field: "conversation_settings.author_note",
            });
        }
        if (!matches!(
            self.author_note_provenance,
            crate::commands::SettingProvenance::CurrentOverride
        )) && self.author_note.is_some()
            || (self.author_note_provenance == crate::commands::SettingProvenance::CurrentOverride
                && self.author_note.is_none())
        {
            return Err(ValidationError::InvalidReference {
                field: "conversation_settings.author_note_provenance",
            });
        }
        if (!matches!(
            self.memory_provenance,
            crate::commands::SettingProvenance::CurrentOverride
        )) && self.memory.is_some()
            || (self.memory_provenance == crate::commands::SettingProvenance::CurrentOverride
                && self.memory.is_none())
        {
            return Err(ValidationError::InvalidReference {
                field: "conversation_settings.memory_provenance",
            });
        }
        if (!matches!(
            self.model_provenance,
            crate::commands::SettingProvenance::CurrentOverride
        )) && self.model_override.is_some()
            || (self.model_provenance == crate::commands::SettingProvenance::CurrentOverride
                && self.model_override.is_none())
        {
            return Err(ValidationError::InvalidReference {
                field: "conversation_settings.model_provenance",
            });
        }
        if (!matches!(
            self.voice_provenance,
            crate::commands::SettingProvenance::CurrentOverride
        )) && self.voice.is_some()
            || (self.voice_provenance == crate::commands::SettingProvenance::CurrentOverride
                && self.voice.is_none())
        {
            return Err(ValidationError::InvalidReference {
                field: "conversation_settings.voice_provenance",
            });
        }
        if (!matches!(
            self.prompt_provenance,
            crate::commands::SettingProvenance::CurrentOverride
        )) && self.prompt.is_some()
            || (self.prompt_provenance == crate::commands::SettingProvenance::CurrentOverride
                && self.prompt.is_none())
        {
            return Err(ValidationError::InvalidReference {
                field: "conversation_settings.prompt_provenance",
            });
        }
        if (!matches!(
            self.lorebooks_provenance,
            crate::commands::SettingProvenance::CurrentOverride
        )) && self.lorebooks.is_some()
            || (self.lorebooks_provenance == crate::commands::SettingProvenance::CurrentOverride
                && self.lorebooks.is_none())
        {
            return Err(ValidationError::InvalidReference {
                field: "conversation_settings.lorebooks_provenance",
            });
        }
        if (!matches!(
            self.persona_provenance,
            crate::commands::SettingProvenance::CurrentOverride
        )) && self.persona.is_some()
            || (self.persona_provenance == crate::commands::SettingProvenance::CurrentOverride
                && self.persona.is_none())
        {
            return Err(ValidationError::InvalidReference {
                field: "conversation_settings.persona_provenance",
            });
        }
        if (!matches!(
            self.scene_provenance,
            crate::commands::SettingProvenance::CurrentOverride
        )) && self.scene.is_some()
            || (self.scene_provenance == crate::commands::SettingProvenance::CurrentOverride
                && self.scene.is_none())
        {
            return Err(ValidationError::InvalidReference {
                field: "conversation_settings.scene_provenance",
            });
        }
        if let Some(memory) = &self.memory {
            memory.validate()?;
        }
        if let Some(model) = &self.model_override {
            model.validate()?;
        }
        if let Some(voice) = &self.voice {
            voice.validate()?;
        }
        if let Some(prompt) = &self.prompt {
            prompt.validate()?;
        }
        if let Some(lorebooks) = &self.lorebooks {
            if lorebooks.is_empty() {
                return Err(ValidationError::InvalidValue {
                    field: "conversation_settings.lorebooks",
                });
            }
            validate_collection("conversation_settings.lorebooks", lorebooks, MAX_LOREBOOKS)?;
            validate_unique(
                "conversation_settings.lorebook_ids",
                lorebooks.iter().map(|book| book.source_id),
            )?;
            for book in lorebooks {
                book.validate()?;
            }
        }
        if let Some(persona) = &self.persona {
            persona.validate()?;
        }
        if let Some(scene) = &self.scene {
            scene.validate()?;
        }
        Ok(())
    }

    pub fn validate_against_kind(&self, kind: &ConversationKind) -> Result<(), ValidationError> {
        self.validate()?;
        let expected_prompt = match kind {
            ConversationKind::Direct(_) => crate::snapshot::PromptPurposeSnapshot::Direct,
            ConversationKind::Group(details) => match details.group.chat_mode {
                crate::snapshot::GroupChatModeSnapshot::Conversation => {
                    crate::snapshot::PromptPurposeSnapshot::GroupConversational
                }
                crate::snapshot::GroupChatModeSnapshot::Roleplay => {
                    crate::snapshot::PromptPurposeSnapshot::GroupRoleplay
                }
            },
        };
        if self
            .prompt
            .as_ref()
            .is_some_and(|prompt| prompt.purpose != expected_prompt)
        {
            return Err(ValidationError::InvalidReference {
                field: "conversation_settings.prompt.purpose",
            });
        }
        if matches!(
            kind,
            ConversationKind::Group(details)
                if details.group.chat_mode
                    == crate::snapshot::GroupChatModeSnapshot::Conversation
                    && self.scene.is_some()
        ) {
            return Err(ValidationError::InvalidReference {
                field: "conversation_settings.scene",
            });
        }
        Ok(())
    }
}

impl ConversationParticipant {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_revision_timestamps(
            "participant.timestamps",
            self.revision,
            self.created_at,
            self.updated_at,
        )?;
        validate_text(
            "participant.display_name",
            &self.display_name,
            MAX_DISPLAY_CHARS * 4,
            false,
        )?;
        if let Some(description) = &self.authored_description {
            validate_text(
                "participant.description",
                description,
                crate::validation::MAX_AUTHORED_TEXT_BYTES,
                true,
            )?;
        }
        self.model_selection.validate("participant.model")?;
        let valid_source = matches!(
            (self.role, self.source),
            (ParticipantRole::User, ParticipantSource::User)
                | (ParticipantRole::Character, ParticipantSource::Character(_))
                | (ParticipantRole::System, ParticipantSource::System)
        );
        if !valid_source {
            return Err(ValidationError::InvalidReference {
                field: "participant.source",
            });
        }
        if self.role != ParticipantRole::Character
            && !matches!(self.model_selection, SnapshotSelection::Disabled)
        {
            return Err(ValidationError::InvalidReference {
                field: "participant.non_character_model",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationLifecycle {
    Active,
    Archived,
    Tombstoned,
}

impl ConversationLifecycle {
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Active, Self::Archived | Self::Tombstoned)
                | (Self::Archived, Self::Active | Self::Tombstoned)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conversation {
    pub id: ConversationId,
    pub lifecycle: ConversationLifecycle,
    pub title: String,
    pub kind: ConversationKind,
    pub active_branch_id: ConversationBranchId,
    pub participants: Vec<ConversationParticipant>,
    /// Mutable session/settings layer; launch snapshots in `kind` remain immutable.
    pub current_settings: Option<CurrentConversationSettings>,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

impl Conversation {
    pub fn transition_to(
        &mut self,
        next: ConversationLifecycle,
        now: TimestampMillis,
    ) -> Result<(), ValidationError> {
        if !self.lifecycle.can_transition_to(next) {
            return Err(ValidationError::IllegalTransition {
                field: "conversation.lifecycle",
            });
        }
        self.lifecycle = next;
        self.revision = self
            .revision
            .next()
            .map_err(|_| ValidationError::OutOfBounds {
                field: "conversation.revision",
            })?;
        self.updated_at = now;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_revision_timestamps(
            "conversation.timestamps",
            self.revision,
            self.created_at,
            self.updated_at,
        )?;
        validate_text(
            "conversation.title",
            &self.title,
            MAX_DISPLAY_CHARS * 4,
            false,
        )?;
        self.kind.validate()?;
        validate_collection(
            "conversation.participants",
            &self.participants,
            MAX_PARTICIPANTS,
        )?;
        validate_unique(
            "conversation.participant_ids",
            self.participants.iter().map(|participant| participant.id),
        )?;
        for (ordinal, participant) in self.participants.iter().enumerate() {
            if participant.ordinal as usize != ordinal {
                return Err(ValidationError::InvalidValue {
                    field: "conversation.participant_ordinals",
                });
            }
            participant.validate()?;
        }
        if let Some(settings) = &self.current_settings {
            settings.validate_against_kind(&self.kind)?;
        }
        match &self.kind {
            ConversationKind::Direct(_) => {
                if self.participants.len() != 2
                    || self
                        .participants
                        .iter()
                        .filter(|participant| participant.role == ParticipantRole::User)
                        .count()
                        != 1
                    || self
                        .participants
                        .iter()
                        .filter(|participant| participant.role == ParticipantRole::Character)
                        .count()
                        != 1
                {
                    return Err(ValidationError::Invariant {
                        field: "conversation.direct.participants",
                    });
                }
                if let ConversationKind::Direct(details) = &self.kind {
                    let character = self
                        .participants
                        .iter()
                        .find(|p| p.role == ParticipantRole::Character);
                    if !character.is_some_and(|p| {
                        p.source == ParticipantSource::Character(details.character.source_id)
                    }) {
                        return Err(ValidationError::InvalidReference {
                            field: "conversation.direct.character_source",
                        });
                    }
                }
            }
            ConversationKind::Group(details) => {
                details.validate_participants(&self.participants)?;
                let characters: std::collections::HashMap<_, _> = self
                    .participants
                    .iter()
                    .filter_map(|participant| match participant.source {
                        ParticipantSource::Character(id) => Some((id, participant)),
                        _ => None,
                    })
                    .collect();
                if characters.len() != details.group.members.len()
                    || details.group.members.iter().any(|member| {
                        characters
                            .get(&member.character.source_id)
                            .is_none_or(|participant| participant.ordinal != member.ordinal + 1)
                    })
                {
                    return Err(ValidationError::InvalidReference {
                        field: "conversation.group.member_bijection",
                    });
                }
                let policy_ids: std::collections::HashSet<_> = details
                    .initial_participant_policy
                    .members
                    .iter()
                    .map(|member| member.participant_id)
                    .collect();
                let character_ids: std::collections::HashSet<_> = self
                    .participants
                    .iter()
                    .filter(|p| p.role == ParticipantRole::Character)
                    .map(|p| p.id)
                    .collect();
                if policy_ids != character_ids {
                    return Err(ValidationError::InvalidReference {
                        field: "conversation.group.policy_ids",
                    });
                }
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn settings_revision(&self) -> Option<Revision> {
        self.current_settings
            .as_ref()
            .map(|settings| settings.revision)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationBranch {
    pub id: ConversationBranchId,
    pub conversation_id: ConversationId,
    pub parent_branch_id: Option<ConversationBranchId>,
    pub fork_message_id: Option<lettuce_types::MessageId>,
    pub head_message_id: Option<lettuce_types::MessageId>,
    pub status: BranchStatus,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BranchStatus {
    Active,
    Archived,
    Tombstoned,
}

impl ConversationBranch {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_revision_timestamps(
            "branch.timestamps",
            self.revision,
            self.created_at,
            self.updated_at,
        )?;
        if self.parent_branch_id == Some(self.id) {
            return Err(ValidationError::Invariant {
                field: "branch.parent_self",
            });
        }
        if self.parent_branch_id.is_some() != self.fork_message_id.is_some() {
            return Err(ValidationError::InvalidReference {
                field: "branch.parent_fork_pair",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationAggregate {
    pub conversation: Conversation,
    pub branches: Vec<ConversationBranch>,
}

impl ConversationAggregate {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.conversation.validate()?;
        validate_collection(
            "conversation.branches",
            &self.branches,
            crate::validation::MAX_BRANCHES,
        )?;
        validate_unique(
            "conversation.branch_ids",
            self.branches.iter().map(|branch| branch.id),
        )?;
        let root_count = self
            .branches
            .iter()
            .filter(|branch| branch.parent_branch_id.is_none())
            .count();
        if root_count != 1
            || !self.branches.iter().any(|branch| {
                branch.id == self.conversation.active_branch_id
                    && branch.status == BranchStatus::Active
            })
        {
            return Err(ValidationError::Invariant {
                field: "conversation.branches.root_active",
            });
        }
        for branch in &self.branches {
            branch.validate()?;
            if branch.conversation_id != self.conversation.id {
                return Err(ValidationError::InvalidReference {
                    field: "branch.conversation_id",
                });
            }
        }
        // A bounded walk proves that parent links cannot contain a cycle.
        for branch in &self.branches {
            let mut current = branch.parent_branch_id;
            let mut steps = 0usize;
            while let Some(parent) = current {
                steps += 1;
                if steps > self.branches.len() {
                    return Err(ValidationError::Invariant {
                        field: "branch.acyclic",
                    });
                }
                current = self
                    .branches
                    .iter()
                    .find(|candidate| candidate.id == parent)
                    .ok_or(ValidationError::InvalidReference {
                        field: "branch.parent_branch_id",
                    })?
                    .parent_branch_id;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationHistoryItem {
    pub message: crate::content::Message,
    pub active_revision: Option<crate::content::MessageRevision>,
    pub active_candidate: Option<crate::content::MessageCandidate>,
    pub depth: u32,
    pub order: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationHistory {
    pub conversation_id: ConversationId,
    pub branch_id: ConversationBranchId,
    /// `None` on a freshly forked branch, whose timeline is then the inherited
    /// ancestry ending at `fork_message_id`.
    pub head_message_id: Option<lettuce_types::MessageId>,
    pub fork_message_id: Option<lettuce_types::MessageId>,
    /// The complete root-to-selected branch ancestry.  A page may omit
    /// ancestors, but a full history may not: this is what lets validation
    /// distinguish a legal fork prefix from an unrelated sibling.
    pub branches: Vec<ConversationBranch>,
    pub items: Vec<ConversationHistoryItem>,
    pub turns: Vec<crate::generation::GenerationTurn>,
}

impl ConversationHistory {
    pub fn validate(
        &self,
        participants: &[ConversationParticipant],
    ) -> Result<(), ValidationError> {
        validate_collection(
            "conversation_history.branches",
            &self.branches,
            crate::validation::MAX_BRANCHES,
        )?;
        if self.branches.is_empty() {
            return Err(ValidationError::Invariant {
                field: "conversation_history.branches",
            });
        }
        let mut branch_records = self.branches.clone();
        // Accept either serialization direction at the boundary, but always
        // validate the ancestry as root-to-selected internally.
        if branch_records
            .first()
            .is_some_and(|branch| branch.parent_branch_id.is_some())
            && branch_records
                .last()
                .is_some_and(|branch| branch.parent_branch_id.is_none())
        {
            branch_records.reverse();
        }
        if branch_records
            .first()
            .is_none_or(|branch| branch.parent_branch_id.is_some())
        {
            return Err(ValidationError::InvalidReference {
                field: "conversation_history.branch_root",
            });
        }
        validate_unique(
            "conversation_history.branch_ids",
            branch_records.iter().map(|branch| branch.id),
        )?;
        for (index, branch) in branch_records.iter().enumerate() {
            branch.validate()?;
            if branch.conversation_id != self.conversation_id {
                return Err(ValidationError::InvalidReference {
                    field: "conversation_history.branch_conversation",
                });
            }
            if index > 0 {
                if branch.parent_branch_id != Some(branch_records[index - 1].id)
                    || branch.fork_message_id.is_none()
                {
                    return Err(ValidationError::InvalidReference {
                        field: "conversation_history.branch_ancestry",
                    });
                }
            } else if branch.fork_message_id.is_some() {
                return Err(ValidationError::InvalidReference {
                    field: "conversation_history.branch_root_fork",
                });
            }
        }
        let selected_branch_index = branch_records
            .iter()
            .position(|branch| branch.id == self.branch_id)
            .ok_or(ValidationError::InvalidReference {
                field: "conversation_history.selected_branch",
            })?;
        if branch_records[selected_branch_index].status != BranchStatus::Active {
            return Err(ValidationError::Invariant {
                field: "conversation_history.selected_branch_active",
            });
        }
        if self.fork_message_id != branch_records[selected_branch_index].fork_message_id {
            return Err(ValidationError::InvalidReference {
                field: "conversation_history.fork",
            });
        }
        if self.items.len() > crate::validation::MAX_PARTS * 32 {
            return Err(ValidationError::TooMany {
                field: "conversation_history.items",
                max: crate::validation::MAX_PARTS * 32,
            });
        }
        let participant_by_id: std::collections::HashMap<_, _> = participants
            .iter()
            .map(|participant| (participant.id, participant))
            .collect();
        let participant_ids: std::collections::HashSet<_> =
            participant_by_id.keys().copied().collect();
        if self.turns.len() > crate::validation::MAX_ATTEMPTS * 32 {
            return Err(ValidationError::TooMany {
                field: "conversation_history.turns",
                max: crate::validation::MAX_ATTEMPTS * 32,
            });
        }
        validate_unique(
            "conversation_history.turn_ids",
            self.turns.iter().map(|turn| turn.id),
        )?;
        let turn_ids: std::collections::HashMap<_, _> =
            self.turns.iter().map(|turn| (turn.id, turn)).collect();
        let ancestry_ids: std::collections::HashSet<_> =
            branch_records.iter().map(|branch| branch.id).collect();
        let is_group = participants
            .iter()
            .filter(|participant| participant.role == ParticipantRole::Character)
            .count()
            > 1;
        for turn in &self.turns {
            if turn.conversation_id != self.conversation_id
                || !ancestry_ids.contains(&turn.branch_id)
            {
                return Err(ValidationError::InvalidReference {
                    field: "conversation_history.turn_identity",
                });
            }
            turn.validate(is_group)?;
        }
        let mut ids = std::collections::HashSet::new();
        let mut by_id = std::collections::HashMap::new();
        for item in &self.items {
            item.message.validate()?;
            if item.message.conversation_id != self.conversation_id
                || !ancestry_ids.contains(&item.message.branch_id)
                || !ids.insert(item.message.id)
            {
                return Err(ValidationError::Invariant {
                    field: "conversation_history.identity",
                });
            }
            let author = item
                .message
                .author_participant_id
                .and_then(|id| participant_by_id.get(&id).copied());
            match (item.message.role, author) {
                (MessageRole::User, Some(participant))
                    if participant.role == ParticipantRole::User => {}
                (MessageRole::Assistant, Some(participant))
                    if participant.role == ParticipantRole::Character => {}
                (MessageRole::System, None) => {}
                (MessageRole::Scene, None) => {}
                _ => {
                    return Err(ValidationError::InvalidReference {
                        field: "conversation_history.author_role",
                    });
                }
            }
            if let Some(author) = item.message.author_participant_id {
                if !participant_ids.contains(&author) {
                    return Err(ValidationError::InvalidReference {
                        field: "conversation_history.author",
                    });
                }
            }
            match item.message.active_render_source {
                crate::content::MessageRenderSource::Revision(id) => {
                    let revision =
                        item.active_revision
                            .as_ref()
                            .ok_or(ValidationError::InvalidReference {
                                field: "conversation_history.render_revision",
                            })?;
                    if revision.id != id
                        || revision.message_id != item.message.id
                        || item.active_candidate.is_some()
                    {
                        return Err(ValidationError::InvalidReference {
                            field: "conversation_history.render_revision",
                        });
                    }
                    revision.validate()?;
                    if let Some(turn_id) = revision.source_turn_id {
                        if !turn_ids.contains_key(&turn_id) {
                            return Err(ValidationError::InvalidReference {
                                field: "conversation_history.revision_turn",
                            });
                        }
                    }
                }
                crate::content::MessageRenderSource::Candidate(id) => {
                    let candidate = item.active_candidate.as_ref().ok_or(
                        ValidationError::InvalidReference {
                            field: "conversation_history.render_candidate",
                        },
                    )?;
                    if candidate.id != id
                        || candidate.message_id != item.message.id
                        || item.active_revision.is_some()
                    {
                        return Err(ValidationError::InvalidReference {
                            field: "conversation_history.render_candidate",
                        });
                    }
                    candidate.validate()?;
                    if participant_by_id
                        .get(&candidate.author_participant_id)
                        .is_none_or(|participant| participant.role != ParticipantRole::Character)
                    {
                        return Err(ValidationError::InvalidReference {
                            field: "conversation_history.candidate_author",
                        });
                    }
                    if item.message.author_participant_id != Some(candidate.author_participant_id) {
                        return Err(ValidationError::InvalidReference {
                            field: "conversation_history.candidate_author_speaker",
                        });
                    }
                    let turn = turn_ids.get(&candidate.turn_id).ok_or(
                        ValidationError::InvalidReference {
                            field: "conversation_history.candidate_turn",
                        },
                    )?;
                    if !turn.attempts.iter().any(|attempt| {
                        attempt.id == candidate.attempt_id
                            && attempt.candidate_ids.contains(&candidate.id)
                    }) || !turn.candidate_ids.contains(&candidate.id)
                    {
                        return Err(ValidationError::InvalidReference {
                            field: "conversation_history.candidate_attempt",
                        });
                    }
                }
            }
            by_id.insert(item.message.id, item);
        }
        let path_tip = self.head_message_id.or(self.fork_message_id);
        if path_tip.is_none() && !self.items.is_empty() {
            return Err(ValidationError::Invariant {
                field: "conversation_history.empty_head",
            });
        }
        let mut path_reverse = Vec::new();
        let mut current = path_tip;
        let mut visited = std::collections::HashSet::new();
        while let Some(id) = current {
            if !visited.insert(id) {
                return Err(ValidationError::Invariant {
                    field: "conversation_history.parent_cycle",
                });
            }
            let item = by_id.get(&id).ok_or(ValidationError::InvalidReference {
                field: "conversation_history.head_parent",
            })?;
            path_reverse.push(id);
            current = item.message.parent_message_id;
        }
        path_reverse.reverse();
        if path_reverse.len() != self.items.len() {
            return Err(ValidationError::Invariant {
                field: "conversation_history.unrelated_item",
            });
        }
        let path_index: std::collections::HashMap<_, _> = path_reverse
            .iter()
            .enumerate()
            .map(|(index, id)| (*id, index))
            .collect();
        for (index, item) in self.items.iter().enumerate() {
            if item.message.id != path_reverse[index]
                || item.depth != u32::try_from(index).unwrap_or(u32::MAX)
                || (index > 0
                    && item.message.parent_message_id != Some(self.items[index - 1].message.id))
                || (index == 0 && item.message.parent_message_id.is_some())
                || (index > 0 && item.order <= self.items[index - 1].order)
            {
                return Err(ValidationError::Invariant {
                    field: "conversation_history.selected_path",
                });
            }
        }
        for (index, branch) in branch_records.iter().enumerate().skip(1) {
            let fork = branch
                .fork_message_id
                .ok_or(ValidationError::InvalidReference {
                    field: "conversation_history.branch_fork",
                })?;
            let fork_item = by_id.get(&fork).ok_or(ValidationError::InvalidReference {
                field: "conversation_history.branch_fork",
            })?;
            if fork_item.message.branch_id != branch_records[index - 1].id
                || path_index[&fork] >= self.items.len()
            {
                return Err(ValidationError::InvalidReference {
                    field: "conversation_history.branch_fork_path",
                });
            }
        }
        for window in self.items.windows(2) {
            if window[0].message.branch_id != window[1].message.branch_id {
                let child_index = branch_records
                    .iter()
                    .position(|branch| branch.id == window[1].message.branch_id)
                    .ok_or(ValidationError::InvalidReference {
                        field: "conversation_history.message_branch",
                    })?;
                if child_index == 0
                    || branch_records[child_index - 1].id != window[0].message.branch_id
                    || branch_records[child_index].fork_message_id != Some(window[0].message.id)
                {
                    return Err(ValidationError::InvalidReference {
                        field: "conversation_history.branch_transition",
                    });
                }
            }
        }
        Ok(())
    }
}

impl GroupConversationDetails {
    pub(crate) fn validate_participants(
        &self,
        participants: &[ConversationParticipant],
    ) -> Result<(), ValidationError> {
        if participants
            .iter()
            .any(|participant| participant.role == ParticipantRole::System)
        {
            return Err(ValidationError::Invariant {
                field: "conversation.group.system_participant",
            });
        }
        if participants
            .iter()
            .filter(|participant| participant.role == ParticipantRole::User)
            .count()
            != 1
            || participants
                .iter()
                .filter(|participant| participant.role == ParticipantRole::Character)
                .count()
                < 1
        {
            return Err(ValidationError::Invariant {
                field: "conversation.group.participants",
            });
        }
        let character_count = participants
            .iter()
            .filter(|participant| participant.role == ParticipantRole::Character)
            .count();
        if self.initial_participant_policy.members.len() != character_count {
            return Err(ValidationError::InvalidReference {
                field: "conversation.group.participant_policy.members",
            });
        }
        Ok(())
    }
}

// Keep these imports visible in generated rustdoc, since callers commonly
// construct launch plans beside the aggregate.
#[allow(dead_code)]
fn _typed_group_source(_: GroupId, _: GroupLaunchSnapshot) {}
