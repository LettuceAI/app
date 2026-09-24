use std::io::Cursor;

use image::{
    DynamicImage, ImageDecoder, ImageFormat, ImageReader, codecs::jpeg::JpegEncoder,
    imageops::FilterType, metadata::Orientation,
};

use crate::ImageInput;

/// Longest edge a reference image may have before it is sent to a remote provider.
pub const MAX_UPLOAD_EDGE: u32 = 2048;
/// Largest encoded size a reference image may have before it is re-encoded.
pub const MAX_UPLOAD_BYTES: usize = 4 * 1024 * 1024;

const JPEG_QUALITIES: [u8; 3] = [90, 82, 74];

/// Reference images and mask as they go to a remote provider, with one note
/// per image that was changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadImages {
    pub images: Vec<ImageInput>,
    pub mask: Option<ImageInput>,
    pub notes: Vec<String>,
}

/// What an input becomes for upload: `image` is the re-encoded input, or
/// `None` when the original bytes are kept; `width` and `height` are the
/// upright upload dimensions the mask follows.
struct ShrunkImage {
    image: Option<ImageInput>,
    width: u32,
    height: u32,
    note: Option<String>,
}

/// Downscales reference images past `MAX_UPLOAD_EDGE` and re-encodes those
/// past `MAX_UPLOAD_BYTES` (PNG when any pixel is transparent, else JPEG at
/// falling quality until it fits), so the request stays within the body
/// limits remote providers enforce. Pixels are turned upright by their EXIF
/// orientation before they are resized. A re-encode that neither resizes nor
/// comes out smaller keeps the original bytes. Whenever the first image is
/// resized or re-encoded, the mask is resized to that image's upright upload
/// dimensions. GIFs, inputs that are not images and inputs that cannot be
/// decoded are passed through unchanged.
#[must_use]
pub fn shrink_for_upload(images: Vec<ImageInput>, mask: Option<ImageInput>) -> UploadImages {
    let mut mask = mask;
    let mut notes = Vec::new();
    let mut shrunk_images = Vec::with_capacity(images.len());
    for (index, image) in images.into_iter().enumerate() {
        let Some(shrunk) = shrink_image(&image) else {
            shrunk_images.push(image);
            continue;
        };
        if let Some(note) = &shrunk.note {
            notes.push(format!("Reference image {}: {note}", index + 1));
        }
        if index == 0
            && let Some(resized_mask) = mask
                .as_ref()
                .and_then(|mask| resize_mask(mask, shrunk.width, shrunk.height))
        {
            notes.push(format!(
                "Inpainting mask resized to {}x{} to match the reference image",
                shrunk.width, shrunk.height
            ));
            mask = Some(resized_mask);
        }
        shrunk_images.push(shrunk.image.unwrap_or(image));
    }
    UploadImages {
        images: shrunk_images,
        mask,
        notes,
    }
}

fn is_image(input: &ImageInput) -> bool {
    input.mime_type.starts_with("image") && !input.mime_type.eq_ignore_ascii_case("image/gif")
}

fn fit_within(width: u32, height: u32, max_edge: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= max_edge {
        return (width, height);
    }
    let scale = f64::from(max_edge) / f64::from(longest);
    let scaled = |edge: u32| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the scaled edge is positive and at most max_edge"
        )]
        let edge = (f64::from(edge) * scale).round() as u32;
        edge.max(1)
    };
    (scaled(width), scaled(height))
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

/// A decoder for a still image, with its EXIF orientation and the
/// dimensions the image has once turned upright.
struct UprightDecoder<D> {
    decoder: D,
    orientation: Orientation,
    width: u32,
    height: u32,
}

impl<D: ImageDecoder> UprightDecoder<D> {
    fn decode(self) -> Option<DynamicImage> {
        let mut image = DynamicImage::from_decoder(self.decoder).ok()?;
        image.apply_orientation(self.orientation);
        Some(image)
    }
}

