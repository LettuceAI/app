//! Atomic conversation creation.
//!
//! This adapter is intentionally separate from the read projections.  A
//! prepared launch owns the immutable payload drafts; this module consumes
//! them on the same SQLite transaction as the conversation, timeline,
//! idempotency record, and creation outbox event.

use std::collections::BTreeMap;

use lettuce_conversations::{
    Conversation, ConversationAggregate, ConversationBranch, ConversationCreator,
    ConversationLifecycle, ConversationOutboxEvent, ConversationOutboxRecord,
    ConversationParticipant, ConversationRepositoryError, InitialMessageOrigin, MessagePart,
    MessageVisibility, OperationKind, OperationRecord, OperationResultRef,
    PreparedConversationLaunch, ProtectedSnapshotRef, SnapshotArtifactDraft,
};
use lettuce_types::{
    ConversationBranchId, ConversationId, OperationRecordId, OutboxEventId, Revision,
    TimestampMillis,
};
use rusqlite::{Transaction, TransactionBehavior, params};

use super::{
    Database, conversation_artifact_adapter, conversation_mutation_kernel, conversation_query,
    conversation_vertical_slice,
};

#[cfg(test)]
use lettuce_conversations::MessageRole;

pub(crate) fn media_role_name(role: lettuce_conversations::MediaAssetRole) -> &'static str {
    match role {
        lettuce_conversations::MediaAssetRole::Inline => "inline",
        lettuce_conversations::MediaAssetRole::Attachment => "attachment",
        lettuce_conversations::MediaAssetRole::Avatar => "avatar",
        lettuce_conversations::MediaAssetRole::Scene => "scene",
        lettuce_conversations::MediaAssetRole::Reference => "reference",
    }
}

fn artifact_error(error: lettuce_conversations::ArtifactError) -> ConversationRepositoryError {
    ConversationRepositoryError::ArtifactReference(error)
}

fn expected_snapshot_refs(
    plan: &lettuce_conversations::CreateConversationPlan,
) -> Result<BTreeMap<String, ProtectedSnapshotRef>, ConversationRepositoryError> {
    let mut references = BTreeMap::new();
    for reference in lettuce_conversations::conversation_launch_snapshot_references(plan) {
        let id = reference.artifact_id.to_string();
        if let Some(existing) = references.get(&id) {
            if existing != reference {
                return Err(ConversationRepositoryError::Invalid(
                    lettuce_conversations::ValidationError::InvalidReference {
                        field: "conversation.snapshot_references",
                    },
                ));
            }
        } else {
            references.insert(id, reference.clone());
        }
    }
    Ok(references)
}

fn stage_artifacts(
    transaction: &Transaction<'_>,
    drafts: Vec<SnapshotArtifactDraft>,
    now: TimestampMillis,
) -> Result<BTreeMap<String, ProtectedSnapshotRef>, ConversationRepositoryError> {
    let mut staged = BTreeMap::new();
    for draft in drafts {
        let reference =
            conversation_artifact_adapter::stage_snapshot_in_transaction(transaction, draft, now)
                .map_err(artifact_error)?;
        if staged
            .insert(reference.artifact_id.to_string(), reference)
            .is_some()
        {
            return Err(ConversationRepositoryError::Invalid(
                lettuce_conversations::ValidationError::Duplicate {
                    field: "conversation.snapshot_artifacts",
                },
            ));
        }
    }
    Ok(staged)
}

fn preflight_media(
    transaction: &Transaction<'_>,
    plan: &lettuce_conversations::CreateConversationPlan,
) -> Result<(), ConversationRepositoryError> {
    for entry in &plan.initial_timeline.entries {
        for part in &entry.parts {
            let MessagePart::MediaAsset { asset_id, .. } = part else {
                continue;
            };
            let exists: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM media_assets WHERE id = ?1)",
                    [asset_id.to_string()],
                    |row| row.get(0),
                )
                .map_err(conversation_vertical_slice::db)?;
            if !exists {
                return Err(ConversationRepositoryError::Storage);
            }
        }
    }
    Ok(())
}

fn make_aggregate(
    plan: &lettuce_conversations::CreateConversationPlan,
    root_branch_id: ConversationBranchId,
    now: TimestampMillis,
) -> Result<ConversationAggregate, ConversationRepositoryError> {
    let participants = plan
        .participants
        .iter()
        .map(|participant| ConversationParticipant {
            id: participant.id,
            role: participant.role,
            ordinal: participant.ordinal,
            enabled: participant.enabled,
            muted: participant.muted,
            source: participant.source,
            display_name: participant.display_name.clone(),
            authored_description: participant.authored_description.clone(),
            model_selection: participant.model_selection.clone(),
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        })
        .collect();
    let conversation = Conversation {
        id: plan.conversation_id,
        lifecycle: ConversationLifecycle::Active,
        title: plan.title.clone(),
        kind: plan.kind.clone(),
        active_branch_id: root_branch_id,
        participants,
        current_settings: None,
        revision: Revision::INITIAL,
        created_at: now,
        updated_at: now,
    };
    let branch = ConversationBranch {
        id: root_branch_id,
        conversation_id: plan.conversation_id,
        parent_branch_id: None,
        fork_message_id: None,
        head_message_id: None,
        status: lettuce_conversations::BranchStatus::Active,
        revision: Revision::INITIAL,
        created_at: now,
        updated_at: now,
    };
    let aggregate = ConversationAggregate {
        conversation,
        branches: vec![branch],
    };
    aggregate
        .validate()
        .map_err(ConversationRepositoryError::Invalid)?;
    Ok(aggregate)
}

