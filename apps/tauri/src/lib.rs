//! The LettuceAI desktop shell: opens the application API, starts it and
//! exposes it to the webview as Tauri commands, events and the asset URI
//! scheme. Every command is a one-line wrapper over `lettuce_app::api`.

#![deny(unsafe_op_in_unsafe_fn)]

mod asset_protocol;
mod bindings;
mod commands;
mod events;
mod files;
mod microphone;

use std::sync::{Arc, Mutex};

use lettuce_app::api::{ApiContext, ApiWorkers};
use tauri::{App, AppHandle, Manager, RunEvent, Runtime, WindowEvent};

pub use bindings::{BINDINGS_PATH, export_bindings, specta_builder};
pub use events::AppEvent;

/// The workers `api::startup` started, stopped on exit.
struct Workers(Mutex<Option<ApiWorkers>>);

/// Builds and runs the app until its last window closes; on Android and
/// iOS this is the mobile entry point.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(error) =
        lettuce_observability::install(lettuce_observability::ObservabilityConfig::default())
    {
        tracing::warn!(%error, "logging could not be installed");
    }
    let builder = specta_builder::<tauri::Wry>();
    #[cfg(debug_assertions)]
    if let Err(error) = export_bindings(std::path::Path::new(BINDINGS_PATH)) {
        tracing::warn!(%error, "TypeScript bindings could not be exported");
    }
    let invoke_handler = builder.invoke_handler();
    let app = tauri::Builder::default();
    #[cfg(desktop)]
    let app = app.plugin(tauri_plugin_dialog::init());
    #[cfg(target_os = "android")]
    let app = app.plugin(tauri_plugin_android_fs::init());
    let app = app
        .invoke_handler(invoke_handler)
        .register_asynchronous_uri_scheme_protocol(
            asset_protocol::ASSET_SCHEME,
            asset_protocol::handle,
        )
        .setup(move |app| {
            builder.mount_events(app);
            start(app)
        })
        .build(tauri::generate_context!());
    match app {
        Ok(app) => app.run(|handle, event| match event {
            RunEvent::WindowEvent {
                event: WindowEvent::Focused(focused),
                ..
            } => {
                if let Some(context) = handle.try_state::<ApiContext>() {
                    context.app_focus_changed(focused);
                }
            }
            #[cfg(mobile)]
            RunEvent::WindowEvent {
                event: WindowEvent::Suspended,
                ..
            } => {
                if let Some(context) = handle.try_state::<ApiContext>() {
                    context.app_focus_changed(false);
                }
            }
            #[cfg(mobile)]
            RunEvent::WindowEvent {
                event: WindowEvent::Resumed,
                ..
            } => {
                if let Some(context) = handle.try_state::<ApiContext>() {
                    context.app_focus_changed(true);
                }
            }
            RunEvent::Exit => stop(handle),
            _ => {}
        }),
        Err(error) => tracing::error!(%error, "the desktop app could not start"),
    }
}

/// Opens the application API under the app data directory and runs
/// `api::startup`, which settles what the previous process left running
/// before commands are served and then starts the workers on their own
/// threads.
fn start<R: Runtime>(app: &mut App<R>) -> Result<(), Box<dyn std::error::Error>> {
    let app_data = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_data)?;
    let context = ApiContext::open_desktop(
        &app_data,
        app.path().resource_dir().ok(),
        Arc::new(lettuce_settings::NativeSecretStore::new()),
        Arc::new(events::TauriEventSink(app.handle().clone())),
        files::file_access(app.handle()),
        asset_protocol::ASSET_URL_BASE.to_owned(),
        microphone::capture(),
    )
    .map_err(|error| error.message)?;
    let workers = tauri::async_runtime::block_on(lettuce_app::api::startup(&context))
        .map_err(|error| error.message)?;
    app.manage(context);
    app.manage(Workers(Mutex::new(Some(workers))));
    Ok(())
}

/// Stops the workers: cancels running work, stops the local diffusion
/// server, records the active time and joins the worker threads.
fn stop<R: Runtime>(handle: &AppHandle<R>) {
    let Some(context) = handle
        .try_state::<ApiContext>()
        .map(|state| state.inner().clone())
    else {
        return;
    };
    let workers = handle
        .try_state::<Workers>()
        .and_then(|workers| workers.0.lock().ok()?.take());
    match workers {
        Some(workers) => tauri::async_runtime::block_on(workers.stop()),
        None => {
            context.begin_shutdown();
            tauri::async_runtime::block_on(context.backend().shutdown());
            context.flush_app_usage();
        }
    }
}
