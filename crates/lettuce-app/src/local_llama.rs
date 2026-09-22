//! The application side of the embedded llama.cpp runtime: runtime reports
//! and generation metrics in the database, and the frontend events legacy
//! emitted (model load progress, GPU fallback, heartbeats, notices, report
//! updates) through the sink the host attaches.

use std::sync::Arc;

use lettuce_database::Database;
use lettuce_local_llm::generation::{
    LlamaHost, LlamaHostEvent, LlamaMetricsRecord, RuntimeReportStore,
};
use serde_json::Value;

/// Where the host sends local runtime events for the frontend.
#[derive(Clone)]
pub struct LlamaEventSink(Arc<dyn Fn(LlamaHostEvent) + Send + Sync>);

impl LlamaEventSink {
    pub fn new(sink: impl Fn(LlamaHostEvent) + Send + Sync + 'static) -> Self {
        Self(Arc::new(sink))
    }
}

impl std::fmt::Debug for LlamaEventSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("LlamaEventSink")
    }
}

#[derive(Debug)]
pub struct DatabaseLlamaHost {
    database: Arc<Database>,
    events: Option<LlamaEventSink>,
}

impl DatabaseLlamaHost {
    #[must_use]
    pub fn new(database: Arc<Database>, events: Option<LlamaEventSink>) -> Self {
        Self { database, events }
    }
}

fn now_millis() -> i64 {
    lettuce_types::TimestampMillis::now().map_or(0, lettuce_types::TimestampMillis::get)
}

impl RuntimeReportStore for DatabaseLlamaHost {
    fn load(&self, model_path: &str) -> Result<Option<Value>, String> {
        self.database
            .llama_runtime_report(model_path)
            .map_err(|error| error.to_string())
    }

    fn store(&self, model_path: &str, report: &Value) -> Result<bool, String> {
        self.database
            .store_llama_runtime_report(model_path, report, now_millis())
            .map_err(|error| error.to_string())
    }
}

impl LlamaHost for DatabaseLlamaHost {
    fn record_metrics(&self, record: LlamaMetricsRecord) {
        if let Err(error) = self.database.record_llm_generation_metrics(
            &record.id,
            Some(&record.model_path),
            &record.summary,
            &record.samples,
            now_millis(),
        ) {
            tracing::warn!(%error, "failed to persist llm metrics");
        }
    }

    fn event(&self, event: LlamaHostEvent) {
        match &self.events {
            Some(sink) => (sink.0)(event),
            None => tracing::debug!(?event, "llama.cpp runtime event"),
        }
    }
}
