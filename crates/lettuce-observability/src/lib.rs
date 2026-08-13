//! Safe tracing configuration and correlation for LettuceAI.
//!
//! The intended ownership, boundaries, migration path, and acceptance gates are
//! specified in the crate PLAN.md. This first slice deliberately does not
//! provide support bundles, exporters, crash upload, or a logging facade.

#![deny(unsafe_op_in_unsafe_fn)]

mod config;
mod correlation;
mod init;
mod redaction;

pub use config::{ConfigError, LocalOutputConfig, ObservabilityConfig, StderrFormat};
pub use correlation::{
    CONVERSATION_ID_FIELD, Correlation, CorrelationContext, GENERATION_TURN_ID_FIELD, JOB_ID_FIELD,
    OPERATION_FIELD, OPERATION_ID_FIELD, REQUEST_ID_FIELD,
};
pub use init::{InitError, install};
pub use redaction::{REDACTED, Sensitive, UserContent};

#[cfg(test)]
mod tests {
    use std::{
        fs, io,
        sync::{Arc, Mutex},
    };

    use lettuce_types::{ConversationId, GenerationTurnId, JobId, OperationId, RequestId};
    use thiserror::Error;
    use tracing_subscriber::{fmt, layer::SubscriberExt, registry};

    use super::{CorrelationContext, REDACTED, Sensitive, UserContent};

    #[derive(Clone, Debug)]
    struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

    impl CaptureWriter {
        fn output(&self) -> String {
            let bytes = match self.0.lock() {
                Ok(bytes) => bytes.clone(),
                Err(poisoned) => poisoned.into_inner().clone(),
            };
            String::from_utf8_lossy(&bytes).into_owned()
        }
    }

    struct CaptureGuard(Arc<Mutex<Vec<u8>>>);

    impl io::Write for CaptureGuard {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            match self.0.lock() {
                Ok(mut output) => output.extend_from_slice(bytes),
                Err(poisoned) => poisoned.into_inner().extend_from_slice(bytes),
            }
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CaptureWriter {
        type Writer = CaptureGuard;

        fn make_writer(&'a self) -> Self::Writer {
            CaptureGuard(Arc::clone(&self.0))
        }
    }

    fn capture_writer() -> CaptureWriter {
        CaptureWriter(Arc::new(Mutex::new(Vec::new())))
    }

    #[test]
    fn wrappers_redact_debug_display_and_error_formatting() {
        let secret = "canary-secret";
        let content = "private-message";
        let wrapped_secret = Sensitive::new(secret.to_owned());
        let wrapped_content = UserContent::new(content.to_owned());

        assert_eq!(format!("{wrapped_secret:?}"), REDACTED);
        assert_eq!(format!("{wrapped_secret}"), REDACTED);
        assert_eq!(format!("{wrapped_content:?}"), REDACTED);
        assert_eq!(format!("{wrapped_content}"), REDACTED);

        #[derive(Debug, Error)]
        #[error("request failed: {secret} ({content:?})")]
        struct ProtectedError {
            secret: Sensitive<String>,
            content: UserContent<String>,
        }

        let error = ProtectedError {
            secret: Sensitive::new(secret.to_owned()),
            content: UserContent::new(content.to_owned()),
        };
        let display = error.to_string();
        let debug = format!("{error:?}");
        assert!(!display.contains(secret));
        assert!(!debug.contains(secret));
        assert!(!display.contains(content));
        assert!(!debug.contains(content));
    }

