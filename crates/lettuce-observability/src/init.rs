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

type Observer = std::sync::Arc<dyn Fn(&str) + Send + Sync>;

#[derive(Clone)]
struct OutputWriter {
    writer: std::sync::Arc<std::sync::Mutex<DailyLogWriter>>,
    observer: std::sync::Arc<std::sync::Mutex<Option<Observer>>>,
}

impl io::Write for OutputWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Err(error) = self
            .writer
            .lock()
            .map_err(|_| io::Error::other("log writer unavailable"))?
            .write_all(bytes)
        {
            eprintln!("log file write failed: {error}");
            return Err(error);
        }
        let observer = self
            .observer
            .lock()
            .map_err(|_| io::Error::other("log observer unavailable"))?
            .clone();
        if let Some(observer) = observer {
            observer(std::str::from_utf8(bytes).map_err(io::Error::other)?);
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer
            .lock()
            .map_err(|_| io::Error::other("log writer unavailable"))?
            .flush()
    }
}

pub(crate) fn local_output_writer(
    config: LocalOutputConfig,
) -> Result<(NonBlocking, LocalOutput), InitError> {
    config.validate()?;
    let output = OutputWriter {
        writer: std::sync::Arc::new(std::sync::Mutex::new(DailyLogWriter::new(config.directory))),
        observer: Default::default(),
    };
    let (writer, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
        .buffered_lines_limit(config.queue_capacity)
        .lossy(false)
        .finish(output.clone());
    Ok((
        writer,
        LocalOutput {
            guard,
            sink: LogSink { writer: output },
        },
    ))
}

pub fn local_output(config: LocalOutputConfig) -> Result<LocalOutput, InitError> {
    local_output_writer(config).map(|(_, output)| output)
}

#[derive(Clone)]
pub struct LogSink {
    writer: OutputWriter,
}

impl std::fmt::Debug for LogSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("LogSink").finish_non_exhaustive()
    }
}

impl LogSink {
    pub fn append(&self, entry: &LogEntry) -> io::Result<()> {
        self.writer.clone().write_all(entry.line().as_bytes())
    }

    pub fn set_observer(&self, observer: impl Fn(&str) + Send + Sync + 'static) {
        *self
            .writer
            .observer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(std::sync::Arc::new(observer));
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
        let (writer, local) = local_output_writer(local_output)?;
        layers.push(Box::new(LineLayer::new(writer.clone())));
        output = Some(local);
    }

    let subscriber = Registry::default().with(layers);
    tracing::subscriber::set_global_default(subscriber).map_err(|_| InitError::AlreadyInstalled)?;

    Ok(output)
}

#[cfg(test)]
mod slice12_tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn append_is_written_before_return_and_mirror_follows_the_write() {
        let directory =
            std::env::temp_dir().join(format!("logs-{}", lettuce_types::OperationId::new()));
        std::fs::create_dir(&directory).expect("directory");
        let output = local_output(LocalOutputConfig::new(&directory)).expect("output");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let capture = seen.clone();
        output
            .sink
            .set_observer(move |line| capture.lock().expect("capture").push(line.to_owned()));
        let entry = LogEntry {
            timestamp: "2026-10-09T12:00:00Z".into(),
            level: "INFO".into(),
            component: "frontend".into(),
            function: None,
            message: "written".into(),
        };
        output.sink.append(&entry).expect("append");
        let logs = crate::LogDirectory::new(directory.clone());
        assert_eq!(
            logs.read(&logs.list().expect("list")[0]).expect("read"),
            entry.line()
        );
        assert_eq!(*seen.lock().expect("capture"), vec![entry.line()]);
        drop(output);
        std::fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn append_failure_is_reported_and_never_mirrored() {
        let directory =
            std::env::temp_dir().join(format!("logs-{}", lettuce_types::OperationId::new()));
        std::fs::create_dir(&directory).expect("directory");
        let output = local_output(LocalOutputConfig::new(&directory)).expect("output");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let capture = seen.clone();
        output
            .sink
            .set_observer(move |line| capture.lock().expect("capture").push(line.to_owned()));
        std::fs::remove_dir(&directory).expect("remove");
        assert!(
            output
                .sink
                .append(&LogEntry {
                    timestamp: "t".into(),
                    level: "INFO".into(),
                    component: "frontend".into(),
                    function: None,
                    message: "failed".into(),
                })
                .is_err()
        );
        assert!(seen.lock().expect("capture").is_empty());
    }
}

#[cfg(test)]
mod queue_tests {
    use super::*;
    use tracing_subscriber::{layer::SubscriberExt, registry};

    #[test]
    fn a_small_queue_preserves_every_line_and_mirrors_written_backend_events() {
        let directory =
            std::env::temp_dir().join(format!("log-queue-{}", lettuce_types::OperationId::new()));
        std::fs::create_dir(&directory).expect("directory");
        let (writer, output) =
            local_output_writer(LocalOutputConfig::new(&directory).with_queue_capacity(1))
                .expect("output");
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let capture = seen.clone();
        output
            .sink
            .set_observer(move |line| capture.lock().expect("capture").push(line.to_owned()));
        tracing::subscriber::with_default(registry().with(LineLayer::new(writer)), || {
            for index in 0..200 {
                tracing::info!(index, "queued line");
            }
        });
        drop(output);
        let logs = crate::LogDirectory::new(directory.clone());
        let name = logs.list().expect("list").remove(0);
        assert_eq!(logs.read(&name).expect("read").lines().count(), 200);
        assert_eq!(seen.lock().expect("capture").len(), 200);
        std::fs::remove_dir_all(directory).expect("cleanup");
    }
}
