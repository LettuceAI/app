//! The gradient drawn behind an avatar: dominant colors found by median-cut
//! quantization of a pixel sample, picked as a dark muted base, a muted
//! companion and a dark vibrant accent.

use std::collections::HashMap;
use std::io::Read;
use std::sync::Mutex;

use lettuce_media::{
    LocalMediaBlobStore, MediaAssetRepository, MediaBlobRepository, MediaStoreError,
};
use lettuce_types::{AssetId, ContentHash};

#[derive(Debug, Clone, PartialEq)]
pub struct GradientColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub hex: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AvatarGradient {
    pub colors: Vec<GradientColor>,
    pub gradient_css: String,
    pub dominant_hue: f64,
    pub text_color: String,
    pub text_secondary: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AvatarGradientError {
    #[error("Avatar not found for {0}")]
    NotFound(AssetId),
    #[error("Failed to load image: {0}")]
    Media(MediaStoreError),
    #[error("Failed to load image: {0}")]
    Decode(String),
}

/// Gradients computed so far, keyed by the image's bytes, so an image is
/// decoded once per process.
#[derive(Debug, Default)]
pub struct AvatarGradients {
    computed: Mutex<HashMap<ContentHash, AvatarGradient>>,
}

impl AvatarGradients {
    /// The gradient of an avatar image asset; `force` recomputes it. The
    /// caller picks the image: the round avatar when the card shows one and it
    /// exists, else the square avatar.
    pub fn gradient<BR: MediaBlobRepository, AR: MediaAssetRepository>(
        &self,
        media: &LocalMediaBlobStore<BR, AR>,
        avatar: AssetId,
        force: bool,
    ) -> Result<AvatarGradient, AvatarGradientError> {
        let mut opened = media.open_ready(avatar).map_err(|error| match error {
            MediaStoreError::AssetNotFound
            | MediaStoreError::BlobNotFound
            | MediaStoreError::NotReady
            | MediaStoreError::ObjectMissing => AvatarGradientError::NotFound(avatar),
            other => AvatarGradientError::Media(other),
        })?;
        let key = opened.blob.content_hash.clone();
        if !force
            && let Some(cached) = self
                .computed
                .lock()
                .ok()
                .and_then(|computed| computed.get(&key).cloned())
        {
            return Ok(cached);
        }
        let mut bytes = Vec::new();
        opened
            .reader
            .read_to_end(&mut bytes)
            .map_err(|error| AvatarGradientError::Decode(error.to_string()))?;
        let image = image::load_from_memory(&bytes)
            .map_err(|error| AvatarGradientError::Decode(error.to_string()))?;
        drop(bytes);
        let gradient = image_gradient(&image.into_rgb8());
        if let Ok(mut computed) = self.computed.lock() {
            computed.insert(key, gradient.clone());
        }
        Ok(gradient)
    }
}

fn image_gradient(image: &image::RgbImage) -> AvatarGradient {
    let (width, height) = image.dimensions();
    let samples = sample_pixels(width, height, |x, y| {
        let pixel = image.get_pixel(x, y);
        (pixel[0], pixel[1], pixel[2])
    });
    gradient_from_samples(&samples)
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the sample step is a positive whole number of pixels"
)]
fn sample_pixels(
    width: u32,
    height: u32,
    pixel: impl Fn(u32, u32) -> (u8, u8, u8),
) -> Vec<(u8, u8, u8)> {
    let total_pixels = f64::from(width) * f64::from(height);
    let target_samples = 100.0;
    let step = (total_pixels / target_samples).sqrt().max(1.0) as usize;
    let mut samples = Vec::new();
    for y in (0..height).step_by(step) {
        for x in (0..width).step_by(step) {
            samples.push(pixel(x, y));
        }
    }
    samples
}

fn gradient_from_samples(samples: &[(u8, u8, u8)]) -> AvatarGradient {
    let Some(dominant) = find_dominant_colors(samples, 8).filter(|colors| !colors.is_empty())
    else {
        return default_gradient();
    };
    let dominant_hue = average_hue(&dominant);
    let colors = gradient_colors(&dominant);
    let gradient_css = css_gradient(&colors);
    let (text_color, text_secondary) = text_colors(&colors);
    AvatarGradient {
        colors,
        gradient_css,
        dominant_hue,
        text_color,
        text_secondary,
    }
}

#[derive(Debug, Clone)]
struct ClusterColor {
    r: u8,
    g: u8,
    b: u8,
    count: usize,
}

