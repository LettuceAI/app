use serde::Deserialize;

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

#[derive(Debug)]
pub(super) struct SproutHardware {
    pub available_ram: u64,
    pub available_vram: Option<u64>,
    pub supports_gpu_offload: bool,
    pub unified: bool,
}

pub(super) async fn fetch_sprout_hardware(
    url: &str,
    api_key: Option<&str>,
) -> Result<SproutHardware, String> {
    let endpoint = format!("{}/specs", url.trim_end_matches('/'));
    let client = super::build_client()?;
    let mut request = client.get(&endpoint);
    if let Some(key) = api_key {
        if !key.trim().is_empty() {
            request = request.bearer_auth(key);
        }
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("Failed to reach Sprout at {endpoint}: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Sprout at {endpoint} returned {}",
            response.status()
        ));
    }

    let specs: SproutSpecs = response
        .json()
        .await
        .map_err(|e| format!("Invalid Sprout response from {endpoint}: {e}"))?;

    derive_hardware(&specs)
}

fn derive_hardware(specs: &SproutSpecs) -> Result<SproutHardware, String> {
    if specs.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported Sprout schema version {}; expected {}",
            specs.schema_version, SUPPORTED_SCHEMA_VERSION
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

    Ok(SproutHardware {
        available_ram: specs.available_memory_bytes,
        available_vram,
        supports_gpu_offload,
        unified: specs.unified_memory || all_integrated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn specs(schema_version: u32) -> SproutSpecs {
        SproutSpecs {
            schema_version,
            available_memory_bytes: 8 * 1024 * 1024 * 1024,
            unified_memory: false,
            gpus: vec![SproutGpu {
                memory_free: 4 * 1024 * 1024 * 1024,
                device_type: "Gpu".to_string(),
            }],
        }
    }

    #[test]
    fn accepts_supported_schema() {
        let hardware = derive_hardware(&specs(SUPPORTED_SCHEMA_VERSION)).unwrap();
        assert_eq!(hardware.available_vram, Some(4 * 1024 * 1024 * 1024));
        assert!(hardware.supports_gpu_offload);
    }

    #[test]
    fn rejects_unsupported_schema() {
        let error = derive_hardware(&specs(SUPPORTED_SCHEMA_VERSION + 1)).unwrap_err();
        assert!(error.contains("Unsupported Sprout schema version"));
    }
}
