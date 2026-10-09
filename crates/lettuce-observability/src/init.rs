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

const MIRROR_CAPACITY: usize = 1024;

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
        .lossy(true)
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

    /// Hands each written line to `observer` on its own thread through a
    /// bounded queue; lines are dropped while the queue is full, so a slow
    /// observer never blocks a log write.
    pub fn set_observer(&self, observer: impl Fn(&str) + Send + Sync + 'static) {
        let (lines, queued) = std::sync::mpsc::sync_channel::<String>(MIRROR_CAPACITY);
        let spawned = std::thread::Builder::new()
            .name("lettuce-log-mirror".into())
            .spawn(move || {
                for line in queued {
                    observer(&line);
                }
            });
        let forward: Option<Observer> = match spawned {
            Ok(_) => Some(std::sync::Arc::new(move |line: &str| {
                let _ = lines.try_send(line.to_owned());
            })),
            Err(error) => {
                eprintln!("log mirror could not start: {error}");
                None
            }
        };
        *self
            .writer
            .observer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = forward;
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
        let (seen, mirrored) = std::sync::mpsc::channel();
        output.sink.set_observer(move |line| {
            let _ = seen.send(line.to_owned());
        });
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
        assert_eq!(
            mirrored
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("mirrored line"),
            entry.line()
        );
        drop(output);
        std::fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn a_stalled_mirror_never_blocks_log_writes() {
        let directory =
            std::env::temp_dir().join(format!("logs-{}", lettuce_types::OperationId::new()));
        std::fs::create_dir(&directory).expect("directory");
        let output = local_output(LocalOutputConfig::new(&directory)).expect("output");
        let (release, stalled) = std::sync::mpsc::channel::<()>();
        let stalled = Mutex::new(stalled);
        output.sink.set_observer(move |_| {
            let _ = stalled.lock().expect("stall").recv();
        });
        let sink = output.sink.clone();
        let (done, finished) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for index in 0..5000 {
                sink.append(&LogEntry {
                    timestamp: "2026-10-09T12:00:00Z".into(),
                    level: "INFO".into(),
                    component: "frontend".into(),
                    function: None,
                    message: format!("line {index}"),
                })
                .expect("append");
            }
            let _ = done.send(());
        });
        let written = finished.recv_timeout(std::time::Duration::from_secs(10));
        drop(release);
        assert!(written.is_ok(), "a stalled mirror blocked the log writer");
        let logs = crate::LogDirectory::new(directory.clone());
        assert_eq!(
            logs.read(&logs.list().expect("list")[0])
                .expect("read")
                .lines()
                .count(),
            5000
        );
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
    fn a_full_queue_drops_backend_lines_instead_of_blocking_the_caller() {
        let directory =
            std::env::temp_dir().join(format!("log-queue-{}", lettuce_types::OperationId::new()));
        std::fs::create_dir(&directory).expect("directory");
        let (writer, output) =
            local_output_writer(LocalOutputConfig::new(&directory).with_queue_capacity(1))
                .expect("output");
        let file = output.sink.writer.writer.clone();
        let stalled = file.lock().expect("stall the file writer");
        let (done, finished) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            tracing::subscriber::with_default(registry().with(LineLayer::new(writer)), || {
                for index in 0..200 {
                    tracing::info!(index, "queued line");
                }
            });
            let _ = done.send(());
        });
        let returned = finished.recv_timeout(std::time::Duration::from_secs(10));
        drop(stalled);
        assert!(returned.is_ok(), "a full log queue blocked the caller");
        drop(output);
        std::fs::remove_dir_all(directory).expect("cleanup");
    }
}
