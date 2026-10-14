#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    webkit_safe_defaults();
    lettuce_tauri::run();
}

/// WebKitGTK's compositing mode and GPU process crash the window on some
/// Wayland and NVIDIA setups, so both are turned off unless the user set
/// them. It runs first on the main thread, before any other thread exists,
/// which is what makes changing the environment sound.
#[cfg(target_os = "linux")]
fn webkit_safe_defaults() {
    for name in [
        "WEBKIT_DISABLE_COMPOSITING_MODE",
        "WEBKIT_DISABLE_GPU_PROCESS",
    ] {
        if std::env::var_os(name).is_none() {
            unsafe { std::env::set_var(name, "1") };
        }
    }
}
