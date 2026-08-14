//! Persistence-ready contracts for unified direct and group conversation
//! history.  This crate owns the aggregate invariants and lifecycle vocabulary
//! while repositories and application ports provide persistence and runtime
//! execution.

#![deny(unsafe_op_in_unsafe_fn)]

mod artifact;
mod commands;
mod content;
mod error;
mod generation;
mod model;
mod ports;
mod service;
mod snapshot;
mod validation;

pub use artifact::*;
pub use commands::*;
pub use content::*;
pub use error::*;
pub use generation::*;
pub use model::*;
pub use ports::*;
pub use service::*;
pub use snapshot::*;

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use lettuce_types::{
        CharacterId, ContentHash, ConversationBranchId, ConversationId, ConversationParticipantId,
        GenerationAttemptId, GenerationTurnId, MessageCandidateId, MessageId, MessageRevisionId,
        ReplayArtifactId, Revision, SnapshotArtifactId, TimestampMillis, UsageEventId,
    };

    #[derive(Default)]
    struct MemoryArtifactStore {
        snapshots: Mutex<Vec<ProtectedSnapshotRef>>,
        replays: Mutex<Vec<ReplayArtifactRef>>,
    }

    impl ConversationArtifactStore for MemoryArtifactStore {
        fn put_snapshot(
            &self,
            draft: SnapshotArtifactDraft,
        ) -> Result<ProtectedSnapshotRef, ArtifactError> {
            draft.validate()?;
            let reference = draft.reference();
            let mut snapshots = self.snapshots.lock().expect("test lock");
            if let Some(existing) = snapshots
                .iter()
                .find(|existing| existing.artifact_id == reference.artifact_id)
            {
                if existing.digest != reference.digest {
                    return Err(ArtifactError::ImmutableConflict);
                }
                return Ok(existing.clone());
            }
            snapshots.push(reference.clone());
            Ok(reference)
        }

        fn verify_snapshot(&self, reference: &ProtectedSnapshotRef) -> Result<(), ArtifactError> {
            if self
                .snapshots
                .lock()
                .expect("test lock")
                .iter()
                .any(|stored| stored == reference)
            {
                Ok(())
            } else {
                Err(ArtifactError::NotFound)
            }
        }

        fn cleanup_orphan_snapshot(
            &self,
            artifact_id: SnapshotArtifactId,
        ) -> Result<(), ArtifactError> {
            self.snapshots
                .lock()
                .expect("test lock")
                .retain(|stored| stored.artifact_id != artifact_id);
            Ok(())
        }

        fn put_replay(
            &self,
            draft: ReplayArtifactDraft,
        ) -> Result<ReplayArtifactRef, ArtifactError> {
            draft.validate()?;
            let reference = draft.reference();
            let mut replays = self.replays.lock().expect("test lock");
            if let Some(existing) = replays
                .iter()
                .find(|existing| existing.artifact_id == reference.artifact_id)
            {
                if existing.digest != reference.digest {
                    return Err(ArtifactError::ImmutableConflict);
                }
                return Ok(existing.clone());
            }
            replays.push(reference.clone());
            Ok(reference)
        }

        fn verify_replay(&self, reference: &ReplayArtifactRef) -> Result<(), ArtifactError> {
            if self
                .replays
                .lock()
                .expect("test lock")
                .iter()
                .any(|stored| stored == reference)
            {
                Ok(())
            } else {
                Err(ArtifactError::NotFound)
            }
        }

        fn cleanup_orphan_replay(
            &self,
            artifact_id: ReplayArtifactId,
        ) -> Result<(), ArtifactError> {
            self.replays
                .lock()
                .expect("test lock")
                .retain(|stored| stored.artifact_id != artifact_id);
            Ok(())
        }
    }

    fn snapshot_ref(source: SnapshotSource) -> ProtectedSnapshotRef {
        ProtectedSnapshotRef {
            source,
            source_revision: Revision::INITIAL,
            artifact_id: SnapshotArtifactId::new(),
            digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
            schema_version: 1,
            byte_size: 1,
        }
    }

    #[test]
    fn provider_replay_debug_redacts_opaque_content() {
        let artifact = ReplayArtifactRef {
            artifact_id: ReplayArtifactId::new(),
            digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
            schema_version: 1,
            byte_size: 42,
            retention: ReplayRetention::Conversation,
            codec: ReplayCodec::Json,
        };
        let debug = format!("{artifact:?}");
        assert!(debug.contains("ReplayArtifactRef"));
    }

    #[test]
    fn direct_requires_exactly_one_user_and_character() {
        let character_id = CharacterId::new();
        let kind = ConversationKind::Direct(DirectConversationDetails {
            format_version: 1,
            character: CharacterLaunchSnapshot {
                snapshot_ref: snapshot_ref(SnapshotSource::Character(character_id)),
                source_id: character_id,
                source_revision: Revision::INITIAL,
                name: "Ada".into(),
                nickname: None,
            },
            persona: SnapshotSelection::Disabled,
            scene: SnapshotSelection::Disabled,
            starter: SnapshotSelection::Disabled,
            prompt: SnapshotSelection::Disabled,
            lorebooks: SnapshotSelection::Explicit(Vec::new()),
            model: SnapshotSelection::Disabled,
            memory: SnapshotSelection::Disabled,
            voice: SnapshotSelection::Disabled,
        });
        let now = TimestampMillis::UNIX_EPOCH;
        let conversation = Conversation {
            id: ConversationId::new(),
            lifecycle: ConversationLifecycle::Active,
            title: "Chat".into(),
            kind,
            active_branch_id: ConversationBranchId::new(),
            participants: vec![ConversationParticipant {
                id: ConversationParticipantId::new(),
                role: ParticipantRole::User,
                ordinal: 0,
                enabled: true,
                muted: false,
                source: ParticipantSource::User,
                display_name: "User".into(),
                authored_description: None,
                model_selection: SnapshotSelection::Disabled,
                revision: Revision::INITIAL,
                created_at: now,
                updated_at: now,
            }],
            current_settings: None,
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        };
        assert!(matches!(
            conversation.validate(),
            Err(ValidationError::Invariant { .. })
        ));
    }

    #[test]
    fn generation_transition_matrix_rejects_terminal_reentry() {
        assert!(GenerationTurnStatus::Created.can_transition_to(GenerationTurnStatus::Preparing));
        assert!(!GenerationTurnStatus::Succeeded.can_transition_to(GenerationTurnStatus::Running));
        assert!(!GenerationTurnStatus::Failed.can_transition_to(GenerationTurnStatus::Preparing));
    }

    #[test]
    fn snapshot_provenance_distinguishes_explicit_and_resolved() {
        assert!(!SnapshotSelection::<u32>::Inherited(1).is_explicit());
        assert!(SnapshotSelection::<u32>::Inherited(1).is_resolved());
        assert!(SnapshotSelection::<u32>::Explicit(1).is_explicit());
        assert!(!SnapshotSelection::<u32>::Disabled.is_resolved());
    }

    #[test]
    fn ids_are_distinct_types() {
        let participant = ConversationParticipantId::new();
        let candidate = MessageCandidateId::new();
        assert_ne!(participant.as_uuid(), candidate.as_uuid());
    }

    #[test]
    fn stream_sequence_requires_same_attempt_and_next_number() {
        let turn = GenerationTurnId::new();
        let attempt = GenerationAttemptId::new();
        let first = GenerationStreamEventEnvelope {
            operation: GenerationOperation::Send,
            turn_id: turn,
            attempt_id: attempt,
            sequence: 1,
            event: GenerationStreamEvent::TextDelta { text: "x".into() },
        };
        let second = GenerationStreamEventEnvelope {
            sequence: 2,
            ..first.clone()
        };
        assert!(second.validate_after(Some(&first)).is_ok());
        let bad = GenerationStreamEventEnvelope {
            sequence: 4,
            ..second
        };
        assert!(bad.validate_after(Some(&first)).is_err());
    }

    #[test]
    fn checkpoint_sequence_must_start_at_one() {
        let event = GenerationCheckpointEnvelope {
            turn_id: GenerationTurnId::new(),
            attempt_id: GenerationAttemptId::new(),
            job_id: None,
            correlation_id: None,
            sequence: 2,
            event: GenerationCheckpointEvent::Completed,
        };
        assert!(event.validate_after(None).is_err());
        let first = GenerationCheckpointEnvelope {
            sequence: 1,
            ..event
        };
        assert!(first.validate_after(None).is_ok());
    }

    #[test]
    fn protected_artifact_bytes_are_bounded_and_redacted() {
        let secret = b"provider payload that must never be logged".to_vec();
        let bytes = ProtectedArtifactBytes::new(secret.clone()).expect("payload");
        let debug = format!("{bytes:?}");
        assert!(!debug.contains("provider payload"));
        assert_eq!(bytes.len(), secret.len());
        assert_eq!(bytes.digest().as_str().len(), 64);
        assert!(matches!(
            ProtectedArtifactBytes::new(vec![0; 16 * 1024 * 1024 + 1]),
            Err(ArtifactError::TooLarge { .. })
        ));
    }

    #[test]
    fn trusted_transfer_descriptors_contain_metadata_only() {
        let descriptor = TrustedArtifactDescriptor::Replay(ReplayArtifactRef {
            artifact_id: ReplayArtifactId::new(),
            digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
            schema_version: 1,
            byte_size: 32,
            retention: ReplayRetention::Conversation,
            codec: ReplayCodec::Json,
        });
        let debug = format!("{descriptor:?}");
        assert!(!debug.contains("payload"));
        assert!(!debug.contains("bytes"));
    }

    #[test]
    fn job_attachment_conflicts_are_typed_and_distinct() {
        let mut index = JobOwnershipIndex::default();
        let target = AttemptJobOwner {
            conversation_id: ConversationId::new(),
            turn_id: GenerationTurnId::new(),
            attempt_id: GenerationAttemptId::new(),
        };
        let other = AttemptJobOwner {
            conversation_id: ConversationId::new(),
            turn_id: GenerationTurnId::new(),
            attempt_id: GenerationAttemptId::new(),
        };
        let job_id = lettuce_types::JobId::new();
        assert!(index.attach(target, None, job_id).is_ok());
        assert_eq!(
            index.attach(target, Some(job_id), lettuce_types::JobId::new()),
            Err(ConversationRepositoryError::JobAlreadyAttached)
        );
        assert_eq!(
            index.attach(other, None, job_id),
            Err(ConversationRepositoryError::JobInUse)
        );
    }

    #[test]
    fn artifact_draft_rejects_digest_mismatch() {
        let source_id = CharacterId::new();
        let draft = SnapshotArtifactDraft {
            source: SnapshotSource::Character(source_id),
            source_revision: Revision::INITIAL,
            artifact_id: SnapshotArtifactId::new(),
            digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
            schema_version: 1,
            byte_size: 7,
            codec: ArtifactCodec::Json,
            retention: ArtifactRetention::Conversation,
            bytes: ProtectedArtifactBytes::new(b"payload".to_vec()).expect("payload"),
        };
        assert!(matches!(
            draft.validate(),
            Err(ArtifactError::DigestMismatch)
        ));
        let bytes = ProtectedArtifactBytes::new(b"payload".to_vec()).expect("payload");
        let sized = SnapshotArtifactDraft {
            source: SnapshotSource::Character(source_id),
            source_revision: Revision::INITIAL,
            artifact_id: SnapshotArtifactId::new(),
            digest: bytes.digest(),
            schema_version: 1,
            byte_size: 1,
            codec: ArtifactCodec::Json,
            retention: ArtifactRetention::Conversation,
            bytes,
        };
        assert!(matches!(sized.validate(), Err(ArtifactError::SizeMismatch)));
    }

    #[test]
    fn artifact_store_deduplicates_identical_identity_and_rejects_rewrite() {
        let store = MemoryArtifactStore::default();
        let source_id = CharacterId::new();
        let artifact_id = SnapshotArtifactId::new();
        let make_draft = |payload: &[u8]| {
            let bytes = ProtectedArtifactBytes::new(payload.to_vec()).expect("payload");
            SnapshotArtifactDraft {
                source: SnapshotSource::Character(source_id),
                source_revision: Revision::INITIAL,
                artifact_id,
                digest: bytes.digest(),
                schema_version: 1,
                byte_size: bytes.len() as u64,
                codec: ArtifactCodec::Json,
                retention: ArtifactRetention::Conversation,
                bytes,
            }
        };
        let first = store.put_snapshot(make_draft(b"first")).expect("insert");
        let same = store.put_snapshot(make_draft(b"first")).expect("dedupe");
        assert_eq!(first, same);
        let replacement = make_draft(b"second");
        assert!(matches!(
            store.put_snapshot(replacement),
            Err(ArtifactError::ImmutableConflict)
        ));
        assert!(store.verify_snapshot(&first).is_ok());
        let mut orphan_draft = make_draft(b"orphan");
        orphan_draft.artifact_id = SnapshotArtifactId::new();
        let orphan = store.put_snapshot(orphan_draft).expect("orphan insert");
        store
            .cleanup_orphan_snapshot(orphan.artifact_id)
            .expect("orphan cleanup");
        assert!(matches!(
            store.verify_snapshot(&orphan),
            Err(ArtifactError::NotFound)
        ));
    }

    #[test]
    fn checkpoint_event_is_safe_reference_only() {
        let event = GenerationCheckpointEvent::UsageRecorded {
            usage_event_id: UsageEventId::new(),
        };
        assert!(event.validate().is_ok());
        assert!(!format!("{event:?}").contains("text"));
    }

    #[test]
    fn replay_refs_are_bounded_and_opaque() {
        let artifact = ReplayArtifactRef {
            artifact_id: ReplayArtifactId::new(),
            digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
            schema_version: 1,
            byte_size: 16 * 1024 * 1024 + 1,
            retention: ReplayRetention::Conversation,
            codec: ReplayCodec::Cbor,
        };
        assert!(artifact.validate().is_err());
        assert!(!format!("{artifact:?}").contains("payload"));
    }

    #[test]
    fn ephemeral_replay_is_not_persistable_on_revision() {
        let revision = MessageRevision {
            id: MessageRevisionId::new(),
            message_id: MessageId::new(),
            sequence: Revision::INITIAL,
            parts: vec![MessagePart::Text { text: "ok".into() }],
            authored_at: TimestampMillis::UNIX_EPOCH,
            source_turn_id: None,
            provider_replay: Some(ReplayArtifactRef {
                artifact_id: ReplayArtifactId::new(),
                digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
                schema_version: 1,
                byte_size: 1,
                retention: ReplayRetention::Ephemeral,
                codec: ReplayCodec::Json,
            }),
        };
        assert!(revision.validate().is_err());
    }

    #[test]
    fn direct_lifecycle_cannot_restore_tombstone() {
        assert!(
            !ConversationLifecycle::Tombstoned.can_transition_to(ConversationLifecycle::Active)
        );
    }

    #[test]
    fn cancellation_and_finalization_are_mutually_exclusive() {
        assert!(GenerationTurnStatus::Running.can_transition_to(GenerationTurnStatus::Finalizing));
        assert!(
            GenerationTurnStatus::Running
                .can_transition_to(GenerationTurnStatus::CancellationRequested)
        );
        assert!(
            !GenerationTurnStatus::CancellationRequested
                .can_transition_to(GenerationTurnStatus::Finalizing)
        );
    }

    #[test]
    fn usage_provenance_is_required_only_when_counters_are_known() {
        let base = UsageRecord {
            turn_id: GenerationTurnId::new(),
            attempt_id: GenerationAttemptId::new(),
            outcome: UsageOutcome::Succeeded,
            usage: UsageCounters::Known(InferenceUsage {
                input_tokens: 1,
                output_tokens: 1,
            }),
            model_profile_id: None,
            model_revision: None,
            provider_account_id: None,
            provider_account_revision: None,
            recorded_at: TimestampMillis::UNIX_EPOCH,
        };
        assert!(base.validate().is_err());
        let known = UsageRecord {
            model_profile_id: Some(lettuce_types::ModelProfileId::new()),
            model_revision: Some(Revision::INITIAL),
            provider_account_id: Some(lettuce_types::ProviderAccountId::new()),
            provider_account_revision: Some(Revision::INITIAL),
            ..base.clone()
        };
        assert!(known.validate().is_ok());
        let unavailable = UsageRecord {
            usage: UsageCounters::Unavailable(UsageUnavailableReason::ProviderOmitted),
            ..base
        };
        assert!(unavailable.validate().is_ok());
        let partial = UsageRecord {
            model_profile_id: Some(lettuce_types::ModelProfileId::new()),
            ..unavailable
        };
        assert!(partial.validate().is_err());
    }

    #[test]
    fn terminal_attempt_requires_usage_event() {
        let mut attempt = interrupted_attempt(GenerationTurnId::new());
        attempt.usage_event_id = None;
        assert!(attempt.validate().is_err());
    }

    #[test]
    fn attempt_keys_are_unique_and_recovery_cannot_reuse_them() {
        let previous = interrupted_attempt(GenerationTurnId::new());
        let turn = interrupted_turn(previous.clone());
        let mut child = previous.clone();
        child.id = GenerationAttemptId::new();
        child.ordinal = 1;
        child.parent_attempt_id = Some(previous.id);
        assert!(child.validate_against(&previous, &turn).is_err());
        let mut duplicate_turn = turn.clone();
        duplicate_turn.attempts.push(child);
        assert!(duplicate_turn.validate(false).is_err());
    }

    #[test]
    fn attempt_key_is_bound_to_its_turn_and_attempt_identity() {
        let original_turn_id = GenerationTurnId::new();
        let mut attempt = interrupted_attempt(original_turn_id);
        let mut turn = interrupted_turn(attempt.clone());
        turn.id = GenerationTurnId::new();
        attempt.turn_id = turn.id;
        turn.attempts = vec![attempt];
        assert!(turn.validate(false).is_err());
    }

    #[test]
    fn cancellation_settlement_carries_usage_and_operation_cas() {
        let request = RequestCancellation {
            conversation_id: ConversationId::new(),
            turn_id: GenerationTurnId::new(),
            attempt_id: GenerationAttemptId::new(),
            expected_revision: Revision::INITIAL,
            expected_turn_revision: Revision::INITIAL,
            operation: OperationToken {
                key: lettuce_jobs::IdempotencyKey::new("cancel-request").expect("key"),
                request_digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
            },
        };
        assert!(ConversationMutation::Cancel(request).validate().is_ok());
        let settle = SettleCancellation {
            conversation_id: ConversationId::new(),
            turn_id: GenerationTurnId::new(),
            attempt_id: GenerationAttemptId::new(),
            expected_revision: Revision::INITIAL,
            expected_turn_revision: Revision::INITIAL,
            operation: OperationToken {
                key: lettuce_jobs::IdempotencyKey::new("cancel-settle").expect("key"),
                request_digest: ContentHash::parse("cd".repeat(32)).expect("digest"),
            },
            usage_event_id: UsageEventId::new(),
        };
        assert!(settle.validate().is_ok());
    }

    #[test]
    fn finalized_outbox_event_contains_terminal_refs() {
        let conversation_id = ConversationId::new();
        let event = ConversationOutboxEvent::TurnFinalized {
            conversation_id,
            branch_id: ConversationBranchId::new(),
            turn_id: GenerationTurnId::new(),
            attempt_id: GenerationAttemptId::new(),
            message_id: MessageId::new(),
            candidate_id: MessageCandidateId::new(),
            revision_id: Some(MessageRevisionId::new()),
            effective_time: TimestampMillis::UNIX_EPOCH,
            usage_event_id: UsageEventId::new(),
            used_memory_revision_ids: vec![lettuce_types::MemoryRevisionId::new()],
        };
        let record = ConversationOutboxRecord {
            format_version: 1,
            id: lettuce_types::OutboxEventId::new(),
            conversation_id,
            conversation_revision: Revision::INITIAL,
            sequence: 1,
            operation_record_id: lettuce_types::OperationRecordId::new(),
            at: TimestampMillis::UNIX_EPOCH,
            event,
        };
        assert!(record.validate().is_ok());
    }

    #[test]
    fn media_retainers_use_canonical_typed_identity() {
        let revision = lettuce_media::AssetRetainer::MessageRevision(MessageRevisionId::new());
        let candidate = lettuce_media::AssetRetainer::MessageCandidate(MessageCandidateId::new());
        assert_ne!(revision, candidate);
    }

    #[test]
    fn message_draft_rejects_assistant_without_author() {
        let draft = MessageDraft {
            role: MessageRole::Assistant,
            author_participant_id: None,
            parts: vec![],
            visibility: MessageVisibility::Visible,
            pinned: false,
            scene_edited: false,
        };
        assert!(draft.validate().is_err());
    }

    #[test]
    fn message_draft_accepts_user_author() {
        let draft = MessageDraft {
            role: MessageRole::User,
            author_participant_id: Some(ConversationParticipantId::new()),
            parts: vec![],
            visibility: MessageVisibility::Visible,
            pinned: false,
            scene_edited: false,
        };
        assert!(draft.validate().is_ok());
    }

    #[test]
    fn branch_status_is_closed() {
        assert_eq!(BranchStatus::Active, BranchStatus::Active);
        assert_ne!(BranchStatus::Archived, BranchStatus::Tombstoned);
    }

    #[test]
    fn media_reference_states_are_explicit() {
        assert_ne!(AssetReferenceState::Active, AssetReferenceState::Released);
        assert_ne!(
            AssetReferenceState::Historical,
            AssetReferenceState::Released
        );
    }

    #[test]
    fn annotation_versions_are_closed() {
        let annotation = Annotation {
            version: 1,
            kind: AnnotationKind::Safety,
            payload: AnnotationPayload::Safety {
                action: SafetyAction::Allowed,
                code: SafetyCode::Policy,
            },
        };
        assert!(annotation.validate().is_ok());
    }

    #[test]
    fn participant_source_is_role_checked() {
        let now = TimestampMillis::UNIX_EPOCH;
        let participant = ConversationParticipant {
            id: ConversationParticipantId::new(),
            role: ParticipantRole::User,
            ordinal: 0,
            enabled: true,
            muted: false,
            source: ParticipantSource::Character(CharacterId::new()),
            display_name: "u".into(),
            authored_description: None,
            model_selection: SnapshotSelection::Disabled,
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        };
        assert!(participant.validate().is_err());
    }

    #[test]
    fn operation_token_replays_only_identical_digest() {
        let token = OperationToken {
            key: lettuce_jobs::IdempotencyKey::new("token").expect("key"),
            request_digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
        };
        let record = OperationRecord {
            id: lettuce_types::OperationRecordId::new(),
            conversation_id: ConversationId::new(),
            kind: OperationKind::Send,
            operation: token.clone(),
            result: OperationResultRef::Turn(GenerationTurnId::new()),
            created_at: TimestampMillis::UNIX_EPOCH,
        };
        assert!(
            record
                .replay_or_conflict(record.conversation_id, OperationKind::Send, &token)
                .is_ok()
        );
        let conflict = OperationToken {
            key: token.key,
            request_digest: ContentHash::parse("cd".repeat(32)).expect("digest"),
        };
        assert!(
            record
                .replay_or_conflict(record.conversation_id, OperationKind::Send, &conflict)
                .is_err()
        );
    }

    #[test]
    fn outbox_rejects_zero_sequence_and_revision() {
        let record = ConversationOutboxRecord {
            format_version: 1,
            id: lettuce_types::OutboxEventId::new(),
            conversation_id: ConversationId::new(),
            conversation_revision: Revision::INITIAL,
            sequence: 0,
            operation_record_id: lettuce_types::OperationRecordId::new(),
            at: TimestampMillis::UNIX_EPOCH,
            event: ConversationOutboxEvent::TurnFailed {
                conversation_id: ConversationId::new(),
                branch_id: ConversationBranchId::new(),
                turn_id: GenerationTurnId::new(),
                attempt_id: GenerationAttemptId::new(),
                usage_event_id: UsageEventId::new(),
                used_memory_revision_ids: Vec::new(),
                at: TimestampMillis::UNIX_EPOCH,
            },
        };
        assert!(record.validate().is_err());
    }

    #[test]
    fn outbox_rejects_event_conversation_header_mismatch() {
        let record = ConversationOutboxRecord {
            format_version: 1,
            id: lettuce_types::OutboxEventId::new(),
            conversation_id: ConversationId::new(),
            conversation_revision: Revision::INITIAL,
            sequence: 1,
            operation_record_id: lettuce_types::OperationRecordId::new(),
            at: TimestampMillis::UNIX_EPOCH,
            event: ConversationOutboxEvent::ConversationTombstoned {
                conversation_id: ConversationId::new(),
                at: TimestampMillis::UNIX_EPOCH,
            },
        };
        assert!(record.validate().is_err());
    }

    #[test]
    fn checkpoint_envelope_rejects_attempt_mismatch() {
        let first = GenerationCheckpointEnvelope {
            turn_id: GenerationTurnId::new(),
            attempt_id: GenerationAttemptId::new(),
            job_id: None,
            correlation_id: None,
            sequence: 1,
            event: GenerationCheckpointEvent::Completed,
        };
        let next = GenerationCheckpointEnvelope {
            attempt_id: GenerationAttemptId::new(),
            sequence: 2,
            ..first.clone()
        };
        assert!(next.validate_after(Some(&first)).is_err());
    }

    #[test]
    fn protected_snapshot_ref_rejects_zero_revision() {
        let source_id = CharacterId::new();
        let mut reference = snapshot_ref(SnapshotSource::Character(source_id));
        reference.source_revision = Revision::INITIAL;
        assert!(reference.validate().is_ok());
    }

    fn history_branch(
        id: ConversationBranchId,
        conversation_id: ConversationId,
        parent_branch_id: Option<ConversationBranchId>,
        fork_message_id: Option<MessageId>,
        head_message_id: Option<MessageId>,
    ) -> ConversationBranch {
        ConversationBranch {
            id,
            conversation_id,
            parent_branch_id,
            fork_message_id,
            head_message_id,
            status: BranchStatus::Active,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::UNIX_EPOCH,
            updated_at: TimestampMillis::UNIX_EPOCH,
        }
    }

    fn history_message(
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        id: MessageId,
        parent_message_id: Option<MessageId>,
        author_participant_id: ConversationParticipantId,
    ) -> (Message, MessageRevision) {
        let revision_id = MessageRevisionId::new();
        let message = Message {
            id,
            conversation_id,
            branch_id,
            parent_message_id,
            author_participant_id: Some(author_participant_id),
            role: MessageRole::User,
            logical_time: TimestampMillis::UNIX_EPOCH,
            effective_time: TimestampMillis::UNIX_EPOCH,
            visibility: MessageVisibility::Visible,
            pinned: false,
            scene_edited: false,
            active_render_source: MessageRenderSource::Revision(revision_id),
            revision: Revision::INITIAL,
            created_at: TimestampMillis::UNIX_EPOCH,
            updated_at: TimestampMillis::UNIX_EPOCH,
        };
        let revision = MessageRevision {
            id: revision_id,
            message_id: id,
            sequence: Revision::INITIAL,
            parts: vec![MessagePart::Text {
                text: "message".into(),
            }],
            authored_at: TimestampMillis::UNIX_EPOCH,
            source_turn_id: None,
            provider_replay: None,
        };
        (message, revision)
    }

    fn history_item(
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        id: MessageId,
        parent_message_id: Option<MessageId>,
        author_participant_id: ConversationParticipantId,
        depth: u32,
        order: u64,
    ) -> ConversationHistoryItem {
        let (message, revision) = history_message(
            conversation_id,
            branch_id,
            id,
            parent_message_id,
            author_participant_id,
        );
        ConversationHistoryItem {
            message,
            active_revision: Some(revision),
            active_candidate: None,
            depth,
            order,
        }
    }

    fn history_participant() -> ConversationParticipant {
        ConversationParticipant {
            id: ConversationParticipantId::new(),
            role: ParticipantRole::User,
            ordinal: 0,
            enabled: true,
            muted: false,
            source: ParticipantSource::User,
            display_name: "User".into(),
            authored_description: None,
            model_selection: SnapshotSelection::Disabled,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::UNIX_EPOCH,
            updated_at: TimestampMillis::UNIX_EPOCH,
        }
    }

    #[test]
    fn full_history_accepts_exact_root_to_child_fork_path() {
        let conversation_id = ConversationId::new();
        let root_id = ConversationBranchId::new();
        let child_id = ConversationBranchId::new();
        let participant = history_participant();
        let a = MessageId::new();
        let b = MessageId::new();
        let c = MessageId::new();
        let history = ConversationHistory {
            conversation_id,
            branch_id: child_id,
            head_message_id: Some(c),
            fork_message_id: Some(b),
            branches: vec![
                history_branch(root_id, conversation_id, None, None, Some(b)),
                history_branch(child_id, conversation_id, Some(root_id), Some(b), Some(c)),
            ],
            items: vec![
                history_item(conversation_id, root_id, a, None, participant.id, 0, 1),
                history_item(conversation_id, root_id, b, Some(a), participant.id, 1, 2),
                history_item(conversation_id, child_id, c, Some(b), participant.id, 2, 3),
            ],
            turns: Vec::new(),
        };
        assert!(history.validate(&[participant]).is_ok());
    }

    #[test]
    fn full_history_rejects_unrelated_sibling_item() {
        let conversation_id = ConversationId::new();
        let branch_id = ConversationBranchId::new();
        let participant = history_participant();
        let a = MessageId::new();
        let b = MessageId::new();
        let unrelated = MessageId::new();
        let history = ConversationHistory {
            conversation_id,
            branch_id,
            head_message_id: Some(b),
            fork_message_id: None,
            branches: vec![history_branch(
                branch_id,
                conversation_id,
                None,
                None,
                Some(b),
            )],
            items: vec![
                history_item(conversation_id, branch_id, a, None, participant.id, 0, 1),
                history_item(
                    conversation_id,
                    branch_id,
                    unrelated,
                    Some(a),
                    participant.id,
                    1,
                    2,
                ),
                history_item(conversation_id, branch_id, b, Some(a), participant.id, 1, 3),
            ],
            turns: Vec::new(),
        };
        assert!(history.validate(&[participant]).is_err());
    }

    #[test]
    fn timeline_second_page_accepts_parent_boundary() {
        let conversation_id = ConversationId::new();
        let branch_id = ConversationBranchId::new();
        let participant = history_participant();
        let a = MessageId::new();
        let b = MessageId::new();
        let c = MessageId::new();
        let (b_message, b_revision) =
            history_message(conversation_id, branch_id, b, Some(a), participant.id);
        let (c_message, c_revision) =
            history_message(conversation_id, branch_id, c, Some(b), participant.id);
        let page = TimelinePage {
            conversation_id,
            selected_branch_id: branch_id,
            branch_path: vec![history_branch(branch_id, conversation_id, None, None, None)],
            items: vec![
                TimelineItem {
                    message: b_message,
                    active_revision: Some(b_revision),
                    active_candidate: None,
                },
                TimelineItem {
                    message: c_message,
                    active_revision: Some(c_revision),
                    active_candidate: None,
                },
            ],
            boundary_parent_id: Some(a),
            next_cursor: None,
        };
        assert!(page.validate_page().is_ok());
        let descending = TimelinePage {
            items: page.items.iter().cloned().rev().collect(),
            ..page
        };
        assert!(descending.validate_page().is_ok());
    }

    #[test]
    fn timeline_rejects_corrupt_active_render_pointer() {
        let conversation_id = ConversationId::new();
        let branch_id = ConversationBranchId::new();
        let participant = history_participant();
        let (mut message, revision) = history_message(
            conversation_id,
            branch_id,
            MessageId::new(),
            None,
            participant.id,
        );
        message.active_render_source = MessageRenderSource::Revision(MessageRevisionId::new());
        let page = TimelinePage {
            conversation_id,
            selected_branch_id: branch_id,
            branch_path: vec![history_branch(branch_id, conversation_id, None, None, None)],
            items: vec![TimelineItem {
                message,
                active_revision: Some(revision),
                active_candidate: None,
            }],
            boundary_parent_id: None,
            next_cursor: None,
        };
        assert!(page.validate_page().is_err());
    }

    #[test]
    fn timeline_rejects_foreign_conversation_and_unrelated_branch() {
        let conversation_id = ConversationId::new();
        let branch_id = ConversationBranchId::new();
        let sibling_branch_id = ConversationBranchId::new();
        let participant = history_participant();
        let (message, revision) = history_message(
            conversation_id,
            sibling_branch_id,
            MessageId::new(),
            None,
            participant.id,
        );
        let base = TimelinePage {
            conversation_id,
            selected_branch_id: branch_id,
            branch_path: vec![history_branch(branch_id, conversation_id, None, None, None)],
            items: vec![TimelineItem {
                message,
                active_revision: Some(revision),
                active_candidate: None,
            }],
            boundary_parent_id: None,
            next_cursor: None,
        };
        assert!(base.validate_page().is_err());

        let mut foreign = base.clone();
        foreign.items[0].message.conversation_id = ConversationId::new();
        assert!(foreign.validate_page().is_err());

        let (ancestor_message, ancestor_revision) = history_message(
            conversation_id,
            branch_id,
            MessageId::new(),
            None,
            participant.id,
        );
        let ancestor_page = TimelinePage {
            conversation_id,
            selected_branch_id: sibling_branch_id,
            branch_path: vec![
                history_branch(branch_id, conversation_id, None, None, None),
                history_branch(
                    sibling_branch_id,
                    conversation_id,
                    Some(branch_id),
                    Some(MessageId::new()),
                    None,
                ),
            ],
            items: vec![TimelineItem {
                message: ancestor_message,
                active_revision: Some(ancestor_revision),
                active_candidate: None,
            }],
            boundary_parent_id: None,
            next_cursor: None,
        };
        assert!(ancestor_page.validate_page().is_ok());
    }

    #[test]
    fn descending_timeline_accepts_child_suffix_and_checks_fork_record() {
        let conversation_id = ConversationId::new();
        let root_id = ConversationBranchId::new();
        let child_id = ConversationBranchId::new();
        let participant = history_participant();
        let a = MessageId::new();
        let b = MessageId::new();
        let c = MessageId::new();
        let (b_message, b_revision) =
            history_message(conversation_id, root_id, b, Some(a), participant.id);
        let (c_message, c_revision) =
            history_message(conversation_id, child_id, c, Some(b), participant.id);
        let root = history_branch(root_id, conversation_id, None, None, Some(b));
        let child = history_branch(child_id, conversation_id, Some(root_id), Some(b), Some(c));
        let page = TimelinePage {
            conversation_id,
            selected_branch_id: child_id,
            branch_path: vec![root.clone(), child.clone()],
            items: vec![
                TimelineItem {
                    message: c_message,
                    active_revision: Some(c_revision),
                    active_candidate: None,
                },
                TimelineItem {
                    message: b_message,
                    active_revision: Some(b_revision),
                    active_candidate: None,
                },
            ],
            boundary_parent_id: Some(a),
            next_cursor: None,
        };
        assert!(page.validate_page().is_ok());

        let mut corrupt = page;
        corrupt.branch_path[1].fork_message_id = Some(MessageId::new());
        assert!(corrupt.validate_page().is_err());
    }

    fn group_details(
        group_id: lettuce_types::GroupId,
        first: CharacterId,
        second: CharacterId,
        first_participant: ConversationParticipantId,
        second_participant: ConversationParticipantId,
    ) -> GroupConversationDetails {
        let character = |id: CharacterId, name: &str| CharacterLaunchSnapshot {
            snapshot_ref: snapshot_ref(SnapshotSource::Character(id)),
            source_id: id,
            source_revision: Revision::INITIAL,
            name: name.into(),
            nickname: None,
        };
        let members = vec![
            GroupMemberLaunchSnapshot {
                character: character(first, "First"),
                ordinal: 0,
                enabled: true,
                muted: false,
                model_override: SnapshotSelection::Disabled,
            },
            GroupMemberLaunchSnapshot {
                character: character(second, "Second"),
                ordinal: 1,
                enabled: true,
                muted: false,
                model_override: SnapshotSelection::Disabled,
            },
        ];
        GroupConversationDetails {
            format_version: 1,
            group: GroupLaunchSnapshot {
                snapshot_ref: snapshot_ref(SnapshotSource::Group(group_id)),
                source_id: group_id,
                source_revision: Revision::INITIAL,
                name: "Group".into(),
                members,
                chat_mode: GroupChatModeSnapshot::Conversation,
                speaker_selection: GroupSpeakerSelectionSnapshot::RoundRobin,
                memory: SnapshotSelection::Disabled,
                disable_character_lorebook: false,
                persona: SnapshotSelection::Disabled,
                scene: SnapshotSelection::Disabled,
                prompt: SnapshotSelection::Disabled,
                lorebooks: SnapshotSelection::Explicit(Vec::new()),
                model: SnapshotSelection::Disabled,
            },
            participant_policy: GroupParticipantPolicyDocument {
                members: vec![
                    GroupParticipantPolicySnapshot {
                        participant_id: first_participant,
                        enabled: true,
                        muted: false,
                        model_override: SnapshotSelection::Disabled,
                    },
                    GroupParticipantPolicySnapshot {
                        participant_id: second_participant,
                        enabled: true,
                        muted: false,
                        model_override: SnapshotSelection::Disabled,
                    },
                ],
                revision: Revision::INITIAL,
                created_at: TimestampMillis::UNIX_EPOCH,
                updated_at: TimestampMillis::UNIX_EPOCH,
            },
        }
    }

    #[test]
    fn group_participants_use_user_first_ordinals() {
        let conversation_id = ConversationId::new();
        let branch_id = ConversationBranchId::new();
        let user_id = ConversationParticipantId::new();
        let first_id = ConversationParticipantId::new();
        let second_id = ConversationParticipantId::new();
        let first_character = CharacterId::new();
        let second_character = CharacterId::new();
        let group_id = lettuce_types::GroupId::new();
        let participant = |id: ConversationParticipantId,
                           ordinal: u32,
                           source: ParticipantSource,
                           name: &str| ConversationParticipant {
            id,
            role: if matches!(source, ParticipantSource::User) {
                ParticipantRole::User
            } else {
                ParticipantRole::Character
            },
            ordinal,
            enabled: true,
            muted: false,
            source,
            display_name: name.into(),
            authored_description: None,
            model_selection: SnapshotSelection::Disabled,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::UNIX_EPOCH,
            updated_at: TimestampMillis::UNIX_EPOCH,
        };
        let kind = ConversationKind::Group(group_details(
            group_id,
            first_character,
            second_character,
            first_id,
            second_id,
        ));
        let conversation = Conversation {
            id: conversation_id,
            lifecycle: ConversationLifecycle::Active,
            title: "Group chat".into(),
            kind,
            active_branch_id: branch_id,
            participants: vec![
                participant(user_id, 0, ParticipantSource::User, "User"),
                participant(
                    first_id,
                    1,
                    ParticipantSource::Character(first_character),
                    "First",
                ),
                participant(
                    second_id,
                    2,
                    ParticipantSource::Character(second_character),
                    "Second",
                ),
            ],
            current_settings: None,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::UNIX_EPOCH,
            updated_at: TimestampMillis::UNIX_EPOCH,
        };
        assert!(conversation.validate().is_ok());
    }

    #[test]
    fn group_policy_values_must_match_hydrated_participants() {
        let conversation_id = ConversationId::new();
        let branch_id = ConversationBranchId::new();
        let user_id = ConversationParticipantId::new();
        let first_id = ConversationParticipantId::new();
        let second_id = ConversationParticipantId::new();
        let first_character = CharacterId::new();
        let second_character = CharacterId::new();
        let group_id = lettuce_types::GroupId::new();
        let details = group_details(
            group_id,
            first_character,
            second_character,
            first_id,
            second_id,
        );
        let participant = |id, ordinal, source| ConversationParticipant {
            id,
            role: if matches!(source, ParticipantSource::User) {
                ParticipantRole::User
            } else {
                ParticipantRole::Character
            },
            ordinal,
            enabled: true,
            muted: false,
            source,
            display_name: "Participant".into(),
            authored_description: None,
            model_selection: SnapshotSelection::Disabled,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::UNIX_EPOCH,
            updated_at: TimestampMillis::UNIX_EPOCH,
        };
        let mut conversation = Conversation {
            id: conversation_id,
            lifecycle: ConversationLifecycle::Active,
            title: "Group chat".into(),
            kind: ConversationKind::Group(details),
            active_branch_id: branch_id,
            participants: vec![
                participant(user_id, 0, ParticipantSource::User),
                participant(first_id, 1, ParticipantSource::Character(first_character)),
                participant(second_id, 2, ParticipantSource::Character(second_character)),
            ],
            current_settings: None,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::UNIX_EPOCH,
            updated_at: TimestampMillis::UNIX_EPOCH,
        };
        assert!(conversation.validate().is_ok());
        if let ConversationKind::Group(details) = &mut conversation.kind {
            details.participant_policy.members[0].enabled = false;
        }
        assert!(conversation.validate().is_err());
    }

    fn interrupted_attempt(turn_id: GenerationTurnId) -> GenerationAttempt {
        let id = GenerationAttemptId::new();
        GenerationAttempt {
            id,
            turn_id,
            ordinal: 0,
            parent_attempt_id: None,
            status: GenerationAttemptStatus::Interrupted,
            job_idempotency_key: attempt_job_idempotency_key(turn_id, id),
            job_id: Some(lettuce_types::JobId::new()),
            started_at: Some(TimestampMillis::new(1)),
            finished_at: Some(TimestampMillis::new(2)),
            candidate_ids: Vec::new(),
            usage_event_id: Some(UsageEventId::new()),
            failure: None,
        }
    }

    fn interrupted_turn(attempt: GenerationAttempt) -> GenerationTurn {
        GenerationTurn {
            id: attempt.turn_id,
            conversation_id: ConversationId::new(),
            branch_id: ConversationBranchId::new(),
            operation: GenerationOperation::Send,
            input: GenerationInput::ExistingHead {
                head_message_id: MessageId::new(),
            },
            idempotency_key: lettuce_jobs::IdempotencyKey::new("recovery-test").expect("key"),
            correlation_id: None,
            status: GenerationTurnStatus::Interrupted,
            selected_speaker: None,
            resolved_model: None,
            prompt: None,
            lorebooks: Vec::new(),
            memory: None,
            candidate_ids: Vec::new(),
            selected_candidate_id: None,
            attempts: vec![attempt],
            failure: None,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::UNIX_EPOCH,
            updated_at: TimestampMillis::new(2),
        }
    }

    #[test]
    fn recovery_requires_a_new_created_child_attempt() {
        let previous = interrupted_attempt(GenerationTurnId::new());
        let turn = interrupted_turn(previous.clone());
        let child_id = GenerationAttemptId::new();
        let child = GenerationAttempt {
            id: child_id,
            turn_id: turn.id,
            ordinal: 1,
            parent_attempt_id: Some(previous.id),
            status: GenerationAttemptStatus::Created,
            job_idempotency_key: attempt_job_idempotency_key(turn.id, child_id),
            job_id: None,
            started_at: None,
            finished_at: None,
            candidate_ids: Vec::new(),
            usage_event_id: None,
            failure: None,
        };
        let recovery = GenerationRecovery {
            turn: turn.clone(),
            attempt: child.clone(),
        };
        assert!(recovery.validate_against(&previous, &turn).is_ok());

        let mut resumed = child.clone();
        resumed.id = previous.id;
        assert!(resumed.validate_against(&previous, &turn).is_err());
        let mut wrong_parent = child.clone();
        wrong_parent.parent_attempt_id = None;
        assert!(wrong_parent.validate_against(&previous, &turn).is_err());
        let mut resumed_state = child;
        resumed_state.status = GenerationAttemptStatus::Running;
        resumed_state.started_at = Some(TimestampMillis::new(3));
        assert!(resumed_state.validate_against(&previous, &turn).is_err());
    }

    #[test]
    fn settings_patch_has_no_revision_and_cas_requirement_is_explicit() {
        let patch = CurrentConversationSettingsPatch {
            author_note: PatchValue::Keep,
            memory: PatchValue::Keep,
            model_override: PatchValue::Keep,
            voice: PatchValue::Keep,
            author_note_provenance: PatchValue::Keep,
            memory_provenance: PatchValue::Keep,
            model_provenance: PatchValue::Keep,
            voice_provenance: PatchValue::Keep,
        };
        let command = UpdateConversationSettings {
            conversation_id: ConversationId::new(),
            expected_settings_revision: None,
            operation: OperationToken {
                key: lettuce_jobs::IdempotencyKey::new("settings-create").expect("key"),
                request_digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
            },
            patch: patch.clone(),
        };
        assert_eq!(
            command.cas_requirement(),
            SettingsCasRequirement::CreateOnly
        );
        let existing = UpdateConversationSettings {
            expected_settings_revision: Some(Revision::INITIAL),
            ..command
        };
        assert_eq!(
            existing.cas_requirement(),
            SettingsCasRequirement::Exact(Revision::INITIAL)
        );
        assert_eq!(patch.author_note, PatchValue::Keep);
    }

    #[test]
    fn persisted_settings_require_a_revision_and_validate_resolved_values() {
        let mut settings = CurrentConversationSettings {
            revision: Revision::INITIAL,
            author_note: None,
            author_note_provenance: SettingProvenance::Disabled,
            memory: None,
            memory_provenance: SettingProvenance::Disabled,
            model_override: None,
            model_provenance: SettingProvenance::Disabled,
            voice: None,
            voice_provenance: SettingProvenance::Disabled,
        };
        assert!(settings.validate().is_ok());
        settings.revision = Revision::new(0);
        assert!(settings.validate().is_err());
    }

    #[test]
    fn active_branch_must_not_be_archived_or_tombstoned() {
        let conversation_id = ConversationId::new();
        let branch_id = ConversationBranchId::new();
        let character_id = CharacterId::new();
        let now = TimestampMillis::UNIX_EPOCH;
        let conversation = Conversation {
            id: conversation_id,
            lifecycle: ConversationLifecycle::Active,
            title: "Chat".into(),
            kind: ConversationKind::Direct(DirectConversationDetails {
                format_version: 1,
                character: CharacterLaunchSnapshot {
                    snapshot_ref: snapshot_ref(SnapshotSource::Character(character_id)),
                    source_id: character_id,
                    source_revision: Revision::INITIAL,
                    name: "Ada".into(),
                    nickname: None,
                },
                persona: SnapshotSelection::Disabled,
                scene: SnapshotSelection::Disabled,
                starter: SnapshotSelection::Disabled,
                prompt: SnapshotSelection::Disabled,
                lorebooks: SnapshotSelection::Explicit(Vec::new()),
                model: SnapshotSelection::Disabled,
                memory: SnapshotSelection::Disabled,
                voice: SnapshotSelection::Disabled,
            }),
            active_branch_id: branch_id,
            participants: vec![
                ConversationParticipant {
                    id: ConversationParticipantId::new(),
                    role: ParticipantRole::User,
                    ordinal: 0,
                    enabled: true,
                    muted: false,
                    source: ParticipantSource::User,
                    display_name: "User".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Disabled,
                    revision: Revision::INITIAL,
                    created_at: now,
                    updated_at: now,
                },
                ConversationParticipant {
                    id: ConversationParticipantId::new(),
                    role: ParticipantRole::Character,
                    ordinal: 1,
                    enabled: true,
                    muted: false,
                    source: ParticipantSource::Character(character_id),
                    display_name: "Ada".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Disabled,
                    revision: Revision::INITIAL,
                    created_at: now,
                    updated_at: now,
                },
            ],
            current_settings: None,
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        };
        let mut branch = history_branch(branch_id, conversation_id, None, None, None);
        branch.status = BranchStatus::Archived;
        let aggregate = ConversationAggregate {
            conversation,
            branches: vec![branch.clone()],
        };
        assert!(aggregate.validate().is_err());
        branch.status = BranchStatus::Active;
        let aggregate = ConversationAggregate {
            conversation: aggregate.conversation,
            branches: vec![branch],
        };
        assert!(aggregate.validate().is_ok());
    }
}
