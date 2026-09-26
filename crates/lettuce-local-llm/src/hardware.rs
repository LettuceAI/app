//! What the machine has for llama.cpp: available RAM, the free VRAM the
//! backend reports (capped on Windows by the DXGI local-memory budget), the
//! GPU devices and their per-device memory, and whether memory is unified.

#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_MEMORY_SEGMENT_GROUP_LOCAL,
    DXGI_QUERY_VIDEO_MEMORY_INFO, IDXGIAdapter1, IDXGIAdapter3, IDXGIFactory6,
};
#[cfg(target_os = "windows")]
use windows::core::Interface;

use llama_cpp_sys_2::{
    GGML_BACKEND_DEVICE_TYPE_ACCEL, GGML_BACKEND_DEVICE_TYPE_GPU, GGML_BACKEND_DEVICE_TYPE_IGPU,
    ggml_backend_dev_count, ggml_backend_dev_get, ggml_backend_dev_memory, ggml_backend_dev_type,
};

use crate::context::{align_per_device_vram, choose_effective_vram_bytes};

/// One GPU or accelerator llama.cpp can use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LlamaGpuDeviceInfo {
    pub index: usize,
    pub name: String,
    pub description: String,
    pub backend: String,
    pub memory_total: u64,
    pub memory_free: u64,
    pub device_type: String,
}

#[must_use]
pub fn get_available_memory_bytes() -> Option<u64> {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    Some(sys.available_memory())
}

fn ggml_available_vram_bytes() -> Option<u64> {
    let mut max_free: u64 = 0;
    unsafe {
        let count = ggml_backend_dev_count();
        for i in 0..count {
            let dev = ggml_backend_dev_get(i);
            if dev.is_null() {
                continue;
            }
            let dev_type = ggml_backend_dev_type(dev);
            let is_gpu_like = dev_type == GGML_BACKEND_DEVICE_TYPE_GPU
                || dev_type == GGML_BACKEND_DEVICE_TYPE_IGPU
                || dev_type == GGML_BACKEND_DEVICE_TYPE_ACCEL;
            if !is_gpu_like {
                continue;
            }
            let mut free: usize = 0;
            let mut total: usize = 0;
            ggml_backend_dev_memory(dev, &mut free, &mut total);
            if total == 0 {
                continue;
            }
            let free_u64 = free as u64;
            if free_u64 > max_free {
                max_free = free_u64;
            }
        }
    }
    if max_free > 0 { Some(max_free) } else { None }
}

#[cfg(target_os = "windows")]
fn windows_local_vram_cap_bytes() -> Option<u64> {
    unsafe {
        let factory: IDXGIFactory6 = CreateDXGIFactory1().ok()?;
        let mut best: u64 = 0;
        let mut index: u32 = 0;

        loop {
            let adapter: IDXGIAdapter1 = match factory.EnumAdapters1(index) {
                Ok(adapter) => adapter,
                Err(_) => break,
            };
            index = index.saturating_add(1);

            let desc = match adapter.GetDesc1() {
                Ok(desc) => desc,
                Err(_) => continue,
            };
            if desc.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0 {
                continue;
            }

            let dedicated_bytes = desc.DedicatedVideoMemory as u64;
            if dedicated_bytes == 0 {
                continue;
            }

            let local_available_bytes = adapter
                .cast::<IDXGIAdapter3>()
                .ok()
                .and_then(|adapter3: IDXGIAdapter3| {
                    let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
                    adapter3
                        .QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut info)
                        .ok()?;
                    Some(
                        info.Budget
                            .saturating_sub(info.CurrentUsage)
                            .min(dedicated_bytes),
                    )
                })
                .unwrap_or(dedicated_bytes);

            if local_available_bytes > best {
                best = local_available_bytes;
            }
        }

        (best > 0).then_some(best)
    }
}

#[cfg(not(target_os = "windows"))]
fn windows_local_vram_cap_bytes() -> Option<u64> {
    None
}

