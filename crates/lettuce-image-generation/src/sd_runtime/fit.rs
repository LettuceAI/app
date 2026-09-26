//! The stable-diffusion.cpp placement estimate, mirroring upstream
//! `src/core/backend_fit.cpp`. Catalog file sizes stand in for tensor byte
//! counts that are unknown before download.

use serde::{Deserialize, Serialize};

use crate::{DiffusionComponentRole, DiffusionProfile, DiffusionVariant};

pub const MIB: u64 = 1024 * 1024;
pub const AUTO_FIT_GPU_MARGIN_BYTES: u64 = 512 * MIB;
const AUTO_FIT_DIT_RESERVE_BYTES: u64 = 2048 * MIB;
const AUTO_FIT_VAE_RESERVE_BYTES: u64 = 1024 * MIB;
const AUTO_FIT_CONDITIONER_RESERVE_BYTES: u64 = 2048 * MIB;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FitComponent {
    #[serde(rename = "DiT")]
    Dit,
    #[serde(rename = "VAE")]
    Vae,
    Conditioner,
}

impl FitComponent {
    /// The module name sd-server's `--backend` option uses.
    #[must_use]
    pub const fn backend_module(self) -> &'static str {
        match self {
            Self::Dit => "diffusion",
            Self::Conditioner => "te",
            Self::Vae => "vae",
        }
    }
}

