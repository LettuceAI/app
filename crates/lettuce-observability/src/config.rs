use std::path::{Component, Path, PathBuf};

use thiserror::Error;

const DEFAULT_FILE_PREFIX: &str = "lettuce";
const DEFAULT_QUEUE_CAPACITY: usize = 1_024;
const MAX_QUEUE_CAPACITY: usize = 1_000_000;

/// Human-readable format used for local stderr output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StderrFormat {
    /// Compact one-line records suitable for a local terminal.
    #[default]
    Compact,
    /// Pretty multi-line records for interactive local debugging.
    Pretty,
}

/// Explicit configuration for optional local rolling output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalOutputConfig {
    /// Directory supplied by the composition root.
    pub directory: PathBuf,
    /// Prefix for the daily rolling files.
    pub file_prefix: String,
    /// Maximum number of queued records before the lossy writer drops them.
    pub queue_capacity: usize,
}

impl LocalOutputConfig {
    #[must_use]
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            file_prefix: DEFAULT_FILE_PREFIX.to_owned(),
            queue_capacity: DEFAULT_QUEUE_CAPACITY,
        }
    }

    #[must_use]
    pub fn with_file_prefix(mut self, file_prefix: impl Into<String>) -> Self {
        self.file_prefix = file_prefix.into();
        self
    }

    #[must_use]
    pub const fn with_queue_capacity(mut self, queue_capacity: usize) -> Self {
        self.queue_capacity = queue_capacity;
        self
    }

    pub(crate) fn validate(&self) -> Result<(), ConfigError> {
        validate_output_directory(&self.directory)?;

        if self.file_prefix.is_empty()
            || Path::new(&self.file_prefix)
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(ConfigError::InvalidFilePrefix);
        }

        if self.queue_capacity == 0 || self.queue_capacity > MAX_QUEUE_CAPACITY {
            return Err(ConfigError::InvalidQueueCapacity);
        }

        Ok(())
    }
}

/// Subscriber and output policy owned by the composition root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservabilityConfig {
    /// `tracing_subscriber::EnvFilter` directives. Invalid directives use the
    /// safe `info` fallback during installation.
    pub filter: String,
    /// Human-readable stderr formatting.
    pub stderr_format: StderrFormat,
    /// Optional explicitly supplied local output directory and queue policy.
    pub local_output: Option<LocalOutputConfig>,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            filter: "info".to_owned(),
            stderr_format: StderrFormat::Compact,
            local_output: None,
        }
    }
}

impl ObservabilityConfig {
    #[must_use]
    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = filter.into();
        self
    }

    #[must_use]
    pub const fn with_stderr_format(mut self, stderr_format: StderrFormat) -> Self {
        self.stderr_format = stderr_format;
        self
    }

    #[must_use]
    pub fn with_local_output(mut self, local_output: LocalOutputConfig) -> Self {
        self.local_output = Some(local_output);
        self
    }

    /// Validates only explicit configuration. No global path discovery or
    /// directory creation is performed here.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if let Some(local_output) = &self.local_output {
            local_output.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("the local observability output directory does not exist")]
    OutputDirectoryMissing,
    #[error("the local observability output path is not a directory")]
    OutputPathNotDirectory,
    #[error("the local observability file prefix is invalid")]
    InvalidFilePrefix,
    #[error("the local observability queue capacity is outside its bounded range")]
    InvalidQueueCapacity,
}

fn validate_output_directory(directory: &Path) -> Result<(), ConfigError> {
    if !directory.exists() {
        return Err(ConfigError::OutputDirectoryMissing);
    }
    if !directory.is_dir() {
        return Err(ConfigError::OutputPathNotDirectory);
    }

    Ok(())
}
