//! Sprout, the hardware probe that runs next to a remote Ollama server: its
//! `/specs` report read into the hardware runnability is judged against.

use serde::Deserialize;

use crate::RunnabilityHardware;

const SUPPORTED_SCHEMA_VERSION: u32 = 1;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SproutGpu {
    #[serde(default)]
    memory_free: u64,
    #[serde(default)]
    device_type: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SproutSpecs {
    schema_version: u32,
    #[serde(default)]
    available_memory_bytes: u64,
    #[serde(default)]
    unified_memory: bool,
    #[serde(default)]
    gpus: Vec<SproutGpu>,
}

/// The specs endpoint of a Sprout base URL.
#[must_use]
pub fn sprout_specs_url(base_url: &str) -> String {
    format!("{}/specs", base_url.trim_end_matches('/'))
}

/// The hardware a Sprout report describes: the largest free VRAM of a
/// discrete GPU, else of the integrated GPUs (which then share memory).
pub fn sprout_hardware(endpoint: &str, body: &[u8]) -> Result<RunnabilityHardware, String> {
    let specs: SproutSpecs = serde_json::from_slice(body)
        .map_err(|error| format!("Invalid Sprout response from {endpoint}: {error}"))?;
    if specs.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported Sprout schema version {}; expected {SUPPORTED_SCHEMA_VERSION}",
            specs.schema_version
        ));
    }
    let discrete_vram = specs
        .gpus
        .iter()
        .filter(|gpu| gpu.device_type != "IntegratedGpu")
        .map(|gpu| gpu.memory_free)
        .max();
    let all_integrated = !specs.gpus.is_empty() && discrete_vram.is_none();
    let (available_vram, supports_gpu_offload) = match discrete_vram {
        Some(vram) => (Some(vram), true),
        None if all_integrated => (specs.gpus.iter().map(|gpu| gpu.memory_free).max(), true),
        None => (None, false),
    };
    Ok(RunnabilityHardware {
        available_ram: Some(specs.available_memory_bytes),
        available_vram,
        supports_gpu_offload,
        unified_memory: specs.unified_memory || all_integrated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_names_the_discrete_gpu_else_the_shared_integrated_one() {
        let hardware = sprout_hardware(
            "http://host/specs",
            br#"{"schemaVersion": 1, "availableMemoryBytes": 8, "gpus": [
                {"memoryFree": 3, "deviceType": "IntegratedGpu"},
                {"memoryFree": 4, "deviceType": "Gpu"}]}"#,
        )
        .expect("hardware");
        assert_eq!(hardware.available_vram, Some(4));
        assert!(hardware.supports_gpu_offload && !hardware.unified_memory);
        let integrated = sprout_hardware(
            "e",
            br#"{"schemaVersion": 1, "gpus": [{"memoryFree": 3, "deviceType": "IntegratedGpu"}]}"#,
        )
        .expect("hardware");
        assert!(integrated.unified_memory);
        assert_eq!(integrated.available_vram, Some(3));
        let none = sprout_hardware("e", br#"{"schemaVersion": 1}"#).expect("hardware");
        assert!(!none.supports_gpu_offload && none.available_vram.is_none());
        assert_eq!(
            sprout_hardware("e", br#"{"schemaVersion": 2}"#),
            Err("Unsupported Sprout schema version 2; expected 1".to_owned())
        );
        assert_eq!(sprout_specs_url("http://h:1/"), "http://h:1/specs");
    }
}
