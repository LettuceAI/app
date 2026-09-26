use std::{path::Path, process::ExitCode};

fn main() -> ExitCode {
    if let Err(error) =
        lettuce_observability::install(lettuce_observability::ObservabilityConfig::default())
    {
        tracing::warn!(%error, "logging could not be installed");
    }
    match lettuce_tauri::export_bindings(Path::new(lettuce_tauri::BINDINGS_PATH)) {
        Ok(()) => {
            tracing::info!(
                path = lettuce_tauri::BINDINGS_PATH,
                "TypeScript bindings written"
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            tracing::error!(%error, "TypeScript bindings could not be written");
            ExitCode::FAILURE
        }
    }
}