const SIGBITS: usize = 5;
const RSHIFT: usize = 8 - SIGBITS;
const HISTOSIZE: usize = 1 << (3 * SIGBITS);

#[derive(Debug, Clone)]
struct VBox {
    r1: usize,
    r2: usize,
    g1: usize,
    g2: usize,
    b1: usize,
    b2: usize,
    count: usize,
}

#[derive(Clone, Copy)]
struct SwatchTarget {
    min_saturation: f64,
    target_saturation: f64,
    max_saturation: f64,
    min_luminance: f64,
    target_luminance: f64,
    max_luminance: f64,
    saturation_weight: f64,
    luminance_weight: f64,
    population_weight: f64,
}

const DARK_MUTED: SwatchTarget = SwatchTarget {
    min_saturation: 0.0,
    target_saturation: 0.30,
    max_saturation: 0.45,
    min_luminance: 0.0,
    target_luminance: 0.26,
    max_luminance: 0.40,
    saturation_weight: 0.24,
    luminance_weight: 0.52,
    population_weight: 0.24,
};

const MUTED: SwatchTarget = SwatchTarget {
    min_saturation: 0.0,
    target_saturation: 0.30,
    max_saturation: 0.45,
    min_luminance: 0.30,
    target_luminance: 0.50,
    max_luminance: 0.70,
    saturation_weight: 0.30,
    luminance_weight: 0.30,
    population_weight: 0.40,
};

const DARK_VIBRANT: SwatchTarget = SwatchTarget {
    min_saturation: 0.35,
    target_saturation: 0.80,
    max_saturation: 1.0,
    min_luminance: 0.0,
    target_luminance: 0.26,
    max_luminance: 0.45,
    saturation_weight: 0.35,
    luminance_weight: 0.35,
    population_weight: 0.30,
};

