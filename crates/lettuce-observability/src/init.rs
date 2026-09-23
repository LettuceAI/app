use std::io::{self, Write as _};

use thiserror::Error;
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt};

use crate::config::{ConfigError, LocalOutputConfig, ObservabilityConfig, StderrFormat};
use crate::line_layer::LineLayer;
use crate::log_files::{DailyLogWriter, LogEntry};

pub(crate) fn build_filter(directives: &str) -> EnvFilter {
    EnvFilter::try_new(directives).unwrap_or_else(|_| EnvFilter::new("info"))
}

/// Errors returned while validating or installing observability.
#[derive(Debug, Error)]
pub enum InitError {
    #[error("invalid observability configuration: {0}")]
    InvalidConfig(#[from] ConfigError),
    #[error("the global tracing subscriber is already installed")]
    AlreadyInstalled,
}

pub(crate) fn local_output_writer(
    config: LocalOutputConfig,
) -> Result<(NonBlocking, WorkerGuard), InitError> {
    config.validate()?;
    Ok(
        tracing_appender::non_blocking::NonBlockingBuilder::default()
            .buffered_lines_limit(config.queue_capacity)
            .lossy(true)
            .finish(DailyLogWriter::new(config.directory)),
    )
}

/// Appends records that do not come from `tracing`, such as the frontend's,
/// to the same daily files.
#[derive(Clone)]
pub struct LogSink {
    writer: NonBlocking,
}

impl std::fmt::Debug for LogSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("LogSink").finish_non_exhaustive()
    }
}

impl LogSink {
    pub(crate) const fn new(writer: NonBlocking) -> Self {
        Self { writer }
    }

    pub fn append(&self, entry: &LogEntry) -> io::Result<()> {
        self.writer.clone().write_all(entry.line().as_bytes())
    }
}

/// The daily log files' writer: the guard flushes queued lines when dropped
/// and must be kept for as long as the files are written.
#[derive(Debug)]
pub struct LocalOutput {
    pub guard: WorkerGuard,
    pub sink: LogSink,
}

/// Installs the single process-wide subscriber. `None` is returned when
/// local output is not configured.
pub fn install(config: ObservabilityConfig) -> Result<Option<LocalOutput>, InitError> {
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

    let mut output = None;
    if let Some(local_output) = config.local_output {
        let (writer, guard) = local_output_writer(local_output)?;
        layers.push(Box::new(LineLayer::new(writer.clone())));
        output = Some(LocalOutput {
            guard,
            sink: LogSink::new(writer),
        });
    }

    let subscriber = Registry::default().with(layers);
    tracing::subscriber::set_global_default(subscriber).map_err(|_| InitError::AlreadyInstalled)?;

    Ok(output)
}
