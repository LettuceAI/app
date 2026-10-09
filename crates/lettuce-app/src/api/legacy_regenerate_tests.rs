use std::sync::Arc;

use lettuce_conversations::ConversationReader;
use lettuce_database::Database;
use lettuce_platform::{DirectorySnapshot, FilesystemAuthority};
use lettuce_transfer::{LegacyBackupDocument, LegacyBackupDocumentKind, LegacyBackupInventory};
use lettuce_transfer::ProviderBackupSource;
use lettuce_types::{ContentHash, OperationId, TimestampMillis};

use super::tests::{RecordingStream, Reply, harness_over, NoImages};
use super::turns_tests::{open, regenerate_request, run_generation};
use super::*;
use crate::legacy::legacy_restore::LegacyRestoreCoordinator;

fn id(value: u128) -> String {
    uuid::Uuid::from_u128(value).to_string()
}

fn document(kind: LegacyBackupDocumentKind, value: serde_json::Value) -> LegacyBackupDocument {
    LegacyBackupDocument {
        kind,
        bytes: zeroize::Zeroizing::new(serde_json::to_vec(&value).expect("document")),
    }
}

fn message(
    message_id: &str,
    role: &str,
    parent: Option<&str>,
    content: &str,
    speaker: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "id": message_id,
        "role": role,
        "content": content,
        "speaker_character_id": speaker,
        "turn_number": 1,
        "created_at": 10,
        "effective_at": 11,
        "visible_in_chat": true,
        "scene_edited": false,
        "prompt_tokens": null,
        "completion_tokens": null,
        "total_tokens": null,
        "first_token_ms": null,
        "tokens_per_second": null,
        "mtp_stats": null,
        "model_id": null,
        "selected_variant_id": null,
        "is_pinned": false,
        "memory_refs": "[]",
        "used_lorebook_entries": "[]",
        "attachments": "[]",
        "reasoning": null,
        "parent_message_id": parent,
        "variants": []
    })
}

