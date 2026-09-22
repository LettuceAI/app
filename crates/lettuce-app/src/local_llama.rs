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

#[derive(Debug, thiserror::Error)]
pub enum LocalLlamaCommandError {
    #[error(transparent)]
    ContextInfo(#[from] lettuce_local_llm::context_info::ContextInfoError),
    #[error(transparent)]
    EmbeddedTemplate(#[from] lettuce_local_llm::llama::EmbeddedTemplateError),
    #[error(transparent)]
    Worker(#[from] lettuce_local_llm::generation::LlamaGenerationError),
    #[error("llama.cpp command task failed")]
    Task,
    #[error("llama.cpp inference worker dropped its response")]
    WorkerDroppedResponse,
}

impl crate::AppBackend {
    /// The GPU devices llama.cpp can use (legacy `llamacpp_backend_devices`).
    #[must_use]
    pub fn llama_backend_devices(&self) -> Vec<lettuce_local_llm::hardware::LlamaGpuDeviceInfo> {
        lettuce_local_llm::hardware::list_gpu_devices()
    }

    /// The model editor's fit estimate (legacy `llamacpp_context_info`).
    pub async fn llama_context_info(
        &self,
        request: lettuce_local_llm::context_info::ContextInfoRequest,
    ) -> Result<lettuce_local_llm::context_info::LlamaCppContextInfo, LocalLlamaCommandError> {
        tokio::task::spawn_blocking(move || lettuce_local_llm::context_info::context_info(request))
            .await
            .map_err(|_| LocalLlamaCommandError::Task)?
            .map_err(Into::into)
    }

    /// The chat template embedded in a model file (legacy
    /// `llamacpp_embedded_chat_template`).
    pub async fn llama_embedded_chat_template(
        &self,
        model_path: String,
    ) -> Result<String, LocalLlamaCommandError> {
        tokio::task::spawn_blocking(move || {
            lettuce_local_llm::llama::embedded_chat_template(&model_path)
        })
        .await
        .map_err(|_| LocalLlamaCommandError::Task)?
        .map_err(Into::into)
    }

    /// Frees the loaded model and its cached contexts (legacy
    /// `llamacpp_unload`); nothing to do before the runtime first ran.
    pub async fn unload_local_llama(&self) -> Result<(), LocalLlamaCommandError> {
        let Some(local_llama) = self.started_local_llama() else {
            return Ok(());
        };
        let (sender, receiver) = tokio::sync::oneshot::channel();
        local_llama.runtime().unload(Box::new(move |result| {
            let _ = sender.send(result);
        }));
        receiver
            .await
            .map_err(|_| LocalLlamaCommandError::WorkerDroppedResponse)?
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use crate::AppBackend;

    fn backend() -> AppBackend {
        AppBackend::open_in_memory(lettuce_types::TimestampMillis::new(1)).expect("backend")
    }

    #[tokio::test]
    async fn unloading_before_the_runtime_started_is_a_no_op() {
        let backend = backend();
        backend.unload_local_llama().await.expect("unload");
        assert!(backend.started_local_llama().is_none());
    }

    #[tokio::test]
    async fn template_reads_reject_missing_files_with_the_legacy_wording() {
        let error = backend()
            .llama_embedded_chat_template("/missing/model.gguf".to_owned())
            .await
            .expect_err("missing file");
        assert_eq!(
            error.to_string(),
            "llama.cpp model path not found: /missing/model.gguf"
        );
    }

    #[tokio::test]
    #[ignore = "needs a local GGUF model in LETTUCE_PLAN_MODEL"]
    async fn reads_the_embedded_template_and_estimates_the_fit() {
        let Ok(path) = std::env::var("LETTUCE_PLAN_MODEL") else {
            return;
        };
        let backend = backend();
        let template = backend
            .llama_embedded_chat_template(path.clone())
            .await
            .expect("template");
        assert!(template.contains("im_start") || template.contains("{%"));
        let info = backend
            .llama_context_info(lettuce_local_llm::context_info::ContextInfoRequest {
                model_path: path,
                ..Default::default()
            })
            .await
            .expect("context info");
        assert!(info.max_context_length > 0);
        let _devices = backend.llama_backend_devices();
    }
}
