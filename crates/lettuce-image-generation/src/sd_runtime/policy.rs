//! Per-runtime GPU selection for stable-diffusion.cpp, as legacy stored it
//! next to each installed engine build.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use super::fit::{AUTO_FIT_GPU_MARGIN_BYTES, FitDevice};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ComputePolicy {
    pub multi_gpu_enabled: bool,
    pub gpu_device_ids: Vec<usize>,
    pub single_gpu_device_id: Option<usize>,
    pub device_budgets_gib: BTreeMap<usize, f64>,
    pub split_mode: String,
}

impl Default for ComputePolicy {
    fn default() -> Self {
        Self {
            multi_gpu_enabled: false,
            gpu_device_ids: Vec::new(),
            single_gpu_device_id: None,
            device_budgets_gib: BTreeMap::new(),
            split_mode: "layer".to_owned(),
        }
    }
}

impl ComputePolicy {
    /// Automatic placement: no multi-GPU set and no single-GPU override.
    #[must_use]
    pub const fn is_automatic(&self) -> bool {
        !self.multi_gpu_enabled && self.single_gpu_device_id.is_none()
    }

    /// Reads a stored policy file, migrating the older name-based shape
    /// (a `mode` key) to hardware device ids.
    pub fn from_stored_json(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        let value: serde_json::Value = serde_json::from_slice(bytes)?;
        if value.get("mode").is_some() {
            let legacy: LegacyComputePolicy = serde_json::from_value(value)?;
            return Ok(migrate_legacy_compute_policy(legacy));
        }
        serde_json::from_value(value)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyComputePolicy {
    mode: String,
    selected_devices: Vec<String>,
    device_budgets_gib: BTreeMap<String, f64>,
    split_mode: String,
}

fn backend_device_index(name: &str) -> Option<usize> {
    let digits = name
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
}

fn migrate_legacy_compute_policy(policy: LegacyComputePolicy) -> ComputePolicy {
    let gpu_device_ids = policy
        .selected_devices
        .iter()
        .filter_map(|name| backend_device_index(name))
        .collect::<Vec<_>>();
    let device_budgets_gib = policy
        .device_budgets_gib
        .into_iter()
        .filter_map(|(name, budget)| backend_device_index(&name).map(|id| (id, budget)))
        .collect();
    ComputePolicy {
        multi_gpu_enabled: policy.mode == "multi",
        single_gpu_device_id: (policy.mode == "single")
            .then(|| gpu_device_ids.first().copied())
            .flatten(),
        gpu_device_ids,
        device_budgets_gib,
        split_mode: policy.split_mode,
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ComputePolicyError {
    #[error("Split mode must be layer or row.")]
    SplitMode,
    #[error("Row splitting is available only with a CUDA engine build.")]
    RowSplitNeedsCuda,
    #[error("Selected GPU devices must be unique.")]
    DuplicateDevices,
    #[error("GPU device #{0} is not available to this engine.")]
    UnavailableDevice(usize),
    #[error("A single-GPU override cannot be active while multi-GPU is enabled.")]
    SingleWithMulti,
    #[error("Multi-GPU mode requires at least two selected GPUs.")]
    MultiNeedsTwo,
    #[error("A VRAM budget was provided for unavailable GPU #{0}.")]
    BudgetForUnavailable(usize),
    #[error("The VRAM budget for GPU #{0} must be greater than zero.")]
    BudgetNotPositive(usize),
    #[error("The VRAM budget for GPU #{id} exceeds its total memory ({total_gib:.1} GiB).")]
    BudgetOverTotal { id: usize, total_gib: f64 },
}

pub fn validate_compute_policy(
    policy: &ComputePolicy,
    backend: &str,
    devices: &[FitDevice],
) -> Result<(), ComputePolicyError> {
    if !matches!(policy.split_mode.as_str(), "layer" | "row") {
        return Err(ComputePolicyError::SplitMode);
    }
    if policy.split_mode == "row" && backend != "cuda" {
        return Err(ComputePolicyError::RowSplitNeedsCuda);
    }
    let available = devices
        .iter()
        .map(|device| device.id)
        .collect::<HashSet<_>>();
    let selected = policy
        .gpu_device_ids
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    if selected.len() != policy.gpu_device_ids.len() {
        return Err(ComputePolicyError::DuplicateDevices);
    }
    if let Some(missing) = policy
        .gpu_device_ids
        .iter()
        .find(|id| !available.contains(id))
    {
        return Err(ComputePolicyError::UnavailableDevice(*missing));
    }
    if let Some(single_gpu_device_id) = policy.single_gpu_device_id {
        if !available.contains(&single_gpu_device_id) {
            return Err(ComputePolicyError::UnavailableDevice(single_gpu_device_id));
        }
        if policy.multi_gpu_enabled {
            return Err(ComputePolicyError::SingleWithMulti);
        }
    }
    if policy.multi_gpu_enabled && policy.gpu_device_ids.len() < 2 {
        return Err(ComputePolicyError::MultiNeedsTwo);
    }
    for (id, budget) in &policy.device_budgets_gib {
        let device = devices
            .iter()
            .find(|device| device.id == *id)
            .ok_or(ComputePolicyError::BudgetForUnavailable(*id))?;
        if !budget.is_finite() || *budget <= 0.0 {
            return Err(ComputePolicyError::BudgetNotPositive(*id));
        }
        let total_gib = device.total_bytes as f64 / 1024_f64.powi(3);
        if *budget > total_gib {
            return Err(ComputePolicyError::BudgetOverTotal { id: *id, total_gib });
        }
    }
    Ok(())
}

pub fn apply_policy_budgets(policy: &ComputePolicy, devices: &mut [FitDevice]) {
    for device in devices.iter_mut() {
        if let Some(gib) = policy.device_budgets_gib.get(&device.id) {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "legacy rounds a validated positive GiB budget to bytes"
            )]
            let requested = (*gib * 1024_f64.powi(3)).round() as u64;
            device.budget_bytes = requested.min(device.free_bytes);
        }
    }
}

/// The devices a policy lets the engine use, with their budgets applied.
#[must_use]
pub fn devices_for_policy(policy: &ComputePolicy, mut devices: Vec<FitDevice>) -> Vec<FitDevice> {
    if policy.multi_gpu_enabled {
        devices.retain(|device| policy.gpu_device_ids.contains(&device.id));
    } else if let Some(single_gpu_device_id) = policy.single_gpu_device_id {
        devices.retain(|device| device.id == single_gpu_device_id);
    }
    apply_policy_budgets(policy, &mut devices);
    devices
}

/// sd-server's `--max-vram` value: runtime device names with trimmed GiB.
#[must_use]
pub fn max_vram_spec(policy: &ComputePolicy, devices: &[FitDevice]) -> Option<String> {
    let assignments = devices
        .iter()
        .filter_map(|device| {
            let budget = policy.device_budgets_gib.get(&device.id)?;
            let mut value = format!("{budget:.3}");
            while value.contains('.') && value.ends_with('0') {
                value.pop();
            }
            if value.ends_with('.') {
                value.pop();
            }
            Some(format!("{}={value}", device.name.to_ascii_lowercase()))
        })
        .collect::<Vec<_>>();
    (!assignments.is_empty()).then(|| assignments.join(","))
}

/// A device line from `sd-server --list-devices`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDevice {
    pub name: String,
    pub description: String,
}

/// Parses `--list-devices` output: tab-separated name and description, the
/// CPU device left out.
#[must_use]
pub fn parse_runtime_devices(output: &str) -> Vec<RuntimeDevice> {
    output
        .lines()
        .filter_map(|line| {
            let (name, description) = line.split_once('\t')?;
            let name = name.trim();
            let description = description.trim();
            (!name.is_empty() && !name.eq_ignore_ascii_case("cpu")).then(|| RuntimeDevice {
                name: name.to_owned(),
                description: description.to_owned(),
            })
        })
        .collect()
}

/// A system GPU with live memory, as the llama.cpp device list reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareGpu {
    pub index: usize,
    pub name: String,
    pub description: String,
    pub memory_total: u64,
    pub memory_free: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "The selected engine reported GPU devices, but their live memory could not be matched to the system GPU inventory."
)]
pub struct UnmatchedRuntimeDevices;

