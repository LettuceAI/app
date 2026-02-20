use serde::{Deserialize, Serialize};

/// Input settings passed from the frontend for theme computation.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatAppearanceInput {
    pub user_bubble_color: String,
    pub assistant_bubble_color: String,
    pub bubble_opacity: f64,
    pub text_mode: String,
}

/// Resolved CSS color values from the frontend (DOM-dependent, so resolved in TS).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedColors {
    pub user_color_css: String,
    pub assistant_color_css: String,
}

/// Theme colors returned to the frontend.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThemeColors {
    pub assistant_bg: String,
    pub assistant_border: String,
    pub assistant_text: String,
    pub user_bg: String,
    pub user_border: String,
    pub user_text: String,
    pub header_overlay: String,
    pub footer_overlay: String,
    pub content_overlay: String,
}

/// Parse a CSS color string (hex, rgb, oklch) to approximate luminance (0.0–1.0).
fn color_to_luminance(color: &str) -> f64 {
    let trimmed = color.trim();

    // Hex: #rgb, #rrggbb, #rrggbbaa
    if trimmed.starts_with('#') {
        let hex = &trimmed[1..];
        let expanded = match hex.len() {
            3 => format!(
                "{}{}{}{}{}{}",
                &hex[0..1],
                &hex[0..1],
                &hex[1..2],
                &hex[1..2],
                &hex[2..3],
                &hex[2..3]
            ),
            6 | 8 => hex[..6].to_string(),
            _ => return 0.5,
        };
        if expanded.len() >= 6 {
            let r = u8::from_str_radix(&expanded[0..2], 16).unwrap_or(128) as f64 / 255.0;
            let g = u8::from_str_radix(&expanded[2..4], 16).unwrap_or(128) as f64 / 255.0;
            let b = u8::from_str_radix(&expanded[4..6], 16).unwrap_or(128) as f64 / 255.0;
            return 0.299 * r + 0.587 * g + 0.114 * b;
        }
    }

    // rgb(r, g, b) or rgba(r, g, b, a)
    if trimmed.starts_with("rgb") {
        let inner = trimmed
            .trim_start_matches("rgba(")
            .trim_start_matches("rgb(")
            .trim_end_matches(')');
        let parts: Vec<&str> = inner.split(|c| c == ',' || c == ' ').collect();
        let nums: Vec<f64> = parts
            .iter()
            .filter_map(|s| s.trim().parse::<f64>().ok())
            .collect();
        if nums.len() >= 3 {
            let r = nums[0] / 255.0;
            let g = nums[1] / 255.0;
            let b = nums[2] / 255.0;
            return 0.299 * r + 0.587 * g + 0.114 * b;
        }
    }

    // oklch(L C H) — approximate by lightness
    if trimmed.starts_with("oklch(") {
        let inner = trimmed
            .trim_start_matches("oklch(")
            .trim_end_matches(')');
        let first_token = inner.split_whitespace().next().unwrap_or("");
        let l = if first_token.ends_with('%') {
            first_token
                .trim_end_matches('%')
                .parse::<f64>()
                .unwrap_or(50.0)
                / 100.0
        } else {
            first_token.parse::<f64>().unwrap_or(0.5)
        };
        return l.clamp(0.0, 1.0);
    }

    0.5
}

/// Determine the text color class based on effective luminance.
fn compute_text_color(
    bg_brightness: Option<f64>,
    bubble_luminance: f64,
    bubble_opacity_01: f64,
    text_mode: &str,
) -> String {
    if text_mode == "light" {
        return "text-white".into();
    }
    if text_mode == "dark" {
        return "text-gray-900".into();
    }

    let effective_lum = match bg_brightness {
        None => bubble_luminance,
        Some(bg) => {
            let bg_lum = bg / 255.0;
            bubble_opacity_01 * bubble_luminance + (1.0 - bubble_opacity_01) * bg_lum
        }
    };

    if effective_lum > 0.45 {
        "text-gray-900".into()
    } else {
        "text-white/95".into()
    }
}