fn upright_decoder(input: &ImageInput) -> Option<UprightDecoder<impl ImageDecoder + '_>> {
    if !is_image(input) {
        return None;
    }
    let reader = ImageReader::new(Cursor::new(input.bytes.as_slice()))
        .with_guessed_format()
        .ok()?;
    if reader
        .format()
        .is_none_or(|format| format == ImageFormat::Gif)
    {
        return None;
    }
    let mut decoder = reader.into_decoder().ok()?;
    let mut limits = image::Limits::default();
    limits.reserve(decoder.total_bytes()).ok()?;
    decoder.set_limits(limits).ok()?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let (width, height) = decoder.dimensions();
    let (width, height) = match orientation {
        Orientation::Rotate90
        | Orientation::Rotate270
        | Orientation::Rotate90FlipH
        | Orientation::Rotate270FlipH => (height, width),
        _ => (width, height),
    };
    Some(UprightDecoder {
        decoder,
        orientation,
        width,
        height,
    })
}

fn shrink_image(input: &ImageInput) -> Option<ShrunkImage> {
    let decoder = upright_decoder(input)?;
    let (width, height) = (decoder.width, decoder.height);
    let (target_width, target_height) = fit_within(width, height, MAX_UPLOAD_EDGE);
    let needs_resize = (target_width, target_height) != (width, height);
    if !needs_resize && input.bytes.len() <= MAX_UPLOAD_BYTES {
        return None;
    }

    let decoded = decoder.decode()?;
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
            let candidate = encode_jpeg(&resized, quality)?;
            let fits = candidate.len() <= MAX_UPLOAD_BYTES;
            encoded = Some(candidate);
            if fits {
                break;
            }
        }
        ("image/jpeg", encoded?)
    };
    if !needs_resize && encoded.len() >= input.bytes.len() {
        return Some(ShrunkImage {
            image: None,
            width,
            height,
            note: None,
        });
    }

    let note = format!(
        "resized from {}x{} ({} KB) to {}x{} ({} KB, {})",
        width,
        height,
        input.bytes.len() / 1024,
        target_width,
        target_height,
        encoded.len() / 1024,
        mime_type
    );
    Some(ShrunkImage {
        image: Some(ImageInput {
            mime_type: mime_type.to_owned(),
            bytes: encoded,
        }),
        width: target_width,
        height: target_height,
        note: Some(note),
    })
}

fn resize_mask(mask: &ImageInput, width: u32, height: u32) -> Option<ImageInput> {
    let decoder = upright_decoder(mask)?;
    if decoder.width == width && decoder.height == height {
        return None;
    }
    let resized = decoder
        .decode()?
        .resize_exact(width, height, FilterType::Nearest);
    Some(ImageInput {
        mime_type: "image/png".to_owned(),
        bytes: encode_png(&resized)?,
    })
}

#[cfg(test)]
mod tests {
    use image::{Rgb, RgbImage, Rgba, RgbaImage};

    use super::*;

    fn png(image: &DynamicImage) -> ImageInput {
        ImageInput {
            mime_type: "image/png".to_owned(),
            bytes: encode_png(image).expect("png"),
        }
    }

    fn dimensions_of(input: &ImageInput) -> (u32, u32) {
        let decoded = image::load_from_memory(&input.bytes).expect("decode");
        (decoded.width(), decoded.height())
    }

