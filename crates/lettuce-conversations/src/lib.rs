//! Persistence-ready contracts for unified direct and group conversation
//! history.  This crate owns the aggregate invariants and lifecycle vocabulary
//! while repositories and application ports provide persistence and runtime
//! execution.

#![deny(unsafe_op_in_unsafe_fn)]

mod artifact;
mod commands;
mod content;
mod document;
mod effective_settings;
mod error;
mod generation;
mod initial_inference;
mod model;
mod ports;
mod service;
mod snapshot;
mod speaker_inference;
mod speaker_policy;
mod tool;
mod validation;

pub use artifact::*;
pub use commands::*;
pub use content::*;
pub use document::*;
pub use effective_settings::*;
pub use error::*;
pub use generation::*;
pub use initial_inference::*;
pub use model::*;
pub use ports::*;
pub use service::*;
pub use snapshot::*;
pub use speaker_inference::*;
pub use speaker_policy::*;
pub use tool::*;

#[cfg(test)]
mod tests {
    #[test]
    fn reported_cost_rejects_invalid_values_and_preserves_small_amounts() {
        use super::ProviderReportedCost;
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.01] {
            assert!(ProviderReportedCost::new(value).is_none());
        }
        for value in [0.0, 0.0000000001, 1.25] {
            let cost = ProviderReportedCost::new(value).expect("valid cost");
            let encoded = serde_json::to_string(&cost).expect("encode");
            let decoded: ProviderReportedCost = serde_json::from_str(&encoded).expect("decode");
            assert_eq!(decoded, cost);
        }
        assert!(serde_json::from_str::<ProviderReportedCost>("-1").is_err());
        let max = ProviderReportedCost::new(f64::MAX).expect("finite");
        assert!(max.checked_add(max).is_none());
    }

    use std::{collections::HashSet, sync::Mutex};

    use super::*;
    use lettuce_types::{
        CharacterId, ContentHash, ConversationBranchId, ConversationId, ConversationParticipantId,
        GenerationAttemptId, GenerationTurnId, GroupId, MessageCandidateId, MessageId,
        MessageRevisionId, ProviderAccountId, ReplayArtifactId, Revision, SceneId,
        SnapshotArtifactId, StarterMessageId, TimestampMillis, UsageEventId,
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
        let bytes = b"snapshot";
        ProtectedSnapshotRef {
            source,
            source_revision: Revision::INITIAL,
            artifact_id: SnapshotArtifactId::new(),
            digest: ContentHash::parse(blake3::hash(bytes).to_hex().to_string()).expect("digest"),
            schema_version: 1,
            byte_size: bytes.len() as u64,
        }
    }

    fn snapshot_draft(reference: &ProtectedSnapshotRef) -> SnapshotArtifactDraft {
        let bytes = ProtectedArtifactBytes::new(b"snapshot".to_vec()).expect("payload");
        SnapshotArtifactDraft {
            source: reference.source,
            source_revision: reference.source_revision,
            artifact_id: reference.artifact_id,
            digest: bytes.digest(),
            schema_version: reference.schema_version,
            byte_size: bytes.len() as u64,
            codec: ArtifactCodec::Json,
            retention: ArtifactRetention::Conversation,
            bytes,
        }
    }

    fn complete_snapshot_drafts(plan: &CreateConversationPlan) -> Vec<SnapshotArtifactDraft> {
        let mut ids = HashSet::new();
        conversation_launch_snapshot_references(plan)
            .into_iter()
            .filter(|reference| ids.insert(reference.artifact_id))
            .map(snapshot_draft)
            .collect()
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
    fn generation_turn_persists_and_validates_its_target_contract() {
        let conversation_id = ConversationId::new();
        let branch_id = ConversationBranchId::new();
        let user_message_id = MessageId::new();
        let turn = GenerationTurn {
            id: GenerationTurnId::new(),
            conversation_id,
            branch_id,
            operation: GenerationOperation::Send,
            input: GenerationInput::UserMessage {
                message_id: user_message_id,
            },
            target: GenerationTarget::NewAssistant {
                message_id: MessageId::new(),
                parent_message_id: Some(user_message_id),
            },
            swap_roles: false,
            retry_of_turn_id: None,
            idempotency_key: lettuce_jobs::IdempotencyKey::new("turn-contract").expect("key"),
            correlation_id: None,
            status: GenerationTurnStatus::Created,
            selected_speaker: None,
            guidance: None,
            requested_model_override: None,
            forced_speaker: None,
            resolved_model: None,
            prompt: None,
            lorebooks: Vec::new(),
            memory: None,
            candidate_ids: Vec::new(),
            selected_candidate_id: None,
            attempts: Vec::new(),
            failure: None,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::UNIX_EPOCH,
            updated_at: TimestampMillis::UNIX_EPOCH,
        };
        turn.validate(false).expect("valid send target");
        let encoded = serde_json::to_value(turn.target).expect("target serialization");
        assert_eq!(encoded["kind"], "new_assistant");
        let mut mismatched = turn.clone();
        mismatched.target = GenerationTarget::ExistingCandidate {
            message_id: MessageId::new(),
            prior_candidate_id: MessageCandidateId::new(),
        };
        assert!(mismatched.validate(false).is_err());
        mismatched = turn.clone();
        mismatched.retry_of_turn_id = Some(mismatched.id);
        assert!(mismatched.validate(false).is_err());

        let mut invalid_speaker = turn;
        invalid_speaker.selected_speaker = Some(SelectedSpeakerDecision {
            participant_id: ConversationParticipantId::new(),
            method: SpeakerDecisionMethod::Explicit,
            fallback: SpeakerFallback::None,
            reference: None,
            rationale_summary: Some("   ".into()),
            decision_model: None,
            usage_event_id: None,
        });
        assert_eq!(
            invalid_speaker.validate(true),
            Err(ValidationError::Blank {
                field: "selected_speaker.rationale_summary",
            })
        );
    }

    #[test]
    fn snapshot_provenance_distinguishes_explicit_and_resolved() {
        assert!(!SnapshotSelection::<u32>::Inherited(1).is_explicit());
        assert!(SnapshotSelection::<u32>::Inherited(1).is_resolved());
        assert!(SnapshotSelection::<u32>::Explicit(1).is_explicit());
        assert!(!SnapshotSelection::<u32>::Disabled.is_resolved());
    }

    fn initial_entry(
        origin: InitialMessageOrigin,
        role: MessageRole,
        author: Option<ConversationParticipantId>,
        text: Option<&str>,
    ) -> InitialMessageDraft {
        InitialMessageDraft {
            message_id: MessageId::new(),
            revision_id: MessageRevisionId::new(),
            origin,
            role,
            author_participant_id: author,
            parts: text
                .into_iter()
                .map(|text| MessagePart::Text { text: text.into() })
                .collect(),
        }
    }

    fn direct_plan(
        initial_timeline: InitialTimelineDraft,
    ) -> (
        CreateConversationPlan,
        ConversationParticipantId,
        ConversationParticipantId,
        ProtectedSnapshotRef,
        ProtectedSnapshotRef,
    ) {
        let user_id = ConversationParticipantId::new();
        let character_participant_id = ConversationParticipantId::new();
        let character_id = CharacterId::new();
        let scene_ref = snapshot_ref(SnapshotSource::Scene(SceneId::new()));
        let starter_ref = snapshot_ref(SnapshotSource::Starter(
            lettuce_types::ConversationStarterId::new(),
        ));
        let plan = CreateConversationPlan {
            conversation_id: ConversationId::new(),
            title: "Direct launch".into(),
            kind: ConversationKind::Direct(DirectConversationDetails {
                format_version: 1,
                character: CharacterLaunchSnapshot {
                    snapshot_ref: snapshot_ref(SnapshotSource::Character(character_id)),
                    source_id: character_id,
                    source_revision: Revision::INITIAL,
                    name: "Character".into(),
                    nickname: None,
                },
                persona: SnapshotSelection::Disabled,
                scene: SnapshotSelection::Explicit(SceneLaunchSnapshot {
                    snapshot_ref: scene_ref.clone(),
                    source_id: match scene_ref.source {
                        SnapshotSource::Scene(id) => id,
                        _ => unreachable!(),
                    },
                    source_revision: Revision::INITIAL,
                    title: "Scene".into(),
                }),
                starter: SnapshotSelection::Explicit(StarterLaunchSnapshot {
                    snapshot_ref: starter_ref.clone(),
                    source_id: match starter_ref.source {
                        SnapshotSource::Starter(id) => id,
                        _ => unreachable!(),
                    },
                    source_revision: Revision::INITIAL,
                    title: "Starter".into(),
                }),
                prompt: SnapshotSelection::Disabled,
                lorebooks: SnapshotSelection::Explicit(Vec::new()),
                model: SnapshotSelection::Disabled,
                memory: SnapshotSelection::Disabled,
                voice: SnapshotSelection::Disabled,
            }),
            participants: vec![
                ConversationParticipantDraft {
                    id: user_id,
                    role: ParticipantRole::User,
                    ordinal: 0,
                    source: ParticipantSource::User,
                    enabled: true,
                    muted: false,
                    display_name: "User".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Disabled,
                },
                ConversationParticipantDraft {
                    id: character_participant_id,
                    role: ParticipantRole::Character,
                    ordinal: 1,
                    source: ParticipantSource::Character(character_id),
                    enabled: true,
                    muted: false,
                    display_name: "Character".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Disabled,
                },
            ],
            initial_timeline,
            operation: OperationToken {
                key: lettuce_jobs::IdempotencyKey::new("create-direct-plan").expect("key"),
                request_digest: ContentHash::parse("cd".repeat(32)).expect("digest"),
            },
        };
        (
            plan,
            user_id,
            character_participant_id,
            scene_ref,
            starter_ref,
        )
    }

    fn group_plan(
        chat_mode: GroupChatModeSnapshot,
        scene: SnapshotSelection<SceneLaunchSnapshot>,
        initial_timeline: InitialTimelineDraft,
    ) -> CreateConversationPlan {
        let user_id = ConversationParticipantId::new();
        let first_id = ConversationParticipantId::new();
        let second_id = ConversationParticipantId::new();
        let first_character = CharacterId::new();
        let second_character = CharacterId::new();
        let mut details = group_details(
            GroupId::new(),
            first_character,
            second_character,
            first_id,
            second_id,
        );
        details.group.chat_mode = chat_mode;
        details.group.scene = scene;
        CreateConversationPlan {
            conversation_id: ConversationId::new(),
            title: "Group launch".into(),
            kind: ConversationKind::Group(details),
            participants: vec![
                ConversationParticipantDraft {
                    id: user_id,
                    role: ParticipantRole::User,
                    ordinal: 0,
                    source: ParticipantSource::User,
                    enabled: true,
                    muted: false,
                    display_name: "User".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Disabled,
                },
                ConversationParticipantDraft {
                    id: first_id,
                    role: ParticipantRole::Character,
                    ordinal: 1,
                    source: ParticipantSource::Character(first_character),
                    enabled: true,
                    muted: false,
                    display_name: "First".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Disabled,
                },
                ConversationParticipantDraft {
                    id: second_id,
                    role: ParticipantRole::Character,
                    ordinal: 2,
                    source: ParticipantSource::Character(second_character),
                    enabled: true,
                    muted: false,
                    display_name: "Second".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Disabled,
                },
            ],
            initial_timeline,
            operation: OperationToken {
                key: lettuce_jobs::IdempotencyKey::new("create-group-plan").expect("key"),
                request_digest: ContentHash::parse("ef".repeat(32)).expect("digest"),
            },
        }
    }

    #[test]
    fn create_plan_validates_direct_launch_timeline_and_refs() {
        let (mut empty, user, character, scene_ref, starter_ref) =
            direct_plan(InitialTimelineDraft {
                format_version: 1,
                entries: Vec::new(),
            });
        assert!(empty.validate().is_ok(), "empty direct launch");

        let scene = initial_entry(
            InitialMessageOrigin::SelectedScene {
                snapshot_ref: scene_ref.clone(),
            },
            MessageRole::Scene,
            None,
            Some("scene"),
        );
        let starter = initial_entry(
            InitialMessageOrigin::StarterMessage {
                snapshot_ref: starter_ref.clone(),
                starter_message_id: StarterMessageId::new(),
            },
            MessageRole::User,
            Some(user),
            Some("starter"),
        );
        empty.initial_timeline.entries = vec![starter.clone()];
        assert!(empty.validate().is_ok(), "starter-only direct launch");
        empty.initial_timeline.entries = vec![scene.clone(), starter];
        assert!(empty.validate().is_ok(), "scene plus starter launch");

        let mut disabled_scene = empty.clone();
        if let ConversationKind::Direct(details) = &mut disabled_scene.kind {
            details.scene = SnapshotSelection::Disabled;
        }
        assert!(disabled_scene.validate().is_err());

        let mut mismatch = empty.clone();
        if let InitialMessageOrigin::SelectedScene {
            snapshot_ref: origin_ref,
        } = &mut mismatch.initial_timeline.entries[0].origin
        {
            *origin_ref = snapshot_ref(SnapshotSource::Scene(SceneId::new()));
        }
        assert!(mismatch.validate().is_err());

        let mut duplicate_revision = mismatch.clone();
        duplicate_revision.initial_timeline.entries[1].revision_id =
            duplicate_revision.initial_timeline.entries[0].revision_id;
        assert!(duplicate_revision.validate().is_err());

        let mut wrong_starter_author = empty;
        wrong_starter_author.initial_timeline.entries[1].author_participant_id = Some(character);
        assert!(wrong_starter_author.validate().is_err());

        let mut duplicate_ids = mismatch;
        duplicate_ids.initial_timeline.entries[0].message_id =
            duplicate_ids.initial_timeline.entries[1].message_id;
        assert!(duplicate_ids.validate().is_err());
    }

    #[test]
    fn participant_models_are_disabled_or_match_the_launch_selection() {
        let model_id = lettuce_types::ModelProfileId::new();
        let model_ref = snapshot_ref(SnapshotSource::Model(model_id));
        let model = ModelSelectionSnapshot {
            snapshot_ref: model_ref.clone(),
            source_id: model_id,
            source_revision: Revision::INITIAL,
            provider_account_id: ProviderAccountId::new(),
            provider_account_revision: Revision::INITIAL,
            provider_protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
            external_model_id: "model".into(),
            display_name: "Model".into(),
            context_length: None,
            max_output_tokens: None,
        };

        let (mut direct, ..) = direct_plan(InitialTimelineDraft {
            format_version: 1,
            entries: Vec::new(),
        });
        direct.participants[0].model_selection = SnapshotSelection::Explicit(model.clone());
        assert!(matches!(
            direct.validate(),
            Err(ValidationError::InvalidReference {
                field: "participant_draft.non_character_model"
            })
        ));

        let (mut direct, ..) = direct_plan(InitialTimelineDraft {
            format_version: 1,
            entries: Vec::new(),
        });
        if let ConversationKind::Direct(details) = &mut direct.kind {
            details.model = SnapshotSelection::Explicit(model.clone());
        }
        direct.participants[1].model_selection = SnapshotSelection::Disabled;
        assert!(matches!(
            direct.validate(),
            Err(ValidationError::InvalidReference {
                field: "conversation_plan.direct.character_model"
            })
        ));

        if let ConversationKind::Direct(details) = &mut direct.kind {
            details.model = SnapshotSelection::Explicit(model.clone());
        }
        direct.participants[1].model_selection = SnapshotSelection::Explicit(model.clone());
        let direct_refs = conversation_launch_snapshot_references(&direct);
        assert_eq!(
            direct_refs
                .iter()
                .filter(|reference| reference.artifact_id == model_ref.artifact_id)
                .count(),
            2,
            "kind and participant model selections are both traversed"
        );
        assert!(direct.validate().is_ok());

        let mut group = group_plan(
            GroupChatModeSnapshot::Conversation,
            SnapshotSelection::Disabled,
            InitialTimelineDraft {
                format_version: 1,
                entries: Vec::new(),
            },
        );
        if let ConversationKind::Group(details) = &mut group.kind {
            details.group.members[0].model_override = SnapshotSelection::Explicit(model.clone());
            details.initial_participant_policy.members[0].model_override =
                SnapshotSelection::Explicit(model.clone());
        }
        group.participants[1].model_selection = SnapshotSelection::Explicit(model);
        let group_refs = conversation_launch_snapshot_references(&group);
        assert_eq!(
            group_refs
                .iter()
                .filter(|reference| reference.artifact_id == model_ref.artifact_id)
                .count(),
            3,
            "member, participant, and initial-policy model selections are traversed"
        );
        assert!(group.validate().is_ok());
    }

    #[test]
    fn create_plan_enforces_group_chat_mode_initial_timeline_policy() {
        let scene_ref = snapshot_ref(SnapshotSource::Scene(SceneId::new()));
        let scene_entry = initial_entry(
            InitialMessageOrigin::SelectedScene {
                snapshot_ref: scene_ref.clone(),
            },
            MessageRole::Scene,
            None,
            Some("group scene"),
        );
        let conversation = group_plan(
            GroupChatModeSnapshot::Conversation,
            SnapshotSelection::Disabled,
            InitialTimelineDraft {
                format_version: 1,
                entries: vec![scene_entry.clone()],
            },
        );
        assert!(conversation.validate().is_err());

        let roleplay = group_plan(
            GroupChatModeSnapshot::Roleplay,
            SnapshotSelection::Explicit(SceneLaunchSnapshot {
                snapshot_ref: scene_ref.clone(),
                source_id: match scene_ref.source {
                    SnapshotSource::Scene(id) => id,
                    _ => unreachable!(),
                },
                source_revision: Revision::INITIAL,
                title: "Scene".into(),
            }),
            InitialTimelineDraft {
                format_version: 1,
                entries: vec![scene_entry],
            },
        );
        assert!(roleplay.validate().is_ok());

        let wrong_scene = group_plan(
            GroupChatModeSnapshot::Roleplay,
            match &roleplay.kind {
                ConversationKind::Group(details) => details.group.scene.clone(),
                ConversationKind::Direct(_) => unreachable!(),
            },
            InitialTimelineDraft {
                format_version: 1,
                entries: vec![initial_entry(
                    InitialMessageOrigin::SelectedScene {
                        snapshot_ref: snapshot_ref(SnapshotSource::Scene(SceneId::new())),
                    },
                    MessageRole::Scene,
                    None,
                    Some("wrong scene"),
                )],
            },
        );
        assert!(wrong_scene.validate().is_err());
    }

    #[test]
    fn initial_timeline_validation_covers_order_identity_and_origin_shapes() {
        let scene_ref = snapshot_ref(SnapshotSource::Scene(SceneId::new()));
        let starter_ref = snapshot_ref(SnapshotSource::Starter(
            lettuce_types::ConversationStarterId::new(),
        ));
        let author = Some(ConversationParticipantId::new());
        let scene = initial_entry(
            InitialMessageOrigin::SelectedScene {
                snapshot_ref: scene_ref.clone(),
            },
            MessageRole::Scene,
            None,
            Some("scene"),
        );
        let starter = initial_entry(
            InitialMessageOrigin::StarterMessage {
                snapshot_ref: starter_ref.clone(),
                starter_message_id: StarterMessageId::new(),
            },
            MessageRole::User,
            author,
            None,
        );
        let mut valid = InitialTimelineDraft {
            format_version: 1,
            entries: vec![scene.clone(), starter.clone()],
        };
        assert!(valid.validate().is_ok());
        valid.entries.remove(0);
        assert!(valid.validate().is_ok());
        let mut duplicate = InitialTimelineDraft {
            format_version: 1,
            entries: vec![starter.clone(), starter.clone()],
        };
        duplicate.entries[1].message_id = MessageId::new();
        duplicate.entries[1].revision_id = MessageRevisionId::new();
        assert!(duplicate.validate().is_err());
        let mut bad_scene = InitialTimelineDraft {
            format_version: 1,
            entries: vec![starter.clone(), scene.clone()],
        };
        assert!(bad_scene.validate().is_err());
        bad_scene.entries = vec![initial_entry(
            InitialMessageOrigin::SelectedScene {
                snapshot_ref: scene_ref,
            },
            MessageRole::User,
            author,
            Some("x"),
        )];
        assert!(bad_scene.validate().is_err());
        bad_scene.entries = vec![initial_entry(
            InitialMessageOrigin::SelectedScene {
                snapshot_ref: starter_ref,
            },
            MessageRole::Scene,
            None,
            Some("x"),
        )];
        assert!(bad_scene.validate().is_err());
    }

    #[test]
    fn prompt_purpose_serde_is_final_and_rejects_obsolete_group() {
        for purpose in [
            PromptPurposeSnapshot::GroupConversational,
            PromptPurposeSnapshot::GroupRoleplay,
        ] {
            let encoded = serde_json::to_string(&purpose).expect("purpose");
            let decoded: PromptPurposeSnapshot = serde_json::from_str(&encoded).expect("decode");
            assert_eq!(decoded, purpose);
        }
        assert!(serde_json::from_str::<PromptPurposeSnapshot>("\"group\"").is_err());
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
    fn prepared_launch_requires_an_exact_redacted_artifact_bundle() {
        let (plan, ..) = direct_plan(InitialTimelineDraft {
            format_version: 1,
            entries: Vec::new(),
        });
        let expected_count = conversation_launch_snapshot_references(&plan).len();
        let drafts = complete_snapshot_drafts(&plan);
        let prepared = PreparedConversationLaunch::new(plan, drafts);
        let prepared = prepared.expect("complete bundle");
        let debug = format!("{prepared:?}");
        assert!(debug.contains(&format!("artifact_count: {expected_count}")));
        assert!(!debug.contains("Direct launch"));
        assert!(!debug.contains("payload"));
    }

    #[test]
    fn prepared_launch_rejects_missing_extra_duplicate_and_divergent_artifacts() {
        let (plan, ..) = direct_plan(InitialTimelineDraft {
            format_version: 1,
            entries: Vec::new(),
        });
        let refs = conversation_launch_snapshot_references(&plan);
        let mut missing = complete_snapshot_drafts(&plan);
        missing.pop();
        assert!(matches!(
            PreparedConversationLaunch::new(plan.clone(), missing),
            Err(PreparedConversationLaunchError::MissingArtifact { .. })
        ));

        let mut extra = complete_snapshot_drafts(&plan);
        extra.push(snapshot_draft(&snapshot_ref(SnapshotSource::Character(
            CharacterId::new(),
        ))));
        assert!(matches!(
            PreparedConversationLaunch::new(plan.clone(), extra),
            Err(PreparedConversationLaunchError::UnexpectedArtifact { .. })
        ));

        let first = refs[0];
        let mut duplicate = complete_snapshot_drafts(&plan);
        duplicate.push(snapshot_draft(first));
        assert!(matches!(
            PreparedConversationLaunch::new(plan.clone(), duplicate),
            Err(PreparedConversationLaunchError::DuplicateArtifact { .. })
        ));

        let mut divergent_reference = first.clone();
        divergent_reference.source = SnapshotSource::Character(CharacterId::new());
        assert!(matches!(
            PreparedConversationLaunch::new(
                plan.clone(),
                vec![snapshot_draft(first), snapshot_draft(&divergent_reference)]
            ),
            Err(PreparedConversationLaunchError::DivergentReference { .. })
        ));

        let mut divergent_plan = plan;
        if let ConversationKind::Direct(details) = &mut divergent_plan.kind {
            let character_artifact_id = details.character.snapshot_ref.artifact_id;
            if let SnapshotSelection::Explicit(scene) = &mut details.scene {
                scene.snapshot_ref.artifact_id = character_artifact_id;
            }
        }
        assert!(matches!(
            PreparedConversationLaunch::new(
                divergent_plan.clone(),
                complete_snapshot_drafts(&divergent_plan)
            ),
            Err(PreparedConversationLaunchError::DivergentReference { .. })
        ));
    }

    #[test]
    fn prepared_settings_bundle_rejects_unused_duplicate_and_mismatched_artifacts() {
        let prompt_id = lettuce_types::PromptDocumentId::new();
        let prompt_ref = snapshot_ref(SnapshotSource::Prompt(prompt_id));
        let prompt = PromptLaunchSnapshot {
            snapshot_ref: prompt_ref.clone(),
            source_id: prompt_id,
            source_revision: Revision::INITIAL,
            title: "Prompt".into(),
            purpose: PromptPurposeSnapshot::Direct,
        };
        let command = |patch: CurrentConversationSettingsPatch| UpdateConversationSettings {
            conversation_id: ConversationId::new(),
            expected_settings_revision: None,
            operation: OperationToken {
                key: lettuce_jobs::IdempotencyKey::new("prepared-settings").expect("key"),
                request_digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
            },
            patch,
        };
        let patch = CurrentConversationSettingsPatch {
            author_note: PatchValue::Keep,
            memory: PatchValue::Keep,
            model_override: PatchValue::Keep,
            voice: PatchValue::Keep,
            prompt: PatchValue::Set(prompt),
            lorebooks: PatchValue::Keep,
            persona: PatchValue::Keep,
            scene: PatchValue::Keep,
        };
        assert!(
            PreparedConversationSettingsUpdate::new(command(patch.clone()), Vec::new()).is_ok()
        );
        let draft = snapshot_draft(&prompt_ref);
        assert!(
            PreparedConversationSettingsUpdate::new(
                command(patch.clone()),
                vec![snapshot_draft(&prompt_ref),]
            )
            .is_ok()
        );
        assert!(matches!(
            PreparedConversationSettingsUpdate::new(
                command(patch.clone()),
                vec![snapshot_draft(&snapshot_ref(SnapshotSource::Character(
                    CharacterId::new()
                )))],
            ),
            Err(PreparedConversationSettingsUpdateError::UnexpectedArtifact { .. })
        ));
        assert!(matches!(
            PreparedConversationSettingsUpdate::new(
                command(patch.clone()),
                vec![snapshot_draft(&prompt_ref), snapshot_draft(&prompt_ref)],
            ),
            Err(PreparedConversationSettingsUpdateError::DuplicateArtifact { .. })
        ));
        let mut mismatched = draft.reference();
        mismatched.source = SnapshotSource::Prompt(lettuce_types::PromptDocumentId::new());
        assert!(matches!(
            PreparedConversationSettingsUpdate::new(
                command(patch),
                vec![snapshot_draft(&mismatched)]
            ),
            Err(PreparedConversationSettingsUpdateError::ReferenceMismatch { .. })
        ));
    }

    #[test]
    fn prepared_launch_traverses_initial_origins_and_deduplicates_shared_refs() {
        let scene_ref = snapshot_ref(SnapshotSource::Scene(SceneId::new()));
        let timeline = InitialTimelineDraft {
            format_version: 1,
            entries: vec![initial_entry(
                InitialMessageOrigin::SelectedScene {
                    snapshot_ref: scene_ref.clone(),
                },
                MessageRole::Scene,
                None,
                Some("opening"),
            )],
        };
        assert_eq!(
            initial_timeline_snapshot_references(&timeline),
            vec![&scene_ref]
        );

        let (mut plan, _, _, selected_scene, _) = direct_plan(InitialTimelineDraft {
            format_version: 1,
            entries: Vec::new(),
        });
        plan.initial_timeline.entries = vec![initial_entry(
            InitialMessageOrigin::SelectedScene {
                snapshot_ref: selected_scene,
            },
            MessageRole::Scene,
            None,
            Some("opening"),
        )];
        let references = conversation_launch_snapshot_references(&plan);
        let distinct = references
            .iter()
            .map(|reference| reference.artifact_id)
            .collect::<HashSet<_>>();
        assert_eq!(references.len(), distinct.len() + 1);
        let prepared =
            PreparedConversationLaunch::new(plan.clone(), complete_snapshot_drafts(&plan));
        assert!(prepared.is_ok(), "shared scene reference is one artifact");
    }

    #[test]
    fn persona_and_group_member_lorebooks_are_frozen_and_referenced() {
        let persona_id = lettuce_types::PersonaId::new();
        let persona_ref = snapshot_ref(SnapshotSource::Persona(persona_id));
        let lorebook_id = lettuce_types::LorebookId::new();
        let lorebook_ref = snapshot_ref(SnapshotSource::Lorebook(lorebook_id));
        let lorebook = LorebookLaunchSnapshot {
            snapshot_ref: lorebook_ref.clone(),
            source_id: lorebook_id,
            source_revision: Revision::INITIAL,
            name: "World facts".into(),
        };
        let (mut direct, ..) = direct_plan(InitialTimelineDraft {
            format_version: 1,
            entries: Vec::new(),
        });
        if let ConversationKind::Direct(details) = &mut direct.kind {
            details.persona = SnapshotSelection::Explicit(PersonaLaunchSnapshot {
                snapshot_ref: persona_ref.clone(),
                source_id: persona_id,
                source_revision: Revision::INITIAL,
                title: "Traveler".into(),
                nickname: None,
                lorebooks: SnapshotSelection::Explicit(vec![lorebook.clone()]),
            });
        }
        direct.validate().expect("persona lorebook is valid");
        let refs = conversation_snapshot_references(&direct.kind);
        assert!(refs.iter().any(|reference| **reference == persona_ref));
        assert!(refs.iter().any(|reference| **reference == lorebook_ref));

        let mut group = group_plan(
            GroupChatModeSnapshot::Conversation,
            SnapshotSelection::Disabled,
            InitialTimelineDraft {
                format_version: 1,
                entries: Vec::new(),
            },
        );
        let group_persona_id = lettuce_types::PersonaId::new();
        let group_persona_ref = snapshot_ref(SnapshotSource::Persona(group_persona_id));
        let group_lorebook_id = lettuce_types::LorebookId::new();
        let group_lorebook_ref = snapshot_ref(SnapshotSource::Lorebook(group_lorebook_id));
        let group_lorebook = LorebookLaunchSnapshot {
            snapshot_ref: group_lorebook_ref.clone(),
            source_id: group_lorebook_id,
            source_revision: Revision::INITIAL,
            name: "Group facts".into(),
        };
        if let ConversationKind::Group(details) = &mut group.kind {
            details.group.members[0].lorebooks =
                SnapshotSelection::Explicit(vec![lorebook.clone()]);
            details.group.persona = SnapshotSelection::Explicit(PersonaLaunchSnapshot {
                snapshot_ref: group_persona_ref.clone(),
                source_id: group_persona_id,
                source_revision: Revision::INITIAL,
                title: "Group traveler".into(),
                nickname: None,
                lorebooks: SnapshotSelection::Explicit(vec![group_lorebook]),
            });
        }
        group.validate().expect("member lorebook is valid");
        let group_refs = conversation_snapshot_references(&group.kind);
        assert!(
            group_refs
                .iter()
                .any(|reference| **reference == lorebook_ref)
        );
        assert!(
            group_refs
                .iter()
                .any(|reference| **reference == group_persona_ref)
        );
        assert!(
            group_refs
                .iter()
                .any(|reference| **reference == group_lorebook_ref)
        );
        let mut missing_group_persona_artifact = complete_snapshot_drafts(&group);
        missing_group_persona_artifact
            .retain(|draft| draft.artifact_id != group_lorebook_ref.artifact_id);
        assert!(matches!(
            PreparedConversationLaunch::new(group.clone(), missing_group_persona_artifact),
            Err(PreparedConversationLaunchError::MissingArtifact { artifact_id })
                if artifact_id == group_lorebook_ref.artifact_id
        ));

        if let ConversationKind::Group(details) = &mut group.kind {
            if let SnapshotSelection::Explicit(books) = &mut details.group.members[0].lorebooks {
                books[0].snapshot_ref.source =
                    SnapshotSource::Lorebook(lettuce_types::LorebookId::new());
            }
        }
        assert!(
            group.validate().is_err(),
            "nested lorebook refs are validated"
        );
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
                provider_reported_cost: None,
                cache_write_tokens: None,
                web_search_requests: None,
                cached_input_tokens: None,
                reasoning_tokens: None,
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
    fn interrupted_and_recovering_outbox_events_validate() {
        let conversation_id = ConversationId::new();
        let turn_id = GenerationTurnId::new();
        let previous_attempt_id = GenerationAttemptId::new();
        for event in [
            ConversationOutboxEvent::TurnInterrupted {
                conversation_id,
                branch_id: ConversationBranchId::new(),
                turn_id,
                attempt_id: previous_attempt_id,
                usage_event_id: UsageEventId::new(),
                used_memory_revision_ids: Vec::new(),
                at: TimestampMillis::UNIX_EPOCH,
            },
            ConversationOutboxEvent::TurnRecovering {
                conversation_id,
                branch_id: ConversationBranchId::new(),
                turn_id,
                previous_attempt_id,
                attempt_id: GenerationAttemptId::new(),
                at: TimestampMillis::UNIX_EPOCH,
            },
        ] {
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
    fn scene_messages_are_system_like_and_cannot_have_an_author() {
        let valid = MessageDraft {
            role: MessageRole::Scene,
            author_participant_id: None,
            parts: vec![],
            visibility: MessageVisibility::Visible,
            pinned: false,
            scene_edited: false,
        };
        assert!(valid.validate().is_ok());
        assert!(
            MessageDraft {
                author_participant_id: Some(ConversationParticipantId::new()),
                ..valid
            }
            .validate()
            .is_err()
        );
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
    fn conversation_created_outbox_requires_a_bounded_head_count_pair() {
        let conversation_id = ConversationId::new();
        let record = ConversationOutboxRecord {
            format_version: 1,
            id: lettuce_types::OutboxEventId::new(),
            conversation_id,
            conversation_revision: Revision::INITIAL,
            sequence: 1,
            operation_record_id: lettuce_types::OperationRecordId::new(),
            at: TimestampMillis::UNIX_EPOCH,
            event: ConversationOutboxEvent::ConversationCreated {
                conversation_id,
                root_branch_id: ConversationBranchId::new(),
                head_message_id: None,
                initial_message_count: 1,
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
                    initial_origin: None,
                },
                TimelineItem {
                    message: c_message,
                    active_revision: Some(c_revision),
                    active_candidate: None,
                    initial_origin: None,
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
                initial_origin: None,
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
                initial_origin: None,
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
                initial_origin: None,
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
                    initial_origin: None,
                },
                TimelineItem {
                    message: b_message,
                    active_revision: Some(b_revision),
                    active_candidate: None,
                    initial_origin: None,
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
                lorebooks: SnapshotSelection::Explicit(Vec::new()),
                prompt: SnapshotSelection::Disabled,
            },
            GroupMemberLaunchSnapshot {
                character: character(second, "Second"),
                ordinal: 1,
                enabled: true,
                muted: false,
                model_override: SnapshotSelection::Disabled,
                lorebooks: SnapshotSelection::Explicit(Vec::new()),
                prompt: SnapshotSelection::Disabled,
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
                speaker_selection_model: None,
                memory: SnapshotSelection::Disabled,
                disable_character_lorebook: false,
                persona: SnapshotSelection::Disabled,
                scene: SnapshotSelection::Disabled,
                prompt: SnapshotSelection::Disabled,
                lorebooks: SnapshotSelection::Explicit(Vec::new()),
                model: SnapshotSelection::Disabled,
            },
            initial_participant_policy: GroupParticipantPolicyDocument {
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
    fn group_current_participant_policy_can_change_after_launch() {
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
        conversation.participants[1].enabled = false;
        assert!(conversation.validate().is_ok());
    }

    #[test]
    fn regenerate_target_requires_current_conversation_and_candidate() {
        let conversation_id = ConversationId::new();
        let branch_id = ConversationBranchId::new();
        let message_id = MessageId::new();
        let candidate_id = MessageCandidateId::new();
        let participant_id = ConversationParticipantId::new();
        let command = RegenerateCandidate {
            conversation_id,
            branch_id,
            message_id,
            turn_id: GenerationTurnId::new(),
            expected_revision: Revision::INITIAL,
            expected_turn_revision: Revision::INITIAL,
            operation: OperationToken {
                key: lettuce_jobs::IdempotencyKey::new("regenerate-context").expect("key"),
                request_digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
            },
            active_candidate_id: candidate_id,
            guidance: None,
            model_override: None,
            forced_speaker: None,
            swap_roles: false,
        };
        let message = |conversation_id, source| Message {
            id: message_id,
            conversation_id,
            branch_id,
            parent_message_id: None,
            author_participant_id: Some(participant_id),
            role: MessageRole::Assistant,
            logical_time: TimestampMillis::UNIX_EPOCH,
            effective_time: TimestampMillis::UNIX_EPOCH,
            visibility: MessageVisibility::Visible,
            pinned: false,
            scene_edited: false,
            active_render_source: source,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::UNIX_EPOCH,
            updated_at: TimestampMillis::UNIX_EPOCH,
        };
        let participants = vec![ConversationParticipant {
            id: participant_id,
            role: ParticipantRole::Character,
            ordinal: 0,
            enabled: true,
            muted: false,
            source: ParticipantSource::Character(CharacterId::new()),
            display_name: "Character".into(),
            authored_description: None,
            model_selection: SnapshotSelection::Disabled,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::UNIX_EPOCH,
            updated_at: TimestampMillis::UNIX_EPOCH,
        }];
        assert!(
            command
                .validate_target_context(
                    &message(
                        ConversationId::new(),
                        MessageRenderSource::Candidate(candidate_id)
                    ),
                    branch_id,
                    Some(message_id),
                    &participants,
                    false,
                )
                .is_err()
        );
        assert!(
            command
                .validate_target_context(
                    &message(
                        conversation_id,
                        MessageRenderSource::Revision(MessageRevisionId::new()),
                    ),
                    branch_id,
                    Some(message_id),
                    &participants,
                    false,
                )
                .is_err()
        );
        assert!(
            command
                .validate_target_context(
                    &message(
                        conversation_id,
                        MessageRenderSource::Candidate(candidate_id)
                    ),
                    branch_id,
                    Some(message_id),
                    &participants,
                    false,
                )
                .is_ok()
        );
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
        let head_message_id = MessageId::new();
        GenerationTurn {
            id: attempt.turn_id,
            conversation_id: ConversationId::new(),
            branch_id: ConversationBranchId::new(),
            operation: GenerationOperation::Continue,
            input: GenerationInput::ExistingHead { head_message_id },
            target: GenerationTarget::NewAssistant {
                message_id: MessageId::new(),
                parent_message_id: Some(head_message_id),
            },
            swap_roles: false,
            retry_of_turn_id: None,
            idempotency_key: lettuce_jobs::IdempotencyKey::new("recovery-test").expect("key"),
            correlation_id: None,
            status: GenerationTurnStatus::Interrupted,
            selected_speaker: None,
            guidance: None,
            requested_model_override: None,
            forced_speaker: None,
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
            prompt: PatchValue::Keep,
            lorebooks: PatchValue::Keep,
            persona: PatchValue::Keep,
            scene: PatchValue::Keep,
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

        let created = patch
            .apply(None, None)
            .expect("materialize absent settings");
        assert_eq!(created.revision, Revision::INITIAL);
        assert_eq!(
            created.author_note_provenance,
            SettingProvenance::LaunchInherited
        );
        let existing = CurrentConversationSettings {
            revision: Revision::INITIAL,
            author_note: None,
            author_note_provenance: SettingProvenance::Disabled,
            memory: None,
            memory_provenance: SettingProvenance::Disabled,
            model_override: None,
            model_provenance: SettingProvenance::Disabled,
            voice: None,
            voice_provenance: SettingProvenance::Disabled,
            prompt: None,
            prompt_provenance: SettingProvenance::Disabled,
            lorebooks: None,
            lorebooks_provenance: SettingProvenance::Disabled,
            persona: None,
            persona_provenance: SettingProvenance::Disabled,
            scene: None,
            scene_provenance: SettingProvenance::Disabled,
        };
        let preserved = patch
            .apply(Some(&existing), Some(Revision::INITIAL))
            .expect("preserve existing settings");
        assert_eq!(preserved.revision, Revision::new(2));
        assert_eq!(
            preserved.author_note_provenance,
            SettingProvenance::Disabled
        );
        let mut blank = patch.clone();
        blank.author_note = PatchValue::Set(String::new());
        assert!(blank.apply(None, None).is_err());
    }

    #[test]
    fn settings_patch_distinguishes_clear_from_launch_default() {
        let model = contract_model();
        let memory = MemorySettingsSnapshot {
            policy_ref: None,
            mode: MemoryModeSnapshot::Manual,
            selected_revision_ids: Vec::new(),
            dynamic_policy: None,
        };
        let voice_id = lettuce_types::VoiceProfileId::new();
        let voice = VoiceSettingsSnapshot {
            snapshot_ref: snapshot_ref(SnapshotSource::Voice(voice_id)),
            source_id: voice_id,
            source_revision: Revision::INITIAL,
            display_name: "Voice".into(),
            autoplay: false,
        };
        let set = CurrentConversationSettingsPatch {
            author_note: PatchValue::Set("note".into()),
            memory: PatchValue::Set(memory.clone()),
            model_override: PatchValue::Set(model.clone()),
            voice: PatchValue::Set(voice.clone()),
            prompt: PatchValue::Keep,
            lorebooks: PatchValue::Keep,
            persona: PatchValue::Keep,
            scene: PatchValue::Keep,
        };
        let created = set.apply(None, None).expect("create settings");
        assert_eq!(created.revision, Revision::INITIAL);
        assert_eq!(
            created.author_note_provenance,
            SettingProvenance::CurrentOverride
        );
        assert_eq!(
            created.memory_provenance,
            SettingProvenance::CurrentOverride
        );
        assert_eq!(created.model_provenance, SettingProvenance::CurrentOverride);
        assert_eq!(created.voice_provenance, SettingProvenance::CurrentOverride);

        let use_launch_default = CurrentConversationSettingsPatch {
            author_note: PatchValue::UseLaunchDefault,
            memory: PatchValue::UseLaunchDefault,
            model_override: PatchValue::UseLaunchDefault,
            voice: PatchValue::UseLaunchDefault,
            prompt: PatchValue::UseLaunchDefault,
            lorebooks: PatchValue::UseLaunchDefault,
            persona: PatchValue::UseLaunchDefault,
            scene: PatchValue::UseLaunchDefault,
        };
        let inherited = use_launch_default
            .apply(Some(&created), Some(Revision::INITIAL))
            .expect("use launch defaults");
        assert_eq!(inherited.revision, Revision::new(2));
        assert_eq!(inherited.author_note, None);
        assert_eq!(
            inherited.author_note_provenance,
            SettingProvenance::LaunchInherited
        );
        assert_eq!(inherited.memory, None);
        assert_eq!(
            inherited.memory_provenance,
            SettingProvenance::LaunchInherited
        );
        assert_eq!(inherited.model_override, None);
        assert_eq!(
            inherited.model_provenance,
            SettingProvenance::LaunchInherited
        );
        assert_eq!(inherited.voice, None);
        assert_eq!(
            inherited.voice_provenance,
            SettingProvenance::LaunchInherited
        );

        let clear = CurrentConversationSettingsPatch {
            author_note: PatchValue::Clear,
            memory: PatchValue::Clear,
            model_override: PatchValue::Clear,
            voice: PatchValue::Clear,
            prompt: PatchValue::Clear,
            lorebooks: PatchValue::Clear,
            persona: PatchValue::Clear,
            scene: PatchValue::Clear,
        };
        let disabled = clear
            .apply(Some(&created), Some(Revision::INITIAL))
            .expect("clear settings");
        assert_eq!(disabled.revision, Revision::new(2));
        assert_eq!(disabled.author_note, None);
        assert_eq!(disabled.author_note_provenance, SettingProvenance::Disabled);
        assert_eq!(disabled.memory, None);
        assert_eq!(disabled.memory_provenance, SettingProvenance::Disabled);
        assert_eq!(disabled.model_override, None);
        assert_eq!(disabled.model_provenance, SettingProvenance::Disabled);
        assert_eq!(disabled.voice, None);
        assert_eq!(disabled.voice_provenance, SettingProvenance::Disabled);

        let first_inherited = use_launch_default
            .apply(None, None)
            .expect("materialize launch defaults");
        assert_eq!(first_inherited.revision, Revision::INITIAL);
        assert_eq!(
            first_inherited.author_note_provenance,
            SettingProvenance::LaunchInherited
        );
        assert_eq!(
            first_inherited.memory_provenance,
            SettingProvenance::LaunchInherited
        );
        assert_eq!(
            first_inherited.model_provenance,
            SettingProvenance::LaunchInherited
        );
        assert_eq!(
            first_inherited.voice_provenance,
            SettingProvenance::LaunchInherited
        );

        let first_disabled = clear
            .apply(None, None)
            .expect("materialize disabled settings");
        assert_eq!(first_disabled.revision, Revision::INITIAL);
        assert_eq!(
            first_disabled.author_note_provenance,
            SettingProvenance::Disabled
        );
        assert_eq!(
            first_disabled.memory_provenance,
            SettingProvenance::Disabled
        );
        assert_eq!(first_disabled.model_provenance, SettingProvenance::Disabled);
        assert_eq!(first_disabled.voice_provenance, SettingProvenance::Disabled);
    }

    #[test]
    fn context_settings_apply_and_validate_against_conversation_kind() {
        let prompt_id = lettuce_types::PromptDocumentId::new();
        let prompt = PromptLaunchSnapshot {
            snapshot_ref: snapshot_ref(SnapshotSource::Prompt(prompt_id)),
            source_id: prompt_id,
            source_revision: Revision::INITIAL,
            title: "Prompt".into(),
            purpose: PromptPurposeSnapshot::Direct,
        };
        let lorebook_id = lettuce_types::LorebookId::new();
        let lorebook = LorebookLaunchSnapshot {
            snapshot_ref: snapshot_ref(SnapshotSource::Lorebook(lorebook_id)),
            source_id: lorebook_id,
            source_revision: Revision::INITIAL,
            name: "Lore".into(),
        };
        let persona_id = lettuce_types::PersonaId::new();
        let persona = PersonaLaunchSnapshot {
            snapshot_ref: snapshot_ref(SnapshotSource::Persona(persona_id)),
            source_id: persona_id,
            source_revision: Revision::INITIAL,
            title: "Persona".into(),
            nickname: None,
            lorebooks: SnapshotSelection::Explicit(vec![lorebook.clone()]),
        };
        let scene_id = SceneId::new();
        let scene = SceneLaunchSnapshot {
            snapshot_ref: snapshot_ref(SnapshotSource::Scene(scene_id)),
            source_id: scene_id,
            source_revision: Revision::INITIAL,
            title: "Scene".into(),
        };
        let patch = CurrentConversationSettingsPatch {
            author_note: PatchValue::Keep,
            memory: PatchValue::Keep,
            model_override: PatchValue::Keep,
            voice: PatchValue::Keep,
            prompt: PatchValue::Set(prompt.clone()),
            lorebooks: PatchValue::Set(vec![lorebook.clone()]),
            persona: PatchValue::Set(persona),
            scene: PatchValue::Set(scene),
        };
        let current = patch.apply(None, None).expect("context settings");
        assert_eq!(current.prompt, Some(prompt.clone()));
        assert_eq!(
            current.prompt_provenance,
            SettingProvenance::CurrentOverride
        );
        assert_eq!(current.lorebooks, Some(vec![lorebook]));
        assert_eq!(
            current.lorebooks_provenance,
            SettingProvenance::CurrentOverride
        );

        let mut direct = direct_plan(InitialTimelineDraft {
            format_version: 1,
            entries: Vec::new(),
        })
        .0;
        assert!(current.validate_against_kind(&direct.kind).is_ok());
        direct.kind = match direct.kind {
            ConversationKind::Direct(mut details) => {
                details.prompt = SnapshotSelection::Explicit(prompt.clone());
                ConversationKind::Direct(details)
            }
            ConversationKind::Group(_) => unreachable!(),
        };
        let mut wrong_prompt = current.clone();
        wrong_prompt.prompt.as_mut().expect("prompt").purpose = PromptPurposeSnapshot::Other;
        assert!(wrong_prompt.validate_against_kind(&direct.kind).is_err());

        let group = group_plan(
            GroupChatModeSnapshot::Conversation,
            SnapshotSelection::Disabled,
            InitialTimelineDraft {
                format_version: 1,
                entries: Vec::new(),
            },
        );
        assert!(current.validate_against_kind(&group.kind).is_err());
        let mut group_roleplay = group.clone();
        if let ConversationKind::Group(details) = &mut group_roleplay.kind {
            details.group.chat_mode = GroupChatModeSnapshot::Roleplay;
        }
        let mut roleplay_settings = current.clone();
        roleplay_settings.prompt.as_mut().expect("prompt").purpose =
            PromptPurposeSnapshot::GroupRoleplay;
        assert!(
            roleplay_settings
                .validate_against_kind(&group_roleplay.kind)
                .is_ok()
        );

        let mut invalid = patch.clone();
        invalid.lorebooks = PatchValue::Set(Vec::new());
        assert!(invalid.apply(None, None).is_err());
        let mut launch_default = patch.clone();
        launch_default.prompt = PatchValue::UseLaunchDefault;
        launch_default.lorebooks = PatchValue::UseLaunchDefault;
        launch_default.persona = PatchValue::UseLaunchDefault;
        launch_default.scene = PatchValue::UseLaunchDefault;
        let inherited = launch_default
            .apply(Some(&current), Some(Revision::INITIAL))
            .expect("reset context settings");
        assert_eq!(inherited.revision, Revision::new(2));
        assert_eq!(
            inherited.prompt_provenance,
            SettingProvenance::LaunchInherited
        );
        assert_eq!(
            inherited.lorebooks_provenance,
            SettingProvenance::LaunchInherited
        );
        assert_eq!(
            inherited.persona_provenance,
            SettingProvenance::LaunchInherited
        );
        assert_eq!(
            inherited.scene_provenance,
            SettingProvenance::LaunchInherited
        );
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
            prompt: None,
            prompt_provenance: SettingProvenance::Disabled,
            lorebooks: None,
            lorebooks_provenance: SettingProvenance::Disabled,
            persona: None,
            persona_provenance: SettingProvenance::Disabled,
            scene: None,
            scene_provenance: SettingProvenance::Disabled,
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

    fn contract_participant(
        ordinal: u32,
        role: ParticipantRole,
        source: ParticipantSource,
    ) -> ConversationParticipant {
        ConversationParticipant {
            id: ConversationParticipantId::new(),
            role,
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
        }
    }

    fn contract_model() -> ModelSelectionSnapshot {
        let model_id = lettuce_types::ModelProfileId::new();
        ModelSelectionSnapshot {
            snapshot_ref: snapshot_ref(SnapshotSource::Model(model_id)),
            source_id: model_id,
            source_revision: Revision::INITIAL,
            provider_account_id: ProviderAccountId::new(),
            provider_account_revision: Revision::INITIAL,
            provider_protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
            external_model_id: "model".into(),
            display_name: "Model".into(),
            context_length: None,
            max_output_tokens: None,
        }
    }

    fn contract_conversation(
        conversation_id: ConversationId,
        active_branch_id: ConversationBranchId,
    ) -> Conversation {
        let character_id = CharacterId::new();
        let now = TimestampMillis::UNIX_EPOCH;
        Conversation {
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
            active_branch_id,
            participants: vec![
                contract_participant(0, ParticipantRole::User, ParticipantSource::User),
                contract_participant(
                    1,
                    ParticipantRole::Character,
                    ParticipantSource::Character(character_id),
                ),
            ],
            current_settings: None,
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        }
    }

    fn candidate_history(
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        participants: &[ConversationParticipant],
        message_author: ConversationParticipantId,
        candidate_author: ConversationParticipantId,
    ) -> ConversationHistory {
        let now = TimestampMillis::UNIX_EPOCH;
        let user_author = participants
            .iter()
            .find(|participant| participant.role == ParticipantRole::User)
            .expect("user participant")
            .id;
        let user_message_id = MessageId::new();
        let assistant_id = MessageId::new();
        let turn_id = GenerationTurnId::new();
        let attempt_id = GenerationAttemptId::new();
        let candidate_id = MessageCandidateId::new();
        let candidate = MessageCandidate {
            id: candidate_id,
            message_id: assistant_id,
            turn_id,
            attempt_id,
            author_participant_id: candidate_author,
            ordinal: 0,
            parts: vec![MessagePart::Text {
                text: "reply".into(),
            }],
            model: contract_model(),
            created_at: now,
            provider_replay: None,
        };
        let assistant = Message {
            id: assistant_id,
            conversation_id,
            branch_id,
            parent_message_id: Some(user_message_id),
            author_participant_id: Some(message_author),
            role: MessageRole::Assistant,
            logical_time: now,
            effective_time: now,
            visibility: MessageVisibility::Visible,
            pinned: false,
            scene_edited: false,
            active_render_source: MessageRenderSource::Candidate(candidate_id),
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        };
        let attempt = GenerationAttempt {
            id: attempt_id,
            turn_id,
            ordinal: 0,
            parent_attempt_id: None,
            status: GenerationAttemptStatus::Succeeded,
            job_idempotency_key: attempt_job_idempotency_key(turn_id, attempt_id),
            job_id: None,
            started_at: Some(now),
            finished_at: Some(now),
            candidate_ids: vec![candidate_id],
            usage_event_id: Some(UsageEventId::new()),
            failure: None,
        };
        let turn = GenerationTurn {
            id: turn_id,
            conversation_id,
            branch_id,
            operation: GenerationOperation::Send,
            input: GenerationInput::UserMessage {
                message_id: user_message_id,
            },
            target: GenerationTarget::NewAssistant {
                message_id: assistant_id,
                parent_message_id: Some(user_message_id),
            },
            swap_roles: false,
            retry_of_turn_id: None,
            idempotency_key: lettuce_jobs::IdempotencyKey::new("candidate-turn").expect("key"),
            correlation_id: None,
            status: GenerationTurnStatus::Succeeded,
            selected_speaker: None,
            guidance: None,
            requested_model_override: None,
            forced_speaker: None,
            resolved_model: None,
            prompt: None,
            lorebooks: Vec::new(),
            memory: None,
            candidate_ids: vec![candidate_id],
            selected_candidate_id: Some(candidate_id),
            attempts: vec![attempt],
            failure: None,
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        };
        ConversationHistory {
            conversation_id,
            branch_id,
            head_message_id: Some(assistant_id),
            fork_message_id: None,
            branches: vec![history_branch(
                branch_id,
                conversation_id,
                None,
                None,
                Some(assistant_id),
            )],
            items: vec![
                history_item(
                    conversation_id,
                    branch_id,
                    user_message_id,
                    None,
                    user_author,
                    0,
                    1,
                ),
                ConversationHistoryItem {
                    message: assistant,
                    active_revision: None,
                    active_candidate: Some(candidate),
                    depth: 1,
                    order: 2,
                },
            ],
            turns: vec![turn],
        }
    }

    #[test]
    fn candidate_author_must_be_a_character_participant() {
        let conversation_id = ConversationId::new();
        let branch_id = ConversationBranchId::new();
        let character_id = CharacterId::new();
        let participants = vec![
            contract_participant(0, ParticipantRole::User, ParticipantSource::User),
            contract_participant(
                1,
                ParticipantRole::Character,
                ParticipantSource::Character(character_id),
            ),
        ];
        let history = candidate_history(
            conversation_id,
            branch_id,
            &participants,
            participants[1].id,
            participants[0].id,
        );
        assert!(matches!(
            history.validate(&participants),
            Err(ValidationError::InvalidReference {
                field: "conversation_history.candidate_author"
            })
        ));
        let unknown = candidate_history(
            conversation_id,
            branch_id,
            &participants,
            participants[1].id,
            ConversationParticipantId::new(),
        );
        assert!(unknown.validate(&participants).is_err());
    }

    #[test]
    fn active_candidate_author_pins_the_message_speaker() {
        let conversation_id = ConversationId::new();
        let branch_id = ConversationBranchId::new();
        let participants = vec![
            contract_participant(0, ParticipantRole::User, ParticipantSource::User),
            contract_participant(
                1,
                ParticipantRole::Character,
                ParticipantSource::Character(CharacterId::new()),
            ),
            contract_participant(
                2,
                ParticipantRole::Character,
                ParticipantSource::Character(CharacterId::new()),
            ),
        ];
        let matching = candidate_history(
            conversation_id,
            branch_id,
            &participants,
            participants[1].id,
            participants[1].id,
        );
        assert!(matching.validate(&participants).is_ok());
        let drifted = candidate_history(
            conversation_id,
            branch_id,
            &participants,
            participants[1].id,
            participants[2].id,
        );
        assert!(matches!(
            drifted.validate(&participants),
            Err(ValidationError::InvalidReference {
                field: "conversation_history.candidate_author_speaker"
            })
        ));
    }

    #[test]
    fn tombstone_fork_selects_a_branch_from_the_parent_message() {
        let conversation_id = ConversationId::new();
        let source_branch_id = ConversationBranchId::new();
        let new_branch_id = ConversationBranchId::new();
        let conversation = contract_conversation(conversation_id, new_branch_id);
        let author = conversation.participants[0].id;
        let parent_id = MessageId::new();
        let target_id = MessageId::new();
        let (message, _) = history_message(
            conversation_id,
            source_branch_id,
            target_id,
            Some(parent_id),
            author,
        );
        let result = TombstoneResult {
            conversation,
            message,
            descendant_count: 0,
            asset_reference_deltas: Vec::new(),
            forked_branch: Some(history_branch(
                new_branch_id,
                conversation_id,
                Some(source_branch_id),
                Some(parent_id),
                None,
            )),
        };
        assert!(result.validate_for_policy(DescendantPolicy::Fork).is_ok());

        let self_fork = TombstoneResult {
            forked_branch: Some(history_branch(
                new_branch_id,
                conversation_id,
                Some(source_branch_id),
                Some(target_id),
                None,
            )),
            ..result.clone()
        };
        assert!(
            self_fork
                .validate_for_policy(DescendantPolicy::Fork)
                .is_err()
        );
    }

    #[test]
    fn tombstone_fork_accepts_a_parent_on_another_branch() {
        let conversation_id = ConversationId::new();
        let ancestor_branch_id = ConversationBranchId::new();
        let source_branch_id = ConversationBranchId::new();
        let new_branch_id = ConversationBranchId::new();
        let conversation = contract_conversation(conversation_id, new_branch_id);
        let author = conversation.participants[0].id;
        let parent_id = MessageId::new();
        let target_id = MessageId::new();
        let (message, _) = history_message(
            conversation_id,
            source_branch_id,
            target_id,
            Some(parent_id),
            author,
        );
        let result = TombstoneResult {
            conversation,
            message,
            descendant_count: 0,
            asset_reference_deltas: Vec::new(),
            forked_branch: Some(history_branch(
                new_branch_id,
                conversation_id,
                Some(ancestor_branch_id),
                Some(parent_id),
                None,
            )),
        };
        assert!(result.validate_for_policy(DescendantPolicy::Fork).is_ok());
    }

    #[test]
    fn tombstone_fork_rejects_a_root_message() {
        let conversation_id = ConversationId::new();
        let source_branch_id = ConversationBranchId::new();
        let new_branch_id = ConversationBranchId::new();
        let conversation = contract_conversation(conversation_id, new_branch_id);
        let author = conversation.participants[0].id;
        let target_id = MessageId::new();
        let (message, _) =
            history_message(conversation_id, source_branch_id, target_id, None, author);
        let result = TombstoneResult {
            conversation,
            message,
            descendant_count: 0,
            asset_reference_deltas: Vec::new(),
            forked_branch: Some(history_branch(
                new_branch_id,
                conversation_id,
                Some(source_branch_id),
                None,
                None,
            )),
        };
        assert!(result.validate_for_policy(DescendantPolicy::Fork).is_err());
    }

    #[test]
    fn fork_result_requires_the_new_branch_to_be_selected() {
        let conversation_id = ConversationId::new();
        let branch_id = ConversationBranchId::new();
        let conversation = contract_conversation(conversation_id, branch_id);
        let branch = history_branch(branch_id, conversation_id, None, None, None);
        let result = BranchResult {
            branch: branch.clone(),
            conversation: conversation.clone(),
        };
        assert!(result.validate().is_ok());

        let unselected = BranchResult {
            branch: history_branch(
                ConversationBranchId::new(),
                conversation_id,
                None,
                None,
                None,
            ),
            conversation: conversation.clone(),
        };
        assert!(unselected.validate().is_err());

        let mut archived = branch;
        archived.status = BranchStatus::Archived;
        assert!(
            BranchResult {
                branch: archived,
                conversation: conversation.clone(),
            }
            .validate()
            .is_err()
        );

        let foreign = BranchResult {
            branch: history_branch(branch_id, ConversationId::new(), None, None, None),
            conversation,
        };
        assert!(foreign.validate().is_err());
    }

    fn outbox_record(
        conversation_id: ConversationId,
        event: ConversationOutboxEvent,
    ) -> ConversationOutboxRecord {
        ConversationOutboxRecord {
            format_version: 1,
            id: lettuce_types::OutboxEventId::new(),
            conversation_id,
            conversation_revision: Revision::INITIAL,
            sequence: 1,
            operation_record_id: lettuce_types::OperationRecordId::new(),
            at: TimestampMillis::UNIX_EPOCH,
            event,
        }
    }

    #[test]
    fn mutation_outbox_variants_bind_to_their_conversation() {
        let conversation_id = ConversationId::new();
        let at = TimestampMillis::UNIX_EPOCH;
        let events = vec![
            ConversationOutboxEvent::CandidateChosen {
                conversation_id,
                message_id: MessageId::new(),
                candidate_id: MessageCandidateId::new(),
                at,
            },
            ConversationOutboxEvent::BranchSelected {
                conversation_id,
                branch_id: ConversationBranchId::new(),
                at,
            },
            ConversationOutboxEvent::ConversationLifecycleChanged {
                conversation_id,
                lifecycle: ConversationLifecycle::Archived,
                at,
            },
            ConversationOutboxEvent::SettingsChanged {
                conversation_id,
                settings_revision: Revision::INITIAL,
                at,
            },
            ConversationOutboxEvent::ParticipantPolicyChanged {
                conversation_id,
                participant_id: ConversationParticipantId::new(),
                at,
            },
            ConversationOutboxEvent::MessageFlagsChanged {
                conversation_id,
                message_id: MessageId::new(),
                pinned: true,
                visibility: MessageVisibility::Hidden,
                at,
            },
        ];
        for event in events {
            let record = outbox_record(conversation_id, event);
            assert!(record.validate().is_ok());
            let foreign = ConversationOutboxRecord {
                conversation_id: ConversationId::new(),
                ..record
            };
            assert!(matches!(
                foreign.validate(),
                Err(ValidationError::InvalidReference {
                    field: "outbox.conversation_id"
                })
            ));
        }
    }

    #[test]
    fn mutation_outbox_variants_reject_out_of_range_payloads() {
        let conversation_id = ConversationId::new();
        let at = TimestampMillis::UNIX_EPOCH;
        assert!(
            outbox_record(
                conversation_id,
                ConversationOutboxEvent::ConversationLifecycleChanged {
                    conversation_id,
                    lifecycle: ConversationLifecycle::Tombstoned,
                    at,
                },
            )
            .validate()
            .is_err()
        );
        assert!(matches!(
            outbox_record(
                conversation_id,
                ConversationOutboxEvent::SettingsChanged {
                    conversation_id,
                    settings_revision: Revision::new(0),
                    at,
                },
            )
            .validate(),
            Err(ValidationError::ZeroRevision)
        ));
        assert!(
            outbox_record(
                conversation_id,
                ConversationOutboxEvent::MessageFlagsChanged {
                    conversation_id,
                    message_id: MessageId::new(),
                    pinned: false,
                    visibility: MessageVisibility::Tombstoned,
                    at,
                },
            )
            .validate()
            .is_err()
        );
    }

    fn flags_command(
        pinned: Option<bool>,
        visibility: Option<MessageVisibility>,
    ) -> UpdateMessageFlags {
        UpdateMessageFlags {
            conversation_id: ConversationId::new(),
            message_id: MessageId::new(),
            expected_revision: Revision::INITIAL,
            operation: OperationToken {
                key: lettuce_jobs::IdempotencyKey::new("flags").expect("key"),
                request_digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
            },
            pinned,
            visibility,
        }
    }

    #[test]
    fn message_flags_command_requires_a_non_tombstoning_patch() {
        assert!(
            ConversationMutation::Flags(flags_command(Some(true), None))
                .validate()
                .is_ok()
        );
        assert!(
            ConversationMutation::Flags(flags_command(None, Some(MessageVisibility::Hidden)))
                .validate()
                .is_ok()
        );
        assert!(matches!(
            flags_command(None, None).validate(),
            Err(ValidationError::Invariant {
                field: "message_flags.empty_patch"
            })
        ));
        assert!(matches!(
            flags_command(Some(true), Some(MessageVisibility::Tombstoned)).validate(),
            Err(ValidationError::InvalidValue {
                field: "message_flags.visibility"
            })
        ));
        let mut stale = flags_command(Some(true), None);
        stale.expected_revision = Revision::new(0);
        assert!(matches!(
            ConversationMutation::Flags(stale).validate(),
            Err(ValidationError::ZeroRevision)
        ));
    }

    #[test]
    fn drafts_cannot_smuggle_a_tombstone_or_a_foreign_scene_edit() {
        let scene = MessageDraft {
            role: MessageRole::Scene,
            author_participant_id: None,
            parts: vec![],
            visibility: MessageVisibility::Visible,
            pinned: false,
            scene_edited: true,
        };
        assert!(scene.validate().is_ok());
        assert!(matches!(
            MessageDraft {
                role: MessageRole::User,
                author_participant_id: Some(ConversationParticipantId::new()),
                ..scene.clone()
            }
            .validate(),
            Err(ValidationError::InvalidValue {
                field: "message_draft.scene_edited"
            })
        ));
        assert!(matches!(
            MessageDraft {
                visibility: MessageVisibility::Tombstoned,
                ..scene
            }
            .validate(),
            Err(ValidationError::InvalidValue {
                field: "message_draft.visibility"
            })
        ));
        assert!(matches!(
            MessageEditDraft {
                parts: vec![],
                visibility: MessageVisibility::Tombstoned,
                pinned: false,
                scene_edited: false,
            }
            .validate(),
            Err(ValidationError::InvalidValue {
                field: "message_edit.visibility"
            })
        ));
    }

    #[test]
    fn a_headless_fork_branch_renders_its_inherited_ancestry() {
        let conversation_id = ConversationId::new();
        let root_id = ConversationBranchId::new();
        let fork_id = ConversationBranchId::new();
        let participant = history_participant();
        let a = MessageId::new();
        let b = MessageId::new();
        let history = ConversationHistory {
            conversation_id,
            branch_id: fork_id,
            head_message_id: None,
            fork_message_id: Some(b),
            branches: vec![
                history_branch(root_id, conversation_id, None, None, Some(b)),
                history_branch(fork_id, conversation_id, Some(root_id), Some(b), None),
            ],
            items: vec![
                history_item(conversation_id, root_id, a, None, participant.id, 0, 1),
                history_item(conversation_id, root_id, b, Some(a), participant.id, 1, 2),
            ],
            turns: Vec::new(),
        };
        assert!(history.validate(std::slice::from_ref(&participant)).is_ok());

        let truncated = ConversationHistory {
            items: vec![history_item(
                conversation_id,
                root_id,
                a,
                None,
                participant.id,
                0,
                1,
            )],
            ..history
        };
        assert!(truncated.validate(&[participant]).is_err());
    }

    #[test]
    fn message_flags_result_must_carry_the_requested_patch() {
        let command = flags_command(Some(true), Some(MessageVisibility::Hidden));
        let (message, _) = history_message(
            command.conversation_id,
            ConversationBranchId::new(),
            command.message_id,
            None,
            ConversationParticipantId::new(),
        );
        assert!(matches!(
            command.validate_result(&message),
            Err(ValidationError::Invariant {
                field: "message_flags.result_patch"
            })
        ));
        let applied = Message {
            pinned: true,
            visibility: MessageVisibility::Hidden,
            ..message
        };
        assert!(command.validate_result(&applied).is_ok());
        let foreign = Message {
            id: MessageId::new(),
            ..applied.clone()
        };
        assert!(matches!(
            command.validate_result(&foreign),
            Err(ValidationError::InvalidReference {
                field: "message_flags.result_identity"
            })
        ));
        let pinned_only = UpdateMessageFlags {
            visibility: None,
            ..command
        };
        let tombstoned = Message {
            visibility: MessageVisibility::Tombstoned,
            ..applied
        };
        assert!(matches!(
            pinned_only.validate_result(&tombstoned),
            Err(ValidationError::InvalidValue {
                field: "message_flags.result_visibility"
            })
        ));
    }
}
