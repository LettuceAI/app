use std::io::Cursor;
use std::sync::Arc;

use lettuce_media::{AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, RetentionClass};

use super::*;
use crate::api::tests::{Reply, harness_in, media_store};
use crate::api::{AssetRange, NoModels, read_asset};

fn context() -> (std::path::PathBuf, ApiContext) {
    let root = std::env::temp_dir().join(format!("lettuce-previews-{}", AssetId::new()));
    std::fs::create_dir_all(&root).expect("root");
    let harness = harness_in(
        Reply::Text("ok"),
        Arc::new(lettuce_jobs::SystemClock),
        Some(media_store(&root)),
        None,
        Arc::new(NoModels),
    );
    (root, harness.context)
}

fn ingest(context: &ApiContext, bytes: &[u8], kind: AssetKind) -> AssetId {
    context.media().expect("store").ingest(bytes, IngestRequest::new(
        kind, AssetOrigin::Upload, RetentionClass::Library, AssetProvenanceV1::default(),
    )).expect("ingest").asset.id
}

async fn preview(context: &ApiContext, id: AssetId) -> AssetBytes {
    let AssetRead::Bytes(bytes) = read_asset_preview(context, &id.to_string()).await.expect("preview") else {
        panic!("preview must be whole");
    };
    bytes
}

