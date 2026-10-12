use crate::api::*;
use lettuce_contracts::{self as dto, ApiErrorCode};

#[tokio::test]
async fn library_media_upload_replays_after_source_disappears_and_closes_reference_gaps() {
    let root = std::env::temp_dir().join(format!(
        "slice5-library-media-{}",
        lettuce_types::RequestId::new()
    ));
    let h = super::media_library_tests::harness(&root);
    let source = root.join("picked.png");
    std::fs::write(&source, crate::api::tests::png_bytes()).expect("source");
    let request = dto::CharacterMediaSetRequest {
        character_id: h.character_id.to_string(),
        slot: dto::LibraryCharacterMediaSlot::Banner,
        ordinal: 0,
        input: dto::LibraryMediaInput::Source {
            source: dto::FileSource {
                uri: source.to_string_lossy().into_owned(),
            },
        },
        expected_revision: 1,
        client_operation_id: "banner-upload".into(),
    };
    let banner = character_media_set(&h.context, request.clone())
        .await
        .expect("upload");
    assert_eq!(banner.revision, 2);
    let asset = banner.draft.media[0].asset_id.clone();
    std::fs::remove_file(source).expect("source gone");
    assert_eq!(
        character_media_set(&h.context, request.clone())
            .await
            .expect("replay"),
        banner
    );
    let mut changed = request.clone();
    changed.ordinal = 1;
    assert_eq!(
        character_media_set(&h.context, changed)
            .await
            .expect_err("digest")
            .code,
        ApiErrorCode::Conflict
    );
    let mut stale = request;
    stale.client_operation_id = "stale-media".into();
    stale.input = dto::LibraryMediaInput::Asset {
        asset_id: asset.clone(),
    };
    assert_eq!(
        character_media_set(&h.context, stale)
            .await
            .expect_err("stale")
            .code,
        ApiErrorCode::Conflict
    );
    let reference = character_media_set(
        &h.context,
        dto::CharacterMediaSetRequest {
            character_id: h.character_id.to_string(),
            slot: dto::LibraryCharacterMediaSlot::DesignReference,
            ordinal: 0,
            input: dto::LibraryMediaInput::Asset {
                asset_id: asset.clone(),
            },
            expected_revision: 2,
            client_operation_id: "duplicate-media".into(),
        },
    )
    .await
    .expect_err("same asset cannot occupy two slots");
    assert_eq!(reference.code, ApiErrorCode::InvalidInput);
    let cleared = character_media_set(
        &h.context,
        dto::CharacterMediaSetRequest {
            character_id: h.character_id.to_string(),
            slot: dto::LibraryCharacterMediaSlot::Banner,
            ordinal: 0,
            input: dto::LibraryMediaInput::None,
            expected_revision: 2,
            client_operation_id: "clear-banner".into(),
        },
    )
    .await
    .expect("clear");
    assert_eq!(cleared.revision, 3);
    assert!(cleared.draft.media.is_empty());
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn library_media_design_references_use_ordinals_for_characters_and_personas() {
    let root = std::env::temp_dir().join(format!(
        "slice5-media-order-{}",
        lettuce_types::RequestId::new()
    ));
    let h = super::media_library_tests::harness(&root);
    let source = root.join("picked.png");
    std::fs::write(&source, crate::api::tests::png_bytes()).expect("source");
    let input = || dto::LibraryMediaInput::Source {
        source: dto::FileSource {
            uri: source.to_string_lossy().into_owned(),
        },
    };
    let request = |ordinal, revision, key: &str, input| dto::CharacterMediaSetRequest {
        character_id: h.character_id.to_string(),
        slot: dto::LibraryCharacterMediaSlot::DesignReference,
        ordinal,
        input,
        expected_revision: revision,
        client_operation_id: key.into(),
    };
    let first = character_media_set(&h.context, request(0, 1, "reference-0", input()))
        .await
        .expect("append 0");
    let second = character_media_set(
        &h.context,
        request(1, first.revision, "reference-1", input()),
    )
    .await
    .expect("append 1");
    assert_ne!(
        second.draft.media[0].asset_id,
        second.draft.media[1].asset_id
    );
    let kept = second.draft.media[1].asset_id.clone();
    let removed = character_media_set(
        &h.context,
        request(0, second.revision, "remove-0", dto::LibraryMediaInput::None),
    )
    .await
    .expect("remove 0");
    assert_eq!(removed.draft.media.len(), 1);
    assert_eq!(removed.draft.media[0].ordinal, 0);
    assert_eq!(removed.draft.media[0].asset_id, kept);
    assert_eq!(
        character_media_set(&h.context, request(2, removed.revision, "gap", input()))
            .await
            .expect_err("gap")
            .code,
        ApiErrorCode::InvalidInput
    );
    let persona = crate::launch::tests::seed_persona(h.context.backend().database(), "Persona");
    let request = |ordinal, revision, key: &str, input| dto::PersonaMediaSetRequest {
        persona_id: persona.to_string(),
        slot: dto::PersonaMediaSlot::DesignReference,
        ordinal,
        input,
        expected_revision: revision,
        client_operation_id: key.into(),
    };
    let first = persona_media_set(
        &h.context,
        request(
            0,
            1,
            "persona-reference-0",
            dto::LibraryMediaInput::Asset { asset_id: kept },
        ),
    )
    .await
    .expect("shared reference");
    let second = persona_media_set(
        &h.context,
        request(1, first.revision, "persona-reference-1", input()),
    )
    .await
    .expect("append 1");
    let kept = second.draft.media[1].asset_id.clone();
    let removed = persona_media_set(
        &h.context,
        request(
            0,
            second.revision,
            "persona-remove-0",
            dto::LibraryMediaInput::None,
        ),
    )
    .await
    .expect("remove 0");
    assert_eq!(removed.draft.media.len(), 1);
    assert_eq!(removed.draft.media[0].ordinal, 0);
    assert_eq!(removed.draft.media[0].asset_id, kept);
    assert_eq!(
        persona_media_set(
            &h.context,
            request(2, removed.revision, "persona-gap", input())
        )
        .await
        .expect_err("gap")
        .code,
        ApiErrorCode::InvalidInput
    );
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn library_media_missing_files_and_shutdown_fail_without_writes() {
    let h = crate::api::tests::harness(crate::api::tests::Reply::Text("reply"));
    let request = dto::CharacterMediaSetRequest {
        character_id: h.character_id.to_string(),
        slot: dto::LibraryCharacterMediaSlot::AvatarOriginal,
        ordinal: 0,
        input: dto::LibraryMediaInput::Source {
            source: dto::FileSource {
                uri: "/missing-picked.png".into(),
            },
        },
        expected_revision: 1,
        client_operation_id: "missing-store".into(),
    };
    assert_eq!(
        character_media_set(&h.context, request.clone())
            .await
            .expect_err("store")
            .code,
        ApiErrorCode::Unavailable
    );
    h.context.shutdown_token().cancel();
    assert_eq!(
        character_media_set(&h.context, request)
            .await
            .expect_err("shutdown")
            .code,
        ApiErrorCode::Cancelled
    );
    assert!(
        h.context
            .backend()
            .database()
            .lookup_api_operation("character_media_set", "missing-store")
            .expect("receipt")
            .is_none()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn library_media_concurrent_uploads_have_one_cas_winner_and_one_catalog_asset() {
    use lettuce_transfer::ProviderBackupSource;
    let root = std::env::temp_dir().join(format!(
        "slice5-media-race-{}",
        lettuce_types::RequestId::new()
    ));
    let h = super::media_library_tests::harness(&root);
    let source = root.join("picked.png");
    std::fs::write(&source, crate::api::tests::png_bytes()).expect("source");
    let request = dto::CharacterMediaSetRequest {
        character_id: h.character_id.to_string(),
        slot: dto::LibraryCharacterMediaSlot::AvatarOriginal,
        ordinal: 0,
        input: dto::LibraryMediaInput::Source {
            source: dto::FileSource {
                uri: source.to_string_lossy().into_owned(),
            },
        },
        expected_revision: 1,
        client_operation_id: "upload-race-a".into(),
    };
    let (a, b) = tokio::join!(
        character_media_set(&h.context, request.clone()),
        character_media_set(
            &h.context,
            dto::CharacterMediaSetRequest {
                client_operation_id: "upload-race-b".into(),
                ..request
            }
        )
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert_eq!(
        a.err().or_else(|| b.err()).expect("loser").code,
        ApiErrorCode::Conflict
    );
    let graph = h
        .context
        .backend()
        .database()
        .read_provider_backup_graph()
        .expect("catalog");
    assert_eq!(graph.authored.media_assets.len(), 1);
    assert_eq!(graph.authored.media_blobs.len(), 1);
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn library_media_invalid_or_missing_source_preserves_the_original_view() {
    let root = std::env::temp_dir().join(format!(
        "slice5-media-invalid-{}",
        lettuce_types::RequestId::new()
    ));
    let h = super::media_library_tests::harness(&root);
    let before = character_get(
        &h.context,
        dto::CharacterGetRequest {
            character_id: h.character_id.to_string(),
        },
    )
    .await
    .expect("before");
    let source = root.join("picked.png");
    let request = |key: &str| dto::CharacterMediaSetRequest {
        character_id: h.character_id.to_string(),
        slot: dto::LibraryCharacterMediaSlot::AvatarOriginal,
        ordinal: 0,
        input: dto::LibraryMediaInput::Source {
            source: dto::FileSource {
                uri: source.to_string_lossy().into_owned(),
            },
        },
        expected_revision: before.revision,
        client_operation_id: key.into(),
    };
    assert_eq!(
        character_media_set(&h.context, request("missing-source"))
            .await
            .expect_err("missing")
            .code,
        ApiErrorCode::NotFound
    );
    std::fs::write(&source, b"not an image").expect("invalid source");
    assert_eq!(
        character_media_set(&h.context, request("malformed-source"))
            .await
            .expect_err("malformed")
            .code,
        ApiErrorCode::InvalidInput
    );
    assert_eq!(
        character_get(
            &h.context,
            dto::CharacterGetRequest {
                character_id: h.character_id.to_string()
            }
        )
        .await
        .expect("after"),
        before
    );
    for key in ["missing-source", "malformed-source"] {
        assert!(
            h.context
                .backend()
                .database()
                .lookup_api_operation("character_media_set", key)
                .expect("receipt")
                .is_none()
        );
    }
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}
