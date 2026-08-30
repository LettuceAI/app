use async_trait::async_trait;
use lettuce_media::AssetRetainer;
use lettuce_models::ModelCapabilities;
use lettuce_types::{
    AssetId, ConversationBranchId, ConversationId, ConversationParticipantId, GenerationAttemptId,
    GenerationTurnId, JobId, MessageCandidateId, MessageId, MessageRevisionId, OperationRecordId,
    Page, PageRequest, TimestampMillis, UsageEventId,
};
use serde::{Deserialize, Serialize};

use crate::commands::{
    ArchiveConversation, AttachAttemptJob, ChooseCandidate, ContinueConversation, EditMessage,
    ForkBranch, RegenerateCandidate, RenameConversation, RestoreConversation, RetryGeneration,
    SelectBranch, SendConversation, SettleCancellation, TombstoneMessage, UpdateMessageFlags,
};
use crate::content::{
    MediaAssetRole, Message, MessageCandidate, MessagePart, MessageRevision, MessageRole,
    ReplayArtifactRef,
};
use crate::document::{
    CharacterSnapshotBodyV1, LorebookSnapshotBodyV1, PersonaSnapshotBodyV1, PromptSnapshotBodyV1,
    SceneSnapshotBodyV1, SnapshotDocumentKind,
};
use crate::error::ConversationRepositoryError;
#[allow(unused_imports)]
pub use crate::generation::GenerationTarget;
use crate::generation::{
    GenerationAttempt, GenerationCheckpointEnvelope, GenerationCheckpointEvent,
    GenerationOperation, GenerationTurn, IdempotencyKey, LorebookAttribution, MemoryAttribution,
    PromptAttribution,
};
use crate::model::{Conversation, ConversationAggregate, ConversationBranch};
use crate::snapshot::{
    CharacterLaunchSnapshot, GroupConversationDetails, LorebookLaunchSnapshot,
    ModelSelectionSnapshot, PersonaLaunchSnapshot, PromptLaunchSnapshot, SceneLaunchSnapshot,
};