fn normalized_device_identity(value: &str) -> String {
    value
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect()
}

/// Matches each engine device to an unused system GPU by name, then by
/// description; the budget leaves the legacy 512 MiB margin.
pub fn match_runtime_devices(
    runtime_devices: Vec<RuntimeDevice>,
    hardware: &[HardwareGpu],
) -> Result<Vec<FitDevice>, UnmatchedRuntimeDevices> {
    let runtime_has_gpu = !runtime_devices.is_empty();
    let mut matched = Vec::new();
    let mut used_hardware = HashSet::new();
    for runtime_device in runtime_devices {
        let runtime_name = normalized_device_identity(&runtime_device.name);
        let runtime_description = normalized_device_identity(&runtime_device.description);
        let hardware_index = hardware
            .iter()
            .enumerate()
            .filter(|(index, _)| !used_hardware.contains(index))
            .find(|(_, device)| normalized_device_identity(&device.name) == runtime_name)
            .map(|(index, _)| index)
            .or_else(|| {
                hardware
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| !used_hardware.contains(index))
                    .find(|(_, device)| {
                        !runtime_description.is_empty()
                            && normalized_device_identity(&device.description)
                                == runtime_description
                    })
                    .map(|(index, _)| index)
            });
        if let Some(hardware_index) = hardware_index {
            used_hardware.insert(hardware_index);
            let device = &hardware[hardware_index];
            matched.push(FitDevice {
                id: device.index,
                name: runtime_device.name,
                description: runtime_device.description,
                total_bytes: device.memory_total,
                free_bytes: device.memory_free,
                budget_bytes: device.memory_free.saturating_sub(AUTO_FIT_GPU_MARGIN_BYTES),
            });
        }
    }
    if runtime_has_gpu && matched.is_empty() {
        return Err(UnmatchedRuntimeDevices);
    }
    Ok(matched)
}