#[tokio::test]
async fn previews_resize_without_changing_originals_and_share_cached_blobs() {
    let (root, context) = context();
    let pixels = image::RgbImage::from_fn(1600, 1200, |x, y| {
        let value = x.wrapping_mul(747_796_405).wrapping_add(y.wrapping_mul(2_891_336_453));
        image::Rgb([value as u8, (value >> 8) as u8, (value >> 16) as u8])
    });
    let mut original = Cursor::new(Vec::new());
    pixels.write_to(&mut original, ImageFormat::Png).expect("png");
    let original = original.into_inner();
    let id = ingest(&context, &original, AssetKind::OtherImage);
    let first = preview(&context, id).await;
    let decoded = image::load_from_memory(&first.bytes).expect("decode preview");
    assert_eq!((decoded.width(), decoded.height()), (512, 384));
    assert_eq!(first.mime_type, "image/webp");
    assert_eq!(first.total_len, first.bytes.len() as u64);
    assert!(first.bytes.len() < original.len() / 4);
    let second_id = ingest(&context, &original, AssetKind::AvatarOriginal);
    assert_ne!(id, second_id);
    assert_eq!(preview(&context, second_id).await, first);
    assert_eq!(context.image_previews().cache.lock().expect("cache").entries.len(), 1);
    let AssetRead::Bytes(whole) = read_asset(&context, &id.to_string(), AssetRange::Whole).await.expect("original") else {
        panic!("original must be whole");
    };
    assert_eq!(whole.bytes, original);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn small_previews_preserve_transparency_and_do_not_upscale() {
    let (root, context) = context();
    let image = image::RgbaImage::from_pixel(48, 24, image::Rgba([100, 80, 60, 0]));
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, ImageFormat::Png).expect("png");
    let id = ingest(&context, bytes.get_ref(), AssetKind::OtherImage);
    let decoded = image::load_from_memory(&preview(&context, id).await.bytes).expect("decode").into_rgba8();
    assert_eq!(decoded.dimensions(), (48, 24));
    assert_eq!(decoded.get_pixel(0, 0).0[3], 0);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn cached_previews_do_not_serve_missing_source_objects() {
    let (root, context) = context();
    let mut bytes = Cursor::new(Vec::new());
    image::RgbImage::new(32, 32).write_to(&mut bytes, ImageFormat::Png).expect("png");
    let id = ingest(&context, bytes.get_ref(), AssetKind::OtherImage);
    preview(&context, id).await;
    let store = context.media().expect("store");
    let blob = store.open_ready(id).expect("open").blob;
    store.release_blob(blob.id, |_| Ok(true)).expect("remove object");
    assert!(read_asset_preview(&context, &id.to_string()).await.is_err());
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn invalid_images_non_images_and_unknown_ids_do_not_fill_the_cache() {
    let (root, context) = context();
    let id = ingest(&context, &crate::api::tests::png_bytes(), AssetKind::OtherImage);
    assert_eq!(read_asset_preview(&context, &id.to_string()).await.expect_err("malformed").code, ApiErrorCode::InvalidInput);
    let document = ingest(&context, b"A plain text document", AssetKind::SourceDocument);
    assert_eq!(read_asset_preview(&context, &document.to_string()).await.expect_err("not an image").code, ApiErrorCode::InvalidInput);
    assert_eq!(read_asset_preview(&context, "bad-id").await.expect_err("invalid id").code, ApiErrorCode::InvalidInput);
    assert_eq!(read_asset_preview(&context, &AssetId::new().to_string()).await.expect_err("missing").code, ApiErrorCode::NotFound);
    assert!(context.image_previews().cache.lock().expect("cache").entries.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_evicts_old_previews_at_its_byte_budget() {
    let mut cache = PreviewCache::default();
    let hash = |value: u8| ContentHash::parse(format!("{value:064x}")).expect("hash");
    let bytes = AssetBytes { mime_type: "image/webp".into(), start: 0, total_len: 17 * 1024 * 1024, bytes: vec![0; 17 * 1024 * 1024] };
    cache.insert(hash(1), bytes.clone());
    cache.insert(hash(2), bytes);
    assert!(cache.get(&hash(1)).is_none());
    assert!(cache.get(&hash(2)).is_some());
    assert!(cache.bytes <= CACHE_BYTES);
    for value in 3..260 {
        cache.insert(ContentHash::parse(format!("{value:064x}")).expect("hash"), AssetBytes {
            mime_type: "image/webp".into(), start: 0, total_len: 1, bytes: vec![0],
        });
    }
    assert!(cache.get(&hash(2)).is_none());
    assert!(cache.entries.len() <= CACHE_ENTRIES);
}

#[tokio::test]
async fn avatar_crops_keep_the_selected_pixels_and_leave_the_original_unchanged() {
    let (root, context) = context();
    let image = image::RgbaImage::from_fn(8, 4, |x, _| {
        if x < 4 { image::Rgba([0, 0, 255, 255]) } else { image::Rgba([255, 0, 0, 128]) }
    });
    let mut original = Cursor::new(Vec::new());
    image.write_to(&mut original, ImageFormat::Png).expect("source image");
    let id = ingest(&context, original.get_ref(), AssetKind::GeneratedImage);
    let cropped = assets_crop_avatar(&context, lettuce_contracts::AssetsCropAvatarRequest {
        asset_id: id.to_string(), x: 4, y: 0, size: 4, height: None,
    }).await.expect("crop");
    let cropped_id = cropped.asset_id.parse().expect("cropped id");
    assert_ne!(id, cropped_id);
    let AssetRead::Bytes(bytes) = read_asset(&context, &cropped.asset_id, AssetRange::Whole).await.expect("crop bytes") else {
        panic!("complete cropped asset");
    };
    let decoded = image::load_from_memory(&bytes.bytes).expect("decode crop").into_rgba8();
    assert_eq!(decoded.dimensions(), (AVATAR_EDGE, AVATAR_EDGE));
    assert_eq!(decoded.get_pixel(512, 512).0, [255, 0, 0, 128]);
    let AssetRead::Bytes(unchanged) = read_asset(&context, &id.to_string(), AssetRange::Whole).await.expect("original") else {
        panic!("complete original asset");
    };
    assert_eq!(unchanged.bytes, *original.get_ref());
    let opened = context.media().expect("media").open_ready(cropped_id).expect("cropped asset");
    assert_eq!(opened.asset.kind, AssetKind::AvatarOriginal);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn avatar_crops_reject_empty_overflowing_and_outside_regions_and_non_images() {
    let (root, context) = context();
    let mut bytes = Cursor::new(Vec::new());
    image::RgbImage::new(8, 4).write_to(&mut bytes, ImageFormat::Png).expect("png");
    let id = ingest(&context, bytes.get_ref(), AssetKind::OtherImage);
    for (x, y, size) in [(0, 0, 0), (0, 0, 5), (5, 0, 4), (0, 1, 4), (u32::MAX, 0, 1)] {
        let error = assets_crop_avatar(&context, lettuce_contracts::AssetsCropAvatarRequest {
            asset_id: id.to_string(), x, y, size, height: None,
        }).await.expect_err("invalid crop");
        assert_eq!(error.code, ApiErrorCode::InvalidInput);
    }
    let document = ingest(&context, b"A plain text document", AssetKind::SourceDocument);
    let error = assets_crop_avatar(&context, lettuce_contracts::AssetsCropAvatarRequest {
        asset_id: document.to_string(), x: 0, y: 0, size: 1, height: None,
    }).await.expect_err("not an image");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn banner_crops_preserve_the_rectangular_frame_and_reject_invalid_height() {
    let (root, context) = context();
    let mut original = Cursor::new(Vec::new());
    image::RgbImage::from_pixel(16, 12, image::Rgb([40, 80, 160]))
        .write_to(&mut original, ImageFormat::Png).expect("source");
    let id = ingest(&context, original.get_ref(), AssetKind::OtherImage);
    let cropped = assets_crop_avatar(&context, lettuce_contracts::AssetsCropAvatarRequest {
        asset_id: id.to_string(), x: 0, y: 2, size: 16, height: Some(8),
    }).await.expect("banner crop");
    let AssetRead::Bytes(bytes) = read_asset(&context, &cropped.asset_id, AssetRange::Whole).await.expect("banner bytes") else {
        panic!("complete banner asset");
    };
    let image = image::load_from_memory(&bytes.bytes).expect("decoded");
    assert_eq!((image.width(), image.height()), (1024, 512));
    for height in [0, 13, u32::MAX] {
        assert_eq!(assets_crop_avatar(&context, lettuce_contracts::AssetsCropAvatarRequest {
            asset_id: id.to_string(), x: 0, y: 2, size: 16, height: Some(height),
        }).await.expect_err("invalid height").code, ApiErrorCode::InvalidInput);
    }
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn saved_banner_framing_retains_the_source_asset_after_reload_and_can_be_removed() {
    use crate::api::{character_draft_default, character_get, character_save};
    use lettuce_contracts as dto;
    let root = std::env::temp_dir().join(format!("lettuce-banner-framing-{}", AssetId::new()));
    std::fs::create_dir_all(&root).expect("root");
    let context = ApiContext::open_desktop(
        &root, None,
        Arc::new(lettuce_settings::InMemorySecretStore::new()),
        Arc::new(crate::api::tests::RecordingEvents::default()),
        Arc::new(crate::api::tests::StdFiles),
        "lettuce-asset://localhost".into(), None,
    ).expect("persistent context");
    let mut original = Cursor::new(Vec::new());
    image::RgbImage::new(16, 12).write_to(&mut original, ImageFormat::Png).expect("png");
    let id = ingest(&context, original.get_ref(), AssetKind::OtherImage);
    let cropped = assets_crop_avatar(&context, dto::AssetsCropAvatarRequest {
        asset_id: id.to_string(), x: 0, y: 2, size: 16, height: Some(8),
    }).await.expect("banner crop");
    let mut draft = character_draft_default(None).await.expect("draft");
    draft.profile.name = "Banner character".into();
    draft.presentation.banner_source_asset_id = Some(id.to_string());
    draft.presentation.banner_crop = Some(dto::LibraryCrop { x: 0.0, y: 0.05, scale: 1.0 });
    draft.media.push(dto::LibraryCharacterMediaLink {
        asset_id: cropped.asset_id.clone(), slot: dto::LibraryCharacterMediaSlot::Banner, ordinal: 0,
    });
    let saved = character_save(&context, dto::CharacterSaveRequest {
        character_id: None, draft, expected_revision: None, client_operation_id: "save-banner-frame".into(),
    }).await.expect("save");
    let mut reloaded = character_get(&context, dto::CharacterGetRequest { character_id: saved.id.clone() })
        .await.expect("reload");
    assert_eq!(reloaded.draft.presentation.banner_source_asset_id, Some(id.to_string()));
    assert!(reloaded.assets.iter().any(|asset| asset.asset_id == id.to_string()));
    reloaded.draft.presentation.banner_source_asset_id = None;
    reloaded.draft.presentation.banner_crop = None;
    reloaded.draft.media.clear();
    let removed = character_save(&context, dto::CharacterSaveRequest {
        character_id: Some(saved.id), draft: reloaded.draft, expected_revision: Some(saved.revision),
        client_operation_id: "remove-banner-frame".into(),
    }).await.expect("remove banner");
    assert!(removed.assets.is_empty());
    let _ = std::fs::remove_dir_all(root);
}
