//! An old Ollama credential's Sprout probe settings, which it kept in its
//! config JSON (the key in plain text).

use lettuce_models::{OllamaConfig, ProviderConfig, SproutConfig};
use serde_json::{Map, Value};

/// The config keys the Sprout settings read.
pub const LEGACY_SPROUT_CONFIG_KEYS: [&str; 3] = ["sproutEnabled", "sproutUrl", "sproutApiKey"];

/// An old credential's Sprout settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacySprout {
    pub config: ProviderConfig,
    /// The non-empty Sprout key.
    pub key: Option<String>,
    /// The stored URL was not a usable http(s) URL and was left out.
    pub url_rejected: bool,
}

/// The Ollama config an old credential's Sprout settings make; `None` when
/// it had none.
#[must_use]
pub fn legacy_sprout_config(
    provider_kind: &str,
    object: &Map<String, Value>,
) -> Option<LegacySprout> {
    if !provider_kind.eq_ignore_ascii_case("ollama")
        || !LEGACY_SPROUT_CONFIG_KEYS
            .iter()
            .any(|key| object.contains_key(*key))
    {
        return None;
    }
    let text = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let key = text("sproutApiKey");
    let url = text("sproutUrl").trim().to_owned();
    let url_rejected = !url.is_empty() && !lettuce_models::is_valid_provider_endpoint(&url);
    Some(LegacySprout {
        config: ProviderConfig::Ollama(OllamaConfig {
            sprout: Some(SproutConfig {
                enabled: object.get("sproutEnabled") == Some(&Value::Bool(true)),
                url: if url_rejected { String::new() } else { url },
                api_key_ref: None,
            }),
        }),
        key: (!key.trim().is_empty()).then_some(key),
        url_rejected,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn only_ollama_credentials_with_sprout_keys_get_an_ollama_config() {
        let object =
            json!({"sproutEnabled": true, "sproutUrl": " http://h:7777 ", "sproutApiKey": "k"});
        let sprout =
            legacy_sprout_config("ollama", object.as_object().expect("object")).expect("sprout");
        assert_eq!(
            sprout
                .config
                .active_sprout()
                .map(|sprout| sprout.url.as_str()),
            Some("http://h:7777")
        );
        assert_eq!(sprout.key.as_deref(), Some("k"));
        let off = json!({"sproutEnabled": "yes", "sproutUrl": "h:7777", "sproutApiKey": " "});
        let sprout =
            legacy_sprout_config("ollama", off.as_object().expect("object")).expect("sprout");
        assert!(sprout.config.active_sprout().is_none());
        assert!(sprout.url_rejected);
        assert_eq!(sprout.key, None);
        assert!(legacy_sprout_config("openai", object.as_object().expect("object")).is_none());
        assert!(legacy_sprout_config("ollama", &Map::new()).is_none());
    }
}
