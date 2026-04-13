mod bootstrap;
mod commands;
mod runtime;

pub(crate) fn run() {
    let aptabase_key = std::env::var("APTABASE_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| option_env!("APTABASE_KEY").map(|value| value.to_string()));
    let aptabase_plugin_enabled = aptabase_key.is_some();
    let aptabase_runtime = if aptabase_plugin_enabled {
        Some(tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime for Aptabase"))
    } else {
        None
    };
    let _aptabase_runtime_guard = aptabase_runtime.as_ref().map(|runtime| runtime.enter());

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init());

    if let Some(key) = aptabase_key.as_deref() {
        builder = builder.plugin(tauri_plugin_aptabase::Builder::new(key).build());
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    let builder = builder.plugin(tauri_plugin_haptics::init());

    #[cfg(any(target_os = "android", target_os = "ios"))]
    let builder = builder.plugin(tauri_plugin_barcode_scanner::init());

    #[cfg(target_os = "android")]
    let builder = builder.plugin(tauri_plugin_android_fs::init());

    builder
        .setup(move |app| bootstrap::setup_app(app, aptabase_plugin_enabled))
        .invoke_handler(commands::invoke_handler!())
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(runtime::handle_run_event);
}