pub fn get_available_vram_bytes() -> Option<u64> {
    choose_effective_vram_bytes(ggml_available_vram_bytes(), windows_local_vram_cap_bytes())
}

/// The GPUs, accelerators and integrated GPUs llama.cpp sees, so an APU (a
/// Ryzen AI handheld, for one) has a device to pick; `device_type` tells them
/// apart (`IntegratedGpu`), and multi-GPU still takes only discrete devices.
pub fn list_gpu_devices() -> Vec<LlamaGpuDeviceInfo> {
    llama_cpp_2::list_llama_ggml_backend_devices()
        .into_iter()
        .filter(|device| {
            matches!(
                device.device_type,
                llama_cpp_2::LlamaBackendDeviceType::Gpu
                    | llama_cpp_2::LlamaBackendDeviceType::Accelerator
                    | llama_cpp_2::LlamaBackendDeviceType::IntegratedGpu
            )
        })
        .map(|device| LlamaGpuDeviceInfo {
            index: device.index,
            name: device.name,
            description: device.description,
            backend: device.backend,
            memory_total: device.memory_total as u64,
            memory_free: device.memory_free as u64,
            device_type: format!("{:?}", device.device_type),
        })
        .collect()
}

/// Per-device free/total VRAM for the explicitly selected device ids, preserving
/// the caller's order. Integrated GPUs are skipped (never used for multi-GPU).
/// Each tuple is `(device_id, free_bytes, total_bytes)`.
pub fn get_per_device_free_vram(
    device_ids: &[usize],
    include_integrated: bool,
) -> Vec<(usize, u64, u64)> {
    let mut out = Vec::new();
    unsafe {
        let count = ggml_backend_dev_count();
        for device_id in device_ids {
            if *device_id >= count {
                continue;
            }
            let dev = ggml_backend_dev_get(*device_id);
            if dev.is_null() {
                continue;
            }
            let dev_type = ggml_backend_dev_type(dev);
            let is_gpu_like = dev_type == GGML_BACKEND_DEVICE_TYPE_GPU
                || dev_type == GGML_BACKEND_DEVICE_TYPE_ACCEL
                || (include_integrated && dev_type == GGML_BACKEND_DEVICE_TYPE_IGPU);
            if !is_gpu_like {
                continue;
            }
            let mut free: usize = 0;
            let mut total: usize = 0;
            ggml_backend_dev_memory(dev, &mut free, &mut total);
            if free == 0 && total == 0 {
                continue;
            }
            let effective_total = if total == 0 { free } else { total };
            out.push((*device_id, free as u64, effective_total as u64));
        }
    }
    out
}

/// Per-device memory for the selected ids; a single selected device may be
/// an integrated GPU, a multi-GPU selection takes discrete devices only.
pub fn get_aligned_per_device_vram(device_ids: &[usize]) -> Vec<(usize, u64, u64)> {
    let per_device = get_per_device_free_vram(device_ids, device_ids.len() == 1);
    align_per_device_vram(device_ids, &per_device)
}

/// Detect if the system uses unified memory (shared RAM/VRAM).
/// True on Apple Silicon (macOS aarch64) or when only iGPU devices are found.
pub fn is_unified_memory() -> bool {
    if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        return true;
    }

    let mut found_gpu = false;
    let mut all_igpu = true;
    unsafe {
        let count = ggml_backend_dev_count();
        for i in 0..count {
            let dev = ggml_backend_dev_get(i);
            if dev.is_null() {
                continue;
            }
            let dev_type = ggml_backend_dev_type(dev);
            if dev_type == GGML_BACKEND_DEVICE_TYPE_GPU
                || dev_type == GGML_BACKEND_DEVICE_TYPE_IGPU
                || dev_type == GGML_BACKEND_DEVICE_TYPE_ACCEL
            {
                found_gpu = true;
                if dev_type != GGML_BACKEND_DEVICE_TYPE_IGPU {
                    all_igpu = false;
                }
            }
        }
    }
    found_gpu && all_igpu
}