/// A GPU the runtime reported, matched to live hardware memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FitDevice {
    pub id: usize,
    pub name: String,
    pub description: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub budget_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateComponent {
    pub name: FitComponent,
    pub params_bytes: u64,
    pub compute_reserve_bytes: u64,
    pub splittable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlanMode {
    DefaultBackend,
    Concurrent,
    TimeShare,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FitPlacement {
    pub component: FitComponent,
    pub params_bytes: u64,
    pub compute_reserve_bytes: u64,
    pub targets: Vec<String>,
    pub cpu: bool,
    pub split: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceSource {
    ConfiguredEnginePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FitEstimate {
    pub model_bytes: u64,
    pub available_ram_bytes: Option<u64>,
    pub plan_mode: PlanMode,
    pub device_source: DeviceSource,
    pub devices: Vec<FitDevice>,
    pub placements: Vec<FitPlacement>,
}

fn components(dit: u64, vae: u64, conditioner: u64) -> Vec<EstimateComponent> {
    vec![
        EstimateComponent {
            name: FitComponent::Dit,
            params_bytes: dit,
            compute_reserve_bytes: AUTO_FIT_DIT_RESERVE_BYTES,
            splittable: true,
        },
        EstimateComponent {
            name: FitComponent::Vae,
            params_bytes: vae,
            compute_reserve_bytes: AUTO_FIT_VAE_RESERVE_BYTES,
            splittable: false,
        },
        EstimateComponent {
            name: FitComponent::Conditioner,
            params_bytes: conditioner,
            compute_reserve_bytes: AUTO_FIT_CONDITIONER_RESERVE_BYTES,
            splittable: true,
        },
    ]
}

/// The estimate inputs of a catalog variant: its diffusion file, the VAE and
/// the text plus vision encoders as the conditioner.
#[must_use]
pub fn catalog_components(
    profile: &DiffusionProfile,
    variant: &DiffusionVariant,
) -> Vec<EstimateComponent> {
    let mut conditioner_bytes = 0_u64;
    let mut vae_bytes = 0_u64;
    for component in &profile.shared_components {
        match component.role {
            DiffusionComponentRole::TextEncoder | DiffusionComponentRole::VisionEncoder => {
                conditioner_bytes = conditioner_bytes.saturating_add(component.bytes);
            }
            DiffusionComponentRole::Vae => vae_bytes = vae_bytes.saturating_add(component.bytes),
            DiffusionComponentRole::DiffusionModel => {}
        }
    }
    components(variant.diffusion.bytes, vae_bytes, conditioner_bytes)
}

/// The estimate inputs from file sizes on disk; a missing file counts as zero.
#[must_use]
pub fn file_components(
    diffusion_bytes: u64,
    text_encoder_bytes: u64,
    vae_bytes: u64,
    vision_encoder_bytes: u64,
) -> Vec<EstimateComponent> {
    components(
        diffusion_bytes,
        vae_bytes,
        text_encoder_bytes.saturating_add(vision_encoder_bytes),
    )
}

#[must_use]
pub fn compute_auto_fit_estimate(
    components: &[EstimateComponent],
    devices: Vec<FitDevice>,
    available_ram_bytes: Option<u64>,
    device_source: DeviceSource,
) -> FitEstimate {
    let model_bytes = components
        .iter()
        .map(|component| component.params_bytes)
        .sum();
    if devices.is_empty() {
        return FitEstimate {
            model_bytes,
            available_ram_bytes,
            plan_mode: PlanMode::DefaultBackend,
            device_source,
            devices,
            placements: components
                .iter()
                .filter(|component| component.params_bytes > 0)
                .map(|component| FitPlacement {
                    component: component.name,
                    params_bytes: component.params_bytes,
                    compute_reserve_bytes: component.compute_reserve_bytes,
                    targets: vec!["CPU".to_owned()],
                    cpu: true,
                    split: false,
                })
                .collect(),
        };
    }

    let mut order = (0..components.len()).collect::<Vec<_>>();
    order.sort_by_key(|index| std::cmp::Reverse(components[*index].params_bytes));

    let mut params_sum = vec![0_u64; devices.len()];
    let mut max_reserve = vec![0_u64; devices.len()];
    let mut concurrent_targets = vec![Vec::<usize>::new(); components.len()];
    let mut concurrent = true;
    for component_index in &order {
        let component = &components[*component_index];
        if component.params_bytes == 0 {
            continue;
        }
        let mut best = None;
        for (device_index, device) in devices.iter().enumerate() {
            let need = params_sum[device_index]
                .saturating_add(component.params_bytes)
                .saturating_add(max_reserve[device_index].max(component.compute_reserve_bytes));
            if need > device.budget_bytes {
                continue;
            }
            let remaining = device.budget_bytes.saturating_sub(params_sum[device_index]);
            if best.is_none_or(|current: usize| {
                remaining
                    > devices[current]
                        .budget_bytes
                        .saturating_sub(params_sum[current])
            }) {
                best = Some(device_index);
            }
        }
        let Some(best) = best else {
            concurrent = false;
            break;
        };
        params_sum[best] = params_sum[best].saturating_add(component.params_bytes);
        max_reserve[best] = max_reserve[best].max(component.compute_reserve_bytes);
        concurrent_targets[*component_index].push(best);
    }

    if concurrent {
        return FitEstimate {
            model_bytes,
            available_ram_bytes,
            plan_mode: PlanMode::Concurrent,
            device_source,
            placements: components
                .iter()
                .enumerate()
                .filter(|(_, component)| component.params_bytes > 0)
                .map(|(index, component)| FitPlacement {
                    component: component.name,
                    params_bytes: component.params_bytes,
                    compute_reserve_bytes: component.compute_reserve_bytes,
                    targets: concurrent_targets[index]
                        .iter()
                        .map(|device_index| devices[*device_index].name.clone())
                        .collect(),
                    cpu: false,
                    split: false,
                })
                .collect(),
            devices,
        };
    }

    let mut targets = vec![Vec::<usize>::new(); components.len()];
    let mut cpu = vec![false; components.len()];
    for component_index in &order {
        let component = &components[*component_index];
        if component.params_bytes == 0 {
            continue;
        }
        let best = devices
            .iter()
            .enumerate()
            .filter(|(_, device)| {
                component
                    .params_bytes
                    .saturating_add(component.compute_reserve_bytes)
                    <= device.budget_bytes
            })
            .max_by_key(|(_, device)| device.budget_bytes)
            .map(|(index, _)| index);
        if let Some(best) = best {
            targets[*component_index].push(best);
            continue;
        }
        if component.splittable && devices.len() > 1 {
            let capacity = devices
                .iter()
                .map(|device| {
                    device
                        .budget_bytes
                        .saturating_sub(component.compute_reserve_bytes)
                })
                .sum::<u64>();
            if component.params_bytes <= capacity {
                let mut device_order = (0..devices.len()).collect::<Vec<_>>();
                device_order.sort_by_key(|index| std::cmp::Reverse(devices[*index].budget_bytes));
                targets[*component_index] = device_order;
                continue;
            }
        }
        cpu[*component_index] = true;
    }

    FitEstimate {
        model_bytes,
        available_ram_bytes,
        plan_mode: PlanMode::TimeShare,
        device_source,
        placements: components
            .iter()
            .enumerate()
            .filter(|(_, component)| component.params_bytes > 0)
            .map(|(index, component)| {
                let on_cpu = cpu[index];
                FitPlacement {
                    component: component.name,
                    params_bytes: component.params_bytes,
                    compute_reserve_bytes: component.compute_reserve_bytes,
                    targets: if on_cpu {
                        vec!["CPU".to_owned()]
                    } else {
                        targets[index]
                            .iter()
                            .map(|device_index| devices[*device_index].name.clone())
                            .collect()
                    },
                    cpu: on_cpu,
                    split: targets[index].len() > 1,
                }
            })
            .collect(),
        devices,
    }
}

/// sd-server's `--backend` value and, for a time-shared plan, the
/// `--params-backend` value that keeps GPU-placed weights on disk.
#[must_use]
pub fn manual_backend_specs(estimate: &FitEstimate) -> (String, Option<String>) {
    let mut runtime = Vec::new();
    let mut params = Vec::new();
    for placement in &estimate.placements {
        let module = placement.component.backend_module();
        let target = if placement.cpu {
            "cpu".to_owned()
        } else {
            placement.targets.join("&")
        };
        runtime.push(format!("{module}={target}"));
        if estimate.plan_mode == PlanMode::TimeShare && !placement.cpu {
            params.push(format!("{module}=disk"));
        }
    }
    (
        runtime.join(","),
        (!params.is_empty()).then(|| params.join(",")),
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn gib(value: u64) -> u64 {
        value * 1024 * MIB
    }

    pub(crate) fn device(name: &str, budget_gib: u64) -> FitDevice {
        let id = name
            .chars()
            .filter(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .unwrap_or(0);
        FitDevice {
            id,
            name: name.to_owned(),
            description: name.to_owned(),
            total_bytes: gib(budget_gib + 1),
            free_bytes: gib(budget_gib) + 512 * MIB,
            budget_bytes: gib(budget_gib),
        }
    }

    fn component(
        name: FitComponent,
        params_gib: u64,
        reserve_gib: u64,
        splittable: bool,
    ) -> EstimateComponent {
        EstimateComponent {
            name,
            params_bytes: gib(params_gib),
            compute_reserve_bytes: gib(reserve_gib),
            splittable,
        }
    }

    fn three_components() -> Vec<EstimateComponent> {
        vec![
            component(FitComponent::Dit, 3, 2, true),
            component(FitComponent::Vae, 1, 1, false),
            component(FitComponent::Conditioner, 2, 2, true),
        ]
    }

    #[test]
    fn auto_fit_estimate_uses_concurrent_placement_when_everything_fits_together() {
        let estimate = compute_auto_fit_estimate(
            &three_components(),
            vec![device("Vulkan0", 8)],
            None,
            DeviceSource::ConfiguredEnginePolicy,
        );
        assert_eq!(estimate.plan_mode, PlanMode::Concurrent);
        assert!(
            estimate
                .placements
                .iter()
                .all(|placement| placement.targets == ["Vulkan0"] && !placement.cpu)
        );
    }

    #[test]
    fn auto_fit_estimate_time_shares_components_that_fit_individually() {
        let estimate = compute_auto_fit_estimate(
            &three_components(),
            vec![device("Vulkan0", 6)],
            None,
            DeviceSource::ConfiguredEnginePolicy,
        );
        assert_eq!(estimate.plan_mode, PlanMode::TimeShare);
        assert!(
            estimate
                .placements
                .iter()
                .all(|placement| placement.targets == ["Vulkan0"] && !placement.cpu)
        );
    }

    #[test]
    fn auto_fit_estimate_splits_only_splittable_components() {
        let components = vec![
            component(FitComponent::Dit, 7, 2, true),
            component(FitComponent::Vae, 6, 1, false),
        ];
        let estimate = compute_auto_fit_estimate(
            &components,
            vec![device("Vulkan0", 6), device("Vulkan1", 6)],
            None,
            DeviceSource::ConfiguredEnginePolicy,
        );
        let find = |name| {
            estimate
                .placements
                .iter()
                .find(|placement| placement.component == name)
                .expect("placement")
        };
        assert!(find(FitComponent::Dit).split);
        assert_eq!(find(FitComponent::Dit).targets, ["Vulkan0", "Vulkan1"]);
        assert!(find(FitComponent::Vae).cpu);
        assert_eq!(find(FitComponent::Vae).targets, ["CPU"]);
    }

    #[test]
    fn auto_fit_estimate_matches_upstream_default_backend_without_a_gpu() {
        let components = vec![component(FitComponent::Dit, 3, 2, true)];
        let estimate = compute_auto_fit_estimate(
            &components,
            Vec::new(),
            Some(gib(16)),
            DeviceSource::ConfiguredEnginePolicy,
        );
        assert_eq!(estimate.plan_mode, PlanMode::DefaultBackend);
        assert!(estimate.placements[0].cpu);
        assert_eq!(estimate.placements[0].targets, ["CPU"]);
    }

    #[test]
    fn manual_backend_specs_preserve_split_targets_and_time_shared_parameters() {
        let estimate = FitEstimate {
            model_bytes: gib(8),
            available_ram_bytes: Some(gib(16)),
            plan_mode: PlanMode::TimeShare,
            device_source: DeviceSource::ConfiguredEnginePolicy,
            devices: vec![device("Vulkan0", 6), device("Vulkan1", 6)],
            placements: vec![
                FitPlacement {
                    component: FitComponent::Dit,
                    params_bytes: gib(7),
                    compute_reserve_bytes: gib(2),
                    targets: vec!["Vulkan0".to_owned(), "Vulkan1".to_owned()],
                    cpu: false,
                    split: true,
                },
                FitPlacement {
                    component: FitComponent::Vae,
                    params_bytes: gib(1),
                    compute_reserve_bytes: gib(1),
                    targets: vec!["CPU".to_owned()],
                    cpu: true,
                    split: false,
                },
            ],
        };
        let (backend, params_backend) = manual_backend_specs(&estimate);
        assert_eq!(backend, "diffusion=Vulkan0&Vulkan1,vae=cpu");
        assert_eq!(params_backend.as_deref(), Some("diffusion=disk"));
    }

    #[test]
    fn catalog_components_sum_encoders_into_the_conditioner() {
        let catalog = crate::diffusion_catalog();
        let qwen = catalog.profile("qwen-image-edit-2511").expect("qwen");
        let variant = &qwen.variants[0];
        let components = catalog_components(qwen, variant);
        let encoders: u64 = qwen
            .shared_components
            .iter()
            .filter(|component| component.role != DiffusionComponentRole::Vae)
            .map(|component| component.bytes)
            .sum();
        assert_eq!(components[0].params_bytes, variant.diffusion.bytes);
        assert_eq!(components[2].params_bytes, encoders);
    }
}
