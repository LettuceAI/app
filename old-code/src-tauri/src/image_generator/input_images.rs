use std::io::Cursor;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage, ImageFormat, ImageReader};
use tauri::AppHandle;

use super::types::ImageGenerationRequest;
use crate::utils::{log_info, log_warn};

/// Longest edge a reference image may have before it is sent to a remote provider.
pub const MAX_UPLOAD_EDGE: u32 = 2048;
/// Largest encoded size a reference image may have before it is re-encoded.
pub const MAX_UPLOAD_BYTES: usize = 4 * 1024 * 1024;

const JPEG_QUALITIES: &[u8] = &[90, 82, 74];

struct ShrunkImage {
    data_url: String,
    width: u32,
    height: u32,
    note: String,
}

struct ShrinkOutcome {
    images: Vec<String>,
    mask: Option<String>,
    notes: Vec<String>,
}

/// Downscales oversized reference images (and the matching inpainting mask) so the
/// request stays within the body limits remote providers enforce. Upscaled or
/// high-resolution sources can otherwise push the JSON payload past what the provider
/// accepts, which surfaces as an HTTP 413.
pub async fn shrink_for_upload(app: &AppHandle, request: &mut ImageGenerationRequest) {
    let Some(images) = request
        .input_images
        .clone()
        .filter(|images| !images.is_empty())
    else {
        return;
    };
    let mask = request.mask_image.clone();

    match tokio::task::spawn_blocking(move || shrink_images(images, mask)).await {
        Ok(outcome) => {
            for note in &outcome.notes {
                log_info(app, "image_generator", note);
            }
            request.input_images = Some(outcome.images);
            request.mask_image = outcome.mask;
        }
        Err(err) => log_warn(
            app,
            "image_generator",
            format!("Skipped reference image resizing: {}", err),
        ),
    }
}

fn shrink_images(images: Vec<String>, mask: Option<String>) -> ShrinkOutcome {
    let mut mask = mask;
    let mut notes = Vec::new();
    let mut shrunk_images = Vec::with_capacity(images.len());

    for (index, image) in images.into_iter().enumerate() {
        match shrink_data_url(&image) {
            Some(shrunk) => {
                notes.push(format!("Reference image {}: {}", index + 1, shrunk.note));
                if index == 0 {
                    if let Some(resized_mask) = mask
                        .as_deref()
                        .and_then(|mask| resize_mask(mask, shrunk.width, shrunk.height))
                    {
                        notes.push(format!(
                            "Inpainting mask resized to {}x{} to match the reference image",
                            shrunk.width, shrunk.height
                        ));
                        mask = Some(resized_mask);
                    }
                }
                shrunk_images.push(shrunk.data_url);
            }
            None => shrunk_images.push(image),
        }
    }

    ShrinkOutcome {
        images: shrunk_images,
        mask,
        notes,
    }
}

fn decode_image_data_url(data_url: &str) -> Option<Vec<u8>> {
    if !data_url.starts_with("data:image") {
        return None;
    }
    let payload = data_url.split_once(',')?.1;
    STANDARD.decode(payload).ok()
}

fn fit_within(width: u32, height: u32, max_edge: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= max_edge {
        return (width, height);
    }
    let scale = max_edge as f64 / longest as f64;
    (
        ((width as f64 * scale).round() as u32).max(1),
        ((height as f64 * scale).round() as u32).max(1),
    )
}

fn uses_transparency(image: &DynamicImage) -> bool {
    image.color().has_alpha() && image.to_rgba8().pixels().any(|pixel| pixel[3] < u8::MAX)
}

fn encode_png(image: &DynamicImage) -> Option<Vec<u8>> {
    let mut buffer = Cursor::new(Vec::new());
    image.write_to(&mut buffer, ImageFormat::Png).ok()?;
    Some(buffer.into_inner())
}

fn encode_jpeg(image: &DynamicImage, quality: u8) -> Option<Vec<u8>> {
    let mut buffer = Cursor::new(Vec::new());
    let encoder = JpegEncoder::new_with_quality(&mut buffer, quality);
    image.to_rgb8().write_with_encoder(encoder).ok()?;
    Some(buffer.into_inner())
}