fn persist_initial_timeline(
    transaction: &Transaction<'_>,
    plan: &lettuce_conversations::CreateConversationPlan,
    root_branch_id: ConversationBranchId,
    now: TimestampMillis,
) -> Result<Option<lettuce_types::MessageId>, ConversationRepositoryError> {
    let mut parent = None;
    for (index, entry) in plan.initial_timeline.entries.iter().enumerate() {
        let ordinal = i64::try_from(index + 1).map_err(|_| ConversationRepositoryError::Storage)?;
        let parts_json = conversation_vertical_slice::encode(&entry.parts)?;
        transaction
            .execute(
                "INSERT INTO conversation_messages (conversation_id, id, branch_id, parent_message_id, author_participant_id, role, timeline_ordinal, logical_time, effective_time, visibility, pinned, scene_edited, active_revision_id, active_candidate_id, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, 'visible', 0, 0, ?9, NULL, 1, ?8, ?8)",
                params![
                    plan.conversation_id.to_string(),
                    entry.message_id.to_string(),
                    root_branch_id.to_string(),
                    parent.map(|id: lettuce_types::MessageId| id.to_string()),
                    entry
                        .author_participant_id
                        .map(|id| id.to_string()),
                    conversation_mutation_kernel::message_role_name(entry.role),
                    ordinal,
                    now.get(),
                    entry.revision_id.to_string(),
                ],
            )
            .map_err(conversation_vertical_slice::db)?;
        transaction
            .execute(
                "INSERT INTO conversation_message_revisions (conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)",
                params![
                    plan.conversation_id.to_string(),
                    entry.revision_id.to_string(),
                    entry.message_id.to_string(),
                    root_branch_id.to_string(),
                    parts_json,
                    now.get(),
                ],
            )
            .map_err(conversation_vertical_slice::db)?;

        let (source_kind, starter_id) = match &entry.origin {
            InitialMessageOrigin::SelectedScene { .. } => ("scene", None),
            InitialMessageOrigin::StarterMessage {
                starter_message_id, ..
            } => ("starter", Some(starter_message_id.to_string())),
        };
        let reference = match &entry.origin {
            InitialMessageOrigin::SelectedScene { snapshot_ref }
            | InitialMessageOrigin::StarterMessage { snapshot_ref, .. } => snapshot_ref,
        };
        transaction
            .execute(
                "INSERT INTO conversation_initial_message_origins (conversation_id, message_id, snapshot_artifact_id, source_kind, starter_message_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    plan.conversation_id.to_string(),
                    entry.message_id.to_string(),
                    reference.artifact_id.to_string(),
                    source_kind,
                    starter_id,
                ],
            )
            .map_err(conversation_vertical_slice::db)?;
        for (part_ordinal, part) in entry.parts.iter().enumerate() {
            let MessagePart::MediaAsset { asset_id, role } = part else {
                continue;
            };
            transaction
                .execute(
                    "INSERT INTO revision_media_refs (conversation_id, message_revision_id, part_ordinal, asset_id, media_role, state, created_at) VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6)",
                    params![
                        plan.conversation_id.to_string(),
                        entry.revision_id.to_string(),
                        i64::try_from(part_ordinal)
                            .map_err(|_| ConversationRepositoryError::Storage)?,
                        asset_id.to_string(),
                        media_role_name(*role),
                        now.get(),
                    ],
                )
                .map_err(conversation_vertical_slice::db)?;
        }
        parent = Some(entry.message_id);
    }
    if let Some(head) = parent {
        transaction
            .execute(
                "UPDATE conversation_branches SET head_message_id = ?1, updated_at = ?2 WHERE conversation_id = ?3 AND id = ?4",
                params![
                    head.to_string(),
                    now.get(),
                    plan.conversation_id.to_string(),
                    root_branch_id.to_string(),
                ],
            )
            .map_err(conversation_vertical_slice::db)?;
    }
    transaction
        .execute(
            "UPDATE conversations SET next_timeline_ordinal = ?1 WHERE id = ?2",
            params![
                i64::try_from(plan.initial_timeline.entries.len() + 1)
                    .map_err(|_| ConversationRepositoryError::Storage)?,
                plan.conversation_id.to_string(),
            ],
        )
        .map_err(conversation_vertical_slice::db)?;
    Ok(parent)
}

