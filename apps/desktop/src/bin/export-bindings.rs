use std::{path::Path, process::ExitCode};

fn main() -> ExitCode {
    if let Err(error) =
        lettuce_observability::install(lettuce_observability::ObservabilityConfig::default())
    {
        tracing::warn!(%error, "logging could not be installed");
    }
    match lettuce_desktop::export_bindings(Path::new(lettuce_desktop::BINDINGS_PATH)) {
        Ok(()) => {
            tracing::info!(
                path = lettuce_desktop::BINDINGS_PATH,
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