fn find_dominant_colors(samples: &[(u8, u8, u8)], k: usize) -> Option<Vec<ClusterColor>> {
    if samples.is_empty() {
        return None;
    }
    let histogram = build_histogram(samples);
    let mut boxes = vec![create_vbox(samples, &histogram)?];
    while boxes.len() < k {
        let Some((index, _)) = boxes
            .iter()
            .enumerate()
            .filter(|(_, vbox)| vbox.count > 0 && vbox_can_split(vbox))
            .max_by(|(_, a), (_, b)| {
                vbox_score(a)
                    .partial_cmp(&vbox_score(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        else {
            break;
        };
        let vbox = boxes.remove(index);
        let (left, right) = split_vbox(vbox, &histogram)?;
        boxes.push(left);
        boxes.push(right);
    }
    let mut result: Vec<ClusterColor> = boxes
        .into_iter()
        .filter_map(|vbox| average_color(&vbox, &histogram))
        .collect();
    result.sort_by_key(|color| std::cmp::Reverse(color.count));
    Some(result)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "populations are pixel sample counts"
)]
fn average_hue(colors: &[ClusterColor]) -> f64 {
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    for color in colors {
        let (h, s, v) = rgb_to_hsv(color.r, color.g, color.b);
        let weight = s * v * color.count as f64;
        let angle = h.to_radians();
        sum_x += angle.cos() * weight;
        sum_y += angle.sin() * weight;
    }
    if sum_x == 0.0 && sum_y == 0.0 {
        0.0
    } else {
        sum_y.atan2(sum_x).to_degrees().rem_euclid(360.0)
    }
}

fn build_histogram(samples: &[(u8, u8, u8)]) -> Vec<usize> {
    let mut histogram = vec![0_usize; HISTOSIZE];
    for &(r, g, b) in samples {
        histogram[color_index(
            usize::from(r >> RSHIFT),
            usize::from(g >> RSHIFT),
            usize::from(b >> RSHIFT),
        )] += 1;
    }
    histogram
}

const fn color_index(r: usize, g: usize, b: usize) -> usize {
    (r << (2 * SIGBITS)) + (g << SIGBITS) + b
}

fn create_vbox(samples: &[(u8, u8, u8)], histogram: &[usize]) -> Option<VBox> {
    let shifted = |value: u8| usize::from(value >> RSHIFT);
    let mut vbox = VBox {
        r1: samples.iter().map(|sample| shifted(sample.0)).min()?,
        r2: samples.iter().map(|sample| shifted(sample.0)).max()?,
        g1: samples.iter().map(|sample| shifted(sample.1)).min()?,
        g2: samples.iter().map(|sample| shifted(sample.1)).max()?,
        b1: samples.iter().map(|sample| shifted(sample.2)).min()?,
        b2: samples.iter().map(|sample| shifted(sample.2)).max()?,
        count: 0,
    };
    vbox.count = vbox_population(&vbox, histogram);
    Some(vbox)
}

fn vbox_population(vbox: &VBox, histogram: &[usize]) -> usize {
    let mut sum = 0;
    for r in vbox.r1..=vbox.r2 {
        for g in vbox.g1..=vbox.g2 {
            for b in vbox.b1..=vbox.b2 {
                sum += histogram[color_index(r, g, b)];
            }
        }
    }
    sum
}

const fn vbox_volume(vbox: &VBox) -> usize {
    (vbox.r2 - vbox.r1 + 1) * (vbox.g2 - vbox.g1 + 1) * (vbox.b2 - vbox.b1 + 1)
}

#[expect(
    clippy::cast_precision_loss,
    reason = "box scores only rank boxes against each other"
)]
fn vbox_score(vbox: &VBox) -> f64 {
    vbox.count as f64 * vbox_volume(vbox) as f64
}

const fn vbox_can_split(vbox: &VBox) -> bool {
    vbox.r1 < vbox.r2 || vbox.g1 < vbox.g2 || vbox.b1 < vbox.b2
}

fn average_color(vbox: &VBox, histogram: &[usize]) -> Option<ClusterColor> {
    let mut total = 0;
    let (mut r_sum, mut g_sum, mut b_sum) = (0, 0, 0);
    for r in vbox.r1..=vbox.r2 {
        for g in vbox.g1..=vbox.g2 {
            for b in vbox.b1..=vbox.b2 {
                let count = histogram[color_index(r, g, b)];
                if count == 0 {
                    continue;
                }
                total += count;
                r_sum += count * ((r << RSHIFT) + (1 << (RSHIFT - 1)));
                g_sum += count * ((g << RSHIFT) + (1 << (RSHIFT - 1)));
                b_sum += count * ((b << RSHIFT) + (1 << (RSHIFT - 1)));
            }
        }
    }
    if total == 0 {
        return None;
    }
    let channel = |sum: usize| u8::try_from((sum / total).min(255)).unwrap_or(u8::MAX);
    Some(ClusterColor {
        r: channel(r_sum),
        g: channel(g_sum),
        b: channel(b_sum),
        count: total,
    })
}

fn split_vbox(vbox: VBox, histogram: &[usize]) -> Option<(VBox, VBox)> {
    let r_range = vbox.r2 - vbox.r1;
    let g_range = vbox.g2 - vbox.g1;
    let b_range = vbox.b2 - vbox.b1;
    let axis = if r_range >= g_range && r_range >= b_range {
        0
    } else if g_range >= r_range && g_range >= b_range {
        1
    } else {
        2
    };
    let (start, end) = match axis {
        0 => (vbox.r1, vbox.r2),
        1 => (vbox.g1, vbox.g2),
        _ => (vbox.b1, vbox.b2),
    };
    let mut partial_sum = Vec::new();
    let mut total = 0;
    for i in start..=end {
        let mut sum = 0;
        match axis {
            0 => {
                for g in vbox.g1..=vbox.g2 {
                    for b in vbox.b1..=vbox.b2 {
                        sum += histogram[color_index(i, g, b)];
                    }
                }
            }
            1 => {
                for r in vbox.r1..=vbox.r2 {
                    for b in vbox.b1..=vbox.b2 {
                        sum += histogram[color_index(r, i, b)];
                    }
                }
            }
            _ => {
                for r in vbox.r1..=vbox.r2 {
                    for g in vbox.g1..=vbox.g2 {
                        sum += histogram[color_index(r, g, i)];
                    }
                }
            }
        }
        total += sum;
        partial_sum.push((i, total));
    }
    if total == 0 {
        return None;
    }
    let mid = total / 2;
    let split_at = partial_sum
        .iter()
        .find(|(_, running)| *running >= mid)
        .map_or(start, |(index, _)| *index);
    let mut left = vbox.clone();
    let mut right = vbox;
    match axis {
        0 => {
            left.r2 = split_at;
            right.r1 = (split_at + 1).min(right.r2);
        }
        1 => {
            left.g2 = split_at;
            right.g1 = (split_at + 1).min(right.g2);
        }
        _ => {
            left.b2 = split_at;
            right.b1 = (split_at + 1).min(right.b2);
        }
    }
    left.count = vbox_population(&left, histogram);
    right.count = vbox_population(&right, histogram);
    if left.count == 0 || right.count == 0 {
        return None;
    }
    Some((left, right))
}

fn text_colors(colors: &[GradientColor]) -> (String, String) {
    let luminances: Vec<f64> = colors
        .iter()
        .map(|color| {
            0.2126 * (f64::from(color.r) / 255.0)
                + 0.7152 * (f64::from(color.g) / 255.0)
                + 0.0722 * (f64::from(color.b) / 255.0)
        })
        .collect();
    let average = if luminances.is_empty() {
        0.0
    } else {
        luminances.iter().sum::<f64>() / f64::from(u32::try_from(luminances.len()).unwrap_or(1))
    };
    if average > 0.5 {
        ("#111827".into(), "#374151".into())
    } else {
        ("#F9FAFB".into(), "#D1D5DB".into())
    }
}

fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let r = f64::from(r) / 255.0;
    let g = f64::from(g) / 255.0;
    let b = f64::from(b) / 255.0;
    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let diff = max - min;
    let s = if max == 0.0 { 0.0 } else { diff / max };
    let h = if diff == 0.0 {
        0.0
    } else if (max - r).abs() < f64::EPSILON {
        60.0 * (((g - b) / diff) % 6.0)
    } else if (max - g).abs() < f64::EPSILON {
        60.0 * ((b - r) / diff + 2.0)
    } else {
        60.0 * ((r - g) / diff + 4.0)
    };
    (if h < 0.0 { h + 360.0 } else { h }, s, max)
}