fn persist_operation_and_outbox(
    transaction: &Transaction<'_>,
    plan: &lettuce_conversations::CreateConversationPlan,
    root_branch_id: ConversationBranchId,
    head_message_id: Option<lettuce_types::MessageId>,
    now: TimestampMillis,
) -> Result<(OperationRecord, ConversationOutboxRecord), ConversationRepositoryError> {
    let operation = conversation_mutation_kernel::insert_operation(
        transaction,
        plan.conversation_id,
        OperationKind::Create,
        &plan.operation,
        &OperationResultRef::Conversation(plan.conversation_id),
        now,
    )?;
    let sequence = conversation_mutation_kernel::next_outbox_sequence(
        transaction,
        plan.conversation_id,
        OperationKind::Create,
    )?;
    if sequence != 1 {
        return Err(ConversationRepositoryError::Storage);
    }
    let event = ConversationOutboxEvent::ConversationCreated {
        conversation_id: plan.conversation_id,
        root_branch_id,
        head_message_id,
        initial_message_count: u16::try_from(plan.initial_timeline.entries.len())
            .map_err(|_| ConversationRepositoryError::Storage)?,
        at: now,
    };
    let outbox = ConversationOutboxRecord {
        format_version: 1,
        id: OutboxEventId::new(),
        conversation_id: plan.conversation_id,
        conversation_revision: Revision::INITIAL,
        sequence,
        operation_record_id: operation.id,
        at: now,
        event,
    };
    conversation_mutation_kernel::insert_outbox(transaction, &outbox)?;
    Ok((operation, outbox))
}

fn hydrate_and_validate(
    transaction: &Transaction<'_>,
    plan: &lettuce_conversations::CreateConversationPlan,
    root_branch_id: ConversationBranchId,
    operation: &OperationRecord,
    outbox: &ConversationOutboxRecord,
) -> Result<ConversationAggregate, ConversationRepositoryError> {
    let aggregate = conversation_vertical_slice::hydrate_conversation(
        transaction,
        plan.conversation_id,
        || {},
    )?;
    let timeline = conversation_query::hydrate_branch_timeline_all(
        transaction,
        plan.conversation_id,
        root_branch_id,
    )?;
    let timeline_matches = timeline.items.len() == plan.initial_timeline.entries.len()
        && timeline
            .items
            .iter()
            .zip(plan.initial_timeline.entries.iter())
            .enumerate()
            .all(|(index, (item, expected))| {
                let expected_parent = index
                    .checked_sub(1)
                    .map(|parent| plan.initial_timeline.entries[parent].message_id);
                item.message.id == expected.message_id
                    && item.message.parent_message_id == expected_parent
                    && item.message.role == expected.role
                    && item.message.author_participant_id == expected.author_participant_id
                    && item.message.branch_id == root_branch_id
                    && item.message.logical_time == item.message.created_at
                    && item.message.effective_time == item.message.created_at
                    && item.message.visibility == MessageVisibility::Visible
                    && !item.message.pinned
                    && !item.message.scene_edited
                    && item.message.revision == Revision::INITIAL
                    && item.active_candidate.is_none()
                    && item.active_revision.as_ref().is_some_and(|revision| {
                        revision.id == expected.revision_id
                            && revision.message_id == expected.message_id
                            && revision.sequence == Revision::INITIAL
                            && revision.parts == expected.parts
                            && revision.authored_at == item.message.created_at
                            && revision.provider_replay.is_none()
                    })
                    && item.initial_origin.as_ref() == Some(&expected.origin)
            });
    if !timeline_matches {
        return Err(ConversationRepositoryError::Storage);
    }
    operation.clone().replay_or_conflict(
        plan.conversation_id,
        OperationKind::Create,
        &plan.operation,
    )?;
    conversation_query::validate_outbox_event_timestamp(outbox)?;
    conversation_query::validate_outbox_event(transaction, outbox)?;
    conversation_query::validate_outbox_event_exact(transaction, outbox)?;
    if outbox.operation_record_id != operation.id
        || outbox.sequence != 1
        || outbox.conversation_revision != Revision::INITIAL
        || outbox.at != operation.created_at
    {
        return Err(ConversationRepositoryError::Storage);
    }
    Ok(aggregate)
}

fn read_operation(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    key: &lettuce_conversations::OperationToken,
) -> Result<Option<OperationRecord>, ConversationRepositoryError> {
    conversation_mutation_kernel::read_operation_any(
        transaction,
        conversation_id,
        OperationKind::Create,
        key,
    )
}

