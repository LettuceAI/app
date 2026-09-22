use std::collections::HashSet;

use lettuce_models::StableDiffusionLora;

/// The model's LoRAs with each request LoRA replacing the entry for the same
/// file and noise stage, new ones appended in request order.
#[must_use]
pub fn merge_loras(
    base: &[StableDiffusionLora],
    request: &[StableDiffusionLora],
) -> Vec<StableDiffusionLora> {
    let mut merged = base.to_vec();
    for lora in request {
        if let Some(existing) = merged.iter_mut().find(|existing| {
            existing.path == lora.path && existing.is_high_noise == lora.is_high_noise
        }) {
            *existing = lora.clone();
        } else {
            merged.push(lora.clone());
        }
    }
    merged
}

/// The trimmed keywords of the active LoRAs, first spelling kept for keywords
/// that differ only in case.
#[must_use]
pub fn lora_keywords(loras: &[StableDiffusionLora]) -> Vec<String> {
    let mut keywords = Vec::new();
    let mut seen = HashSet::new();
    for keyword in loras.iter().flat_map(|lora| lora.keywords.iter()) {
        let keyword = keyword.trim();
        if keyword.is_empty() {
            continue;
        }
        if seen.insert(keyword.to_lowercase()) {
            keywords.push(keyword.to_owned());
        }
    }
    keywords
}

/// The prompt sent to every image provider: the model's pre-prompt, then the
/// LoRA keywords the prompt does not already contain, then the prompt.
#[must_use]
pub fn compose_image_prompt(
    prompt: &str,
    pre_prompt: Option<&str>,
    lora_keywords: &[String],
) -> String {
    let mut parts = Vec::new();
    let existing_prompt_text =
        format!("{}\n{}", pre_prompt.unwrap_or_default(), prompt).to_lowercase();
    if let Some(pre_prompt) = pre_prompt.map(str::trim).filter(|value| !value.is_empty()) {
        parts.push(pre_prompt.to_owned());
    }
    parts.extend(
        lora_keywords
            .iter()
            .map(|keyword| keyword.trim())
            .filter(|keyword| !keyword.is_empty())
            .filter(|keyword| !existing_prompt_text.contains(&keyword.to_lowercase()))
            .map(str::to_owned),
    );
    let prompt = prompt.trim();
    if !prompt.is_empty() {
        parts.push(prompt.to_owned());
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lora(path: &str, multiplier: f64, keywords: &[&str]) -> StableDiffusionLora {
        StableDiffusionLora {
            path: path.to_owned(),
            multiplier,
            is_high_noise: false,
            keywords: keywords.iter().map(|&keyword| keyword.to_owned()).collect(),
        }
    }

    #[test]
    fn pre_prompt_is_applied_once_before_the_user_prompt() {
        assert_eq!(
            compose_image_prompt("a portrait", Some("cinematic lighting"), &[]),
            "cinematic lighting, a portrait"
        );
        assert_eq!(
            compose_image_prompt("a portrait", Some("  "), &[]),
            "a portrait"
        );
        assert_eq!(
            compose_image_prompt("  ", Some("cinematic lighting"), &[]),
            "cinematic lighting"
        );
    }

    #[test]
    fn lora_keywords_are_inserted_between_the_pre_prompt_and_user_prompt() {
        let keywords = vec!["ArsMovieStill".to_owned(), "cinematic still".to_owned()];
        assert_eq!(
            compose_image_prompt("a portrait", Some("high detail"), &keywords),
            "high detail, ArsMovieStill, cinematic still, a portrait"
        );
    }

    #[test]
    fn lora_keywords_already_written_by_the_scene_writer_are_not_duplicated() {
        let keywords = vec!["ArsSamuel".to_owned(), "MayaTrigger".to_owned()];
        assert_eq!(
            compose_image_prompt("ArsSamuel offers a mug to MayaTrigger", None, &keywords),
            "ArsSamuel offers a mug to MayaTrigger"
        );
    }

    #[test]
    fn request_lora_keywords_override_model_level_keywords_for_the_same_lora() {
        let base = vec![lora("style.safetensors", 0.8, &["old trigger"])];
        let request = vec![lora("style.safetensors", 1.0, &["new trigger"])];
        assert_eq!(
            lora_keywords(&merge_loras(&base, &request)),
            vec!["new trigger"]
        );
    }

    #[test]
    fn high_noise_loras_are_separate_entries_and_keywords_dedupe_by_case() {
        let base = vec![lora("a.safetensors", 0.8, &["Trigger", " "])];
        let mut high_noise = lora("a.safetensors", 0.5, &["trigger", "second"]);
        high_noise.is_high_noise = true;
        let merged = merge_loras(&base, std::slice::from_ref(&high_noise));
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[1], high_noise);
        assert_eq!(lora_keywords(&merged), vec!["Trigger", "second"]);
    }
}
