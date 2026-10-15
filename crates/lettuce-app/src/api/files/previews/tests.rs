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