/// Compute full chat theme from appearance settings, background brightness, and resolved CSS colors.
///
/// `bg_brightness`: None if no background image, Some(0.0–255.0) otherwise.
/// `resolved`: CSS color strings resolved from the DOM for the bubble color tokens.
#[tauri::command]
pub fn compute_chat_theme(
    settings: ChatAppearanceInput,
    bg_brightness: Option<f64>,
    resolved: ResolvedColors,
) -> Result<ThemeColors, String> {
    let opacity = settings.bubble_opacity;
    let opacity_01 = opacity / 100.0;
    let opacity_int = opacity as u32;

    let user_lum = color_to_luminance(&resolved.user_color_css);
    let assistant_lum = color_to_luminance(&resolved.assistant_color_css);

    let user_text = compute_text_color(bg_brightness, user_lum, opacity_01, &settings.text_mode);

    let is_neutral_assistant = settings.assistant_bubble_color == "neutral";

    let assistant_text = if is_neutral_assistant && bg_brightness.is_none() {
        // No background: neutral assistant uses bg-fg/5, text inherits fg
        "text-fg".into()
    } else if is_neutral_assistant && bg_brightness.is_some() {
        // With background: neutral assistant uses bg-black or bg-gray-600,
        // NOT the fg token. Use the actual bubble color luminance.
        let is_light = bg_brightness.unwrap() > 127.5;
        let actual_bubble_lum = if is_light { 0.0 } else { 0.35 }; // black ≈ 0, gray-600 ≈ 0.35
        compute_text_color(bg_brightness, actual_bubble_lum, opacity_01, &settings.text_mode)
    } else {
        compute_text_color(bg_brightness, assistant_lum, opacity_01, &settings.text_mode)
    };

    let user_token = &settings.user_bubble_color;
    let assistant_token = &settings.assistant_bubble_color;

    let theme = match bg_brightness {
        None => {
            // No background image
            ThemeColors {
                user_bg: format!("bg-{user_token}/{opacity_int}"),
                user_border: format!("border-{user_token}/50"),
                user_text,
                assistant_bg: if is_neutral_assistant {
                    "bg-fg/5".into()
                } else {
                    format!("bg-{assistant_token}/{opacity_int}")
                },
                assistant_border: if is_neutral_assistant {
                    "border-fg/10".into()
                } else {
                    format!("border-{assistant_token}/50")
                },
                assistant_text,
                header_overlay: String::new(),
                footer_overlay: String::new(),
                content_overlay: String::new(),
            }
        }
        Some(brightness) => {
            let is_light = brightness > 127.5;
            let reduced_opacity = (opacity * 0.85).round() as u32;

            ThemeColors {
                user_bg: format!("bg-{user_token}/{opacity_int}"),
                user_border: format!("border-{user_token}/50"),
                user_text,
                assistant_bg: if is_neutral_assistant {
                    if is_light {
                        format!("bg-black/{opacity_int}")
                    } else {
                        format!("bg-gray-600/{reduced_opacity}")
                    }
                } else {
                    format!("bg-{assistant_token}/{opacity_int}")
                },
                assistant_border: if is_neutral_assistant {
                    if is_light {
                        "border-black/40".into()
                    } else {
                        "border-gray-400/40".into()
                    }
                } else {
                    format!("border-{assistant_token}/50")
                },
                assistant_text,
                header_overlay: if is_light {
                    "bg-white/45 backdrop-blur-md".into()
                } else {
                    "bg-[#050505]/40 backdrop-blur-md".into()
                },
                footer_overlay: if is_light {
                    "bg-white/50 backdrop-blur-md".into()
                } else {
                    "bg-[#050505]/45 backdrop-blur-md".into()
                },
                content_overlay: if is_light {
                    "rgba(255, 255, 255, 0.20)".into()
                } else {
                    "rgba(5, 5, 5, 0.15)".into()
                },
            }
        }
    };

    Ok(theme)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