fn shrink_data_url(data_url: &str) -> Option<ShrunkImage> {
    let bytes = decode_image_data_url(data_url)?;
    let reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .ok()?;
    let (width, height) = reader.into_dimensions().ok()?;

    let (target_width, target_height) = fit_within(width, height, MAX_UPLOAD_EDGE);
    let needs_resize = (target_width, target_height) != (width, height);
    if !needs_resize && bytes.len() <= MAX_UPLOAD_BYTES {
        return None;
    }

    let decoded = image::load_from_memory(&bytes).ok()?;
    let resized = if needs_resize {
        decoded.resize_exact(target_width, target_height, FilterType::Lanczos3)
    } else {
        decoded
    };

    let (mime_type, encoded) = if uses_transparency(&resized) {
        ("image/png", encode_png(&resized)?)
    } else {
        let mut encoded = None;
        for quality in JPEG_QUALITIES {
            let candidate = encode_jpeg(&resized, *quality)?;
            let fits = candidate.len() <= MAX_UPLOAD_BYTES;
            encoded = Some(candidate);
            if fits {
                break;
            }
        }
        ("image/jpeg", encoded?)
    };

    let note = format!(
        "resized from {}x{} ({} KB) to {}x{} ({} KB, {})",
        width,
        height,
        bytes.len() / 1024,
        target_width,
        target_height,
        encoded.len() / 1024,
        mime_type
    );

    Some(ShrunkImage {
        data_url: format!("data:{};base64,{}", mime_type, STANDARD.encode(&encoded)),
        width: target_width,
        height: target_height,
        note,
    })
}

fn resize_mask(data_url: &str, width: u32, height: u32) -> Option<String> {
    let bytes = decode_image_data_url(data_url)?;
    let mask = image::load_from_memory(&bytes).ok()?;
    if mask.width() == width && mask.height() == height {
        return None;
    }
    let resized = mask.resize_exact(width, height, FilterType::Nearest);
    let encoded = encode_png(&resized)?;
    Some(format!("data:image/png;base64,{}", STANDARD.encode(&encoded)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage, Rgba, RgbaImage};

    fn png_data_url(image: &DynamicImage) -> String {
        format!(
            "data:image/png;base64,{}",
            STANDARD.encode(encode_png(image).unwrap())
        )
    }

    fn dimensions_of(data_url: &str) -> (u32, u32) {
        let bytes = decode_image_data_url(data_url).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        (decoded.width(), decoded.height())
    }

    fn noisy_rgb(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
            Rgb([
                (x.wrapping_mul(31) ^ y.wrapping_mul(17)) as u8,
                (x.wrapping_add(y).wrapping_mul(7)) as u8,
                (x ^ y) as u8,
            ])
        }))
    }

    #[test]
    fn fit_within_preserves_aspect_ratio() {
        assert_eq!(fit_within(4096, 2048, 2048), (2048, 1024));
        assert_eq!(fit_within(1000, 3000, 2048), (683, 2048));
        assert_eq!(fit_within(1024, 1024, 2048), (1024, 1024));
    }

    #[test]
    fn small_images_and_remote_urls_are_left_alone() {
        let small = png_data_url(&noisy_rgb(64, 64));
        assert!(shrink_data_url(&small).is_none());
        assert!(shrink_data_url("https://example.com/image.png").is_none());
        assert!(shrink_data_url("not a data url").is_none());
    }

    #[test]
    fn oversized_opaque_images_become_jpeg_within_the_edge_limit() {
        let large = png_data_url(&noisy_rgb(2600, 1300));
        let shrunk = shrink_data_url(&large).expect("large image should be shrunk");
        assert!(shrunk.data_url.starts_with("data:image/jpeg;base64,"));
        assert_eq!((shrunk.width, shrunk.height), (2048, 1024));
        assert_eq!(dimensions_of(&shrunk.data_url), (2048, 1024));
    }

    #[test]
    fn transparent_images_stay_png_when_resized() {
        let transparent = DynamicImage::ImageRgba8(RgbaImage::from_fn(2200, 2200, |x, _| {
            Rgba([255, 0, 0, if x % 2 == 0 { 0 } else { 255 }])
        }));
        let shrunk = shrink_data_url(&png_data_url(&transparent)).expect("should be shrunk");
        assert!(shrunk.data_url.starts_with("data:image/png;base64,"));
        assert_eq!((shrunk.width, shrunk.height), (2048, 2048));
    }

    #[test]
    fn the_mask_follows_the_first_reference_image() {
        let source = png_data_url(&noisy_rgb(2600, 1300));
        let mask = png_data_url(&DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            2600,
            1300,
            Rgba([0, 0, 0, 0]),
        )));
        let outcome = shrink_images(vec![source, "https://example.com/ref.png".to_string()], Some(mask));
        assert_eq!(outcome.images.len(), 2);
        assert_eq!(outcome.images[1], "https://example.com/ref.png");
        assert_eq!(dimensions_of(&outcome.images[0]), (2048, 1024));
        assert_eq!(dimensions_of(outcome.mask.as_deref().unwrap()), (2048, 1024));
        assert_eq!(outcome.notes.len(), 2);
    }

    #[test]
    fn masks_are_untouched_when_the_reference_image_is_not_resized() {
        let source = png_data_url(&noisy_rgb(64, 64));
        let mask = png_data_url(&noisy_rgb(64, 64));
        let outcome = shrink_images(vec![source.clone()], Some(mask.clone()));
        assert_eq!(outcome.images[0], source);
        assert_eq!(outcome.mask.as_deref(), Some(mask.as_str()));
        assert!(outcome.notes.is_empty());
    }
}