fn read_creation_outbox(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    operation_id: OperationRecordId,
) -> Result<ConversationOutboxRecord, ConversationRepositoryError> {
    let records = conversation_mutation_kernel::read_outbox_for_operation(
        transaction,
        conversation_id,
        operation_id,
    )?;
    let [record] = records.as_slice() else {
        return Err(ConversationRepositoryError::Storage);
    };
    Ok(record.clone())
}

impl ConversationCreator for Database {
    fn create(
        &self,
        launch: PreparedConversationLaunch,
        now: TimestampMillis,
    ) -> Result<lettuce_conversations::CreateConversationResult, ConversationRepositoryError> {
        launch
            .plan()
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        let plan_id = launch.plan().conversation_id;
        let token = launch.plan().operation.clone();
        let mut connection = self
            .connection()
            .map_err(|_| ConversationRepositoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(conversation_vertical_slice::db)?;

        if let Some(operation) = read_operation(&transaction, plan_id, &token)? {
            if operation.operation.request_digest != token.request_digest {
                return Err(ConversationRepositoryError::Conflict);
            }
            let aggregate =
                conversation_vertical_slice::hydrate_conversation(&transaction, plan_id, || {})?;
            let outbox = read_creation_outbox(&transaction, plan_id, operation.id)?;
            let ConversationOutboxEvent::ConversationCreated {
                conversation_id,
                root_branch_id: _,
                head_message_id: _,
                initial_message_count,
                at,
            } = &outbox.event
            else {
                return Err(ConversationRepositoryError::Storage);
            };
            if *conversation_id != plan_id
                || outbox.operation_record_id != operation.id
                || outbox.sequence != 1
                || outbox.conversation_revision != Revision::INITIAL
                || outbox.at != operation.created_at
                || *at != outbox.at
                || usize::from(*initial_message_count)
                    != transaction
                        .query_row(
                            "SELECT count(*) FROM conversation_initial_message_origins WHERE conversation_id = ?1",
                            [plan_id.to_string()],
                            |row| row.get::<_, i64>(0),
                        )
                        .map_err(conversation_vertical_slice::db)?
                        as usize
            {
                return Err(ConversationRepositoryError::Storage);
            }
            conversation_query::validate_outbox_event_timestamp(&outbox)?;
            conversation_query::validate_outbox_event(&transaction, &outbox)?;
            conversation_query::validate_outbox_event_exact(&transaction, &outbox)?;
            transaction
                .commit()
                .map_err(conversation_vertical_slice::db)?;
            return Ok(lettuce_conversations::MutationCommit {
                value: aggregate,
                operation,
                outbox: vec![outbox],
            });
        }
        if transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
                [plan_id.to_string()],
                |row| row.get::<_, bool>(0),
            )
            .map_err(conversation_vertical_slice::db)?
        {
            return Err(ConversationRepositoryError::Conflict);
        }

        let (plan, drafts) = launch.into_parts();
        let expected = expected_snapshot_refs(&plan)?;
        let staged = stage_artifacts(&transaction, drafts, now)?;
        if staged != expected {
            return Err(ConversationRepositoryError::ArtifactReference(
                lettuce_conversations::ArtifactError::InvalidReference(
                    lettuce_conversations::ValidationError::InvalidReference {
                        field: "conversation.snapshot_references",
                    },
                ),
            ));
        }
        preflight_media(&transaction, &plan)?;

        let root_branch_id = ConversationBranchId::new();
        let aggregate = make_aggregate(&plan, root_branch_id, now)?;
        conversation_vertical_slice::save_conversation(&transaction, &aggregate.conversation)?;
        conversation_vertical_slice::save_branch(&transaction, &aggregate.branches[0])?;
        for reference in expected.values() {
            transaction
                .execute(
                    "INSERT INTO conversation_snapshot_refs (conversation_id, artifact_id) VALUES (?1, ?2)",
                    params![plan.conversation_id.to_string(), reference.artifact_id.to_string()],
                )
                .map_err(conversation_vertical_slice::db)?;
        }
        let head_message_id = persist_initial_timeline(&transaction, &plan, root_branch_id, now)?;
        let (operation, outbox) = persist_operation_and_outbox(
            &transaction,
            &plan,
            root_branch_id,
            head_message_id,
            now,
        )?;
        let aggregate =
            hydrate_and_validate(&transaction, &plan, root_branch_id, &operation, &outbox)?;
        transaction
            .commit()
            .map_err(conversation_vertical_slice::db)?;
        Ok(lettuce_conversations::MutationCommit {
            value: aggregate,
            operation,
            outbox: vec![outbox],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_conversations::{
        ArtifactCodec, ArtifactRetention, CharacterLaunchSnapshot, ConversationKind,
        ConversationParticipantDraft, DirectConversationDetails, OperationToken, ParticipantRole,
        ParticipantSource, PreparedConversationLaunch, ProtectedArtifactBytes, SceneLaunchSnapshot,
        SnapshotSelection, SnapshotSource, StarterLaunchSnapshot,
    };
    use lettuce_types::{
        AssetId, CharacterId, ContentHash, ConversationParticipantId, ConversationStarterId,
        SceneId, SnapshotArtifactId, StarterMessageId,
    };

    fn prepared(
        conversation_id: ConversationId,
        character_id: CharacterId,
    ) -> PreparedConversationLaunch {
        let bytes = ProtectedArtifactBytes::new(b"character launch".to_vec()).expect("bytes");
        let reference = ProtectedSnapshotRef {
            source: SnapshotSource::Character(character_id),
            source_revision: Revision::INITIAL,
            artifact_id: SnapshotArtifactId::new(),
            digest: bytes.digest(),
            schema_version: 1,
            byte_size: bytes.len() as u64,
        };
        let user_id = ConversationParticipantId::new();
        let character_participant_id = ConversationParticipantId::new();
        let plan = lettuce_conversations::CreateConversationPlan {
            conversation_id,
            title: "Atomic create".into(),
            kind: ConversationKind::Direct(DirectConversationDetails {
                format_version: 1,
                character: CharacterLaunchSnapshot {
                    snapshot_ref: reference.clone(),
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
                    display_name: "Ada".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Disabled,
                },
            ],
            initial_timeline: lettuce_conversations::InitialTimelineDraft {
                format_version: 1,
                entries: Vec::new(),
            },
            operation: OperationToken {
                key: lettuce_jobs::IdempotencyKey::new("atomic-create").expect("key"),
                request_digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
            },
        };
        PreparedConversationLaunch::new(
            plan,
            vec![SnapshotArtifactDraft {
                source: reference.source,
                source_revision: reference.source_revision,
                artifact_id: reference.artifact_id,
                digest: reference.digest,
                schema_version: reference.schema_version,
                byte_size: reference.byte_size,
                codec: ArtifactCodec::Json,
                retention: ArtifactRetention::Conversation,
                bytes,
            }],
        )
        .expect("prepared launch")
    }

    fn timeline_prepared(
        conversation_id: ConversationId,
        character_id: CharacterId,
    ) -> PreparedConversationLaunch {
        let user_id = ConversationParticipantId::new();
        let character_participant_id = ConversationParticipantId::new();
        let scene_id = SceneId::new();
        let starter_id = ConversationStarterId::new();
        let make_reference = |source: SnapshotSource, payload: &[u8]| {
            let bytes = ProtectedArtifactBytes::new(payload.to_vec()).expect("bytes");
            (
                ProtectedSnapshotRef {
                    source,
                    source_revision: Revision::INITIAL,
                    artifact_id: SnapshotArtifactId::new(),
                    digest: bytes.digest(),
                    schema_version: 1,
                    byte_size: bytes.len() as u64,
                },
                bytes,
            )
        };
        let (character_ref, character_bytes) =
            make_reference(SnapshotSource::Character(character_id), b"character");
        let (scene_ref, scene_bytes) = make_reference(SnapshotSource::Scene(scene_id), b"scene");
        let (starter_ref, starter_bytes) =
            make_reference(SnapshotSource::Starter(starter_id), b"starter");
        let plan = lettuce_conversations::CreateConversationPlan {
            conversation_id,
            title: "Timeline create".into(),
            kind: ConversationKind::Direct(DirectConversationDetails {
                format_version: 1,
                character: CharacterLaunchSnapshot {
                    snapshot_ref: character_ref.clone(),
                    source_id: character_id,
                    source_revision: Revision::INITIAL,
                    name: "Ada".into(),
                    nickname: None,
                },
                persona: SnapshotSelection::Disabled,
                scene: SnapshotSelection::Explicit(SceneLaunchSnapshot {
                    snapshot_ref: scene_ref.clone(),
                    source_id: scene_id,
                    source_revision: Revision::INITIAL,
                    title: "Opening".into(),
                }),
                starter: SnapshotSelection::Explicit(StarterLaunchSnapshot {
                    snapshot_ref: starter_ref.clone(),
                    source_id: starter_id,
                    source_revision: Revision::INITIAL,
                    title: "Greeting".into(),
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
                    display_name: "Ada".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Disabled,
                },
            ],
            initial_timeline: lettuce_conversations::InitialTimelineDraft {
                format_version: 1,
                entries: vec![
                    lettuce_conversations::InitialMessageDraft {
                        message_id: lettuce_types::MessageId::new(),
                        revision_id: lettuce_types::MessageRevisionId::new(),
                        origin: InitialMessageOrigin::SelectedScene {
                            snapshot_ref: scene_ref.clone(),
                        },
                        role: MessageRole::Scene,
                        author_participant_id: None,
                        parts: vec![MessagePart::Text {
                            text: "The room is quiet.".into(),
                        }],
                    },
                    lettuce_conversations::InitialMessageDraft {
                        message_id: lettuce_types::MessageId::new(),
                        revision_id: lettuce_types::MessageRevisionId::new(),
                        origin: InitialMessageOrigin::StarterMessage {
                            snapshot_ref: starter_ref.clone(),
                            starter_message_id: StarterMessageId::new(),
                        },
                        role: MessageRole::User,
                        author_participant_id: Some(user_id),
                        parts: vec![MessagePart::Text {
                            text: "Hello".into(),
                        }],
                    },
                    lettuce_conversations::InitialMessageDraft {
                        message_id: lettuce_types::MessageId::new(),
                        revision_id: lettuce_types::MessageRevisionId::new(),
                        origin: InitialMessageOrigin::StarterMessage {
                            snapshot_ref: starter_ref.clone(),
                            starter_message_id: StarterMessageId::new(),
                        },
                        role: MessageRole::Assistant,
                        author_participant_id: Some(character_participant_id),
                        parts: vec![MessagePart::Text {
                            text: "Welcome.".into(),
                        }],
                    },
                ],
            },
            operation: OperationToken {
                key: lettuce_jobs::IdempotencyKey::new("timeline-create").expect("key"),
                request_digest: ContentHash::parse("ef".repeat(32)).expect("digest"),
            },
        };
        let drafts = vec![
            (character_ref, character_bytes),
            (scene_ref, scene_bytes),
            (starter_ref, starter_bytes),
        ]
        .into_iter()
        .map(|(reference, bytes)| SnapshotArtifactDraft {
            source: reference.source,
            source_revision: reference.source_revision,
            artifact_id: reference.artifact_id,
            digest: reference.digest,
            schema_version: reference.schema_version,
            byte_size: reference.byte_size,
            codec: ArtifactCodec::Json,
            retention: ArtifactRetention::Conversation,
            bytes,
        })
        .collect();
        PreparedConversationLaunch::new(plan, drafts).expect("prepared timeline launch")
    }

    #[test]
    fn create_and_exact_replay_are_atomic() {
        let database = Database::open_in_memory().expect("database");
        let conversation_id = ConversationId::new();
        let character_id = CharacterId::new();
        let first = ConversationCreator::create(
            &database,
            timeline_prepared(conversation_id, character_id),
            TimestampMillis::new(10),
        )
        .expect("create");
        assert_eq!(first.value.conversation.id, conversation_id);
        assert_eq!(first.operation.kind, OperationKind::Create);
        assert_eq!(first.outbox.len(), 1);
        assert!(matches!(
            first.outbox[0].event,
            ConversationOutboxEvent::ConversationCreated {
                head_message_id: Some(_),
                initial_message_count: 3,
                ..
            }
        ));
        let root_branch = first.value.conversation.active_branch_id;
        let original_head: lettuce_types::MessageId = database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT head_message_id FROM conversation_branches WHERE conversation_id = ?1 AND id = ?2",
                params![conversation_id.to_string(), root_branch.to_string()],
                |row| row.get::<_, Option<String>>(0),
            )
            .expect("head")
            .expect("empty head")
            .parse()
            .expect("head id");
        let later_message = lettuce_types::MessageId::new();
        let later_revision = lettuce_types::MessageRevisionId::new();
        let character_participant = first
            .value
            .conversation
            .participants
            .iter()
            .find(|participant| participant.role == ParticipantRole::Character)
            .expect("character")
            .id;
        let mut connection = database.connection().expect("connection");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction");
        transaction
            .execute(
                "INSERT INTO conversation_messages (conversation_id, id, branch_id, parent_message_id, author_participant_id, role, timeline_ordinal, logical_time, effective_time, visibility, pinned, scene_edited, active_revision_id, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 'assistant', 4, 11, 11, 'visible', 0, 0, ?6, 1, 11, 11)",
                params![
                    conversation_id.to_string(),
                    later_message.to_string(),
                    root_branch.to_string(),
                    original_head.to_string(),
                    character_participant.to_string(),
                    later_revision.to_string(),
                ],
            )
            .expect("later message");
        transaction
            .execute(
                "INSERT INTO conversation_message_revisions (conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at) VALUES (?1, ?2, ?3, ?4, 1, ?5, 11)",
                params![
                    conversation_id.to_string(),
                    later_revision.to_string(),
                    later_message.to_string(),
                    root_branch.to_string(),
                    conversation_vertical_slice::encode(&vec![MessagePart::Text {
                        text: "later".into()
                    }])
                    .expect("parts"),
                ],
            )
            .expect("later revision");
        transaction
            .execute(
                "UPDATE conversation_branches SET head_message_id = ?1 WHERE conversation_id = ?2 AND id = ?3",
                params![later_message.to_string(), conversation_id.to_string(), root_branch.to_string()],
            )
            .expect("advance head");
        transaction.commit().expect("commit");
        drop(connection);
        let replay = ConversationCreator::create(
            &database,
            timeline_prepared(conversation_id, character_id),
            TimestampMillis::new(99),
        )
        .expect("replay");
        assert_eq!(replay.operation, first.operation);
        assert_eq!(replay.outbox, first.outbox);
        assert_eq!(
            replay.value.conversation.active_branch_id,
            first.value.conversation.active_branch_id
        );
        assert_eq!(
            replay.value.branches[0].head_message_id,
            Some(later_message)
        );
        assert_eq!(
            ConversationCreator::create(
                &database,
                {
                    let launch = prepared(conversation_id, character_id);
                    let (mut plan, drafts) = launch.into_parts();
                    plan.operation.request_digest =
                        ContentHash::parse("cd".repeat(32)).expect("digest");
                    PreparedConversationLaunch::new(plan, drafts).expect("changed launch")
                },
                TimestampMillis::new(99),
            ),
            Err(ConversationRepositoryError::Conflict)
        );
    }

    #[test]
    fn create_persists_ordered_initial_scene_and_starters() {
        let database = Database::open_in_memory().expect("database");
        let conversation_id = ConversationId::new();
        let result = ConversationCreator::create(
            &database,
            timeline_prepared(conversation_id, CharacterId::new()),
            TimestampMillis::new(10),
        )
        .expect("create timeline");
        let timeline = lettuce_conversations::ConversationReader::timeline_page(
            &database,
            conversation_id,
            result.value.conversation.active_branch_id,
            &lettuce_types::PageRequest {
                cursor: None,
                limit: lettuce_types::PageLimit::new(20),
            },
        )
        .expect("timeline");
        assert_eq!(timeline.items.len(), 3);
        assert_eq!(timeline.items[0].message.role, MessageRole::Assistant);
        assert_eq!(timeline.items[2].message.role, MessageRole::Scene);
        assert_eq!(
            database
                .connection()
                .expect("connection")
                .query_row(
                    "SELECT next_timeline_ordinal FROM conversations WHERE id = ?1",
                    [conversation_id.to_string()],
                    |row| row.get::<_, i64>(0),
                )
                .expect("ordinal"),
            4
        );
    }

    #[test]
    fn create_rolls_back_staged_artifacts_when_outbox_insert_aborts() {
        let database = Database::open_in_memory().expect("database");
        database
            .connection()
            .expect("connection")
            .execute_batch(
                "CREATE TRIGGER fail_test_create_outbox BEFORE INSERT ON conversation_outbox BEGIN SELECT RAISE(ABORT, 'test rollback'); END;",
            )
            .expect("failure trigger");
        let conversation_id = ConversationId::new();
        let result = ConversationCreator::create(
            &database,
            prepared(conversation_id, CharacterId::new()),
            TimestampMillis::new(10),
        );
        assert_eq!(result, Err(ConversationRepositoryError::Storage));
        let connection = database.connection().expect("connection");
        for table in [
            "conversations",
            "conversation_participants",
            "conversation_branches",
            "conversation_messages",
            "conversation_message_revisions",
            "conversation_initial_message_origins",
            "revision_media_refs",
            "conversation_snapshot_artifacts",
            "conversation_snapshot_refs",
            "conversation_operations",
            "conversation_outbox",
        ] {
            let count: i64 = connection
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("rollback count");
            assert_eq!(count, 0, "rows leaked in {table}");
        }
    }

    #[test]
    fn file_backed_replay_converges_and_different_key_conflicts() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-create-replay-{}.db",
            ConversationId::new()
        ));
        let first_database = Database::open(&path).expect("first database");
        let second_database = Database::open(&path).expect("second database");
        let conversation_id = ConversationId::new();
        let character_id = CharacterId::new();
        let first = ConversationCreator::create(
            &first_database,
            prepared(conversation_id, character_id),
            TimestampMillis::new(10),
        )
        .expect("first create");
        let replay = ConversationCreator::create(
            &second_database,
            prepared(conversation_id, character_id),
            TimestampMillis::new(99),
        )
        .expect("post-lock replay");
        assert_eq!(replay.operation, first.operation);
        assert_eq!(replay.outbox, first.outbox);
        let mut different_key = prepared(conversation_id, character_id).into_parts();
        different_key.0.operation.key =
            lettuce_jobs::IdempotencyKey::new("different-key").expect("key");
        let changed =
            PreparedConversationLaunch::new(different_key.0, different_key.1).expect("launch");
        assert_eq!(
            ConversationCreator::create(&second_database, changed, TimestampMillis::new(100)),
            Err(ConversationRepositoryError::Conflict)
        );
        let connection = second_database.connection().expect("connection");
        for table in ["conversation_operations", "conversation_outbox"] {
            let count: i64 = connection
                .query_row(
                    &format!("SELECT count(*) FROM {table} WHERE conversation_id = ?1"),
                    [conversation_id.to_string()],
                    |row| row.get(0),
                )
                .expect("count");
            assert_eq!(count, 1, "duplicate rows in {table}");
        }
        drop(connection);
        drop(second_database);
        drop(first_database);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(format!("{}-wal", path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    }

    #[test]
    fn missing_initial_media_asset_rolls_back_staged_launch() {
        let database = Database::open_in_memory().expect("database");
        let conversation_id = ConversationId::new();
        let launch = timeline_prepared(conversation_id, CharacterId::new()).into_parts();
        let (mut plan, drafts) = launch;
        plan.initial_timeline.entries[0].parts = vec![
            MessagePart::Text {
                text: "The room is quiet.".into(),
            },
            MessagePart::MediaAsset {
                asset_id: AssetId::new(),
                role: lettuce_conversations::MediaAssetRole::Inline,
            },
        ];
        let launch = PreparedConversationLaunch::new(plan, drafts).expect("media launch");
        assert_eq!(
            ConversationCreator::create(&database, launch, TimestampMillis::new(10)),
            Err(ConversationRepositoryError::Storage)
        );
        let connection = database.connection().expect("connection");
        for table in [
            "conversations",
            "conversation_snapshot_artifacts",
            "conversation_snapshot_refs",
            "conversation_operations",
            "conversation_outbox",
        ] {
            let count: i64 = connection
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("rollback count");
            assert_eq!(count, 0, "rows leaked in {table}");
        }
    }

    #[test]
    fn schema_rejects_mismatched_and_duplicate_creation_events() {
        let database = Database::open_in_memory().expect("database");
        let conversation_id = ConversationId::new();
        let first = ConversationCreator::create(
            &database,
            prepared(conversation_id, CharacterId::new()),
            TimestampMillis::new(10),
        )
        .expect("create");
        let connection = database.connection().expect("connection");
        let event_json =
            conversation_vertical_slice::encode(&ConversationOutboxEvent::ConversationCreated {
                conversation_id,
                root_branch_id: first.value.conversation.active_branch_id,
                head_message_id: None,
                initial_message_count: 0,
                at: TimestampMillis::new(10),
            })
            .expect("event");
        let result_json =
            conversation_vertical_slice::encode(&OperationResultRef::Conversation(conversation_id))
                .expect("result");
        let send_operation_id = OperationRecordId::new();
        connection
            .execute(
                "INSERT INTO conversation_operations (id, conversation_id, kind, operation_key, request_digest, result_kind, result_id, result_json, created_at) VALUES (?1, ?2, 'send', 'send-key', ?3, 'conversation', ?2, ?4, 10)",
                params![send_operation_id.to_string(), conversation_id.to_string(), "11".repeat(32), result_json],
            )
            .expect("send operation");
        assert!(connection
            .execute(
                "INSERT INTO conversation_outbox (conversation_id, id, sequence, conversation_revision, operation_record_id, at, event_json) VALUES (?1, ?2, 2, 1, ?3, 10, ?4)",
                params![conversation_id.to_string(), OutboxEventId::new().to_string(), send_operation_id.to_string(), event_json.clone()],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO conversation_outbox (conversation_id, id, sequence, conversation_revision, operation_record_id, at, event_json) VALUES (?1, ?2, 2, 1, ?3, 10, ?4)",
                params![conversation_id.to_string(), OutboxEventId::new().to_string(), first.operation.id.to_string(), event_json],
            )
            .is_err());
        let different_event =
            conversation_vertical_slice::encode(&ConversationOutboxEvent::MessageCommitted {
                conversation_id,
                branch_id: first.value.conversation.active_branch_id,
                message_id: lettuce_types::MessageId::new(),
                revision_id: None,
                candidate_id: None,
                at: TimestampMillis::new(10),
            })
            .expect("different event");
        assert!(connection
            .execute(
                "INSERT INTO conversation_outbox (conversation_id, id, sequence, conversation_revision, operation_record_id, at, event_json) VALUES (?1, ?2, 2, 1, ?3, 10, ?4)",
                params![conversation_id.to_string(), OutboxEventId::new().to_string(), first.operation.id.to_string(), different_event],
            )
            .is_err());

        connection
            .execute_batch("DROP TRIGGER conversation_created_event_requires_create_operation")
            .expect("drop linkage trigger");
        let mismatched_event_id = OutboxEventId::new();
        connection
            .execute(
                "INSERT INTO conversation_outbox (conversation_id, id, sequence, conversation_revision, operation_record_id, at, event_json) VALUES (?1, ?2, 2, 1, ?3, 10, ?4)",
                params![conversation_id.to_string(), mismatched_event_id.to_string(), send_operation_id.to_string(), event_json],
            )
            .expect("tampered event");
        drop(connection);
        assert!(matches!(
            lettuce_conversations::ConversationReader::page_outbox(
                &database,
                conversation_id,
                &lettuce_types::PageRequest::default()
            ),
            Err(ConversationRepositoryError::Storage)
        ));
    }
}
