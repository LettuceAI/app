//! Sprout, the hardware probe that runs next to a remote Ollama server: its
//! `/specs` report read into the hardware a fit preview is judged against.

use serde::Deserialize;

const SUPPORTED_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SproutGpu {
    #[serde(default)]
    pub index: Option<u32>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub backend: String,
    #[serde(default)]
    pub memory_total: u64,
    #[serde(default)]
    pub memory_free: u64,
    #[serde(default)]
    pub device_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SproutSpecs {
    #[serde(default)]
    pub total_memory_bytes: u64,
    schema_version: u32,
    #[serde(default)]
    pub available_memory_bytes: u64,
    #[serde(default)]
    pub unified_memory: bool,
    #[serde(default)]
    pub gpus: Vec<SproutGpu>,
}

/// The specs endpoint of a Sprout base URL.
#[must_use]
pub fn sprout_specs_url(base_url: &str) -> String {
    format!("{}/specs", base_url.trim_end_matches('/'))
}

pub fn sprout_preview_specs(endpoint: &str, body: &[u8]) -> Result<SproutSpecs, String> {
    let specs: SproutSpecs = serde_json::from_slice(body)
        .map_err(|error| format!("Invalid Sprout response from {endpoint}: {error}"))?;
    if specs.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported Sprout schema version {}; expected {SUPPORTED_SCHEMA_VERSION}",
            specs.schema_version
        ));
    }
    Ok(specs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_lists_its_gpus_and_refuses_another_schema() {
        let specs = sprout_preview_specs(
            "http://host/specs",
            br#"{"schemaVersion": 1, "availableMemoryBytes": 8, "gpus": [
                {"memoryFree": 3, "deviceType": "IntegratedGpu"},
                {"memoryFree": 4, "deviceType": "Gpu"}]}"#,
        )
        .expect("specs");
        assert_eq!(specs.available_memory_bytes, 8);
        assert_eq!(specs.gpus.len(), 2);
        assert_eq!(specs.gpus[1].memory_free, 4);
        assert!(!specs.unified_memory);
        assert_eq!(
            sprout_preview_specs("e", br#"{"schemaVersion": 2}"#).map(|_| ()),
            Err("Unsupported Sprout schema version 2; expected 1".to_owned())
        );
        assert_eq!(sprout_specs_url("http://h:1/"), "http://h:1/specs");
    }
}
