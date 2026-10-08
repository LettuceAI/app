use std::sync::Arc;

use lettuce_characters::CharacterRepository;
use lettuce_context::{
    BindingInsertionTarget, CharacterLorebookBindingRepository, GroupLorebookBindingRepository,
    LorebookBindingCreate, LorebookRepository, PersonaLorebookBindingRepository,
};
use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_conversations::{
    ArtifactTransferError, ConversationArtifactTransferPort, ConversationReader,
    TrustedArtifactDescriptor, TrustedArtifactSink,
};
use lettuce_transfer::{
    BackupConversationArtifact, ProviderBackupGraph, ProviderBackupRestoreWriter,
    ProviderBackupSource,
};
use lettuce_types::{CharacterId, ConversationId, LorebookId, TimestampMillis};

use super::tests::{Harness, RecordingStream, Reply, harness, launch, send};
use super::turns_tests::run_generation;
use super::*;

pub(super) fn entry(title: &str, content: &str, keyword: Option<&str>) -> dto::LorebookEntryInput {
    dto::LorebookEntryInput {
        title: title.into(),
        enabled: true,
        always_active: keyword.is_none(),
        keywords: keyword
            .map(|keyword| vec![keyword.into()])
            .unwrap_or_default(),
        case_sensitive: false,
        keyword_mode: dto::LorebookKeywordMode::Literal,
        content: content.into(),
        priority: 0,
    }
}

pub(super) async fn create(
    harness: &Harness,
    key: &str,
    name: &str,
    entries: Vec<dto::LorebookEntryInput>,
) -> dto::LorebookView {
    lorebook_create(
        &harness.context,
        dto::LorebookCreateRequest {
            client_operation_id: key.into(),
            metadata: dto::LorebookMetadataInput {
                name: name.into(),
                detection: dto::LorebookDetection::RecentMessages,
                icon_asset_id: None,
            },
            entries,
        },
    )
    .await
    .expect("create lorebook")
}

pub(super) fn bind_character(harness: &Harness, character: CharacterId, book: &str) {
    let database = harness.context.backend().database();
    let revision = CharacterRepository::get(database, character)
        .expect("character")
        .expect("exists")
        .character
        .revision;
    database
        .bind_character_lorebook(
            character,
            revision,
            LorebookBindingCreate {
                lorebook_id: book.parse().expect("id"),
                target: BindingInsertionTarget::Append,
            },
            TimestampMillis::new(2),
        )
        .expect("bind");
}

struct Collected(Vec<u8>);

impl TrustedArtifactSink for Collected {
    fn begin(&mut self, _: &TrustedArtifactDescriptor) -> Result<(), ArtifactTransferError> {
        Ok(())
    }

    fn chunk(&mut self, bytes: &[u8]) -> Result<(), ArtifactTransferError> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }

    fn finish(&mut self) -> Result<(), ArtifactTransferError> {
        Ok(())
    }
}

/// Exports the database with its artifacts, restores it into an empty one
/// and returns the restored database once its graph reads back equal.
pub(super) fn backup_round_trip(
    database: &lettuce_database::Database,
) -> lettuce_database::Database {
    let mut graph: ProviderBackupGraph = database.read_provider_backup_graph().expect("export");
    lettuce_transfer::canonicalize_and_validate(&mut graph).expect("canonical graph");
    let artifacts = lettuce_transfer::provider_backup_artifact_requirements(&graph)
        .expect("artifact requirements")
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
            BackupConversationArtifact {
                descriptor,
                bytes: zeroize::Zeroizing::new(sink.0),
            }
        })
        .collect::<Vec<_>>();
    let restored = lettuce_database::Database::open_in_memory().expect("restore target");
    restored
        .restore_provider_backup_graph(&graph, &artifacts)
        .expect("restore");
    let mut round_trip = restored.read_provider_backup_graph().expect("reread");
    lettuce_transfer::canonicalize_and_validate(&mut round_trip).expect("canonical restored");
    assert_eq!(round_trip, graph);
    restored
}

pub(super) fn turn_lorebooks(
    database: &lettuce_database::Database,
    conversation: &str,
) -> Vec<(LorebookId, String)> {
    let id = conversation.parse::<ConversationId>().expect("id");
    database
        .read_provider_backup_graph()
        .expect("graph")
        .conversation_runtime
        .conversations
        .iter()
        .filter(|runtime| runtime.conversation_id == id)
        .flat_map(|runtime| runtime.turns.iter())
        .flat_map(|turn| turn.turn.lorebooks.iter())
        .map(|book| (book.lorebook_id, book.name.clone()))
        .collect()
}