    fn patterned_rgb(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
            Rgb([
                (x.wrapping_mul(31) ^ y.wrapping_mul(17)) as u8,
                (x.wrapping_add(y).wrapping_mul(7)) as u8,
                (x ^ y) as u8,
            ])
        }))
    }

    fn noise_rgb(width: u32, height: u32) -> DynamicImage {
        let mut state = 0x2545_f491_u32;
        DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |_, _| {
            let mut next = || {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state.to_le_bytes()[0]
            };
            Rgb([next(), next(), next()])
        }))
    }

    #[test]
    fn fit_within_preserves_aspect_ratio() {
        assert_eq!(fit_within(4096, 2048, 2048), (2048, 1024));
        assert_eq!(fit_within(1000, 3000, 2048), (683, 2048));
        assert_eq!(fit_within(1024, 1024, 2048), (1024, 1024));
        assert_eq!(fit_within(9000, 1, 2048), (2048, 1));
    }

    #[test]
    fn small_images_and_non_images_are_left_alone() {
        let small = png(&patterned_rgb(64, 64));
        assert!(shrink_image(&small).is_none());
        let not_an_image = ImageInput {
            mime_type: "application/octet-stream".to_owned(),
            bytes: small.bytes.clone(),
        };
        assert!(shrink_image(&not_an_image).is_none());
        let undecodable = ImageInput {
            mime_type: "image/png".to_owned(),
            bytes: vec![0; MAX_UPLOAD_BYTES + 1],
        };
        assert!(shrink_image(&undecodable).is_none());
    }

    #[test]
    fn oversized_opaque_images_become_jpeg_within_the_edge_limit() {
        let shrunk = shrink_image(&png(&patterned_rgb(2600, 1300))).expect("shrunk");
        assert_eq!((shrunk.width, shrunk.height), (2048, 1024));
        let encoded = shrunk.image.expect("re-encoded");
        assert_eq!(encoded.mime_type, "image/jpeg");
        assert_eq!(dimensions_of(&encoded), (2048, 1024));
        assert_eq!(
            image::guess_format(&encoded.bytes).expect("format"),
            ImageFormat::Jpeg
        );
    }

    #[test]
    fn transparent_images_stay_png_when_resized() {
        let transparent = DynamicImage::ImageRgba8(RgbaImage::from_fn(2200, 64, |x, _| {
            Rgba([255, 0, 0, if x % 2 == 0 { 0 } else { 255 }])
        }));
        let shrunk = shrink_image(&png(&transparent)).expect("shrunk");
        assert_eq!(shrunk.image.expect("re-encoded").mime_type, "image/png");
        assert_eq!((shrunk.width, shrunk.height), (2048, 60));
    }

    #[test]
    fn heavy_images_within_the_edge_are_reencoded_as_jpeg() {
        let heavy = png(&noise_rgb(1400, 1400));
        assert!(heavy.bytes.len() > MAX_UPLOAD_BYTES);
        let shrunk = shrink_image(&heavy).expect("examined");
        assert_eq!((shrunk.width, shrunk.height), (1400, 1400));
        let encoded = shrunk.image.expect("re-encoded");
        assert_eq!(encoded.mime_type, "image/jpeg");
        assert!(encoded.bytes.len() <= MAX_UPLOAD_BYTES);
    }

    #[test]
    fn heavy_transparent_images_that_do_not_shrink_are_kept() {
        let mut state = 0x9e37_79b9_u32;
        let noisy = DynamicImage::ImageRgba8(RgbaImage::from_fn(1100, 1100, |_, _| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            Rgba(state.to_le_bytes())
        }));
        let heavy = png(&noisy);
        assert!(heavy.bytes.len() > MAX_UPLOAD_BYTES);
        let kept = shrink_image(&heavy).expect("examined");
        assert!(kept.image.is_none());
        assert!(kept.note.is_none());
    }

    fn jpeg_with_orientation(image: &DynamicImage, orientation: u8) -> ImageInput {
        let jpeg = encode_jpeg(image, 90).expect("jpeg");
        let mut app1 = b"Exif\0\0MM\0\x2a\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01\0".to_vec();
        app1.extend_from_slice(&[orientation, 0, 0, 0, 0, 0, 0]);
        let length = u16::try_from(app1.len() + 2).expect("segment length");
        let mut bytes = jpeg[..2].to_vec();
        bytes.extend_from_slice(&[0xFF, 0xE1]);
        bytes.extend_from_slice(&length.to_be_bytes());
        bytes.extend_from_slice(&app1);
        bytes.extend_from_slice(&jpeg[2..]);
        ImageInput {
            mime_type: "image/jpeg".to_owned(),
            bytes,
        }
    }

    #[test]
    fn exif_orientation_is_applied_before_resizing() {
        let sideways = jpeg_with_orientation(&patterned_rgb(2400, 100), 6);
        let mask = png(&DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            100,
            2400,
            Rgba([0, 0, 0, 255]),
        )));
        let outcome = shrink_for_upload(vec![sideways], Some(mask));
        assert_eq!(outcome.images[0].mime_type, "image/jpeg");
        assert_eq!(dimensions_of(&outcome.images[0]), (85, 2048));
        assert_eq!(
            dimensions_of(outcome.mask.as_ref().expect("mask")),
            (85, 2048)
        );
    }

    #[test]
    fn oriented_images_within_limits_keep_their_original_bytes() {
        let small = jpeg_with_orientation(&patterned_rgb(64, 32), 6);
        let outcome = shrink_for_upload(vec![small.clone()], None);
        assert_eq!(outcome.images, vec![small]);
        assert!(outcome.notes.is_empty());
    }

    #[test]
    fn images_too_large_to_decode_pass_through() {
        let mut ihdr = b"IHDR".to_vec();
        ihdr.extend_from_slice(&30_000_u32.to_be_bytes());
        ihdr.extend_from_slice(&30_000_u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.extend_from_slice(&13_u32.to_be_bytes());
        bytes.extend_from_slice(&ihdr);
        bytes.extend_from_slice(&crc32(&ihdr).to_be_bytes());
        bytes.extend_from_slice(&0_u32.to_be_bytes());
        bytes.extend_from_slice(b"IEND");
        bytes.extend_from_slice(&crc32(b"IEND").to_be_bytes());
        let huge = ImageInput {
            mime_type: "image/png".to_owned(),
            bytes,
        };
        let outcome = shrink_for_upload(vec![huge.clone()], None);
        assert_eq!(outcome.images, vec![huge]);
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0_u32;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    #[test]
    fn gifs_are_never_reencoded() {
        let mut bytes = Vec::new();
        image::codecs::gif::GifEncoder::new(&mut bytes)
            .encode_frame(image::Frame::new(RgbaImage::from_pixel(
                2100,
                4,
                Rgba([1, 2, 3, 255]),
            )))
            .expect("gif");
        for mime_type in ["image/gif", "image/png"] {
            let gif = ImageInput {
                mime_type: mime_type.to_owned(),
                bytes: bytes.clone(),
            };
            let mask = png(&patterned_rgb(8, 8));
            let outcome = shrink_for_upload(vec![gif.clone()], Some(mask.clone()));
            assert_eq!(outcome.images, vec![gif]);
            assert_eq!(outcome.mask, Some(mask));
        }
    }

    #[test]
    fn the_mask_follows_a_kept_first_image_that_was_reencoded_without_gain() {
        let mut state = 0x9e37_79b9_u32;
        let noisy = DynamicImage::ImageRgba8(RgbaImage::from_fn(1100, 1100, |_, _| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            Rgba(state.to_le_bytes())
        }));
        let heavy = png(&noisy);
        let outcome = shrink_for_upload(vec![heavy.clone()], Some(png(&patterned_rgb(50, 50))));
        assert_eq!(outcome.images, vec![heavy]);
        assert_eq!(
            dimensions_of(outcome.mask.as_ref().expect("mask")),
            (1100, 1100)
        );
        assert_eq!(
            outcome.notes,
            vec!["Inpainting mask resized to 1100x1100 to match the reference image".to_owned()]
        );
    }

    #[test]
    fn the_mask_follows_the_first_reference_image() {
        let mask = png(&DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            2600,
            1300,
            Rgba([0, 0, 0, 0]),
        )));
        let untouched = png(&patterned_rgb(32, 32));
        let outcome = shrink_for_upload(
            vec![png(&patterned_rgb(2600, 1300)), untouched.clone()],
            Some(mask),
        );
        assert_eq!(outcome.images.len(), 2);
        assert_eq!(outcome.images[1], untouched);
        assert_eq!(dimensions_of(&outcome.images[0]), (2048, 1024));
        let mask = outcome.mask.expect("mask");
        assert_eq!(mask.mime_type, "image/png");
        assert_eq!(dimensions_of(&mask), (2048, 1024));
        assert_eq!(outcome.notes.len(), 2);
    }

    #[test]
    fn masks_are_untouched_when_the_reference_image_is_not_resized() {
        let source = png(&patterned_rgb(64, 64));
        let mask = png(&patterned_rgb(80, 80));
        let outcome = shrink_for_upload(vec![source.clone()], Some(mask.clone()));
        assert_eq!(outcome.images, vec![source]);
        assert_eq!(outcome.mask, Some(mask));
        assert!(outcome.notes.is_empty());
    }
}
