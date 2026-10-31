//! The LettuceAI desktop shell: opens the application API, starts it and
//! exposes it to the webview as Tauri commands, events and the asset URI
//! scheme. Every command is a one-line wrapper over `lettuce_app::api`.

#![deny(unsafe_op_in_unsafe_fn)]

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod active_work;
mod asset_protocol;
mod bindings;
mod commands;
mod events;
mod files;
mod host;
mod microphone;
mod reset;
mod startup;

use std::sync::{Arc, Mutex};

use lettuce_app::api::{ApiContext, ApiContextSlot, ApiWorkers};
use tauri::{App, AppHandle, Manager, RunEvent, Runtime, WindowEvent};

pub use bindings::{BINDINGS_PATH, export_bindings, specta_builder};
pub use events::AppEvent;

/// The workers `api::startup` started, stopped on exit.
struct Workers(Mutex<Option<ApiWorkers>>);

/// Builds and runs the app until its last window closes; on Android and
/// iOS this is the mobile entry point.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = specta_builder::<tauri::DynRuntime>();
    #[cfg(debug_assertions)]
    if let Err(error) = export_bindings(std::path::Path::new(BINDINGS_PATH)) {
        tracing::warn!(%error, "TypeScript bindings could not be exported");
    }
    let invoke_handler = builder.invoke_handler();
    let app = tauri::Builder::default().runtime(tauri_runtime_wry::Wry::default());
    #[cfg(desktop)]
    let app = app
        .plugin(tauri_plugin_single_window::init_with(
            host::single_window_config(),
        ))
        .plugin(tauri_plugin_dialog::init());
    #[cfg(target_os = "android")]
    let app = app.plugin(tauri_plugin_android_fs::init());
    let app = app
        .manage(startup::BackendStartup::default())
        .plugin(tauri_plugin_opener::Builder::new().open_js_links_on_click(false).build())
        .plugin(tauri_plugin_haptics::init())
        .plugin(host::plugin())
        .invoke_handler(invoke_handler)
        .register_asynchronous_uri_scheme_protocol(
            asset_protocol::ASSET_SCHEME,
            asset_protocol::handle,
        )
        .setup(move |app| {
            builder.mount_events(app);
            let result = start(app);
            app.state::<startup::BackendStartup>().finish(
                result.as_ref().map(|_| ()).map_err(|error| lettuce_contracts::ApiError {
                    code: lettuce_contracts::ApiErrorCode::Internal,
                    message: error.to_string(),
                    details: None,
                }),
            );
            #[cfg(target_os = "android")]
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move { host::finish_startup(&handle).await });
            }
            result
        })
        .build(tauri::generate_context!());
    match app {
        Ok(app) => app.run(|handle, event| match event {
            RunEvent::WindowEvent {
                event: WindowEvent::Focused(focused),
                ..
            } => {
                if let Some(slot) = handle.try_state::<ApiContextSlot>()
                    && let Ok(context) = slot.snapshot()
                {
                    context.app_focus_changed(focused);
                }
            }
            #[cfg(mobile)]
            RunEvent::WindowEvent {
                event: WindowEvent::Suspended,
                ..
            } => {
                if let Some(slot) = handle.try_state::<ApiContextSlot>()
                    && let Ok(context) = slot.snapshot()
                {
                    context.app_focus_changed(false);
                }
            }
            #[cfg(mobile)]
            RunEvent::WindowEvent {
                event: WindowEvent::Resumed,
                ..
            } => {
                if let Some(slot) = handle.try_state::<ApiContextSlot>()
                    && let Ok(context) = slot.snapshot()
                {
                    context.app_focus_changed(true);
                }
            }
            #[cfg(target_os = "macos")]
            RunEvent::Opened { urls } => host::opened_paths(
                handle,
                urls.iter().filter_map(|url| url.to_file_path().ok()).collect(),
            ),
            RunEvent::Exit => stop(handle),
            _ => {}
        }),
        Err(error) => {
            tracing::error!(%error, "the desktop app could not start");
            eprintln!("The app could not start: {error}");
        }
    }
}

/// Opens the application API under the app data directory and runs
/// `api::startup`, which settles what the previous process left running
/// before commands are served and then starts the workers on their own
/// threads.
fn start<R: Runtime>(app: &mut App<R>) -> Result<(), Box<dyn std::error::Error>> {
    let app_data = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_data)?;
    let log_directory = app.path().app_log_dir()?;
    std::fs::create_dir_all(&log_directory)?;
    let output = lettuce_observability::install(
        lettuce_observability::ObservabilityConfig::default().with_local_output(
            lettuce_observability::LocalOutputConfig::new(&log_directory),
        ),
    )?
    .ok_or("file logging is unavailable")?;
    lettuce_observability::install_panic_reports(log_directory.clone());
    app.manage(output);
    let secrets = Arc::new(lettuce_settings::NativeSecretStore::try_new()?);
    let context = open_context(app.handle(), secrets).map_err(|error| error.message)?;
    let workers = tauri::async_runtime::block_on(lettuce_app::api::startup(&context))
        .map_err(|error| error.message)?;
    app.manage(ApiContextSlot::new(Arc::new(context)));
    app.manage(Workers(Mutex::new(Some(workers))));
    Ok(())
}

fn reload_failure() -> lettuce_contracts::ApiError {
    lettuce_contracts::ApiError {
        code: lettuce_contracts::ApiErrorCode::Unavailable,
        message: "database reload requires reopening the application".into(),
        details: Some(lettuce_contracts::ApiErrorDetails::Backup { reason: lettuce_contracts::BackupFailureReason::RecoveryRequired }),
    }
}

fn open_context<R: Runtime>(handle: &AppHandle<R>, secrets: Arc<dyn lettuce_settings::SecretStore>) -> Result<ApiContext, lettuce_contracts::ApiError> {
    let app_data = handle.path().app_data_dir().map_err(|_| reload_failure())?;
    let log_directory = handle.path().app_log_dir().map_err(|_| reload_failure())?;
    let output = handle.try_state::<lettuce_observability::LocalOutput>().ok_or_else(reload_failure)?;
    let context = ApiContext::open_desktop(
        &app_data,
        handle.path().resource_dir().ok(),
        secrets,
        Arc::new(events::TauriEventSink(handle.clone())),
        files::file_access(handle),
        asset_protocol::ASSET_URL_BASE.to_owned(),
        microphone::capture(handle),
    )?;
    #[cfg(target_os = "android")]
    host::follow_active_work(handle, context.watch_active_work());
    context.attach_logs(log_directory, output.sink.clone());
    context.attach_reset_host(Arc::new(reset::ResetHost(handle.clone())))?;
    Ok(context)
}

/// Stops the workers: cancels running work, stops the local diffusion
/// server, records the active time and joins the worker threads.
fn stop<R: Runtime>(handle: &AppHandle<R>) {
    let workers = handle
        .try_state::<Workers>()
        .and_then(|workers| workers.0.lock().ok()?.take());
    match workers {
        Some(workers) => tauri::async_runtime::block_on(workers.stop()),
        None => {
            let Some(context) = handle
                .try_state::<ApiContextSlot>()
                .and_then(|slot| slot.snapshot().ok())
            else {
                return;
            };
            context.begin_shutdown();
            tauri::async_runtime::block_on(context.stop_sync_session());
            tauri::async_runtime::block_on(context.backend().shutdown());
            context.flush_app_usage();
        }
    }
}
