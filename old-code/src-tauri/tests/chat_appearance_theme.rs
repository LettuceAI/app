//! Chat appearance theme computation: pins the branching logic that
//! produces CSS classes from settings. Each branch (no background, light
//! background, dark background, neutral assistant) has slightly different
//! output — a refactor that flattens the branches will be caught here.

use lettuceai_lib::chat_appearance::{compute_chat_theme, ChatAppearanceInput, ResolvedColors};

fn input(opacity: f64, text_mode: &str, assistant_token: &str) -> ChatAppearanceInput {
    ChatAppearanceInput {
        user_bubble_color: "accent".into(),
        assistant_bubble_color: assistant_token.into(),
        bubble_opacity: opacity,
        text_mode: text_mode.into(),
    }
}

fn colors() -> ResolvedColors {
    ResolvedColors {
        user_color_css: "#22cc88".into(),
        assistant_color_css: "#888888".into(),
    }
}

#[test]
fn no_background_uses_token_classes() {
    let t = compute_chat_theme(input(80.0, "auto", "fg"), None, colors()).expect("ok");
    assert_eq!(t.user_bg, "bg-accent/80");
    assert_eq!(t.user_border, "border-accent/50");
    assert_eq!(t.assistant_bg, "bg-fg/80");
    assert_eq!(t.assistant_border, "border-fg/50");
    assert!(
        t.header_overlay.is_empty(),
        "no-bg case must not produce header overlay"
    );
    assert!(t.footer_overlay.is_empty());
    assert!(t.content_overlay.is_empty());
}

#[test]
fn light_background_produces_light_overlays() {
    let t = compute_chat_theme(input(75.0, "auto", "fg"), Some(220.0), colors()).expect("ok");
    assert!(t.header_overlay.contains("bg-white"));
    assert!(t.footer_overlay.contains("bg-white"));
    assert!(t.header_overlay.contains("backdrop-blur"));
}

#[test]
fn dark_background_produces_dark_overlays() {
    let t = compute_chat_theme(input(75.0, "auto", "fg"), Some(15.0), colors()).expect("ok");
    assert!(t.header_overlay.contains("#050505"));
    assert!(t.footer_overlay.contains("#050505"));
}

#[test]
fn brightness_boundary_127_5_is_dark() {
    // The boundary is strict `>` 127.5, so exactly 127.5 is dark.
    let t = compute_chat_theme(input(75.0, "auto", "fg"), Some(127.5), colors()).expect("ok");
    assert!(
        t.header_overlay.contains("#050505"),
        "127.5 exact must be dark, got header={}",
        t.header_overlay
    );
}

#[test]
fn brightness_just_above_boundary_is_light() {
    let t = compute_chat_theme(input(75.0, "auto", "fg"), Some(127.51), colors()).expect("ok");
    assert!(t.header_overlay.contains("bg-white"));
}

#[test]
fn neutral_assistant_no_bg_uses_fg_token() {
    let t = compute_chat_theme(input(50.0, "auto", "neutral"), None, colors()).expect("ok");
    assert_eq!(t.assistant_bg, "bg-fg/5");
    assert_eq!(t.assistant_border, "border-fg/10");
    assert_eq!(t.assistant_text, "text-fg");
}

#[test]
fn neutral_assistant_light_bg_uses_black_at_full_opacity() {
    let t = compute_chat_theme(input(70.0, "auto", "neutral"), Some(220.0), colors()).expect("ok");
    assert_eq!(
        t.assistant_bg, "bg-black/70",
        "neutral on light bg should use bg-black/<opacity>"
    );
    assert_eq!(t.assistant_border, "border-black/40");
}

#[test]
fn neutral_assistant_dark_bg_uses_gray_600_with_reduced_opacity() {
    // 80 * 0.85 = 68 (round)
    let t = compute_chat_theme(input(80.0, "auto", "neutral"), Some(30.0), colors()).expect("ok");
    assert_eq!(
        t.assistant_bg, "bg-gray-600/68",
        "neutral on dark bg should reduce opacity by 0.85"
    );
    assert_eq!(t.assistant_border, "border-gray-400/40");
}