/// A bounded keyset result.  Implementations must never fetch more than the
/// caller's `PageRequest.limit` and should return an opaque continuation token.
pub type KeysetPage<T> = Page<T>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationSummary {
    pub id: ConversationId,
    pub title: String,
    pub lifecycle: crate::model::ConversationLifecycle,
    pub kind: ConversationKindTag,
    pub revision: lettuce_types::Revision,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationKindTag {
    Direct,
    Group,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConversationQuery {
    pub lifecycle: Option<crate::model::ConversationLifecycle>,
    pub page: PageRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineItem {
    pub message: Message,
    pub active_revision: Option<MessageRevision>,
    pub active_candidate: Option<MessageCandidate>,
    pub initial_origin: Option<crate::commands::InitialMessageOrigin>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelinePage {
    pub conversation_id: ConversationId,
    pub selected_branch_id: ConversationBranchId,
    /// The complete root-to-selected branch path. Persisted branch records
    /// prove this is an ancestor chain rather than a sibling ID list.
    pub branch_path: Vec<ConversationBranch>,
    pub items: Vec<TimelineItem>,
    pub boundary_parent_id: Option<MessageId>,
    pub next_cursor: Option<String>,
}

impl TimelinePage {
    pub fn validate_page(&self) -> Result<(), crate::ValidationError> {
        if self.branch_path.is_empty()
            || self.branch_path.len() > crate::validation::MAX_BRANCHES
            || self.branch_path.last().map(|branch| branch.id) != Some(self.selected_branch_id)
        {
            return Err(crate::ValidationError::InvalidReference {
                field: "timeline_page.branch_path",
            });
        }
        let mut path_ids = std::collections::HashSet::new();
        for (index, branch) in self.branch_path.iter().enumerate() {
            branch.validate()?;
            if branch.conversation_id != self.conversation_id || !path_ids.insert(branch.id) {
                return Err(crate::ValidationError::InvalidReference {
                    field: "timeline_page.branch_path_identity",
                });
            }
            if index == 0 {
                if branch.parent_branch_id.is_some() || branch.fork_message_id.is_some() {
                    return Err(crate::ValidationError::InvalidReference {
                        field: "timeline_page.branch_path_root",
                    });
                }
            } else if branch.parent_branch_id != Some(self.branch_path[index - 1].id)
                || branch.fork_message_id.is_none()
            {
                return Err(crate::ValidationError::InvalidReference {
                    field: "timeline_page.branch_path_ancestry",
                });
            }
        }
        if self
            .branch_path
            .last()
            .is_some_and(|branch| branch.status != crate::model::BranchStatus::Active)
        {
            return Err(crate::ValidationError::Invariant {
                field: "timeline_page.selected_branch_active",
            });
        }
        if self.items.len() > crate::validation::MAX_PARTS * 32 {
            return Err(crate::ValidationError::TooMany {
                field: "timeline_page.items",
                max: crate::validation::MAX_PARTS * 32,
            });
        }
        for item in &self.items {
            item.message.validate()?;
            if item.message.conversation_id != self.conversation_id
                || !path_ids.contains(&item.message.branch_id)
            {
                return Err(crate::ValidationError::InvalidReference {
                    field: "timeline_page.item_provenance",
                });
            }
            validate_timeline_item_render(item)?;
        }
        for window in self.items.windows(2) {
            let adjacent = window[1].message.parent_message_id == Some(window[0].message.id)
                || window[0].message.parent_message_id == Some(window[1].message.id);
            if !adjacent {
                return Err(crate::ValidationError::InvalidReference {
                    field: "timeline_page.adjacency",
                });
            }
            if window[0].message.branch_id != window[1].message.branch_id {
                let (parent_item, child_item) =
                    if window[1].message.parent_message_id == Some(window[0].message.id) {
                        (&window[0], &window[1])
                    } else {
                        (&window[1], &window[0])
                    };
                let child = self
                    .branch_path
                    .iter()
                    .find(|branch| branch.id == child_item.message.branch_id)
                    .ok_or(crate::ValidationError::InvalidReference {
                        field: "timeline_page.item_branch",
                    })?;
                if child.parent_branch_id != Some(parent_item.message.branch_id)
                    || child.fork_message_id != Some(parent_item.message.id)
                {
                    return Err(crate::ValidationError::InvalidReference {
                        field: "timeline_page.branch_transition",
                    });
                }
            }
        }
        if let Some(boundary_parent_id) = self.boundary_parent_id {
            if self
                .items
                .iter()
                .any(|item| item.message.id == boundary_parent_id)
            {
                return Err(crate::ValidationError::Invariant {
                    field: "timeline_page.boundary_inside_page",
                });
            }
            if let Some(first) = self.items.first() {
                let ascending = self.items.get(1).is_none_or(|second| {
                    second.message.parent_message_id == Some(first.message.id)
                });
                let boundary_item = if ascending {
                    self.items.first()
                } else {
                    self.items.last()
                };
                if boundary_item.and_then(|item| item.message.parent_message_id)
                    != Some(boundary_parent_id)
                {
                    return Err(crate::ValidationError::InvalidReference {
                        field: "timeline_page.boundary_parent",
                    });
                }
                if let Some(first) = self.items.first()
                    && first.message.branch_id != self.branch_path[0].id
                    && first.message.parent_message_id == Some(boundary_parent_id)
                {
                    let child = self
                        .branch_path
                        .iter()
                        .find(|branch| branch.id == first.message.branch_id)
                        .ok_or(crate::ValidationError::InvalidReference {
                            field: "timeline_page.boundary_branch",
                        })?;
                    if child.fork_message_id != Some(boundary_parent_id) {
                        return Err(crate::ValidationError::InvalidReference {
                            field: "timeline_page.boundary_fork",
                        });
                    }
                }
            }
        }
        if let Some(cursor) = &self.next_cursor {
            if cursor.trim().is_empty() || cursor.len() > 4096 {
                return Err(crate::ValidationError::InvalidValue {
                    field: "timeline_page.cursor",
                });
            }
        }
        Ok(())
    }
}

fn validate_timeline_item_render(item: &TimelineItem) -> Result<(), crate::ValidationError> {
    match item.message.active_render_source {
        crate::content::MessageRenderSource::Revision(id) => {
            let revision =
                item.active_revision
                    .as_ref()
                    .ok_or(crate::ValidationError::InvalidReference {
                        field: "timeline_page.render_revision",
                    })?;
            if revision.id != id || revision.message_id != item.message.id {
                return Err(crate::ValidationError::InvalidReference {
                    field: "timeline_page.render_revision",
                });
            }
            if item.active_candidate.is_some() {
                return Err(crate::ValidationError::Invariant {
                    field: "timeline_page.render_source_exclusive",
                });
            }
            revision.validate()
        }
        crate::content::MessageRenderSource::Candidate(id) => {
            let candidate =
                item.active_candidate
                    .as_ref()
                    .ok_or(crate::ValidationError::InvalidReference {
                        field: "timeline_page.render_candidate",
                    })?;
            if candidate.id != id || candidate.message_id != item.message.id {
                return Err(crate::ValidationError::InvalidReference {
                    field: "timeline_page.render_candidate",
                });
            }
            if item.active_revision.is_some() {
                return Err(crate::ValidationError::Invariant {
                    field: "timeline_page.render_source_exclusive",
                });
            }
            candidate.validate()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeginGeneration {
    pub conversation: Conversation,
    pub turn: GenerationTurn,
    pub attempt: GenerationAttempt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizationDraft {
    pub parts: Vec<MessagePart>,
    pub ordinal: u16,
    pub model: ModelSelectionSnapshot,
    pub replay: Option<ReplayArtifactRef>,
    pub outcome: GenerationCheckpointEvent,
}

/// Atomic mutation result: state, idempotency record, and durable outbox
/// records are committed together by a repository adapter.
///
/// A repository implementation must insert the aggregate change, the
/// [`OperationRecord`], and every returned outbox row in one database
/// transaction.  Callers must never observe one without the other.  Reads
/// intentionally return their value directly because they do not create an
/// operation or an outbox event.
///
/// A replayed mutation returns the original operation and outbox records
/// unchanged, while `value` is rehydrated from current state: the same
/// identity, but the live value rather than a snapshot of the first commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationCommit<T> {
    pub value: T,
    pub operation: OperationRecord,
    pub outbox: Vec<ConversationOutboxRecord>,
}

pub type CreateConversationResult = MutationCommit<ConversationAggregate>;
pub type SendConversationResult = MutationCommit<BeginGeneration>;
pub type ContinueConversationResult = MutationCommit<BeginGeneration>;
pub type RegenerateCandidateResult = MutationCommit<BeginGeneration>;
pub type RetryGenerationResult = MutationCommit<BeginGeneration>;
pub type ResolveGroupSpeakerResult = MutationCommit<GenerationTurn>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationFinalization {
    pub turn: GenerationTurn,
    pub assistant_message: Message,
    pub candidate: MessageCandidate,
    /// Adapters write `None` in M8. A `Some` value must carry
    /// `source_turn_id` equal to the finalized turn and never becomes the
    /// active render source.
    pub revision: Option<MessageRevision>,
    pub asset_reference_deltas: Vec<AssetReferenceDelta>,
    pub usage_event_id: UsageEventId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationFailure {
    pub turn: GenerationTurn,
    pub failure: crate::generation::GenerationFailureCode,
    pub usage_event_id: UsageEventId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationCancellation {
    pub turn: GenerationTurn,
    pub attempt_id: GenerationAttemptId,
    pub usage_event_id: UsageEventId,
}

pub type RequestCancellationResult = MutationCommit<GenerationTurn>;
pub type SettleCancellationResult = MutationCommit<GenerationCancellation>;
pub type CancelGenerationResult = SettleCancellationResult;
pub type AttachAttemptJobResult = MutationCommit<GenerationAttempt>;
pub type ChooseCandidateResult = MutationCommit<Message>;
pub type UpdateMessageFlagsResult = MutationCommit<Message>;
pub type ArchiveConversationResult = MutationCommit<Conversation>;
pub type RestoreConversationResult = MutationCommit<Conversation>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationRecovery {
    pub turn: GenerationTurn,
    pub attempt: GenerationAttempt,
}

impl GenerationRecovery {
    /// Recovery creates a new child attempt. It does not resume the
    /// interrupted attempt in place.
    pub fn validate_against(
        &self,
        previous_attempt: &GenerationAttempt,
        turn: &GenerationTurn,
    ) -> Result<(), crate::ValidationError> {
        if self.turn.id != turn.id || self.turn.id != previous_attempt.turn_id {
            return Err(crate::ValidationError::InvalidReference {
                field: "generation_recovery.turn",
            });
        }
        self.attempt.validate_against(previous_attempt, turn)
    }
}

pub type AppendCheckpointResult = MutationCommit<GenerationTurn>;
pub type GenerationFinalizationResult = MutationCommit<GenerationFinalization>;
pub type GenerationFailureResult = MutationCommit<GenerationFailure>;
pub type GenerationInterruptionResult = MutationCommit<GenerationTurn>;
pub type GenerationRecoveryResult = MutationCommit<GenerationRecovery>;
pub type EditMessageResult = MutationCommit<EditResult>;
pub type ForkBranchResult = MutationCommit<BranchResult>;
pub type SelectBranchResult = MutationCommit<Conversation>;
pub type TombstoneMessageResult = MutationCommit<TombstoneResult>;
pub type ParticipantPolicyResult = MutationCommit<Conversation>;
pub type SettingsResult = MutationCommit<Conversation>;
pub type RenameConversationResult = MutationCommit<Conversation>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditResult {
    pub message: Message,
    pub revision: MessageRevision,
    pub asset_reference_deltas: Vec<AssetReferenceDelta>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchResult {
    pub branch: ConversationBranch,
    pub conversation: Conversation,
}

impl BranchResult {
    /// A fork always selects the branch it just created.
    pub fn validate(&self) -> Result<(), crate::ValidationError> {
        self.conversation.validate()?;
        self.branch.validate()?;
        if self.branch.conversation_id != self.conversation.id
            || self.branch.status != crate::model::BranchStatus::Active
            || self.conversation.active_branch_id != self.branch.id
        {
            return Err(crate::ValidationError::InvalidReference {
                field: "branch_result.selected_branch",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TombstoneResult {
    pub conversation: Conversation,
    pub message: Message,
    pub descendant_count: u32,
    pub asset_reference_deltas: Vec<AssetReferenceDelta>,
    /// The preservation branch selected atomically by a `Fork` tombstone.
    /// Other descendant policies must leave this absent. The old branch keeps
    /// the tombstoned subtree; this new active branch continues from the
    /// message before it, and forks from whichever branch owns that parent.
    pub forked_branch: Option<ConversationBranch>,
}

impl TombstoneResult {
    pub fn validate_for_policy(
        &self,
        policy: crate::commands::DescendantPolicy,
    ) -> Result<(), crate::ValidationError> {
        self.conversation.validate()?;
        self.message.validate()?;
        if self.message.conversation_id != self.conversation.id {
            return Err(crate::ValidationError::InvalidReference {
                field: "tombstone.message_conversation",
            });
        }
        match (policy, &self.forked_branch) {
            (crate::commands::DescendantPolicy::Fork, Some(branch)) => {
                branch.validate()?;
                if branch.conversation_id != self.conversation.id
                    || self.conversation.active_branch_id != branch.id
                    || branch.status != crate::model::BranchStatus::Active
                    || branch.parent_branch_id.is_none()
                    || branch.fork_message_id.is_none()
                    || branch.fork_message_id != self.message.parent_message_id
                {
                    return Err(crate::ValidationError::InvalidReference {
                        field: "tombstone.forked_branch",
                    });
                }
            }
            (crate::commands::DescendantPolicy::Fork, None) => {
                return Err(crate::ValidationError::InvalidReference {
                    field: "tombstone.forked_branch",
                });
            }
            (_, Some(_)) => {
                return Err(crate::ValidationError::InvalidReference {
                    field: "tombstone.forked_branch",
                });
            }
            (_, None) => {}
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetReferenceDelta {
    pub asset_id: lettuce_types::AssetId,
    pub retainer: AssetRetainer,
    pub state: AssetReferenceState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum AssetReferenceState {
    Active,
    Historical,
    /// Never persisted in a `*_media_refs` table. This is a retention signal
    /// for the media crate alone.
    Released,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ConversationOutboxEvent {
    ConversationCreated {
        conversation_id: ConversationId,
        root_branch_id: ConversationBranchId,
        head_message_id: Option<MessageId>,
        initial_message_count: u16,
        at: TimestampMillis,
    },
    MessageCommitted {
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        message_id: MessageId,
        revision_id: Option<MessageRevisionId>,
        candidate_id: Option<MessageCandidateId>,
        at: TimestampMillis,
    },
    MessageRevised {
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        message_id: MessageId,
        revision_id: MessageRevisionId,
        at: TimestampMillis,
    },
    MessageTombstoned {
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        message_id: MessageId,
        descendants: crate::commands::DescendantPolicy,
        affected_message_ids: Vec<MessageId>,
        /// One id per affected message that renders from a revision: the
        /// revision it was rendering, not that message's whole history.
        affected_revision_ids: Vec<MessageRevisionId>,
        asset_reference_deltas: Vec<AssetReferenceDelta>,
        at: TimestampMillis,
    },
    TurnFinalized {
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        message_id: MessageId,
        candidate_id: MessageCandidateId,
        revision_id: Option<MessageRevisionId>,
        effective_time: TimestampMillis,
        usage_event_id: UsageEventId,
        /// Zero or one id, taken from the turn's memory attribution.
        used_memory_revision_ids: Vec<lettuce_types::MemoryRevisionId>,
    },
    TurnFailed {
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        usage_event_id: UsageEventId,
        /// Zero or one id, taken from the turn's memory attribution.
        used_memory_revision_ids: Vec<lettuce_types::MemoryRevisionId>,
        at: TimestampMillis,
    },
    TurnInterrupted {
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        usage_event_id: UsageEventId,
        used_memory_revision_ids: Vec<lettuce_types::MemoryRevisionId>,
        at: TimestampMillis,
    },
    TurnRecovering {
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        turn_id: GenerationTurnId,
        previous_attempt_id: GenerationAttemptId,
        attempt_id: GenerationAttemptId,
        at: TimestampMillis,
    },
    TurnCancellationRequested {
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        at: TimestampMillis,
    },
    TurnCancelled {
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        usage_event_id: UsageEventId,
        /// Zero or one id, taken from the turn's memory attribution.
        used_memory_revision_ids: Vec<lettuce_types::MemoryRevisionId>,
        at: TimestampMillis,
    },
    BranchForked {
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        at: TimestampMillis,
    },
    CandidateChosen {
        conversation_id: ConversationId,
        message_id: MessageId,
        candidate_id: MessageCandidateId,
        at: TimestampMillis,
    },
    BranchSelected {
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        at: TimestampMillis,
    },
    ConversationLifecycleChanged {
        conversation_id: ConversationId,
        lifecycle: crate::model::ConversationLifecycle,
        at: TimestampMillis,
    },
    TitleChanged {
        conversation_id: ConversationId,
        title: String,
        at: TimestampMillis,
    },
    SettingsChanged {
        conversation_id: ConversationId,
        settings_revision: lettuce_types::Revision,
        at: TimestampMillis,
    },
    ParticipantPolicyChanged {
        conversation_id: ConversationId,
        participant_id: ConversationParticipantId,
        at: TimestampMillis,
    },
    MessageFlagsChanged {
        conversation_id: ConversationId,
        message_id: MessageId,
        pinned: bool,
        visibility: crate::content::MessageVisibility,
        at: TimestampMillis,
    },
    /// Its producer arrives with the future purge slice.
    ConversationTombstoned {
        conversation_id: ConversationId,
        at: TimestampMillis,
    },
    AssetReferencesChanged {
        conversation_id: ConversationId,
        message_revision_id: Option<MessageRevisionId>,
        candidate_id: Option<MessageCandidateId>,
        changes: Vec<AssetReferenceDelta>,
        at: TimestampMillis,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationOutboxRecord {
    pub format_version: u32,
    pub id: lettuce_types::OutboxEventId,
    pub conversation_id: ConversationId,
    pub conversation_revision: lettuce_types::Revision,
    pub sequence: u64,
    pub operation_record_id: OperationRecordId,
    pub at: TimestampMillis,
    pub event: ConversationOutboxEvent,
}

impl ConversationOutboxRecord {
    pub fn validate(&self) -> Result<(), crate::ValidationError> {
        if self.format_version != 1 {
            return Err(crate::ValidationError::UnsupportedVersion {
                field: "outbox.format_version",
                version: self.format_version,
            });
        }
        if self.sequence == 0 || self.conversation_revision.get() == 0 {
            return Err(crate::ValidationError::InvalidValue {
                field: "outbox.sequence_revision",
            });
        }
        let event_conversation_id = match &self.event {
            ConversationOutboxEvent::ConversationCreated {
                conversation_id, ..
            }
            | ConversationOutboxEvent::MessageCommitted {
                conversation_id, ..
            }
            | ConversationOutboxEvent::MessageRevised {
                conversation_id, ..
            }
            | ConversationOutboxEvent::MessageTombstoned {
                conversation_id, ..
            }
            | ConversationOutboxEvent::TurnFinalized {
                conversation_id, ..
            }
            | ConversationOutboxEvent::TurnFailed {
                conversation_id, ..
            }
            | ConversationOutboxEvent::TurnInterrupted {
                conversation_id, ..
            }
            | ConversationOutboxEvent::TurnRecovering {
                conversation_id, ..
            }
            | ConversationOutboxEvent::TurnCancellationRequested {
                conversation_id, ..
            }
            | ConversationOutboxEvent::TurnCancelled {
                conversation_id, ..
            }
            | ConversationOutboxEvent::BranchForked {
                conversation_id, ..
            }
            | ConversationOutboxEvent::CandidateChosen {
                conversation_id, ..
            }
            | ConversationOutboxEvent::BranchSelected {
                conversation_id, ..
            }
            | ConversationOutboxEvent::ConversationLifecycleChanged {
                conversation_id, ..
            }
            | ConversationOutboxEvent::TitleChanged {
                conversation_id, ..
            }
            | ConversationOutboxEvent::SettingsChanged {
                conversation_id, ..
            }
            | ConversationOutboxEvent::ParticipantPolicyChanged {
                conversation_id, ..
            }
            | ConversationOutboxEvent::MessageFlagsChanged {
                conversation_id, ..
            }
            | ConversationOutboxEvent::ConversationTombstoned {
                conversation_id, ..
            }
            | ConversationOutboxEvent::AssetReferencesChanged {
                conversation_id, ..
            } => *conversation_id,
        };
        if event_conversation_id != self.conversation_id {
            return Err(crate::ValidationError::InvalidReference {
                field: "outbox.conversation_id",
            });
        }
        if let ConversationOutboxEvent::ConversationCreated {
            head_message_id,
            initial_message_count,
            ..
        } = &self.event
        {
            if *initial_message_count > 512
                || (*initial_message_count == 0) != head_message_id.is_none()
            {
                return Err(crate::ValidationError::InvalidValue {
                    field: "outbox.conversation_created.initial_timeline",
                });
            }
        }
        if let ConversationOutboxEvent::MessageTombstoned {
            affected_message_ids,
            affected_revision_ids,
            asset_reference_deltas,
            ..
        } = &self.event
        {
            if affected_message_ids.len() > crate::validation::MAX_PARTS * 32
                || affected_revision_ids.len() > crate::validation::MAX_PARTS * 32
                || asset_reference_deltas.len() > crate::validation::MAX_PARTS * 32
            {
                return Err(crate::ValidationError::TooMany {
                    field: "outbox.tombstone.refs",
                    max: crate::validation::MAX_PARTS * 32,
                });
            }
        }
        if let ConversationOutboxEvent::ConversationLifecycleChanged { lifecycle, .. } = &self.event
            && *lifecycle == crate::model::ConversationLifecycle::Tombstoned
        {
            return Err(crate::ValidationError::InvalidValue {
                field: "outbox.lifecycle_changed.tombstoned",
            });
        }
        if let ConversationOutboxEvent::SettingsChanged {
            settings_revision, ..
        } = &self.event
            && settings_revision.get() == 0
        {
            return Err(crate::ValidationError::ZeroRevision);
        }
        if let ConversationOutboxEvent::MessageFlagsChanged { visibility, .. } = &self.event
            && *visibility == crate::content::MessageVisibility::Tombstoned
        {
            return Err(crate::ValidationError::InvalidValue {
                field: "outbox.message_flags.visibility",
            });
        }
        if let ConversationOutboxEvent::TitleChanged { title, .. } = &self.event {
            crate::validation::validate_text(
                "conversation.title",
                title,
                crate::validation::MAX_DISPLAY_CHARS * 4,
                false,
            )?;
        }
        let used_memory_revision_ids = match &self.event {
            ConversationOutboxEvent::TurnFinalized {
                used_memory_revision_ids,
                ..
            }
            | ConversationOutboxEvent::TurnFailed {
                used_memory_revision_ids,
                ..
            }
            | ConversationOutboxEvent::TurnInterrupted {
                used_memory_revision_ids,
                ..
            }
            | ConversationOutboxEvent::TurnCancelled {
                used_memory_revision_ids,
                ..
            } => used_memory_revision_ids,
            _ => return Ok(()),
        };
        if used_memory_revision_ids.len() > crate::validation::MAX_MEMORY_REVISIONS {
            return Err(crate::ValidationError::TooMany {
                field: "outbox.used_memory_revision_ids",
                max: crate::validation::MAX_MEMORY_REVISIONS,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum OperationKind {
    Create,
    Send,
    Continue,
    Regenerate,
    Retry,
    Checkpoint,
    Cancel,
    Finalize,
    Fail,
    Interrupt,
    Recover,
    ChooseCandidate,
    Edit,
    Flags,
    Fork,
    SelectBranch,
    Tombstone,
    Archive,
    Restore,
    Rename,
    ParticipantPolicy,
    Settings,
    AttachJob,
    ResolveSpeaker,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "id",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum OperationResultRef {
    Conversation(ConversationId),
    Turn(GenerationTurnId),
    Message(MessageId),
    Candidate(MessageCandidateId),
    Branch(ConversationBranchId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationRecord {
    pub id: OperationRecordId,
    pub conversation_id: ConversationId,
    pub kind: OperationKind,
    pub operation: crate::commands::OperationToken,
    pub result: OperationResultRef,
    pub created_at: TimestampMillis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AttemptJobOwner {
    pub conversation_id: ConversationId,
    pub turn_id: GenerationTurnId,
    pub attempt_id: GenerationAttemptId,
}

/// Deterministic reference index for the repository-wide attach invariant.
/// SQLite adapters enforce the same rule with a partial unique index on
/// `generation_attempts(job_id)` inside the attach transaction.
#[derive(Debug, Default)]
pub struct JobOwnershipIndex {
    owners: std::collections::HashMap<JobId, AttemptJobOwner>,
}

impl JobOwnershipIndex {
    pub fn attach(
        &mut self,
        target: AttemptJobOwner,
        current_job_id: Option<JobId>,
        requested_job_id: JobId,
    ) -> Result<(), ConversationRepositoryError> {
        if current_job_id.is_some() {
            return Err(ConversationRepositoryError::JobAlreadyAttached);
        }
        if self.owners.contains_key(&requested_job_id) {
            return Err(ConversationRepositoryError::JobInUse);
        }
        self.owners.insert(requested_job_id, target);
        Ok(())
    }
}

impl OperationRecord {
    pub fn replay_or_conflict(
        &self,
        conversation_id: ConversationId,
        kind: OperationKind,
        token: &crate::commands::OperationToken,
    ) -> Result<Option<OperationResultRef>, ConversationRepositoryError> {
        if self.conversation_id == conversation_id
            && self.kind == kind
            && self.operation.key == token.key
            && self.operation.request_digest == token.request_digest
        {
            Ok(Some(self.result.clone()))
        } else {
            Err(ConversationRepositoryError::Conflict)
        }
    }
}

/// Synchronous persistence boundary.  Every mutating method returns a
/// [`MutationCommit`], and its aggregate change, operation record, and outbox
/// records must be inserted atomically in one transaction.  Each
/// begin/finalize operation is one transaction in an adapter, while
/// network/provider work occurs outside it.  Read methods return plain values.
pub trait ConversationReader: Send + Sync {
    fn get(&self, id: ConversationId)
    -> Result<ConversationAggregate, ConversationRepositoryError>;
    fn page(
        &self,
        query: &ConversationQuery,
    ) -> Result<KeysetPage<ConversationSummary>, ConversationRepositoryError>;
    fn timeline_page(
        &self,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        page: &PageRequest,
    ) -> Result<TimelinePage, ConversationRepositoryError>;
    fn get_message_revision(
        &self,
        id: MessageRevisionId,
    ) -> Result<MessageRevision, ConversationRepositoryError>;
    fn page_message_revisions(
        &self,
        message_id: MessageId,
        page: &PageRequest,
    ) -> Result<KeysetPage<MessageRevision>, ConversationRepositoryError>;
    fn get_candidate(
        &self,
        id: MessageCandidateId,
    ) -> Result<MessageCandidate, ConversationRepositoryError>;
    fn page_candidates(
        &self,
        message_id: MessageId,
        page: &PageRequest,
    ) -> Result<KeysetPage<MessageCandidate>, ConversationRepositoryError>;
    fn get_turn(&self, id: GenerationTurnId)
    -> Result<GenerationTurn, ConversationRepositoryError>;
    fn page_turns(
        &self,
        conversation_id: ConversationId,
        page: &PageRequest,
    ) -> Result<KeysetPage<GenerationTurn>, ConversationRepositoryError>;
    fn operation_record(
        &self,
        conversation_id: ConversationId,
        kind: OperationKind,
        token: &crate::commands::OperationToken,
    ) -> Result<Option<OperationRecord>, ConversationRepositoryError>;
    fn page_outbox(
        &self,
        conversation_id: ConversationId,
        page: &PageRequest,
    ) -> Result<KeysetPage<ConversationOutboxRecord>, ConversationRepositoryError>;
}

/// Materializes one protected launch snapshot through the conversation's
/// ownership reference. Implementations must read the reference and artifact
/// from the same storage snapshot and must never resolve live source rows.
pub trait ConversationSnapshotMaterializer: Send + Sync {
    fn materialize_character(
        &self,
        conversation_id: ConversationId,
        snapshot: &CharacterLaunchSnapshot,
    ) -> Result<CharacterSnapshotBodyV1, crate::ArtifactError>;
    fn materialize_persona(
        &self,
        conversation_id: ConversationId,
        snapshot: &PersonaLaunchSnapshot,
    ) -> Result<PersonaSnapshotBodyV1, crate::ArtifactError>;
    fn materialize_scene(
        &self,
        conversation_id: ConversationId,
        snapshot: &SceneLaunchSnapshot,
    ) -> Result<SceneSnapshotBodyV1, crate::ArtifactError>;
    fn materialize_prompt(
        &self,
        conversation_id: ConversationId,
        snapshot: &PromptLaunchSnapshot,
    ) -> Result<PromptSnapshotBodyV1, crate::ArtifactError>;
    fn materialize_lorebook(
        &self,
        conversation_id: ConversationId,
        snapshot: &LorebookLaunchSnapshot,
    ) -> Result<LorebookSnapshotBodyV1, crate::ArtifactError>;
}

pub trait ConversationCreator: ConversationReader {
    /// Consume a validated launch bundle. The adapter must stage its artifact
    /// drafts and aggregate rows in one transaction, together with the
    /// idempotency record and creation outbox event.
    fn create(
        &self,
        launch: crate::PreparedConversationLaunch,
        now: TimestampMillis,
    ) -> Result<CreateConversationResult, ConversationRepositoryError>;
}

/// Mutations other than [`Self::restore`] require an Active conversation;
/// adapters answer [`ConversationRepositoryError::Conflict`] otherwise.  The
/// begin methods additionally require that no non-terminal turn exists on the
/// conversation.  Adapters enforce that single in-flight rule; its supporting
/// index is deferred.
pub trait ConversationRepository: ConversationCreator {
    /// Same-database artifact access used by replay finalization and trusted
    /// retention workflows. Conversation creation receives its artifact
    /// drafts through [`ConversationCreator::create`] instead of using this
    /// verifier, avoiding a check-then-use race.
    fn artifact_store(&self) -> &dyn crate::ConversationArtifactStore;

    fn begin_send(
        &self,
        command: &SendConversation,
        now: TimestampMillis,
    ) -> Result<SendConversationResult, ConversationRepositoryError>;
    fn begin_continue(
        &self,
        command: &ContinueConversation,
        now: TimestampMillis,
    ) -> Result<ContinueConversationResult, ConversationRepositoryError>;
    fn begin_regenerate(
        &self,
        command: &RegenerateCandidate,
        now: TimestampMillis,
    ) -> Result<RegenerateCandidateResult, ConversationRepositoryError>;
    fn begin_retry(
        &self,
        command: &RetryGeneration,
        now: TimestampMillis,
    ) -> Result<RetryGenerationResult, ConversationRepositoryError>;
    fn append_event(
        &self,
        turn_id: GenerationTurnId,
        expected_turn_revision: lettuce_types::Revision,
        operation: &crate::commands::OperationToken,
        event: GenerationCheckpointEnvelope,
        now: TimestampMillis,
    ) -> Result<AppendCheckpointResult, ConversationRepositoryError>;
    #[allow(clippy::too_many_arguments)]
    /// Before staging this mutation, verify the exact replay reference in the
    /// draft through [`Self::artifact_store`]. A failed pre-staging write must
    /// use the store's orphan-cleanup contract.
    fn finalize_generation(
        &self,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        expected_conversation_revision: lettuce_types::Revision,
        expected_turn_revision: lettuce_types::Revision,
        operation: &crate::commands::OperationToken,
        draft: FinalizationDraft,
        usage_event_id: UsageEventId,
        now: TimestampMillis,
    ) -> Result<GenerationFinalizationResult, ConversationRepositoryError>;
    #[allow(clippy::too_many_arguments)]
    fn fail_generation(
        &self,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        expected_conversation_revision: lettuce_types::Revision,
        expected_turn_revision: lettuce_types::Revision,
        operation: &crate::commands::OperationToken,
        failure: crate::generation::GenerationFailureCode,
        usage_event_id: UsageEventId,
        now: TimestampMillis,
    ) -> Result<GenerationFailureResult, ConversationRepositoryError>;
    /// Settles the attempt as interrupted with its usage event and moves the
    /// turn to Interrupted; recovery then appends a child attempt.
    #[allow(clippy::too_many_arguments)]
    fn interrupt_generation(
        &self,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        expected_conversation_revision: lettuce_types::Revision,
        expected_turn_revision: lettuce_types::Revision,
        operation: &crate::commands::OperationToken,
        usage_event_id: UsageEventId,
        now: TimestampMillis,
    ) -> Result<GenerationInterruptionResult, ConversationRepositoryError>;
    fn request_cancellation(
        &self,
        command: &crate::commands::CancelGeneration,
        now: TimestampMillis,
    ) -> Result<RequestCancellationResult, ConversationRepositoryError>;
    fn settle_cancellation(
        &self,
        command: &SettleCancellation,
        now: TimestampMillis,
    ) -> Result<SettleCancellationResult, ConversationRepositoryError>;
    /// Atomically attaches a job only when the target attempt is still
    /// unattached and in a pre-run state. The uniqueness check is repository
    /// wide: a non-null `JobId` may belong to exactly one attempt across all
    /// conversations. Adapters report [`ConversationRepositoryError::JobAlreadyAttached`]
    /// for a populated target and `JobInUse` when another attempt owns it.
    /// M8 must enforce this with a partial unique index on `job_id`.
    fn attach_attempt_job(
        &self,
        command: &AttachAttemptJob,
        now: TimestampMillis,
    ) -> Result<AttachAttemptJobResult, ConversationRepositoryError>;
    fn recover_generation(
        &self,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        expected_conversation_revision: lettuce_types::Revision,
        expected_turn_revision: lettuce_types::Revision,
        operation: &crate::commands::OperationToken,
        now: TimestampMillis,
    ) -> Result<GenerationRecoveryResult, ConversationRepositoryError>;

    fn resolve_group_speaker(
        &self,
        command: &crate::commands::ResolveGroupSpeaker,
        now: TimestampMillis,
    ) -> Result<ResolveGroupSpeakerResult, ConversationRepositoryError>;

    fn choose_candidate(
        &self,
        command: &ChooseCandidate,
        now: TimestampMillis,
    ) -> Result<ChooseCandidateResult, ConversationRepositoryError>;
    fn fork_branch(
        &self,
        command: &ForkBranch,
        now: TimestampMillis,
    ) -> Result<ForkBranchResult, ConversationRepositoryError>;
    fn select_branch(
        &self,
        command: &SelectBranch,
        now: TimestampMillis,
    ) -> Result<SelectBranchResult, ConversationRepositoryError>;
    /// A tombstoned message can never be restored, so adapters answer
    /// [`ConversationRepositoryError::Conflict`] for one.
    fn edit_message(
        &self,
        command: &EditMessage,
        now: TimestampMillis,
    ) -> Result<EditMessageResult, ConversationRepositoryError>;
    /// Emits [`ConversationOutboxEvent::MessageFlagsChanged`].  A tombstoned
    /// message can never be restored, so adapters answer
    /// [`ConversationRepositoryError::Conflict`] for one.
    fn update_message_flags(
        &self,
        command: &UpdateMessageFlags,
        now: TimestampMillis,
    ) -> Result<UpdateMessageFlagsResult, ConversationRepositoryError>;
    /// Tombstoning a root message under [`crate::commands::DescendantPolicy::Fork`]
    /// is a conflict: there is no parent message to fork from.
    fn tombstone_message(
        &self,
        command: &TombstoneMessage,
        now: TimestampMillis,
    ) -> Result<TombstoneMessageResult, ConversationRepositoryError>;
    fn archive(
        &self,
        command: &ArchiveConversation,
        now: TimestampMillis,
    ) -> Result<ArchiveConversationResult, ConversationRepositoryError>;
    fn restore(
        &self,
        command: &RestoreConversation,
        now: TimestampMillis,
    ) -> Result<RestoreConversationResult, ConversationRepositoryError>;
    fn rename(
        &self,
        command: &RenameConversation,
        now: TimestampMillis,
    ) -> Result<RenameConversationResult, ConversationRepositoryError>;
    fn update_participant_policy(
        &self,
        command: &crate::commands::UpdateParticipantPolicy,
        now: TimestampMillis,
    ) -> Result<ParticipantPolicyResult, ConversationRepositoryError>;
    fn update_settings(
        &self,
        update: crate::PreparedConversationSettingsUpdate,
        now: TimestampMillis,
    ) -> Result<SettingsResult, ConversationRepositoryError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchResolveRequest {
    pub conversation_id: ConversationId,
    pub operation: GenerationOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchResolution {
    pub conversation: Conversation,
    pub group: Option<GroupConversationDetails>,
    pub model: Option<ModelSelectionSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextRequest {
    pub conversation_id: ConversationId,
    pub branch_id: ConversationBranchId,
    pub branch_path: Vec<ConversationBranchId>,
    pub source_message_id: MessageId,
    pub operation: GenerationOperation,
    pub swap_roles: bool,
    pub guidance: Option<String>,
    pub window: ContextWindowPolicy,
    pub selected_speaker: Option<crate::generation::SelectedSpeakerDecision>,
    pub capabilities: ModelCapabilities,
    pub safety: SafetyContext,
    pub prompt_runtime: PromptRuntimeFacts,
    pub prompt_values: PromptRuntimeValues,
    pub memory: Option<MemoryContribution>,
    pub timeline: Vec<TimelineItem>,
}

impl ContextRequest {
    pub fn validate(&self) -> Result<(), crate::ValidationError> {
        if self.timeline.len() > 512 {
            return Err(crate::ValidationError::TooMany {
                field: "context_request.timeline",
                max: 512,
            });
        }
        if self.branch_path.is_empty()
            || self.branch_path.len() > crate::validation::MAX_BRANCHES
            || self.branch_path.last().copied() != Some(self.branch_id)
        {
            return Err(crate::ValidationError::InvalidReference {
                field: "context_request.branch_path",
            });
        }
        crate::validation::validate_unique(
            "context_request.branch_path",
            self.branch_path.iter().copied(),
        )?;
        self.window.validate()?;
        self.capabilities
            .validate()
            .map_err(|_| crate::ValidationError::InvalidValue {
                field: "context_request.capabilities",
            })?;
        self.prompt_runtime.validate()?;
        self.prompt_values.validate()?;
        if let Some(guidance) = &self.guidance {
            crate::validation::validate_text(
                "context_request.guidance",
                guidance,
                crate::validation::MAX_REASONING_BYTES,
                false,
            )?;
        }
        if let Some(memory) = &self.memory {
            memory.validate()?;
        }
        let source_item = self
            .timeline
            .iter()
            .find(|item| item.message.id == self.source_message_id)
            .ok_or(crate::ValidationError::InvalidReference {
                field: "context_request.source_message_id",
            })?;
        let source_role_is_coherent = match self.operation {
            GenerationOperation::Send => source_item.message.role == MessageRole::User,
            // Continue may be requested at either side of a sparse or
            // imported head, so role is intentionally left open here.
            GenerationOperation::Continue => true,
            GenerationOperation::Regenerate => source_item.message.role == MessageRole::Assistant,
        };
        if !source_role_is_coherent {
            return Err(crate::ValidationError::InvalidReference {
                field: "context_request.source_message_id",
            });
        }
        for item in &self.timeline {
            if item.message.conversation_id != self.conversation_id {
                return Err(crate::ValidationError::InvalidReference {
                    field: "context_request.timeline_conversation",
                });
            }
            if !self.branch_path.contains(&item.message.branch_id) {
                return Err(crate::ValidationError::InvalidReference {
                    field: "context_request.timeline_branch",
                });
            }
            item.message.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextWindowPolicy {
    pub recent_non_pinned_limit: usize,
}

impl Default for ContextWindowPolicy {
    fn default() -> Self {
        Self {
            recent_non_pinned_limit: 64,
        }
    }
}

impl ContextWindowPolicy {
    fn validate(self) -> Result<(), crate::ValidationError> {
        if self.recent_non_pinned_limit > 512 {
            return Err(crate::ValidationError::OutOfBounds {
                field: "context_window.recent_non_pinned_limit",
            });
        }
        Ok(())
    }
}

/// Runtime values required by the prompt condition vocabulary.  Authored
/// snapshot facts remain owned by the assembler; these values are supplied by
/// the current model/runtime admission step.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PromptRuntimeFacts {
    pub provider_id: Option<String>,
    pub provider_label: Option<String>,
    pub input_scopes: Vec<String>,
    pub output_scopes: Vec<String>,
    pub scene_generation_enabled: bool,
    pub avatar_generation_enabled: bool,
    pub is_local_image_generation_model: bool,
    pub is_scene_generation_local_image_model: bool,
    pub dynamic_memory_enabled: bool,
    pub has_active_scheduled_note: bool,
    pub time_awareness_enabled: bool,
    pub companion_mode_enabled: bool,
}

impl PromptRuntimeFacts {
    fn validate(&self) -> Result<(), crate::ValidationError> {
        for (field, value) in [
            ("context_runtime.provider_id", self.provider_id.as_deref()),
            (
                "context_runtime.provider_label",
                self.provider_label.as_deref(),
            ),
        ] {
            if let Some(value) = value {
                crate::validation::validate_text(
                    field,
                    value,
                    crate::validation::MAX_DISPLAY_CHARS,
                    false,
                )?;
            }
        }
        for (field, scopes) in [
            ("context_runtime.input_scopes", self.input_scopes.as_slice()),
            (
                "context_runtime.output_scopes",
                self.output_scopes.as_slice(),
            ),
        ] {
            crate::validation::validate_collection(field, scopes, 64)?;
            for scope in scopes {
                crate::validation::validate_text(
                    "context_runtime.scope",
                    scope,
                    crate::validation::MAX_DISPLAY_CHARS,
                    false,
                )?;
            }
        }
        Ok(())
    }
}

/// Pre-resolved authored/runtime strings which the pure prompt renderer cannot
/// derive from booleans. `None` means the value is unavailable; the assembler
/// must not manufacture a replacement.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PromptRuntimeValues {
    pub content_rules: Option<String>,
    pub companion_state: Option<String>,
    pub scheduled_notes: Option<String>,
    pub date: Option<String>,
    pub date_full: Option<String>,
    pub weekday: Option<String>,
    pub time_hour: Option<String>,
    pub time_minute: Option<String>,
    pub time_second: Option<String>,
    pub time_full: Option<String>,
    pub time_12hour_format: Option<String>,
    pub time_timezone: Option<String>,
    pub time_timezone_name: Option<String>,
    pub datetime_iso: Option<String>,
}

impl PromptRuntimeValues {
    fn validate(&self) -> Result<(), crate::ValidationError> {
        for (field, value) in [
            (
                "context_runtime_values.content_rules",
                self.content_rules.as_deref(),
            ),
            (
                "context_runtime_values.companion_state",
                self.companion_state.as_deref(),
            ),
            (
                "context_runtime_values.scheduled_notes",
                self.scheduled_notes.as_deref(),
            ),
        ] {
            if let Some(value) = value {
                crate::validation::validate_text(
                    field,
                    value,
                    crate::validation::MAX_AUTHORED_TEXT_BYTES,
                    false,
                )?;
            }
        }
        for (field, value) in [
            ("context_runtime_values.date", self.date.as_deref()),
            (
                "context_runtime_values.date_full",
                self.date_full.as_deref(),
            ),
            ("context_runtime_values.weekday", self.weekday.as_deref()),
            (
                "context_runtime_values.time_hour",
                self.time_hour.as_deref(),
            ),
            (
                "context_runtime_values.time_minute",
                self.time_minute.as_deref(),
            ),
            (
                "context_runtime_values.time_second",
                self.time_second.as_deref(),
            ),
            (
                "context_runtime_values.time_full",
                self.time_full.as_deref(),
            ),
            (
                "context_runtime_values.time_12hour_format",
                self.time_12hour_format.as_deref(),
            ),
            (
                "context_runtime_values.time_timezone",
                self.time_timezone.as_deref(),
            ),
            (
                "context_runtime_values.time_timezone_name",
                self.time_timezone_name.as_deref(),
            ),
            (
                "context_runtime_values.datetime_iso",
                self.datetime_iso.as_deref(),
            ),
        ] {
            if let Some(value) = value {
                crate::validation::validate_text(
                    field,
                    value,
                    crate::validation::MAX_DISPLAY_CHARS * 4,
                    false,
                )?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryContribution {
    pub attribution: MemoryAttribution,
    pub summary: Option<String>,
    pub key_memories: Vec<String>,
}

impl MemoryContribution {
    fn validate(&self) -> Result<(), crate::ValidationError> {
        if let Some(summary) = &self.summary {
            crate::validation::validate_text(
                "memory_contribution.summary",
                summary,
                crate::validation::MAX_REASONING_BYTES,
                false,
            )?;
        }
        crate::validation::validate_collection(
            "memory_contribution.key_memories",
            &self.key_memories,
            crate::validation::MAX_MEMORY_REVISIONS,
        )?;
        for memory in &self.key_memories {
            crate::validation::validate_text(
                "memory_contribution.key_memory",
                memory,
                crate::validation::MAX_REASONING_BYTES,
                false,
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyContext {
    Standard,
    Restricted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderContextPart {
    Text {
        text: String,
    },
    MediaAsset {
        asset_id: AssetId,
        role: MediaAssetRole,
    },
    ToolCall(crate::TranscriptToolCall),
    ToolResult(crate::TranscriptToolResult),
}

impl ProviderContextPart {
    fn validate(&self) -> Result<(), crate::ValidationError> {
        match self {
            Self::Text { text } => crate::validation::validate_text(
                "provider_context_part.text",
                text,
                crate::validation::MAX_AUTHORED_TEXT_BYTES,
                false,
            )?,
            Self::ToolCall(call) => call.validate()?,
            Self::ToolResult(result) => result.validate()?,
            Self::MediaAsset { .. } => {}
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderNeutralMessage {
    pub role: MessageRole,
    pub parts: Vec<ProviderContextPart>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderNeutralContext {
    pub messages: Vec<ProviderNeutralMessage>,
    pub attributions: ContextAttributions,
    pub budget: ContextBudgetReport,
}

impl ProviderNeutralContext {
    pub fn validate(&self) -> Result<(), crate::ValidationError> {
        if self.messages.len() > 512 {
            return Err(crate::ValidationError::TooMany {
                field: "provider_context.messages",
                max: 512,
            });
        }
        if self.attributions.lorebooks.len() > crate::validation::MAX_LOREBOOKS {
            return Err(crate::ValidationError::TooMany {
                field: "provider_context.lorebooks",
                max: crate::validation::MAX_LOREBOOKS,
            });
        }
        if let Some(prompt) = &self.attributions.prompt {
            if prompt.revision.get() == 0 {
                return Err(crate::ValidationError::ZeroRevision);
            }
            crate::validation::validate_collection(
                "provider_context.prompt_entry_ids",
                &prompt.selected_entry_ids,
                crate::validation::MAX_DOCUMENT_ENTRIES,
            )?;
            crate::validation::validate_unique(
                "provider_context.prompt_entry_ids",
                prompt.selected_entry_ids.iter().copied(),
            )?;
        }
        for lorebook in &self.attributions.lorebooks {
            if lorebook.revision.get() == 0 {
                return Err(crate::ValidationError::ZeroRevision);
            }
            crate::validation::validate_collection(
                "provider_context.lorebook_entry_ids",
                &lorebook.activated_entry_ids,
                crate::validation::MAX_DOCUMENT_ENTRIES,
            )?;
            crate::validation::validate_unique(
                "provider_context.lorebook_entry_ids",
                lorebook.activated_entry_ids.iter().copied(),
            )?;
        }
        let mut tool_calls = std::collections::HashMap::new();
        let mut tool_results = std::collections::HashSet::new();
        for message in &self.messages {
            if message.parts.len() > crate::validation::MAX_PARTS {
                return Err(crate::ValidationError::TooMany {
                    field: "provider_context.parts",
                    max: crate::validation::MAX_PARTS,
                });
            }
            for part in &message.parts {
                part.validate()?;
                match part {
                    ProviderContextPart::ToolCall(call) => {
                        if message.role != MessageRole::Assistant {
                            return Err(crate::ValidationError::Invariant {
                                field: "provider_context.tool_call_role",
                            });
                        }
                        if tool_calls
                            .insert(
                                call.execution_id,
                                (call.name.as_str(), call.provider_call_id.as_deref()),
                            )
                            .is_some()
                        {
                            return Err(crate::ValidationError::Duplicate {
                                field: "provider_context.tool_call",
                            });
                        }
                    }
                    ProviderContextPart::ToolResult(result) => {
                        if message.role != MessageRole::User {
                            return Err(crate::ValidationError::Invariant {
                                field: "provider_context.tool_result_role",
                            });
                        }
                        let Some((name, provider_call_id)) = tool_calls.get(&result.execution_id)
                        else {
                            return Err(crate::ValidationError::InvalidReference {
                                field: "provider_context.tool_result",
                            });
                        };
                        if *name != result.name
                            || *provider_call_id != result.provider_call_id.as_deref()
                        {
                            return Err(crate::ValidationError::Invariant {
                                field: "provider_context.tool_result_identity",
                            });
                        }
                        if !tool_results.insert(result.execution_id) {
                            return Err(crate::ValidationError::Duplicate {
                                field: "provider_context.tool_result",
                            });
                        }
                    }
                    ProviderContextPart::Text { .. } | ProviderContextPart::MediaAsset { .. } => {}
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContextAttributions {
    pub prompt: Option<PromptAttribution>,
    pub lorebooks: Vec<LorebookAttribution>,
    pub memory: Option<MemoryAttribution>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ContextBudgetReport {
    pub selected_messages: u32,
    pub omitted_messages: u32,
    pub input_bytes: u32,
    /// A conservative estimate intended for admission and reporting. It is
    /// not a provider tokenizer result.
    pub estimated_input_tokens: u32,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeakerPolicyRequest {
    pub conversation_id: ConversationId,
    pub branch_id: ConversationBranchId,
    pub operation: GenerationOperation,
    pub forced_speaker: Option<ConversationParticipantId>,
    pub mention_source: Option<ConversationParticipantId>,
    pub participants: Vec<SpeakerParticipantState>,
    pub prior_speaker: Option<ConversationParticipantId>,
    pub timeline: Vec<TimelineItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeakerParticipantState {
    pub id: ConversationParticipantId,
    pub eligible: bool,
    pub muted: bool,
    pub speak_count: u32,
    pub last_spoke_turn: Option<GenerationTurnId>,
    pub last_spoke_at: Option<TimestampMillis>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelResolveRequest {
    pub conversation_id: ConversationId,
    pub operation: GenerationOperation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InferenceRequest {
    pub turn_id: GenerationTurnId,
    pub attempt_id: GenerationAttemptId,
    pub operation: GenerationOperation,
    pub profile: ResolvedInferenceProfile,
    pub context: ProviderNeutralContext,
    pub cancellation: Option<JobId>,
    pub stream_sink: Option<lettuce_types::RequestId>,
    pub media_grants: Vec<lettuce_types::AssetId>,
    pub tools: Option<crate::ToolRequest>,
}

impl InferenceRequest {
    pub fn validate(&self) -> Result<(), crate::ValidationError> {
        self.context.validate()?;
        match (self.profile.tool_policy, &self.tools) {
            (ToolPolicy::Disabled, None) => {}
            (ToolPolicy::Disabled, Some(_))
            | (ToolPolicy::Allowed | ToolPolicy::Required, None) => {
                return Err(crate::ValidationError::Invariant {
                    field: "inference_request.tools",
                });
            }
            (ToolPolicy::Allowed, Some(tools)) => tools.validate()?,
            (ToolPolicy::Required, Some(tools)) => {
                tools.validate()?;
                if !matches!(
                    tools.choice,
                    crate::ToolChoice::Required | crate::ToolChoice::Named { .. }
                ) {
                    return Err(crate::ValidationError::Invariant {
                        field: "inference_request.tool_choice",
                    });
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedInferenceProfile {
    pub chat_profile: lettuce_models::ResolvedChatProfile,
    pub tool_policy: ToolPolicy,
    pub output_policy: OutputPolicy,
    pub safety_policy: SafetyContext,
    pub correlation_id: Option<lettuce_types::RequestId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolPolicy {
    Disabled,
    Allowed,
    Required,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputPolicy {
    Plain,
    Structured,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceCandidate {
    pub ordinal: u16,
    pub parts: Vec<MessagePart>,
    pub tool_calls: Vec<crate::ProposedToolCall>,
    pub provider_replay: Option<ReplayArtifactRef>,
}

impl InferenceCandidate {
    pub fn validate(&self) -> Result<(), crate::ValidationError> {
        crate::validation::validate_collection(
            "inference_candidate.parts",
            &self.parts,
            crate::validation::MAX_PARTS,
        )?;
        crate::validation::validate_collection(
            "inference_candidate.tool_calls",
            &self.tool_calls,
            crate::MAX_TOOL_CALLS_PER_RESPONSE,
        )?;
        for part in &self.parts {
            part.validate()?;
        }
        for call in &self.tool_calls {
            call.validate()?;
        }
        crate::validation::validate_unique(
            "inference_candidate.provider_call_ids",
            self.tool_calls
                .iter()
                .filter_map(|call| call.provider_call_id.as_deref()),
        )?;
        if let Some(replay) = &self.provider_replay {
            replay.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceOutcome {
    pub candidates: Vec<InferenceCandidate>,
    pub usage: Option<InferenceUsage>,
    pub finish_reason: FinishReason,
    /// Provider-native finish reason for diagnostics and compatibility.
    pub provider_finish_reason: Option<String>,
    /// Bounded request identifier returned in the provider's response headers.
    pub provider_request_id: Option<String>,
    pub warning_codes: Vec<InferenceWarningCode>,
}

impl InferenceOutcome {
    pub fn validate(&self) -> Result<(), crate::ValidationError> {
        if self.candidates.is_empty() {
            return Err(crate::ValidationError::InvalidValue {
                field: "inference_outcome.candidates",
            });
        }
        for candidate in &self.candidates {
            candidate.validate()?;
        }
        let candidates_with_tools = self
            .candidates
            .iter()
            .filter(|candidate| !candidate.tool_calls.is_empty())
            .count();
        if candidates_with_tools > 0 && self.candidates.len() != 1 {
            return Err(crate::ValidationError::Invariant {
                field: "inference_outcome.tool_candidate",
            });
        }
        crate::validation::validate_unique(
            "inference_outcome.candidate_ordinals",
            self.candidates.iter().map(|candidate| candidate.ordinal),
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinishReason {
    Stop,
    Length,
    Cancelled,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InferenceWarningCode {
    Truncated,
    SafetyTransformed,
    ProviderDegraded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRequest {
    pub turn_id: GenerationTurnId,
    pub asset_ids: Vec<lettuce_types::AssetId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRequest {
    pub conversation_id: ConversationId,
    pub branch_id: ConversationBranchId,
    pub source_message_id: Option<MessageId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryResult {
    pub revision_id: Option<lettuce_types::MemoryRevisionId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionRequest {
    pub conversation_id: ConversationId,
    pub turn_id: GenerationTurnId,
    pub message_revision_id: MessageRevisionId,
    pub effective_time: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionEffectProposal {
    pub effect_id: lettuce_types::CompanionEffectId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageRecord {
    pub turn_id: GenerationTurnId,
    pub attempt_id: GenerationAttemptId,
    pub outcome: UsageOutcome,
    pub usage: UsageCounters,
    pub model_profile_id: Option<lettuce_types::ModelProfileId>,
    pub model_revision: Option<lettuce_types::Revision>,
    pub provider_account_id: Option<lettuce_types::ProviderAccountId>,
    pub provider_account_revision: Option<lettuce_types::Revision>,
    pub recorded_at: TimestampMillis,
}

impl UsageRecord {
    pub fn validate(&self) -> Result<(), crate::ValidationError> {
        let model_pair_complete = self.model_profile_id.is_some() == self.model_revision.is_some();
        let provider_pair_complete =
            self.provider_account_id.is_some() == self.provider_account_revision.is_some();
        if !model_pair_complete || !provider_pair_complete {
            return Err(crate::ValidationError::InvalidReference {
                field: "usage_record.provenance_pair",
            });
        }
        match &self.usage {
            UsageCounters::Known(_) => {
                if self.model_profile_id.is_none() || self.provider_account_id.is_none() {
                    return Err(crate::ValidationError::InvalidReference {
                        field: "usage_record.known_provenance",
                    });
                }
            }
            UsageCounters::Unavailable(_) => {}
        }
        if self
            .model_revision
            .is_some_and(|revision| revision.get() == 0)
            || self
                .provider_account_revision
                .is_some_and(|revision| revision.get() == 0)
        {
            return Err(crate::ValidationError::ZeroRevision);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsageCounters {
    Known(InferenceUsage),
    Unavailable(UsageUnavailableReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageUnavailableReason {
    NotAdmitted,
    CancelledBeforeResponse,
    ProviderOmitted,
    TransportFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageOutcome {
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderFailureKind {
    CredentialRejected,
    RequestRejected,
    Unavailable,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProviderFailure {
    pub kind: ProviderFailureKind,
    pub status: u16,
    pub code: Option<String>,
    pub message: Option<String>,
    pub request_id: Option<String>,
}

impl std::fmt::Debug for ProviderFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderFailure")
            .field("kind", &self.kind)
            .field("status", &self.status)
            .field("code", &self.code)
            .field("message", &self.message.as_ref().map(|_| "[REDACTED]"))
            .field("request_id", &self.request_id)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PortError {
    #[error("conversation dependency is unavailable")]
    Unavailable,
    #[error("conversation dependency rejected the operation")]
    Rejected,
    #[error("conversation dependency returned no result")]
    Empty,
    #[error("conversation operation was cancelled")]
    Cancelled,
    #[error("provider request failed")]
    Provider(ProviderFailure),
}

/// Redacted failures emitted while assembling provider-neutral context.
///
/// Variants intentionally carry no authored text, provider payload, or
/// dependency error. Callers can map these stable categories to UX and
/// telemetry without accidentally logging prompt content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ContextAssemblyError {
    #[error("context request is invalid")]
    InvalidRequest,
    #[error("conversation is unavailable")]
    ConversationUnavailable,
    #[error("snapshot is unavailable ({kind:?})")]
    SnapshotUnavailable { kind: SnapshotDocumentKind },
    #[error("snapshot is invalid ({kind:?})")]
    SnapshotInvalid { kind: SnapshotDocumentKind },
    #[error("prompt rendering failed")]
    PromptRender,
    #[error("lorebook activation failed")]
    LorebookActivation,
    #[error("conversation speaker is missing")]
    MissingSpeaker,
    #[error("conversation timeline is invalid")]
    InvalidTimeline,
    #[error("context part is unsupported")]
    UnsupportedPart,
    #[error("context exceeds its size limit")]
    SizeLimit,
}

#[async_trait]
pub trait LaunchResolver: Send + Sync {
    async fn resolve(&self, request: LaunchResolveRequest) -> Result<LaunchResolution, PortError>;
}

#[async_trait]
pub trait ContextAssembler: Send + Sync {
    async fn assemble(
        &self,
        request: ContextRequest,
    ) -> Result<ProviderNeutralContext, ContextAssemblyError>;
}

#[async_trait]
pub trait SpeakerPolicy: Send + Sync {
    async fn choose(
        &self,
        request: SpeakerPolicyRequest,
    ) -> Result<crate::generation::SelectedSpeakerDecision, PortError>;
}

#[async_trait]
pub trait ModelResolver: Send + Sync {
    async fn resolve(
        &self,
        request: ModelResolveRequest,
    ) -> Result<ResolvedInferenceProfile, PortError>;
}

#[async_trait]
pub trait InferencePort: Send + Sync {
    async fn run(&self, request: InferenceRequest) -> Result<InferenceOutcome, PortError>;
}

#[async_trait]
pub trait MediaPort: Send + Sync {
    async fn validate_assets(&self, request: MediaRequest) -> Result<(), PortError>;
}

#[async_trait]
pub trait MemoryPort: Send + Sync {
    async fn resolve(&self, request: MemoryRequest) -> Result<MemoryResult, PortError>;
}

#[async_trait]
pub trait CompanionPort: Send + Sync {
    async fn propose(
        &self,
        request: CompanionRequest,
    ) -> Result<Vec<CompanionEffectProposal>, PortError>;
}

#[async_trait]
pub trait UsagePort: Send + Sync {
    async fn record(&self, record: UsageRecord) -> Result<UsageEventId, PortError>;
}

#[async_trait]
pub trait JobPort: Send + Sync {
    async fn start(&self, spec: AttemptJobSpec) -> Result<JobId, PortError>;
    async fn cancel(&self, job_id: JobId) -> Result<(), PortError>;
    async fn emit(&self, event: GenerationCheckpointEnvelope) -> Result<(), PortError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptJobSpec {
    pub turn_id: GenerationTurnId,
    pub attempt_id: GenerationAttemptId,
    pub idempotency_key: IdempotencyKey,
}

#[async_trait]
pub trait Clock: Send + Sync {
    async fn now(&self) -> Result<TimestampMillis, PortError>;
}

#[async_trait]
pub trait ConversationApplication: Send + Sync {
    async fn send(&self, command: SendConversation) -> Result<SendConversationResult, PortError>;
    async fn continue_generation(
        &self,
        command: ContinueConversation,
    ) -> Result<ContinueConversationResult, PortError>;
    async fn regenerate(
        &self,
        command: RegenerateCandidate,
    ) -> Result<RegenerateCandidateResult, PortError>;
    async fn retry(&self, command: RetryGeneration) -> Result<RetryGenerationResult, PortError>;
    async fn cancel(
        &self,
        command: crate::commands::CancelGeneration,
    ) -> Result<CancelGenerationResult, PortError>;
    async fn get_operation(
        &self,
        conversation_id: ConversationId,
        operation_id: OperationRecordId,
    ) -> Result<OperationRecord, PortError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_request_requires_a_source_message_in_the_timeline() {
        let branch_id = ConversationBranchId::new();
        let request = ContextRequest {
            conversation_id: ConversationId::new(),
            branch_id,
            branch_path: vec![branch_id],
            source_message_id: MessageId::new(),
            operation: GenerationOperation::Send,
            swap_roles: false,
            guidance: None,
            window: ContextWindowPolicy::default(),
            selected_speaker: None,
            capabilities: ModelCapabilities::default(),
            safety: SafetyContext::Standard,
            prompt_runtime: PromptRuntimeFacts::default(),
            prompt_values: PromptRuntimeValues::default(),
            memory: None,
            timeline: Vec::new(),
        };
        assert!(matches!(
            request.validate(),
            Err(crate::ValidationError::InvalidReference {
                field: "context_request.source_message_id"
            })
        ));
    }

    #[test]
    fn provider_context_parts_are_limited_to_text_and_media() {
        let context = ProviderNeutralContext {
            messages: vec![ProviderNeutralMessage {
                role: MessageRole::System,
                parts: vec![
                    ProviderContextPart::Text {
                        text: "rules".into(),
                    },
                    ProviderContextPart::MediaAsset {
                        asset_id: AssetId::new(),
                        role: MediaAssetRole::Inline,
                    },
                ],
            }],
            attributions: ContextAttributions::default(),
            budget: ContextBudgetReport {
                selected_messages: 1,
                omitted_messages: 0,
                input_bytes: 5,
                estimated_input_tokens: 2,
                truncated: false,
            },
        };
        assert!(context.validate().is_ok());
    }

    #[test]
    fn runtime_facts_and_memory_text_are_bounded() {
        let runtime = PromptRuntimeFacts {
            provider_label: Some("x".repeat(crate::validation::MAX_DISPLAY_CHARS + 1)),
            ..PromptRuntimeFacts::default()
        };
        assert!(runtime.validate().is_err());

        let memory = MemoryContribution {
            attribution: MemoryAttribution {
                revision_id: lettuce_types::MemoryRevisionId::new(),
            },
            summary: Some("summary".into()),
            key_memories: vec!["a durable fact".into()],
        };
        assert!(memory.validate().is_ok());
    }

    #[test]
    fn pre_resolved_prompt_runtime_values_are_bounded_and_optional() {
        let values = PromptRuntimeValues {
            content_rules: Some("keep replies concise".into()),
            date: Some("2026-08-25".into()),
            ..PromptRuntimeValues::default()
        };
        assert!(values.validate().is_ok());
        let oversized = PromptRuntimeValues {
            companion_state: Some("x".repeat(crate::validation::MAX_AUTHORED_TEXT_BYTES + 1)),
            ..PromptRuntimeValues::default()
        };
        assert!(oversized.validate().is_err());
    }

    #[test]
    fn context_assembly_errors_do_not_include_authored_text() {
        let errors = [
            ContextAssemblyError::InvalidRequest,
            ContextAssemblyError::ConversationUnavailable,
            ContextAssemblyError::SnapshotUnavailable {
                kind: SnapshotDocumentKind::Prompt,
            },
            ContextAssemblyError::SnapshotInvalid {
                kind: SnapshotDocumentKind::Lorebook,
            },
            ContextAssemblyError::PromptRender,
            ContextAssemblyError::LorebookActivation,
            ContextAssemblyError::MissingSpeaker,
            ContextAssemblyError::InvalidTimeline,
            ContextAssemblyError::UnsupportedPart,
            ContextAssemblyError::SizeLimit,
        ];
        for error in errors {
            assert!(!format!("{error:?}").contains("secret"));
            assert!(!error.to_string().contains("secret"));
        }
    }
}