async fn restored_harness() -> (super::tests::Harness, Database) {
    let root = std::env::temp_dir().join(format!("legacy-regenerate-{}", OperationId::new()));
    std::fs::create_dir_all(&root).expect("root");
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
        .expect("authority");
    let location = crate::AppDatabaseLocation::new(
        root.join("private-persistent-v2"),
        &authority,
    )
    .expect("location");
    let secrets = lettuce_settings::InMemorySecretStore::new();
    let workspace = root.join("workspace");
    let coordinator = LegacyRestoreCoordinator::new(&location, &authority, &workspace, &secrets);
    let (provider, model, ada, grace) = (id(1), id(2), id(3), id(4));
    let (session, group, group_session) = (id(5), id(6), id(7));
    let members = serde_json::to_string(&[&ada, &grace]).expect("members");
    let compatibility = lettuce_transfer::plan_legacy_backup_compatibility(LegacyBackupInventory {
        version: 1,
        created_at: 1,
        app_version: "legacy".into(),
        source_hash: ContentHash::parse("ef".repeat(32)).expect("hash"),
        documents: vec![
            document(LegacyBackupDocumentKind::AudioProviders, serde_json::json!([{
                "id": id(8), "provider_type": "fish_speech", "label": "Local narrator",
                "base_url": "http://127.0.0.1:8080", "created_at": 1, "updated_at": 1
            }])),
            document(
                LegacyBackupDocumentKind::ProviderCredentials,
                serde_json::json!([{
                    "id": provider, "provider_id": "openrouter", "label": "Router",
                    "api_key_ref": null, "api_key": "secret",
                    "base_url": "https://openrouter.ai/api/v1", "default_model": null,
                    "headers": null, "config": null
                }]),
            ),
            document(
                LegacyBackupDocumentKind::Models,
                serde_json::json!([{
                    "id": model, "name": "gpt-6-astra", "provider_id": "openrouter",
                    "provider_credential_id": provider, "provider_label": "Router",
                    "display_name": "GPT-4o", "created_at": 1, "model_type": "chat",
                    "input_scopes": "[\"text\"]", "output_scopes": "[\"text\"]"
                }]),
            ),
            document(
                LegacyBackupDocumentKind::Settings,
                serde_json::json!({
                    "default_provider_credential_id": provider,
                    "default_model_id": model,
                    "app_state": {}, "advanced_model_settings": null,
                    "prompt_template_id": null, "system_prompt": null,
                    "migration_version": 92, "advanced_settings": null,
                    "created_at": 1, "updated_at": 1
                }),
            ),
            document(
                LegacyBackupDocumentKind::Characters,
                serde_json::json!([
                    {"id": ada, "name": "Ada", "created_at": 1, "updated_at": 1,
                     "voice_config": serde_json::to_string(&serde_json::json!({"source": "provider", "providerId": id(8), "voiceId": "legacy-narrator", "modelId": "server-default", "voiceName": "Narrator"})).expect("voice")},
                    {"id": grace, "name": "Grace", "created_at": 1, "updated_at": 1}
                ]),
            ),
            document(
                LegacyBackupDocumentKind::Sessions,
                serde_json::json!([{
                    "id": session, "character_id": ada, "title": "Direct",
                    "mode": "roleplay", "persona_disabled": false,
                    "lorebook_ids_override": "[]", "memories": "[]",
                    "memory_embeddings": "[]", "memory_summary_token_count": 0,
                    "memory_tool_events": "[]", "archived": false,
                    "created_at": 1, "updated_at": 20,
                    "messages": [
                        message(&id(10), "user", None, "Hello Ada", None),
                        message(&id(11), "assistant", Some(&id(10)), "Imported reply", None)
                    ]
                }]),
            ),
            document(
                LegacyBackupDocumentKind::GroupCharacters,
                serde_json::json!([{
                    "id": group, "name": "Room", "character_ids": members,
                    "muted_character_ids": "[]", "created_at": 1, "updated_at": 1,
                    "archived": false, "chat_type": "conversation",
                    "lorebook_ids": "[]", "disable_character_lorebooks": false,
                    "speaker_selection_method": "heuristic", "memory_type": "manual"
                }]),
            ),
            document(
                LegacyBackupDocumentKind::GroupSessions,
                serde_json::json!([{
                    "id": group_session, "group_character_id": group, "name": "Room chat",
                    "character_ids": members, "muted_character_ids": "[]",
                    "created_at": 1, "updated_at": 1, "archived": false,
                    "chat_type": "conversation", "lorebook_ids": "[]",
                    "disable_character_lorebooks": false, "memories": "[]",
                    "memory_embeddings": "[]", "memory_summary": "",
                    "memory_summary_token_count": 0, "memory_tool_events": "[]",
                    "speaker_selection_method": "heuristic", "memory_type": "manual",
                    "config_overrides": "{\"version\":1}", "character_model_overrides": "{}",
                    "participation": [],
                    "messages": [
                        message(&id(20), "user", None, "Hello all", None),
                        message(&id(21), "assistant", Some(&id(20)), "Who said this", None)
                    ]
                }]),
            ),
        ],
        media: Vec::new(),
    })
    .expect("plan");
    let plan = compatibility.legacy_import_plan();
    let receipt = coordinator
        .replace(
            OperationId::new(),
            &crate::legacy::LegacyDatabaseImportPlan {
                compatibility,
                plan,
                preserved: Vec::new(),
                message_conflicts: Vec::new(),
            },
            None,
            None,
            TimestampMillis::new(1_700_000_000_100),
        )
        .await
        .expect("import");
    let backend = Arc::new(
        crate::AppBackend::open(&receipt.database_path, TimestampMillis::new(1_700_000_000_200))
            .expect("backend"),
    );
    let harness = harness_over(
        backend,
        Reply::Text("Fresh reply."),
        Arc::new(lettuce_jobs::SystemClock),
        None,
        None,
        Arc::new(NoModels),
        Arc::new(NoImages),
    );
    let database = Database::open(&receipt.database_path).expect("database");
    (harness, database)
}

#[tokio::test]
async fn synthetic_import_usage_has_explicit_origin_and_live_regeneration_does_not_inherit_it() {
    let (harness, database) = restored_harness().await;
    let imported = database.read_provider_backup_graph().expect("imported graph");
    assert!(!imported.conversation_usage.events.is_empty());
    for entry in &imported.conversation_usage.events {
        assert_eq!(
            serde_json::to_value(&entry.event).expect("origin payload")["origin"],
            serde_json::json!("legacy_import")
        );
    }
    regenerate_imported(&harness, &database, 11).await;
    let graph = database.read_provider_backup_graph().expect("regenerated graph");
    let new_events = graph.conversation_usage.events.iter().filter(|entry| {
        !imported.conversation_usage.events.iter().any(|old| old.event.id == entry.event.id)
    }).collect::<Vec<_>>();
    assert_eq!(new_events.len(), 1);
    assert_eq!(
        serde_json::to_value(&new_events[0].event).expect("live payload")["origin"],
        serde_json::json!("live")
    );
    for old in &imported.conversation_usage.events {
        assert!(graph.conversation_usage.events.iter().any(|entry| entry.event == old.event));
    }
}

