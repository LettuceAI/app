use lettuce_companions::{
    CompanionEmotionDelta, CompanionSignalChanges, EmotionVector, EmotionalState,
    GroupCompanionCheckpoint, GroupCompanionEffect, GroupCompanionEffectReason,
    GroupCompanionMessageSource, GroupCompanionRelationshipChange, GroupCompanionTarget,
    GroupMemberRelationship, RelationshipDelta, RelationshipState,
};
use lettuce_sync::{
    ChangeOperation, IncomingChangeRepository, LocalChangeJournal, NewCanonicalChange, SyncEntity,
};
use lettuce_types::{
    CharacterId, ConversationBranchId, ConversationId, MessageId, MessageRevisionId, OperationId,
    TimestampMillis,
};
use rusqlite::params;

use crate::Database;

struct Fixture {
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
    message_id: MessageId,
    characters: [CharacterId; 2],
}

fn seed(database: &Database, fixture: &Fixture) {
    let message_revision_id = MessageRevisionId::new();
    let mut connection = database.connection().expect("connection");
    let tx = connection.transaction().expect("transaction");
    for (character_id, name) in [
        (fixture.characters[0], "First"),
        (fixture.characters[1], "Second"),
    ] {
        tx.execute(
            "INSERT INTO characters (
               id, status, name, normalized_name, profile_json,
               provenance_json, defaults_json, interaction_mode,
               memory_policy, voice_autoplay, presentation_json, revision,
               created_at, updated_at
             ) VALUES (?1, 'active', ?2, ?3, '{}', '{}', '{}',
                       'companion', 'manual', 0, '{}', 1, 1, 1)",
            params![character_id.to_string(), name, name.to_ascii_lowercase()],
        )
        .expect("character");
    }
    tx.execute(
        "INSERT INTO conversations (
           id, kind, lifecycle, title, active_branch_id, kind_json,
           revision, created_at, updated_at
         ) VALUES (?1, 'group', 'active', 'Group', ?2,
                   '{\"format_version\":1}', 1, 1, 1)",
        params![
            fixture.conversation_id.to_string(),
            fixture.branch_id.to_string()
        ],
    )
    .expect("conversation");
    tx.execute(
        "INSERT INTO conversation_branches (
           conversation_id, id, status, revision, created_at, updated_at
         ) VALUES (?1, ?2, 'active', 1, 1, 1)",
        params![
            fixture.conversation_id.to_string(),
            fixture.branch_id.to_string()
        ],
    )
    .expect("branch");
    tx.execute(
        "INSERT INTO conversation_messages (
           conversation_id, id, branch_id, author_participant_id, role,
           timeline_ordinal, logical_time, effective_time, visibility, pinned,
           scene_edited, active_revision_id, revision, created_at, updated_at
         ) VALUES (?1, ?2, ?3, NULL, 'system', 1, 10, 10, 'visible', 0, 0, ?4, 1, 10, 10)",
        params![
            fixture.conversation_id.to_string(),
            fixture.message_id.to_string(),
            fixture.branch_id.to_string(),
            message_revision_id.to_string()
        ],
    )
    .expect("message");
    tx.execute(
        "INSERT INTO conversation_message_revisions (
           conversation_id, id, message_id, branch_id, sequence, parts_json,
           authored_at
         ) VALUES (?1, ?2, ?3, ?4, 1, '{\"format_version\":1}', 10)",
        params![
            fixture.conversation_id.to_string(),
            message_revision_id.to_string(),
            fixture.message_id.to_string(),
            fixture.branch_id.to_string()
        ],
    )
    .expect("revision");
    tx.commit().expect("commit");
}

fn state() -> EmotionalState {
    EmotionalState {
        felt: EmotionVector {
            warmth: 0.4,
            ..EmotionVector::default()
        },
        expressed: EmotionVector {
            warmth: 0.3,
            ..EmotionVector::default()
        },
        blocked: EmotionVector::default(),
        momentum: EmotionVector {
            warmth: 0.1,
            ..EmotionVector::default()
        },
        active_drivers: vec!["first".into()],
        confidence: 0.6,
        updated_at: TimestampMillis::new(10),
    }
}

fn relationship(target_character_id: CharacterId) -> GroupMemberRelationship {
    GroupMemberRelationship {
        target_character_id,
        state: RelationshipState {
            closeness: 0.2,
            trust: 0.3,
            affection: 0.1,
            tension: 0.05,
            stability: 0.7,
            interaction_count: 2,
            last_interaction_at: TimestampMillis::new(10),
        },
    }
}

fn effect(target: CharacterId, signal: &str) -> GroupCompanionEffect {
    GroupCompanionEffect {
        reason: GroupCompanionEffectReason::Witnessed,
        social_event: None,
        emotion_delta: CompanionEmotionDelta::default(),
        relationship_change: Some(GroupCompanionRelationshipChange {
            target: GroupCompanionTarget::Character {
                character_id: target,
            },
            delta: RelationshipDelta::default(),
        }),
        signal_changes: CompanionSignalChanges {
            added: vec![signal.into()],
            removed: Vec::new(),
        },
        by_user: false,
    }
}

