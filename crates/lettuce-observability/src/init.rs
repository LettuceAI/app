use std::io;

use thiserror::Error;
use tracing_appender::{
    non_blocking::{NonBlocking, WorkerGuard},
    rolling,
};
use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt};

use crate::config::{ConfigError, LocalOutputConfig, ObservabilityConfig, StderrFormat};

pub(crate) fn build_filter(directives: &str) -> EnvFilter {
    EnvFilter::try_new(directives).unwrap_or_else(|_| EnvFilter::new("info"))
}

/// Errors returned while validating or installing observability.
#[derive(Debug, Error)]
pub enum InitError {
    #[error("invalid observability configuration: {0}")]
    InvalidConfig(#[from] ConfigError),
    #[error("the local observability output could not be opened")]
    LocalOutput(#[source] rolling::InitError),
    #[error("the global tracing subscriber is already installed")]
    AlreadyInstalled,
}

pub(crate) fn local_output_writer(
    config: LocalOutputConfig,
) -> Result<(NonBlocking, WorkerGuard), InitError> {
    config.validate()?;
    let appender = rolling::Builder::new()
        .rotation(rolling::Rotation::DAILY)
        .filename_prefix(config.file_prefix)
        .build(config.directory)
        .map_err(InitError::LocalOutput)?;

    Ok(
        tracing_appender::non_blocking::NonBlockingBuilder::default()
            .buffered_lines_limit(config.queue_capacity)
            .lossy(true)
            .finish(appender),
    )
}

/// Installs the single process-wide subscriber.
///
/// The returned guard must be retained for as long as local file output is
/// needed. `None` is returned when local output is not configured.
pub fn install(config: ObservabilityConfig) -> Result<Option<WorkerGuard>, InitError> {
    config.validate()?;

    let filter = build_filter(&config.filter);
    let mut layers: Vec<Box<dyn tracing_subscriber::Layer<Registry> + Send + Sync>> = Vec::new();
    layers.push(Box::new(filter));

    match config.stderr_format {
        StderrFormat::Compact => layers.push(Box::new(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_ansi(false)
                .with_writer(io::stderr),
        )),
        StderrFormat::Pretty => layers.push(Box::new(
            tracing_subscriber::fmt::layer()
                .pretty()
                .with_ansi(true)
                .with_writer(io::stderr),
        )),
    }

    let mut worker_guard = None;
    if let Some(local_output) = config.local_output {
        let (writer, guard) = local_output_writer(local_output)?;
        let file_layer = tracing_subscriber::fmt::layer()
            .compact()
            .with_ansi(false)
            .with_writer(writer);
        layers.push(Box::new(file_layer));
        worker_guard = Some(guard);
    }

    let subscriber = Registry::default().with(layers);
    tracing::subscriber::set_global_default(subscriber).map_err(|_| InitError::AlreadyInstalled)?;

    Ok(worker_guard)
}