#[cfg(test)]
mod tests {
    use super::super::fit::tests::{device, gib};
    use super::*;

    #[test]
    fn compute_policy_validates_manual_device_counts_and_split_support() {
        let devices = vec![device("Vulkan0", 8), device("Vulkan1", 8)];
        let conflicting_modes = ComputePolicy {
            multi_gpu_enabled: true,
            gpu_device_ids: vec![0, 1],
            single_gpu_device_id: Some(0),
            ..ComputePolicy::default()
        };
        assert!(validate_compute_policy(&conflicting_modes, "vulkan", &devices).is_err());
        let multi_with_one_device = ComputePolicy {
            multi_gpu_enabled: true,
            gpu_device_ids: vec![0],
            ..ComputePolicy::default()
        };
        assert_eq!(
            validate_compute_policy(&multi_with_one_device, "vulkan", &devices),
            Err(ComputePolicyError::MultiNeedsTwo)
        );
        let row_split = ComputePolicy {
            multi_gpu_enabled: true,
            gpu_device_ids: vec![0, 1],
            split_mode: "row".to_owned(),
            ..ComputePolicy::default()
        };
        assert_eq!(
            validate_compute_policy(&row_split, "vulkan", &devices),
            Err(ComputePolicyError::RowSplitNeedsCuda)
        );
        assert!(validate_compute_policy(&row_split, "cuda", &devices).is_ok());
        let over_budget = ComputePolicy {
            device_budgets_gib: BTreeMap::from([(0, 10.0)]),
            ..ComputePolicy::default()
        };
        assert_eq!(
            validate_compute_policy(&over_budget, "vulkan", &devices)
                .map_err(|error| error.to_string()),
            Err("The VRAM budget for GPU #0 exceeds its total memory (9.0 GiB).".to_owned())
        );
    }

