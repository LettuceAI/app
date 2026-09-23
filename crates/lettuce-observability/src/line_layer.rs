//! The tracing layer that writes each event as a log file line.

use std::fmt::Write as _;
use std::io::Write as _;

use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::log_files::{LogEntry, SECRET_PARAMETERS, sanitize_message};

#[derive(Default)]
struct EventFields {
    component: Option<String>,
    message: Option<String>,
    rest: Vec<(&'static str, String)>,
}

impl EventFields {
    fn record(&mut self, name: &'static str, value: String) {
        match name {
            "component" => self.component = Some(value),
            "message" | "msg" => self.message = Some(value),
            name => self.rest.push((name, value)),
        }
    }
}

impl Visit for EventFields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.record(field.name(), value.to_owned());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record(field.name(), format!("{value:?}"));
    }
}

/// Writes every event it sees as one sanitized log line.
pub struct LineLayer<W> {
    writer: W,
}

impl<W> std::fmt::Debug for LineLayer<W> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("LineLayer").finish_non_exhaustive()
    }
}

impl<W> LineLayer<W> {
    pub const fn new(writer: W) -> Self {
        Self { writer }
    }
}

pub(crate) fn event_entry<S>(event: &Event<'_>, context: &Context<'_, S>) -> LogEntry
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let mut fields = EventFields::default();
    event.record(&mut fields);
    let metadata = event.metadata();
    let component = fields
        .component
        .unwrap_or_else(|| metadata.target().to_owned());
    let function = metadata
        .file()
        .filter(|file| {
            std::path::Path::new(file).is_relative() && !file.contains(char::is_whitespace)
        })
        .zip(metadata.line())
        .map(|(file, line)| format!("{file}:{line}:1"));
    let span_path = context.event_scope(event).map(|scope| {
        scope
            .from_root()
            .map(|span| span.name())
            .collect::<Vec<_>>()
            .join(" > ")
    });
    let mut message = sanitize_message(
        &component,
        fields.message.as_deref().unwrap_or("(no message)"),
    );
    for (name, value) in &fields.rest {
        if SECRET_PARAMETERS.contains(&name.to_ascii_lowercase().as_str()) {
            let _ = write!(message, " {name}=***");
        } else {
            let _ = write!(message, " {name}={}", sanitize_message(&component, value));
        }
    }
    if let Some(path) = span_path {
        let _ = write!(message, " [span={path}]");
    }
    LogEntry {
        timestamp: chrono::Local::now().to_rfc3339(),
        level: metadata.level().as_str().to_owned(),
        component,
        function,
        message,
    }
}

impl<S, W> Layer<S> for LineLayer<W>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    W: for<'w> MakeWriter<'w> + 'static,
{
    fn on_event(&self, event: &Event<'_>, context: Context<'_, S>) {
        if *event.metadata().level() == tracing::Level::TRACE {
            return;
        }
        let line = event_entry(event, &context).line();
        let _ = self.writer.make_writer().write_all(line.as_bytes());
    }
}
