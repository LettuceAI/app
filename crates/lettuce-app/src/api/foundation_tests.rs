use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiErrorCode, ApiErrorDetails};
use lettuce_jobs::SystemClock;
use lettuce_types::{OperationId, TimestampMillis};
use serde_json::json;

use super::tests::{Reply, harness, harness_in, media_store, png_bytes};
use super::{
    app_status, app_ui_state_update, assets_ingest, files_inspect, purge_notice_dismiss,
    purge_notices_list,
};

fn temp_root(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("lettuce-api-{label}-{}", OperationId::new()));
    std::fs::create_dir_all(&root).expect("root");
    root
}

fn wav_bytes() -> Vec<u8> {
    let samples = [0_u8; 8];
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&(36 + samples.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&8_000_u32.to_le_bytes());
    bytes.extend_from_slice(&16_000_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(samples.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&samples);
    bytes
}

fn source(path: &std::path::Path) -> dto::FileSource {
    dto::FileSource {
        uri: path.display().to_string(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn files_inspect_detects_what_a_file_holds() {
    let harness = harness(Reply::Text("Hello."));
    let root = temp_root("inspect");
    let card = json!({
        "spec": "chara_card_v2",
        "spec_version": "2.0",
        "data": {"name": "Ada", "description": "An engineer"}
    });
    let persona = json!({
        "version": 1,
        "persona": {"title": "Me", "description": "The user"},
        "avatarData": null
    });
    let lorebook = json!({"name": "World", "entries": {"0": {"key": ["a"], "content": "A"}}});
    let preset = json!({
        "prompts": [{"identifier": "main", "name": "Main", "content": "Main prompt"}],
        "prompt_order": [{"character_id": 100001, "order": [{"identifier": "main", "enabled": true}]}]
    });
    let chat = "{\"user_name\":\"User\",\"character_name\":\"Ada\"}\n{\"name\":\"Ada\",\"is_user\":false,\"mes\":\"Hi\"}\n";
    let mut backup = b"LETTUCE-BACKUP2\0".to_vec();
    backup.extend_from_slice(&[0; 32]);
    let cases: Vec<(&str, Vec<u8>, dto::FileKind)> = vec![
        (
            "card.json",
            card.to_string().into_bytes(),
            dto::FileKind::CharacterCard,
        ),
        (
            "persona.json",
            persona.to_string().into_bytes(),
            dto::FileKind::PersonaFile,
        ),
        (
            "world.json",
            lorebook.to_string().into_bytes(),
            dto::FileKind::Lorebook,
        ),
        (
            "preset.json",
            preset.to_string().into_bytes(),
            dto::FileKind::PromptPreset,
        ),
        (
            "chat.jsonl",
            chat.as_bytes().to_vec(),
            dto::FileKind::ChatJsonl,
        ),
        ("backup.lettuce", backup, dto::FileKind::BackupV2),
        (
            "old.zip",
            b"PK\x03\x04rest of a zip".to_vec(),
            dto::FileKind::BackupV1,
        ),
        (
            "app.db",
            b"SQLite format 3\0rest".to_vec(),
            dto::FileKind::LegacyDatabase,
        ),
        (
            "model.gguf",
            b"GGUF\x03\0\0\0".to_vec(),
            dto::FileKind::GgufModel,
        ),
        ("picture.png", png_bytes(), dto::FileKind::Image),
        ("voice.wav", wav_bytes(), dto::FileKind::Audio),
        (
            "notes.txt",
            b"just some notes".to_vec(),
            dto::FileKind::Other,
        ),
    ];
    for (name, bytes, kind) in cases {
        let path = root.join(name);
        std::fs::write(&path, &bytes).expect("write");
        let inspection = files_inspect(
            &harness.context,
            dto::FilesInspectRequest {
                source: source(&path),
            },
        )
        .await
        .expect("inspect");
        assert_eq!(
            inspection,
            dto::FileInspection {
                name: name.into(),
                size: bytes.len() as u64,
                kind,
            },
            "{name}"
        );
    }
    let missing = files_inspect(
        &harness.context,
        dto::FilesInspectRequest {
            source: source(&root.join("missing.json")),
        },
    )
    .await
    .expect_err("missing");
    assert_eq!(missing.code, ApiErrorCode::NotFound);
    let empty = files_inspect(
        &harness.context,
        dto::FilesInspectRequest {
            source: dto::FileSource { uri: " ".into() },
        },
    )
    .await
    .expect_err("empty");
    assert_eq!(
        empty.details,
        Some(ApiErrorDetails::InvalidField {
            field: "source".into()
        })
    );
    std::fs::remove_dir_all(root).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn assets_ingest_stores_media_the_role_accepts() {
    let root = temp_root("ingest");
    let store = media_store(&root.join("media"));
    let harness = harness_in(
        Reply::Text("Hello."),
        Arc::new(SystemClock),
        Some(store),
        None,
        Arc::new(super::NoModels),
    );
    let image = root.join("avatar.png");
    std::fs::write(&image, png_bytes()).expect("image");
    let asset = assets_ingest(
        &harness.context,
        dto::AssetsIngestRequest {
            source: source(&image),
            role: dto::AssetIngestRole::Avatar,
        },
    )
    .await
    .expect("ingest");
    assert_eq!(asset.url, format!("test-asset://host/{}", asset.asset_id));

    let audio = root.join("voice.wav");
    std::fs::write(&audio, wav_bytes()).expect("audio");
    assets_ingest(
        &harness.context,
        dto::AssetsIngestRequest {
            source: source(&audio),
            role: dto::AssetIngestRole::Attachment,
        },
    )
    .await
    .expect("audio attachment");
    let wrong = assets_ingest(
        &harness.context,
        dto::AssetsIngestRequest {
            source: source(&audio),
            role: dto::AssetIngestRole::Background,
        },
    )
    .await
    .expect_err("audio is not a background");
    assert_eq!(
        wrong.details,
        Some(ApiErrorDetails::InvalidField {
            field: "source".into()
        })
    );
    let text = root.join("notes.txt");
    std::fs::write(&text, b"not media").expect("text");
    let error = assets_ingest(
        &harness.context,
        dto::AssetsIngestRequest {
            source: source(&text),
            role: dto::AssetIngestRole::ReferenceImage,
        },
    )
    .await
    .expect_err("not media");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);

    let without_store = harness_in(
        Reply::Text("Hello."),
        Arc::new(SystemClock),
        None,
        None,
        Arc::new(super::NoModels),
    );
    let unavailable = assets_ingest(
        &without_store.context,
        dto::AssetsIngestRequest {
            source: source(&image),
            role: dto::AssetIngestRole::Avatar,
        },
    )
    .await
    .expect_err("no store");
    assert_eq!(unavailable.code, ApiErrorCode::Unavailable);
    std::fs::remove_dir_all(root).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn app_status_reports_the_build_and_what_needs_attention() {
    let harness = harness(Reply::Text("Hello."));
    let context = &harness.context;
    let status = app_status(context).await.expect("status");
    assert_eq!(
        status.version,
        crate::app_version(env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(
        status.build_variant == dto::BuildVariant::Cuda,
        status.version.ends_with("-cuda")
    );
    if cfg!(target_os = "linux") {
        assert_eq!(status.platform, dto::AppPlatform::Linux);
    }
    assert!(status.ui_state.is_empty());
    assert!(!status.legacy_database_detected);
    assert_eq!(
        (status.unresolved_sync_conflicts, status.purge_notices),
        (0, 0)
    );

    let mut patch = serde_json::Map::new();
    patch.insert("onboarding".into(), json!({"step": 2}));
    patch.insert("last_route".into(), json!("/chats"));
    let view = app_ui_state_update(context, dto::AppUiStateUpdateRequest { patch })
        .await
        .expect("update");
    assert_eq!(view.state.len(), 2);
    let mut patch = serde_json::Map::new();
    patch.insert("last_route".into(), serde_json::Value::Null);
    let view = app_ui_state_update(context, dto::AppUiStateUpdateRequest { patch })
        .await
        .expect("remove");
    assert_eq!(view.state.get("onboarding"), Some(&json!({"step": 2})));
    assert!(!view.state.contains_key("last_route"));
    let status = app_status(context).await.expect("status");
    assert_eq!(status.ui_state, view.state);
    let mut patch = serde_json::Map::new();
    patch.insert(" ".into(), json!(true));
    let error = app_ui_state_update(context, dto::AppUiStateUpdateRequest { patch })
        .await
        .expect_err("blank key");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
}

#[tokio::test(flavor = "multi_thread")]
async fn purge_notices_are_listed_counted_and_dismissed() {
    let harness = harness(Reply::Text("Hello."));
    let context = &harness.context;
    context
        .backend()
        .database()
        .record_media_collection_skipped("old.sqlite3", TimestampMillis::new(5))
        .expect("notice");
    let list = purge_notices_list(context).await.expect("list");
    assert_eq!(list.items.len(), 1);
    let notice = &list.items[0];
    assert_eq!(notice.entity, dto::PurgeNoticeEntityDto::DatabaseFile);
    assert_eq!(notice.entity_id, "old.sqlite3");
    assert_eq!(
        notice.reason,
        dto::PurgeNoticeReasonDto::MediaCollectionSkipped
    );
    assert_eq!(notice.recorded_at, 5);
    assert_eq!(app_status(context).await.expect("status").purge_notices, 1);

    purge_notice_dismiss(
        context,
        dto::PurgeNoticeDismissRequest {
            id: notice.id.clone(),
        },
    )
    .await
    .expect("dismiss");
    assert!(
        purge_notices_list(context)
            .await
            .expect("list")
            .items
            .is_empty()
    );
    let again = purge_notice_dismiss(
        context,
        dto::PurgeNoticeDismissRequest {
            id: notice.id.clone(),
        },
    )
    .await
    .expect_err("already dismissed");
    assert_eq!(again.code, ApiErrorCode::NotFound);
    let invalid = purge_notice_dismiss(context, dto::PurgeNoticeDismissRequest { id: "x".into() })
        .await
        .expect_err("invalid");
    assert_eq!(invalid.code, ApiErrorCode::InvalidInput);
}