async fn regenerate_imported(
    harness: &super::tests::Harness,
    database: &Database,
    reply: u128,
) -> (String, String) {
    let graph = database.read_provider_backup_graph().expect("graph");
    let found = graph
        .conversation_history
        .conversations
        .iter()
        .flat_map(|conversation| {
            conversation
                .messages
                .iter()
                .map(move |message| (conversation, message))
        })
        .find(|(_, message)| {
            message.candidates.iter().any(|candidate| {
                candidate.parts.iter().any(|part| {
                    matches!(part, lettuce_conversations::MessagePart::Text { text }
                        if text == if reply == 11 { "Imported reply" } else { "Who said this" })
                })
            })
        })
        .expect("the imported reply is a candidate");
    let (conversation, reply_message) = found;
    assert_eq!(reply_message.candidates.len(), 1);
    assert!(reply_message.candidates[0].model.is_none());
    let chat = conversation.aggregate.conversation.id.to_string();
    let message_id = reply_message.message.id.to_string();
    let request = regenerate_request(harness, &chat, &message_id, &format!("regen-{reply}"));
    let stream = Arc::new(RecordingStream::default());
    let accepted = conversation_regenerate(&harness.context, request, stream)
        .await
        .expect("regenerate an imported reply");
    run_generation(harness).await;
    let turn = ConversationReader::get_turn(
        harness.context.backend().database(),
        accepted.turn_id.parse().expect("turn id"),
    )
    .expect("turn");
    assert_eq!(turn.status, lettuce_conversations::GenerationTurnStatus::Succeeded);
    (chat, message_id)
}