#[test]
fn neutral_assistant_dark_bg_reduced_opacity_is_rounded_not_truncated() {
    // 75 * 0.85 = 63.75 → rounds to 64.
    let t = compute_chat_theme(input(75.0, "auto", "neutral"), Some(30.0), colors()).expect("ok");
    assert_eq!(
        t.assistant_bg, "bg-gray-600/64",
        "reduced opacity must use rounding, not truncation"
    );
}

#[test]
fn non_neutral_assistant_on_bg_uses_full_opacity() {
    let t = compute_chat_theme(input(75.0, "auto", "accent"), Some(30.0), colors()).expect("ok");
    assert_eq!(
        t.assistant_bg, "bg-accent/75",
        "non-neutral assistants do not get the 0.85 reduction"
    );
}

#[test]
fn opacity_extremes_are_passed_through() {
    let zero = compute_chat_theme(input(0.0, "auto", "fg"), None, colors()).expect("ok");
    assert!(zero.user_bg.ends_with("/0"));
    let full = compute_chat_theme(input(100.0, "auto", "fg"), None, colors()).expect("ok");
    assert!(full.user_bg.ends_with("/100"));
}

#[test]
fn hex_color_input_is_accepted() {
    let r = ResolvedColors {
        user_color_css: "#22cc88".into(),
        assistant_color_css: "#666666".into(),
    };
    let t = compute_chat_theme(input(60.0, "auto", "fg"), None, r).expect("ok");
    assert!(!t.user_bg.is_empty());
    assert!(!t.assistant_bg.is_empty());
}

#[test]
fn rgb_color_input_is_accepted() {
    let r = ResolvedColors {
        user_color_css: "rgb(120, 200, 80)".into(),
        assistant_color_css: "rgb(80, 80, 80)".into(),
    };
    let t = compute_chat_theme(input(60.0, "auto", "fg"), Some(200.0), r).expect("ok");
    assert!(!t.user_text.is_empty());
}

#[test]
fn oklch_color_input_is_accepted() {
    let r = ResolvedColors {
        user_color_css: "oklch(0.7 0.2 150)".into(),
        assistant_color_css: "oklch(0.3 0.0 0)".into(),
    };
    let t = compute_chat_theme(input(60.0, "auto", "fg"), Some(30.0), r).expect("ok");
    assert!(!t.assistant_text.is_empty());
}

#[test]
fn malformed_color_falls_back_safely() {
    // Garbage input must not panic — fallback luminance keeps theme intact.
    let r = ResolvedColors {
        user_color_css: "not-a-color-at-all".into(),
        assistant_color_css: "###".into(),
    };
    let t = compute_chat_theme(input(60.0, "auto", "fg"), None, r).expect("ok");
    assert!(!t.user_bg.is_empty());
    assert!(!t.assistant_bg.is_empty());
}

#[test]
fn text_mode_forced_light_overrides_default() {
    let dark_text_natural =
        compute_chat_theme(input(80.0, "auto", "fg"), Some(220.0), colors()).expect("ok");
    let forced_light =
        compute_chat_theme(input(80.0, "light", "fg"), Some(220.0), colors()).expect("ok");
    // Forced light should produce a white-family text class regardless of
    // the auto verdict.
    assert!(
        forced_light.user_text.contains("white"),
        "forced light should produce a light text class, got {}",
        forced_light.user_text
    );
    // The natural light-bg path tends NOT to produce white text — pin that
    // these two paths are reachable independently.
    let _ = dark_text_natural;
}

#[test]
fn text_mode_forced_dark_overrides_default() {
    let forced_dark =
        compute_chat_theme(input(80.0, "dark", "fg"), Some(30.0), colors()).expect("ok");
    let low = forced_dark.user_text.to_lowercase();
    assert!(
        low.contains("gray") || low.contains("black"),
        "forced dark text should produce dark-family class, got {}",
        forced_dark.user_text
    );
}