#[tokio::test]
async fn create_replays_by_key_and_writes_conflict_on_stale_revisions() {
    let harness = harness(Reply::Text("ok"));
    let created = create(
        &harness,
        "book-1",
        "World",
        vec![entry("Sea", "Salt", None)],
    )
    .await;
    assert_eq!(
        create(
            &harness,
            "book-1",
            "World",
            vec![entry("Sea", "Salt", None)]
        )
        .await,
        created
    );
    let changed = lorebook_create(
        &harness.context,
        dto::LorebookCreateRequest {
            client_operation_id: "book-1".into(),
            metadata: dto::LorebookMetadataInput {
                name: "Other".into(),
                detection: dto::LorebookDetection::RecentMessages,
                icon_asset_id: None,
            },
            entries: Vec::new(),
        },
    )
    .await
    .expect_err("changed request");
    assert_eq!(changed.code, ApiErrorCode::Conflict);
    let page = lorebooks_list(&harness.context, dto::LorebooksListRequest::default())
        .await
        .expect("list");
    assert_eq!(page.items.len(), 1);

    let mutated = lorebook_entries_mutate(
        &harness.context,
        dto::LorebookEntriesMutateRequest {
            client_operation_id: "LorebookEntriesMutateRequest-180".into(),
            lorebook_id: created.lorebook.id.clone(),
            expected_revision: created.lorebook.revision,
            mutations: vec![
                dto::LorebookEntryMutationInput::Add {
                    entry: entry("Hero", "Brave", Some("hero")),
                    index: Some(0),
                },
                dto::LorebookEntryMutationInput::Remove {
                    entry_id: created.entries[0].id.clone(),
                },
            ],
        },
    )
    .await
    .expect("mutate");
    assert_eq!(mutated.lorebook.revision, created.lorebook.revision + 1);
    assert_eq!(mutated.entries.len(), 1);
    assert_eq!(mutated.entries[0].title, "Hero");

    let stale = lorebook_update_metadata(
        &harness.context,
        dto::LorebookUpdateMetadataRequest {
            client_operation_id: "LorebookUpdateMetadataRequest-202".into(),
            lorebook_id: created.lorebook.id.clone(),
            expected_revision: created.lorebook.revision,
            metadata: dto::LorebookMetadataInput {
                name: "Renamed".into(),
                detection: dto::LorebookDetection::LatestUserMessage,
                icon_asset_id: None,
            },
        },
    )
    .await
    .expect_err("stale revision");
    assert_eq!(stale.code, ApiErrorCode::Conflict);
    let failing = lorebook_entries_mutate(
        &harness.context,
        dto::LorebookEntriesMutateRequest {
            client_operation_id: "LorebookEntriesMutateRequest-218".into(),
            lorebook_id: created.lorebook.id.clone(),
            expected_revision: mutated.lorebook.revision,
            mutations: vec![
                dto::LorebookEntryMutationInput::Add {
                    entry: entry("Kept?", "No", None),
                    index: None,
                },
                dto::LorebookEntryMutationInput::Add {
                    entry: dto::LorebookEntryInput {
                        keyword_mode: dto::LorebookKeywordMode::Regex,
                        ..entry("Bad", "Regex", Some("["))
                    },
                    index: None,
                },
            ],
        },
    )
    .await
    .expect_err("invalid regex");
    assert_eq!(failing.code, ApiErrorCode::InvalidInput);
    let current = lorebook_get(
        &harness.context,
        dto::LorebookGetRequest {
            lorebook_id: created.lorebook.id.clone(),
        },
    )
    .await
    .expect("get");
    assert_eq!(current, mutated);

    let archived = lorebook_archive(
        &harness.context,
        dto::LorebookRevisionRequest {
            client_operation_id: "LorebookRevisionRequest-253".into(),
            lorebook_id: created.lorebook.id.clone(),
            expected_revision: current.lorebook.revision,
        },
    )
    .await
    .expect("archive");
    assert_eq!(archived.lorebook.status, dto::LorebookStatus::Archived);
    assert!(
        lorebooks_list(&harness.context, dto::LorebooksListRequest::default())
            .await
            .expect("list")
            .items
            .is_empty()
    );
    let restored = lorebook_restore(
        &harness.context,
        dto::LorebookRevisionRequest {
            client_operation_id: "LorebookRevisionRequest-270".into(),
            lorebook_id: created.lorebook.id.clone(),
            expected_revision: archived.lorebook.revision,
        },
    )
    .await
    .expect("restore");
    assert_eq!(restored.lorebook.status, dto::LorebookStatus::Active);
    let named = lorebooks_list(
        &harness.context,
        dto::LorebooksListRequest {
            query: Some("wor".into()),
            ..dto::LorebooksListRequest::default()
        },
    )
    .await
    .expect("search");
    assert_eq!(named.items.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn hard_delete_cleans_live_references_and_keeps_history_through_backup_and_sync() {
    let harness = harness(Reply::Text("A reply."));
    let deleted = create(
        &harness,
        "deleted",
        "Old harbour",
        vec![entry("Port", "The old harbour.", None)],
    )
    .await;
    let kept = create(
        &harness,
        "kept",
        "Kept",
        vec![entry("Tide", "The late tide.", None)],
    )
    .await;
    bind_character(&harness, harness.character_id, &deleted.lorebook.id);
    let chat = launch(&harness, "lore-chat").await;
    send(
        &harness,
        &chat,
        "lore-send",
        "Hello",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    run_generation(&harness).await;
    let database = harness.context.backend().database();
    assert_eq!(
        turn_lorebooks(database, &chat),
        vec![(
            deleted.lorebook.id.parse().expect("id"),
            "Old harbour".to_owned()
        )]
    );

    let override_chat = launch(&harness, "override-chat").await;
    conversation_settings_update(
        &harness.context,
        dto::ConversationSettingsUpdateRequest {
            conversation_id: override_chat.clone(),
            expected_settings_revision: None,
            patch: dto::ConversationSettingsPatch {
                lorebooks: Some(dto::LorebooksChange::Set {
                    ids: vec![deleted.lorebook.id.clone(), kept.lorebook.id.clone()],
                }),
                ..dto::ConversationSettingsPatch::default()
            },
        },
    )
    .await
    .expect("override");
    let lone_chat = launch(&harness, "lone-chat").await;
    conversation_settings_update(
        &harness.context,
        dto::ConversationSettingsUpdateRequest {
            conversation_id: lone_chat.clone(),
            expected_settings_revision: None,
            patch: dto::ConversationSettingsPatch {
                lorebooks: Some(dto::LorebooksChange::Set {
                    ids: vec![deleted.lorebook.id.clone()],
                }),
                ..dto::ConversationSettingsPatch::default()
            },
        },
    )
    .await
    .expect("lone override");
    let ada = crate::launch::tests::seed_named_character(database, "Ada");
    let bea = crate::launch::tests::seed_named_character(database, "Bea");
    let group = crate::launch::tests::seed_group(
        database,
        vec![
            crate::launch::tests::member(ada, 0),
            crate::launch::tests::member(bea, 1),
        ],
        None,
        |_| {},
    );
    database
        .bind_group_lorebook(
            group,
            lettuce_characters::GroupRepository::get(database, group)
                .expect("group")
                .expect("exists")
                .group
                .revision,
            LorebookBindingCreate {
                lorebook_id: deleted.lorebook.id.parse().expect("id"),
                target: BindingInsertionTarget::Append,
            },
            TimestampMillis::new(3),
        )
        .expect("group binding");
    let override_revision = ConversationReader::get(
        database,
        override_chat.parse::<ConversationId>().expect("id"),
    )
    .expect("chat")
    .conversation
    .revision;

    let removed = lorebook_delete(
        &harness.context,
        dto::LorebookRevisionRequest {
            client_operation_id: "LorebookRevisionRequest-375".into(),
            lorebook_id: deleted.lorebook.id.clone(),
            expected_revision: deleted.lorebook.revision,
        },
    )
    .await
    .expect("delete");
    assert_eq!(
        removed.character_ids,
        vec![harness.character_id.to_string()]
    );
    assert_eq!(removed.group_ids, vec![group.to_string()]);
    let mut conversations = vec![override_chat.clone(), lone_chat.clone()];
    conversations.sort();
    assert_eq!(removed.conversation_ids, conversations);
    assert!(
        database
            .list_character_bindings(harness.character_id)
            .expect("bindings")
            .is_empty()
    );
    assert!(
        database
            .list_group_bindings(group)
            .expect("bindings")
            .is_empty()
    );
    let settings = |chat: &str| {
        ConversationReader::get(database, chat.parse::<ConversationId>().expect("id"))
            .expect("chat")
            .conversation
    };
    let overridden = settings(&override_chat);
    assert!(overridden.revision > override_revision);
    let own = overridden.current_settings.expect("settings");
    assert_eq!(
        own.lorebooks
            .expect("override")
            .iter()
            .map(|book| book.source_id.to_string())
            .collect::<Vec<_>>(),
        vec![kept.lorebook.id.clone()]
    );
    assert_eq!(
        settings(&lone_chat)
            .current_settings
            .expect("settings")
            .lorebooks_provenance,
        lettuce_conversations::SettingProvenance::Disabled
    );
    let missing = lorebook_get(
        &harness.context,
        dto::LorebookGetRequest {
            lorebook_id: deleted.lorebook.id.clone(),
        },
    )
    .await
    .expect_err("deleted");
    assert_eq!(missing.code, ApiErrorCode::NotFound);
    let history = vec![(
        deleted.lorebook.id.parse::<LorebookId>().expect("id"),
        "Old harbour".to_owned(),
    )];
    assert_eq!(turn_lorebooks(database, &chat), history);

    let restored = backup_round_trip(database);
    assert_eq!(turn_lorebooks(&restored, &chat), history);
    assert!(
        LorebookRepository::get(&restored, deleted.lorebook.id.parse().expect("id"))
            .expect("read")
            .is_none()
    );

    let peer = lettuce_database::Database::open_in_memory().expect("peer");
    crate::sync::sync_exchange::tests::exchange(
        database,
        &peer,
        700,
        TimestampMillis::now().expect("clock"),
    )
    .await;
    assert_eq!(turn_lorebooks(&peer, &chat), history);
}

fn turn_entries(database: &lettuce_database::Database, conversation: &str) -> Vec<String> {
    let id = conversation.parse::<ConversationId>().expect("id");
    let graph = database.read_provider_backup_graph().expect("graph");
    let runtime = graph
        .conversation_runtime
        .conversations
        .iter()
        .find(|runtime| runtime.conversation_id == id)
        .expect("runtime");
    runtime
        .turns
        .last()
        .expect("turn")
        .turn
        .lorebooks
        .iter()
        .flat_map(|book| book.activated_entry_ids.iter().map(ToString::to_string))
        .collect()
}

async fn preview(
    harness: &Harness,
    conversation: &str,
    composer: &str,
    speaker: Option<CharacterId>,
) -> dto::LorebookTriggerPreview {
    lorebook_trigger_preview(
        &harness.context,
        dto::LorebookTriggerPreviewRequest::Conversation {
            conversation_id: conversation.into(),
            composer_text: Some(composer.into()),
            speaker_character_id: speaker.map(|id| id.to_string()),
        },
    )
    .await
    .expect("preview")
}

fn preview_ids(preview: &dto::LorebookTriggerPreview) -> Vec<String> {
    preview
        .entries
        .iter()
        .map(|entry| entry.entry_id.clone())
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn trigger_preview_matches_what_the_next_turn_injects() {
    let harness = harness(Reply::Text("A reply."));
    let database = harness.context.backend().database();
    let character_book = create(
        &harness,
        "c",
        "Character lore",
        vec![entry("Sea", "Salt sea.", None)],
    )
    .await;
    let persona_book = create(
        &harness,
        "p",
        "Persona lore",
        vec![
            entry("Dragon", "Dragons fly.", Some("dragon")),
            entry("Elf", "Elves sing.", Some("elf")),
        ],
    )
    .await;
    let own_book = create(
        &harness,
        "o",
        "Own lore",
        vec![dto::LorebookEntryInput {
            keyword_mode: dto::LorebookKeywordMode::Regex,
            ..entry("Moon", "Moon rises.", Some(r"\S+oon"))
        }],
    )
    .await;
    bind_character(&harness, harness.character_id, &character_book.lorebook.id);
    let persona = crate::launch::tests::seed_persona(database, "Traveller");
    database
        .bind_persona_lorebook(
            persona,
            lettuce_characters::PersonaRepository::get(database, persona)
                .expect("persona")
                .expect("exists")
                .revision,
            LorebookBindingCreate {
                lorebook_id: persona_book.lorebook.id.parse().expect("id"),
                target: BindingInsertionTarget::Append,
            },
            TimestampMillis::new(3),
        )
        .expect("persona binding");
    let chat = launch(&harness, "direct").await;
    conversation_settings_update(
        &harness.context,
        dto::ConversationSettingsUpdateRequest {
            conversation_id: chat.clone(),
            expected_settings_revision: None,
            patch: dto::ConversationSettingsPatch {
                persona: Some(dto::ChoiceChange::Set {
                    id: persona.to_string(),
                }),
                ..dto::ConversationSettingsPatch::default()
            },
        },
    )
    .await
    .expect("persona");
    let shown = preview(&harness, &chat, "A dragon appears", None).await;
    assert_eq!(
        shown
            .entries
            .iter()
            .map(|entry| (entry.title.as_str(), entry.source.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                "Sea",
                dto::LorebookSourceTier::Character {
                    character_id: harness.character_id.to_string()
                }
            ),
            (
                "Dragon",
                dto::LorebookSourceTier::Persona {
                    persona_id: persona.to_string()
                }
            ),
        ]
    );
    assert_eq!(shown.entries[1].matched_keywords, vec!["dragon".to_owned()]);
    assert!(shown.entries.iter().all(|entry| entry.token_count > 0));
    send(
        &harness,
        &chat,
        "direct-send",
        "A dragon appears",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    run_generation(&harness).await;
    assert_eq!(turn_entries(database, &chat), preview_ids(&shown));

    let own_chat = launch(&harness, "own").await;
    conversation_settings_update(
        &harness.context,
        dto::ConversationSettingsUpdateRequest {
            conversation_id: own_chat.clone(),
            expected_settings_revision: None,
            patch: dto::ConversationSettingsPatch {
                lorebooks: Some(dto::LorebooksChange::Set {
                    ids: vec![own_book.lorebook.id.clone()],
                }),
                ..dto::ConversationSettingsPatch::default()
            },
        },
    )
    .await
    .expect("own books");
    let own = preview(&harness, &own_chat, "THE MOON", None).await;
    assert_eq!(own.entries.len(), 1);
    assert_eq!(own.entries[0].source, dto::LorebookSourceTier::Conversation);
    send(
        &harness,
        &own_chat,
        "own-send",
        "THE MOON",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    run_generation(&harness).await;
    assert_eq!(turn_entries(database, &own_chat), preview_ids(&own));

    let editor = lorebook_trigger_preview(
        &harness.context,
        dto::LorebookTriggerPreviewRequest::Editor {
            lorebook_id: persona_book.lorebook.id.clone(),
            text: "an elf and a dragon".into(),
        },
    )
    .await
    .expect("editor");
    assert_eq!(
        editor
            .entries
            .iter()
            .map(|entry| (entry.title.as_str(), entry.source.clone()))
            .collect::<Vec<_>>(),
        vec![
            ("Dragon", dto::LorebookSourceTier::Editor),
            ("Elf", dto::LorebookSourceTier::Editor)
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn group_trigger_preview_uses_the_speakers_books_unless_disabled() {
    let harness = harness(Reply::Text("A reply."));
    let database = harness.context.backend().database();
    let cast = super::turns_tests::group_cast(&harness, "group").await;
    let group_book = create(
        &harness,
        "g",
        "Group lore",
        vec![entry("Hall", "The hall.", None)],
    )
    .await;
    let ada_book = create(
        &harness,
        "a",
        "Ada lore",
        vec![entry("Ada", "Ada's secret.", None)],
    )
    .await;
    database
        .bind_group_lorebook(
            cast.group_id,
            lettuce_characters::GroupRepository::get(database, cast.group_id)
                .expect("group")
                .expect("exists")
                .group
                .revision,
            LorebookBindingCreate {
                lorebook_id: group_book.lorebook.id.parse().expect("id"),
                target: BindingInsertionTarget::Append,
            },
            TimestampMillis::new(3),
        )
        .expect("group binding");
    bind_character(&harness, cast.ada_character, &ada_book.lorebook.id);
    let shown = preview(&harness, &cast.chat, "Hello all", Some(cast.ada_character)).await;
    assert_eq!(
        shown
            .entries
            .iter()
            .map(|entry| entry.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Hall", "Ada"]
    );
    send(
        &harness,
        &cast.chat,
        "group-send",
        "Hello all",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    run_generation(&harness).await;
    assert_eq!(turn_entries(database, &cast.chat), preview_ids(&shown));

    conversation_settings_update(
        &harness.context,
        dto::ConversationSettingsUpdateRequest {
            conversation_id: cast.chat.clone(),
            expected_settings_revision: None,
            patch: dto::ConversationSettingsPatch {
                disable_character_lorebooks: Some(dto::FlagChange::Set { value: true }),
                ..dto::ConversationSettingsPatch::default()
            },
        },
    )
    .await
    .expect("disable character lorebooks");
    let disabled = preview(
        &harness,
        &cast.chat,
        "Hello again",
        Some(cast.ada_character),
    )
    .await;
    assert_eq!(
        disabled
            .entries
            .iter()
            .map(|entry| entry.title.as_str())
            .collect::<Vec<_>>(),
        vec!["Hall"]
    );
}
