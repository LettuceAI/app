mod bootstrap;
mod commands;
mod runtime;

fn dispatch_invoke<F>(handler: &F, invoke: tauri::ipc::Invoke<tauri::Wry>)
where
    F: Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool,
{
    let command = invoke.message.command().to_string();
    let resolver = invoke.resolver.clone();
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler(invoke))) {
        Ok(true) => {}
        Ok(false) => {
            resolver.reject(format!("The {} command is not registered", command));
        }
        Err(_) => {
            crate::utils::log_error_global(
                "ipc",
                format!("Command {} panicked; rejecting the invoke", command),
            );
            resolver.reject(format!("The {} command failed unexpectedly", command));
        }
    }
}

fn worker_invoke_handler<F>(
    handler: F,
) -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static
where
    F: Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static,
{
    let (sender, receiver) = std::sync::mpsc::channel::<tauri::ipc::Invoke<tauri::Wry>>();
    std::thread::Builder::new()
        .name("lettuce-ipc".to_string())
        .spawn(move || {
            for invoke in receiver {
                dispatch_invoke(&handler, invoke);
            }
        })
        .expect("Failed to start the IPC command thread");

    let sender = std::sync::Mutex::new(sender);
    move |invoke: tauri::ipc::Invoke<tauri::Wry>| {
        let command = invoke.message.command().to_string();
        let resolver = invoke.resolver.clone();
        let queued = match sender.lock() {
            Ok(sender) => sender.send(invoke).is_ok(),
            Err(_) => false,
        };
        if !queued {
            crate::utils::log_error_global(
                "ipc",
                format!("Could not queue command {}; the IPC thread is gone", command),
            );
            resolver.reject(format!("The {} command could not be scheduled", command));
        }
        true
    }
}

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
        .invoke_handler(worker_invoke_handler(commands::invoke_handler!()))
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(runtime::handle_run_event);
}
