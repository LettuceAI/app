//! What the playground form offers per provider and model: sizes, samplers,
//! schedulers, whether a negative prompt applies and the options only some
//! providers take. The values are a bundled resource, read once.

use std::sync::OnceLock;

use serde::Deserialize;

const CAPABILITIES_JSON: &str = include_str!("../resources/image-capabilities.json");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelSizes {
    #[serde(rename = "match")]
    matching: SizeMatch,
    id: String,
    sizes: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SizeMatch {
    Exact,
    Prefix,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderSizes {
    providers: Vec<String>,
    #[serde(default)]
    models: Vec<ModelSizes>,
    #[serde(default)]
    sizes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderOptions {
    providers: Vec<String>,
    #[serde(default)]
    quality: Vec<String>,
    #[serde(default)]
    styles: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SdcppOptions {
    samplers: Vec<String>,
    schedulers: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageCapabilityCatalog {
    #[serde(rename = "format_version")]
    _format_version: u32,
    max_count: u32,
    negative_prompt_providers: Vec<String>,
    default_sizes: Vec<String>,
    sizes: Vec<ProviderSizes>,
    options: Vec<ProviderOptions>,
    sdcpp: SdcppOptions,
}

/// What a provider and model offer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageCapabilityValues {
    pub sizes: Vec<String>,
    pub samplers: Vec<String>,
    pub schedulers: Vec<String>,
    pub negative_prompt: bool,
    pub quality: Vec<String>,
    pub styles: Vec<String>,
    pub max_count: u32,
}

/// The bundled capability values.
///
/// # Panics
///
/// Panics when the bundled resource is not valid, which the crate's tests
/// rule out.
#[must_use]
pub fn image_capability_catalog() -> &'static ImageCapabilityCatalog {
    static CATALOG: OnceLock<ImageCapabilityCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(CAPABILITIES_JSON).expect("the bundled image capabilities are valid")
    })
}

fn listed(providers: &[String], provider_kind: &str) -> bool {
    providers
        .iter()
        .any(|provider| provider.eq_ignore_ascii_case(provider_kind))
}

impl ImageCapabilityCatalog {
    /// The values for a provider kind and, where sizes depend on it, the
    /// model's id at the provider.
    #[must_use]
    pub fn values(&self, provider_kind: &str, model: Option<&str>) -> ImageCapabilityValues {
        let model = model.unwrap_or_default();
        let sizes = self
            .sizes
            .iter()
            .filter(|entry| listed(&entry.providers, provider_kind))
            .find_map(|entry| {
                entry
                    .models
                    .iter()
                    .find(|candidate| match candidate.matching {
                        SizeMatch::Exact => candidate.id == model,
                        SizeMatch::Prefix => model.starts_with(&candidate.id),
                    })
                    .map(|candidate| candidate.sizes.clone())
                    .or_else(|| (!entry.sizes.is_empty()).then(|| entry.sizes.clone()))
            })
            .unwrap_or_else(|| self.default_sizes.clone());
        let local = provider_kind.eq_ignore_ascii_case(crate::LOCAL_DIFFUSION_PROVIDER_KIND);
        let options = self
            .options
            .iter()
            .find(|entry| listed(&entry.providers, provider_kind));
        ImageCapabilityValues {
            sizes,
            samplers: if local {
                self.sdcpp.samplers.clone()
            } else {
                Vec::new()
            },
            schedulers: if local {
                self.sdcpp.schedulers.clone()
            } else {
                Vec::new()
            },
            negative_prompt: listed(&self.negative_prompt_providers, provider_kind),
            quality: options
                .map(|entry| entry.quality.clone())
                .unwrap_or_default(),
            styles: options
                .map(|entry| entry.styles.clone())
                .unwrap_or_default(),
            max_count: self.max_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_follow_the_provider_and_model() {
        let catalog = image_capability_catalog();
        assert_eq!(
            catalog.values("openai", Some("dall-e-3")).sizes,
            ["1024x1024", "1024x1792", "1792x1024"]
        );
        assert_eq!(
            catalog.values("openai", Some("gpt-image-1.5")).sizes.last(),
            Some(&"auto".to_owned())
        );
        assert_eq!(
            catalog.values("openai", Some("some-other")).sizes,
            ["1024x1024"]
        );
        assert_eq!(catalog.values("sdcpp", None).sizes.len(), 5);
        assert_eq!(catalog.values("unknown", None).sizes, ["1024x1024"]);
    }

    #[test]
    fn options_and_lists_belong_to_their_providers() {
        let catalog = image_capability_catalog();
        let openai = catalog.values("openai", Some("dall-e-3"));
        assert_eq!(openai.quality, ["standard", "hd"]);
        assert_eq!(openai.styles, ["vivid", "natural"]);
        assert!(!openai.negative_prompt);
        assert!(openai.samplers.is_empty());
        let local = catalog.values("sdcpp", None);
        assert!(local.negative_prompt);
        assert!(local.samplers.contains(&"euler_a".to_owned()));
        assert!(local.schedulers.contains(&"karras".to_owned()));
        assert!(local.quality.is_empty());
        assert_eq!(local.max_count, 8);
    }
}