fn perceived_luminance(r: u8, g: u8, b: u8) -> f64 {
    (0.299 * f64::from(r) + 0.587 * f64::from(g) + 0.114 * f64::from(b)) / 255.0
}

fn color_distance(a: &ClusterColor, b: &ClusterColor) -> f64 {
    let dr = f64::from(a.r) - f64::from(b.r);
    let dg = f64::from(a.g) - f64::from(b.g);
    let db = f64::from(a.b) - f64::from(b.b);
    (dr * dr + dg * dg + db * db).sqrt()
}

#[expect(
    clippy::cast_precision_loss,
    reason = "populations are pixel sample counts"
)]
fn score_target(color: &ClusterColor, target: SwatchTarget, max_population: usize) -> Option<f64> {
    let (_, saturation, _) = rgb_to_hsv(color.r, color.g, color.b);
    let luminance = perceived_luminance(color.r, color.g, color.b);
    if saturation < target.min_saturation
        || saturation > target.max_saturation
        || luminance < target.min_luminance
        || luminance > target.max_luminance
    {
        return None;
    }
    let saturation_score = 1.0 - (saturation - target.target_saturation).abs();
    let luminance_score = 1.0 - (luminance - target.target_luminance).abs();
    let population_score = color.count as f64 / max_population.max(1) as f64;
    Some(
        saturation_score * target.saturation_weight
            + luminance_score * target.luminance_weight
            + population_score * target.population_weight,
    )
}