fn checkpoint(
    message_id: MessageId,
    character_id: CharacterId,
    target: CharacterId,
    signal: &str,
) -> GroupCompanionCheckpoint {
    GroupCompanionCheckpoint {
        message_id,
        character_id,
        source: GroupCompanionMessageSource {
            revision_id: Some(MessageRevisionId::new()),
            candidate_id: None,
        },
        emotional_state: state(),
        member_relationships: vec![relationship(target)],
        effects: vec![effect(target, signal)],
        created_at: TimestampMillis::new(10),
    }
}

fn sync_to(from: &Database, to: &Database, at: i64) {
    let batch = from
        .outbound_changes(
            &to.local_frontier().expect("target frontier"),
            lettuce_sync::MAX_OUTBOUND_CHANGES,
            lettuce_sync::MAX_OUTBOUND_PAYLOAD_BYTES,
        )
        .expect("outbound");
    let id = lettuce_types::OperationId::new();
    to.stage_incoming_batch(
        lettuce_sync::SyncDeviceId::new(),
        id,
        &lettuce_sync::canonical_batch_hash(&batch.changes),
        &batch.changes,
        TimestampMillis::new(at),
    )
    .expect("stage");
    let result = to
        .apply_incoming_batch(id, TimestampMillis::new(at + 1))
        .expect("apply");
    assert_eq!(result.state, lettuce_sync::IncomingBatchState::Committed);
}

#[test]
fn group_companion_message_round_trips_checkpoints_effects_and_applications() {
    use lettuce_companions::GroupCompanionRepository;

    let source = Database::open_in_memory().expect("source");
    let target = Database::open_in_memory().expect("target");
    let fixture = Fixture {
        conversation_id: ConversationId::new(),
        branch_id: ConversationBranchId::new(),
        message_id: MessageId::new(),
        characters: [CharacterId::new(), CharacterId::new()],
    };
    seed(&source, &fixture);
    seed(&target, &fixture);
    let [first, second] = fixture.characters;
    let checkpoints = vec![
        checkpoint(fixture.message_id, first, second, "first"),
        checkpoint(fixture.message_id, second, first, "second"),
    ];
    {
        let mut connection = source.connection().expect("connection");
        let tx = connection.transaction().expect("transaction");
        crate::conversation::group_companion_adapter::record_persona_application_in(
            &tx,
            fixture.conversation_id,
            fixture.message_id,
            first,
            "persona-one",
            TimestampMillis::new(20),
        )
        .expect("application");
        crate::conversation::group_companion_adapter::replace_message_checkpoints_in(
            &tx,
            fixture.conversation_id,
            fixture.message_id,
            &checkpoints,
        )
        .expect("checkpoints");
        tx.commit().expect("commit");
    }
    let entity_id = format!("{}:{}", fixture.conversation_id, fixture.message_id);
    let payload = {
        let connection = source.connection().expect("connection");
        let message = crate::sync::group_companion_sync_adapter::sync_load_group_companion_message(
            &connection,
            &entity_id,
        )
        .expect("load")
        .expect("message");
        crate::sync::group_companion_sync_adapter::canonical_group_companion_message_payload(
            &message,
        )
        .expect("payload")
    };
    source
        .record_local_change(
            OperationId::new(),
            NewCanonicalChange::new(
                SyncEntity::new(lettuce_sync::GROUP_COMPANION_MESSAGE_SYNC_KIND, entity_id)
                    .expect("entity"),
                ChangeOperation::Insert,
                None,
                Some(payload),
            )
            .expect("change"),
            TimestampMillis::new(90),
        )
        .expect("journal");

    sync_to(&source, &target, 100);

    let mut expected = checkpoints.clone();
    expected.sort_by_key(|value| value.character_id.to_string());
    assert_eq!(
        source
            .checkpoints_for_message(fixture.conversation_id, fixture.message_id)
            .expect("source checkpoints"),
        expected
    );
    assert_eq!(
        target
            .checkpoints_for_message(fixture.conversation_id, fixture.message_id)
            .expect("target checkpoints"),
        expected
    );
    let applications = |database: &Database| {
        database
            .connection()
            .expect("connection")
            .prepare(
                "SELECT character_id, persona_key, applied_at
                 FROM group_companion_persona_applications
                 WHERE conversation_id = ?1 AND message_id = ?2
                 ORDER BY character_id",
            )
            .expect("statement")
            .query_map(
                params![
                    fixture.conversation_id.to_string(),
                    fixture.message_id.to_string()
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .expect("rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("applications")
    };
    assert_eq!(applications(&source), applications(&target));
    assert_eq!(
        applications(&target),
        vec![(first.to_string(), "persona-one".to_owned(), 20)]
    );
}