#[tokio::test(flavor = "multi_thread")]
async fn an_imported_reply_without_variants_regenerates_and_keeps_its_text_as_the_first_variant() {
    let (harness, database) = restored_harness().await;
    let (chat, reply) = regenerate_imported(&harness, &database, 11).await;
    let view = open(&harness, &chat).await;
    let shown = view
        .messages
        .items
        .iter()
        .find(|message| message.id == reply)
        .expect("reply");
    assert_eq!(shown.candidate_count, 2);
    assert_eq!(shown.candidate_index, Some(1));
    let aggregate = ConversationReader::get(
        harness.context.backend().database(),
        chat.parse().expect("id"),
    )
    .expect("conversation");
    assert!(aggregate.conversation.revision.get() > 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_speakerless_group_reply_imports_and_regenerates_through_speaker_selection() {
    let (harness, database) = restored_harness().await;
    let graph = database.read_provider_backup_graph().expect("graph");
    let group = graph
        .conversation_history
        .conversations
        .iter()
        .find(|conversation| {
            matches!(
                conversation.aggregate.conversation.kind,
                lettuce_conversations::ConversationKind::Group(_)
            )
        })
        .expect("the group conversation imported");
    let unknown = group
        .aggregate.conversation
        .participants
        .iter()
        .find(|participant| !participant.enabled && participant.display_name == "Unknown")
        .expect("a disabled unknown participant");
    let original_author = unknown.id;
    let (chat, reply) = regenerate_imported(&harness, &database, 21).await;
    let view = open(&harness, &chat).await;
    let shown = view
        .messages
        .items
        .iter()
        .find(|message| message.id == reply)
        .expect("reply");
    assert_eq!(shown.candidate_count, 2);
    let conversation = ConversationReader::get(
        harness.context.backend().database(),
        chat.parse().expect("id"),
    )
    .expect("conversation");
    let speaker = conversation
        .conversation
        .participants
        .iter()
        .find(|participant| Some(participant.id.to_string()) == shown.author_participant_id);
    assert!(speaker.is_some_and(|speaker| speaker.enabled && speaker.id != original_author));
}

struct Collected(Vec<u8>);

impl lettuce_conversations::TrustedArtifactSink for Collected {
    fn begin(
        &mut self,
        _: &lettuce_conversations::TrustedArtifactDescriptor,
    ) -> Result<(), lettuce_conversations::ArtifactTransferError> {
        Ok(())
    }

    fn chunk(&mut self, bytes: &[u8]) -> Result<(), lettuce_conversations::ArtifactTransferError> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }

    fn finish(&mut self) -> Result<(), lettuce_conversations::ArtifactTransferError> {
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn imported_candidates_and_the_unknown_speaker_survive_a_backup_round_trip() {
    use lettuce_conversations::{ConversationArtifactTransferPort, TrustedArtifactDescriptor};
    use lettuce_transfer::ProviderBackupRestoreWriter;
    let (_harness, database) = restored_harness().await;
    let mut graph = database.read_provider_backup_graph().expect("graph");
    lettuce_transfer::canonicalize_and_validate(&mut graph).expect("canonical graph");
    let artifacts = lettuce_transfer::provider_backup_artifact_requirements(&graph)
        .expect("requirements")
        .into_iter()
        .map(|descriptor| {
            let mut sink = Collected(Vec::new());
            match &descriptor {
                TrustedArtifactDescriptor::Snapshot(reference) => {
                    database.export_snapshot(reference.artifact_id, &mut sink)
                }
                TrustedArtifactDescriptor::Replay(reference) => {
                    database.export_replay(reference.artifact_id, &mut sink)
                }
            }
            .expect("export artifact");
            lettuce_transfer::BackupConversationArtifact {
                descriptor,
                bytes: zeroize::Zeroizing::new(sink.0),
            }
        })
        .collect::<Vec<_>>();
    let target = Database::open_in_memory().expect("target");
    target
        .restore_provider_backup_graph(&graph, &artifacts)
        .expect("restore graph");
    let mut round_trip = target.read_provider_backup_graph().expect("read restored");
    lettuce_transfer::canonicalize_and_validate(&mut round_trip).expect("canonical restored");
    assert_eq!(round_trip, graph);
    let imported = round_trip
        .conversation_history
        .conversations
        .iter()
        .flat_map(|conversation| &conversation.messages)
        .flat_map(|message| &message.candidates)
        .filter(|candidate| candidate.model.is_none())
        .count();
    assert_eq!(imported, 2);
    let secrets = lettuce_transfer::provider_backup_secret_requirements(&graph).expect("secrets")
        .into_iter().map(|(reference, purpose)| lettuce_transfer::ProviderBackupSecret {
            reference, purpose, generation: 1,
            value: lettuce_settings::SecretValue::new("backup-origin-test").expect("secret"),
        }).collect();
    let sections = lettuce_transfer::provider_backup_sections(graph, secrets, Vec::new(), artifacts)
        .expect("sections");
    let sealed = lettuce_transfer::seal_backup("origin-test", lettuce_types::TimestampMillis::new(10),
        "password", sections.clone()).expect("sealed");
    let decoded = lettuce_transfer::decode_provider_backup_restore_plan(std::io::Cursor::new(sealed), "password")
        .expect("origin-bearing backup decodes");
    assert_eq!(decoded.graph.conversation_usage.version, 3);
    assert!(!decoded.graph.conversation_usage.events.is_empty());
    for version in [1, 2] {
        let mut old_sections = sections.clone();
        let usage = old_sections.iter_mut().find(|section| section.name == "data/conversation-usage.json")
            .expect("usage section");
        let mut payload: serde_json::Value = serde_json::from_slice(&usage.bytes).expect("usage JSON");
        payload["version"] = serde_json::json!(version);
        for event in payload["events"].as_array_mut().expect("events") {
            event["event"].as_object_mut().expect("event").remove("origin");
        }
        usage.schema = format!("conversation-usage.v{version}");
        usage.bytes = zeroize::Zeroizing::new(serde_json::to_vec(&payload).expect("old usage JSON"));
        let sealed = lettuce_transfer::seal_backup("origin-test", lettuce_types::TimestampMillis::new(10),
            "password", old_sections).expect("sealed old backup");
        assert!(matches!(lettuce_transfer::decode_provider_backup_restore_plan(std::io::Cursor::new(sealed), "password"),
            Err(lettuce_transfer::ProviderBackupRestorePlanError::InvalidInventory)));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn message_playback_resolves_the_provider_voice_from_a_legacy_restore() {
    use lettuce_speech::SynthesisRepository;
    let (harness, database) = restored_harness().await;
    let graph = database.read_provider_backup_graph().expect("imported graph");
    let message = graph.conversation_history.conversations.iter().flat_map(|conversation| &conversation.messages)
        .find(|message| message.candidates.iter().any(|candidate| candidate.parts.iter().any(|part|
            matches!(part, lettuce_conversations::MessagePart::Text { text } if text == "Imported reply"))))
        .expect("imported assistant message");
    let accepted = message_speak(&harness.context, lettuce_contracts::MessageSpeakRequest {
        request_id: lettuce_types::RequestId::new().to_string(), message_id: message.message.id.to_string(),
        voice_override: None, swap_places: false,
    }).await.expect("play imported message");
    let frozen = SynthesisRepository::get(&database, accepted.job_id.parse().expect("job")).expect("synthesis");
    assert_eq!(frozen.request.voice_id, "legacy-narrator");
    assert_eq!(frozen.request.provider.label, "Local narrator");
    assert_eq!(frozen.request.model_id, "server-default");
    assert_eq!(frozen.request.text, "Imported reply");
}