fn best_swatch<'a>(
    candidates: impl Iterator<Item = &'a ClusterColor>,
    target: SwatchTarget,
    max_population: usize,
) -> Option<ClusterColor> {
    candidates
        .max_by(|a, b| {
            score_target(a, target, max_population)
                .unwrap_or(f64::MIN)
                .partial_cmp(&score_target(b, target, max_population).unwrap_or(f64::MIN))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .and_then(|entry| score_target(entry, target, max_population).map(|_| entry.clone()))
}

fn distinct_swatch(
    colors: &[ClusterColor],
    selected: &[ClusterColor],
    target: SwatchTarget,
    max_population: usize,
) -> Option<ClusterColor> {
    best_swatch(
        colors.iter().filter(|entry| {
            selected
                .iter()
                .all(|chosen| color_distance(entry, chosen) > 34.0)
        }),
        target,
        max_population,
    )
}

fn gradient_colors(colors: &[ClusterColor]) -> Vec<GradientColor> {
    if colors.is_empty() {
        return Vec::new();
    }
    let max_population = colors.iter().map(|color| color.count).max().unwrap_or(1);
    let base = best_swatch(colors.iter(), DARK_MUTED, max_population)
        .or_else(|| best_swatch(colors.iter(), MUTED, max_population))
        .or_else(|| best_swatch(colors.iter(), DARK_VIBRANT, max_population))
        .unwrap_or_else(|| colors[0].clone());
    let companion = distinct_swatch(colors, std::slice::from_ref(&base), MUTED, max_population)
        .or_else(|| {
            distinct_swatch(
                colors,
                std::slice::from_ref(&base),
                DARK_MUTED,
                max_population,
            )
        });
    let chosen = companion.as_ref().map_or_else(
        || vec![base.clone()],
        |color| vec![base.clone(), color.clone()],
    );
    let accent = distinct_swatch(colors, &chosen, DARK_VIBRANT, max_population);
    let mut selected = vec![base];
    selected.extend(companion);
    selected.extend(accent);
    if selected.len() == 1 && colors.len() > 1 {
        selected.push(colors[1].clone());
    }
    selected
        .into_iter()
        .map(|color| GradientColor {
            r: color.r,
            g: color.g,
            b: color.b,
            hex: format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b),
        })
        .collect()
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a gradient has at most three stops"
)]
fn css_gradient(colors: &[GradientColor]) -> String {
    if colors.is_empty() {
        return "linear-gradient(135deg, #6366f1, #8b5cf6)".to_owned();
    }
    if let [color] = colors {
        return format!("linear-gradient(135deg, {0} 0%, {0} 100%)", color.hex);
    }
    let stops: Vec<String> = colors
        .iter()
        .enumerate()
        .map(|(index, color)| {
            let percent = (index as f64 / (colors.len() - 1) as f64) * 100.0;
            format!("{} {}%", color.hex, percent)
        })
        .collect();
    format!("linear-gradient(135deg, {})", stops.join(", "))
}

fn default_gradient() -> AvatarGradient {
    let color = |r, g, b, hex: &str| GradientColor {
        r,
        g,
        b,
        hex: hex.to_owned(),
    };
    AvatarGradient {
        colors: vec![
            color(99, 102, 241, "#6366f1"),
            color(139, 92, 246, "#8b5cf6"),
            color(236, 72, 153, "#ec4899"),
        ],
        gradient_css: "linear-gradient(135deg, #6366f1 0%, #8b5cf6 50%, #ec4899 100%)".to_owned(),
        dominant_hue: 0.0,
        text_color: "#F9FAFB".into(),
        text_secondary: "#D1D5DB".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0 >> 33
        }
    }

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the fixture generator truncates random words to channels"
    )]
    fn samples(seed: u64) -> Vec<(u8, u8, u8)> {
        let mut rng = Lcg(seed);
        let count = (rng.next() % 160) as usize;
        let palette: Vec<(u8, u8, u8)> = (0..1 + rng.next() % 6)
            .map(|_| (rng.next() as u8, rng.next() as u8, rng.next() as u8))
            .collect();
        (0..count)
            .map(|_| {
                let base = palette[(rng.next() as usize) % palette.len()];
                let jitter = (rng.next() % 40) as i32 - 20;
                let channel = |value: u8| (i32::from(value) + jitter).clamp(0, 255) as u8;
                (channel(base.0), channel(base.1), channel(base.2))
            })
            .collect()
    }

    #[test]
    fn gradients_match_the_old_extraction() {
        for line in include_str!("../../tests/fixtures/legacy_avatar_gradients.tsv").lines() {
            let fields: Vec<&str> = line.split('\t').collect();
            let seed: u64 = fields[0].parse().expect("seed");
            let gradient = gradient_from_samples(&samples(seed));
            let hexes: Vec<&str> = gradient
                .colors
                .iter()
                .map(|color| color.hex.as_str())
                .collect();
            assert_eq!(hexes.join(","), fields[1], "seed {seed}");
            assert_eq!(gradient.gradient_css, fields[2], "seed {seed}");
            assert_eq!(
                format!("{:.9}", gradient.dominant_hue),
                fields[3],
                "seed {seed}"
            );
            assert_eq!(gradient.text_color, fields[4], "seed {seed}");
            assert_eq!(gradient.text_secondary, fields[5], "seed {seed}");
        }
    }

    #[test]
    fn a_single_color_image_keeps_its_color() {
        let image = image::RgbImage::from_pixel(64, 64, image::Rgb([200, 40, 40]));
        let gradient = image_gradient(&image);
        assert_eq!(gradient.colors.len(), 1);
        assert_eq!(
            gradient.gradient_css,
            format!(
                "linear-gradient(135deg, {0} 0%, {0} 100%)",
                gradient.colors[0].hex
            )
        );
    }
}
