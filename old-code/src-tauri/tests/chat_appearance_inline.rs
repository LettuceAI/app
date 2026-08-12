//! Gathered from inline tests in src/chat_appearance/mod.rs.

use lettuceai_lib::chat_appearance::{color_to_luminance, compute_text_color};

#[test]
fn test_hex_luminance() {
    assert!((color_to_luminance("#ffffff") - 1.0).abs() < 0.01);
    assert!((color_to_luminance("#000000") - 0.0).abs() < 0.01);
    assert!((color_to_luminance("#808080") - 0.5).abs() < 0.05);
}

#[test]
fn test_rgb_luminance() {
    assert!((color_to_luminance("rgb(255, 255, 255)") - 1.0).abs() < 0.01);
    assert!((color_to_luminance("rgb(0, 0, 0)") - 0.0).abs() < 0.01);
}

#[test]
fn test_oklch_luminance() {
    assert!((color_to_luminance("oklch(0.8 0.1 200)") - 0.8).abs() < 0.01);
    assert!((color_to_luminance("oklch(50% 0.1 200)") - 0.5).abs() < 0.01);
}

#[test]
fn test_text_color_auto_dark_bg() {
    let result = compute_text_color(Some(30.0), 0.3, 0.35, "auto");
    assert_eq!(result, "text-white/95");
}

#[test]
fn test_text_color_auto_light_bg() {
    let result = compute_text_color(Some(200.0), 0.8, 0.35, "auto");
    assert_eq!(result, "text-gray-900");
}

#[test]
fn test_text_color_forced() {
    assert_eq!(compute_text_color(None, 0.5, 0.5, "light"), "text-white");
    assert_eq!(compute_text_color(None, 0.5, 0.5, "dark"), "text-gray-900");
}