    #[test]
    fn correlation_fields_propagate_without_content() {
        let writer = capture_writer();
        let subscriber = registry()
            .with(tracing_subscriber::EnvFilter::new("trace"))
            .with(fmt::layer().with_ansi(false).with_writer(writer.clone()));
        let operation_id = OperationId::new();
        let request_id = RequestId::new();
        let job_id = JobId::new();
        let conversation_id = ConversationId::new();
        let generation_turn_id = GenerationTurnId::new();
        let context = CorrelationContext::new(operation_id)
            .with_request_id(request_id)
            .with_job_id(job_id)
            .with_conversation_id(conversation_id)
            .with_generation_turn_id(generation_turn_id);

        tracing::subscriber::with_default(subscriber, || {
            let span = context.parent_span("test-operation");
            let _entered = span.enter();
            tracing::info!("event-without-user-content");
            let child = tracing::info_span!("child-operation");
            let _child_entered = child.enter();
            tracing::debug!("nested-event");
        });

        let output = writer.output();
        assert!(output.contains(&operation_id.to_string()));
        assert!(output.contains(&request_id.to_string()));
        assert!(output.contains(&job_id.to_string()));
        assert!(output.contains(&conversation_id.to_string()));
        assert!(output.contains(&generation_turn_id.to_string()));
        assert!(output.contains("test-operation"));
        assert!(output.contains("nested-event"));
        assert!(!output.contains("private-message"));
    }

    #[test]
    fn canary_values_are_redacted_in_captured_events() {
        let writer = capture_writer();
        let subscriber = registry()
            .with(tracing_subscriber::EnvFilter::new("trace"))
            .with(fmt::layer().with_ansi(false).with_writer(writer.clone()));

        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(
                secret = %Sensitive::new("canary-secret"),
                content = ?UserContent::new("private-message"),
                "redaction-canary"
            );
        });

        let output = writer.output();
        assert!(output.contains(REDACTED));
        assert!(!output.contains("canary-secret"));
        assert!(!output.contains("private-message"));
    }

    #[test]
    fn invalid_explicit_output_directory_is_rejected() {
        let config = super::ObservabilityConfig::default().with_local_output(
            super::LocalOutputConfig::new("/definitely/not/a/lettuce-directory"),
        );

        assert_eq!(
            config.validate(),
            Err(super::ConfigError::OutputDirectoryMissing)
        );
    }

    #[test]
    fn invalid_filter_uses_safe_info_fallback() {
        let writer = capture_writer();
        let subscriber = registry()
            .with(super::init::build_filter("["))
            .with(fmt::layer().with_ansi(false).with_writer(writer.clone()));

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("safe-fallback-event");
        });

        assert!(writer.output().contains("safe-fallback-event"));
    }

    #[test]
    fn local_daily_output_flushes_redacted_event() {
        let directory = std::env::temp_dir().join(format!(
            "lettuce-observability-{}",
            lettuce_types::OperationId::new()
        ));
        assert!(fs::create_dir(&directory).is_ok());

        let (writer, guard) = super::init::local_output_writer(
            super::LocalOutputConfig::new(&directory)
                .with_file_prefix("canary")
                .with_queue_capacity(8),
        )
        .expect("isolated temporary output directory should be writable");
        let subscriber = registry()
            .with(tracing_subscriber::EnvFilter::new("trace"))
            .with(fmt::layer().with_ansi(false).with_writer(writer));

        tracing::subscriber::with_default(subscriber, || {
            tracing::error!(
                secret = %Sensitive::new("file-canary-secret"),
                content = ?UserContent::new("file-private-content"),
                "local-output-canary"
            );
        });
        drop(guard);

        let entries = fs::read_dir(&directory).expect("daily appender should create a file");
        let files = entries
            .map(|entry| entry.expect("directory entry should be readable").path())
            .filter(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("canary"))
            })
            .collect::<Vec<_>>();
        assert_eq!(files.len(), 1);
        let output = fs::read_to_string(&files[0]).expect("flushed log file should be readable");
        assert!(output.contains("local-output-canary"));
        assert!(output.contains(REDACTED));
        assert!(!output.contains("file-canary-secret"));
        assert!(!output.contains("file-private-content"));

        fs::remove_dir_all(directory).expect("test output directory should be removable");
    }
}
