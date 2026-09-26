//! The LettuceAI desktop shell: opens the application API, runs its workers
//! and exposes it to the webview as Tauri commands, events and the asset URI
//! scheme. Every command is a one-line wrapper over `lettuce_app::api`.

#![deny(unsafe_op_in_unsafe_fn)]

mod asset_protocol;
mod bindings;
mod commands;
mod events;

use std::{
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

use lettuce_app::api::{ApiContext, ConversationGenerationWorker};
use tauri::{App, AppHandle, Manager, RunEvent, Runtime};

pub use bindings::{BINDINGS_PATH, export_bindings, specta_builder};
pub use events::AppEvent;

/// The generation worker's thread and the signal that stops it.
struct Workers {
    running: Mutex<Option<(tokio::sync::oneshot::Sender<()>, JoinHandle<()>)>>,
}

/// Builds and runs the desktop app until its last window closes.
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
    let app = tauri::Builder::default()
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
        Ok(app) => app.run(|handle, event| {
            if let RunEvent::Exit = event {
                stop(handle);
            }
        }),
        Err(error) => tracing::error!(%error, "the desktop app could not start"),
    }
}

/// Opens the application API under the app data directory, settles what the
/// previous process left running, then starts the generation worker on its
/// own thread and runtime.
fn start<R: Runtime>(app: &mut App<R>) -> Result<(), Box<dyn std::error::Error>> {
    let app_data = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_data)?;
    let context = ApiContext::open_desktop(
        &app_data,
        Arc::new(lettuce_settings::NativeSecretStore::new()),
        Arc::new(events::TauriEventSink(app.handle().clone())),
    )
    .map_err(|error| error.message)?;
    context
        .recover_after_restart()
        .map_err(|error| error.message)?;
    let worker = ConversationGenerationWorker::new(context.clone());
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let thread = std::thread::Builder::new()
        .name("conversation-generation".into())
        .spawn(move || {
            match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime.block_on(worker.run(async move {
                    let _ = stopped.await;
                })),
                Err(error) => {
                    tracing::error!(%error, "the generation worker runtime could not start");
                }
            }
        })?;
    app.manage(context);
    app.manage(Workers {
        running: Mutex::new(Some((stop, thread))),
    });
    Ok(())
}

/// Cancels running inference, stops and joins the worker, then stops the
/// backend's local servers.
fn stop<R: Runtime>(handle: &AppHandle<R>) {
    let Some(context) = handle
        .try_state::<ApiContext>()
        .map(|state| state.inner().clone())
    else {
        return;
    };
    context.begin_shutdown();
    let running = handle
        .try_state::<Workers>()
        .and_then(|workers| workers.running.lock().ok()?.take());
    if let Some((stop, thread)) = running {
        let _ = stop.send(());
        if thread.join().is_err() {
            tracing::error!("the generation worker thread panicked");
        }
    }
    tauri::async_runtime::block_on(context.backend().shutdown());
}