    #[test]
    fn compute_policy_filters_devices_and_applies_per_device_budgets() {
        let policy = ComputePolicy {
            multi_gpu_enabled: true,
            gpu_device_ids: vec![0, 2],
            device_budgets_gib: BTreeMap::from([(0, 4.5)]),
            ..ComputePolicy::default()
        };
        let selected = devices_for_policy(
            &policy,
            vec![
                device("Vulkan0", 8),
                device("Vulkan1", 8),
                device("Vulkan2", 8),
            ],
        );
        assert_eq!(
            selected
                .iter()
                .map(|device| device.name.as_str())
                .collect::<Vec<_>>(),
            ["Vulkan0", "Vulkan2"]
        );
        assert_eq!(
            selected[0].budget_bytes,
            (4.5 * 1024_f64.powi(3)).round() as u64
        );
        assert_eq!(selected[1].budget_bytes, gib(8));
    }

    #[test]
    fn compute_policy_single_gpu_override_uses_the_hardware_device_id() {
        let policy = ComputePolicy {
            gpu_device_ids: vec![0, 2],
            single_gpu_device_id: Some(2),
            ..ComputePolicy::default()
        };
        let selected = devices_for_policy(
            &policy,
            vec![
                device("Vulkan0", 8),
                device("Vulkan1", 8),
                device("Vulkan2", 8),
            ],
        );
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, 2);
        assert_eq!(selected[0].name, "Vulkan2");
    }

    #[test]
    fn max_vram_uses_runtime_names_for_hardware_id_budgets() {
        let policy = ComputePolicy {
            multi_gpu_enabled: true,
            gpu_device_ids: vec![0, 2],
            device_budgets_gib: BTreeMap::from([(0, 4.5), (2, 7.0)]),
            ..ComputePolicy::default()
        };
        let devices = devices_for_policy(
            &policy,
            vec![device("CUDA0", 8), device("CUDA1", 8), device("CUDA2", 8)],
        );
        assert_eq!(
            max_vram_spec(&policy, &devices).as_deref(),
            Some("cuda0=4.5,cuda2=7")
        );
    }

    #[test]
    fn legacy_name_based_policy_migrates_to_hardware_device_ids() {
        let migrated = ComputePolicy::from_stored_json(
            br#"{"mode":"multi","selectedDevices":["Vulkan0","Vulkan2"],"deviceBudgetsGib":{"Vulkan2":6.5},"splitMode":"layer"}"#,
        )
        .expect("legacy policy");
        assert!(migrated.multi_gpu_enabled);
        assert_eq!(migrated.gpu_device_ids, [0, 2]);
        assert_eq!(migrated.single_gpu_device_id, None);
        assert_eq!(migrated.device_budgets_gib.get(&2), Some(&6.5));
        let current =
            ComputePolicy::from_stored_json(br#"{"singleGpuDeviceId":1}"#).expect("current policy");
        assert_eq!(current.single_gpu_device_id, Some(1));
        assert_eq!(current.split_mode, "layer");
    }

    #[test]
    fn engine_devices_match_hardware_by_name_then_description() {
        let runtime = parse_runtime_devices(
            "CPU\tAMD Ryzen\nVulkan0\tAMD Radeon RX 7900 XTX\nVulkan1\tNVIDIA RTX 4090\n",
        );
        assert_eq!(runtime.len(), 2);
        let hardware = vec![
            HardwareGpu {
                index: 3,
                name: "NVIDIA RTX 4090".to_owned(),
                description: "NVIDIA RTX 4090".to_owned(),
                memory_total: gib(24),
                memory_free: gib(20),
            },
            HardwareGpu {
                index: 0,
                name: "Vulkan0".to_owned(),
                description: "AMD Radeon RX 7900 XTX".to_owned(),
                memory_total: gib(24),
                memory_free: gib(22),
            },
        ];
        let matched = match_runtime_devices(runtime, &hardware).expect("matched");
        assert_eq!(matched[0].id, 0);
        assert_eq!(matched[1].id, 3);
        assert_eq!(matched[1].budget_bytes, gib(20) - 512 * 1024 * 1024);
        assert_eq!(
            match_runtime_devices(
                vec![RuntimeDevice {
                    name: "CUDA0".to_owned(),
                    description: "Unknown".to_owned(),
                }],
                &[],
            ),
            Err(UnmatchedRuntimeDevices)
        );
    }
}
