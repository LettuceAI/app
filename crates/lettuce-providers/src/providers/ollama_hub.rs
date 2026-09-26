//! An Ollama server's own model store: what it has, pulling a model into it
//! and deleting one.

use lettuce_models::{ProviderAccount, ProviderConfig, ProviderProtocol};
use lettuce_network::{JsonAuth, JsonClient, JsonSecretHeader, RequestPolicy, RequestTimeout};
use lettuce_settings::SecretStore;
use serde_json::{Value, json};

use crate::common::{
    AuthPlan, Credentials, STANDARD_HEADERS, generation_policy, load_auth, load_secret_headers,
};
use crate::providers::ollama::{DEFAULT_ENDPOINT, api_base};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OllamaHubError {
    #[error("Selected provider is not an Ollama provider")]
    NotOllama,
    #[error("Model reference is empty")]
    EmptyReference,
    #[error("The provider's credentials could not be read.")]
    Credentials,
    #[error("{0}")]
    Message(String),
}

/// A model the Ollama server has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OllamaInstalledModel {
    pub name: String,
    pub size: Option<u64>,
    pub modified_at: Option<String>,
    pub digest: Option<String>,
    pub parameter_size: Option<String>,
    pub quantization_level: Option<String>,
    pub family: Option<String>,
}

/// One progress line of a pull.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OllamaPullProgress {
    /// `downloading` until the pull reports success, then `complete`.
    pub status: &'static str,
    pub completed: u64,
    pub total: u64,
    pub speed_bytes_per_sec: u64,
}

struct Connection<'a> {
    base: std::borrow::Cow<'a, str>,
    auth: JsonAuth,
    secret_headers: Vec<JsonSecretHeader>,
    policy: RequestPolicy,
}

async fn connect<'a, S: SecretStore + ?Sized>(
    secret_store: &S,
    account: &'a ProviderAccount,
) -> Result<Connection<'a>, OllamaHubError> {
    if account.protocol != ProviderProtocol::Ollama
        || !account.provider_kind.eq_ignore_ascii_case("ollama")
        || !matches!(
            account.config,
            ProviderConfig::Standard | ProviderConfig::Ollama(_)
        )
    {
        return Err(OllamaHubError::NotOllama);
    }
    let credentials = Credentials::from(account);
    let auth = load_auth(AuthPlan::OptionalBearer, secret_store, &credentials)
        .await
        .map_err(|_| OllamaHubError::Credentials)?;
    let secret_headers = load_secret_headers(secret_store, &credentials)
        .await
        .map_err(|_| OllamaHubError::Credentials)?;
    Ok(Connection {
        base: api_base(account.endpoint.as_deref().unwrap_or(DEFAULT_ENDPOINT)),
        auth,
        secret_headers,
        policy: generation_policy(&credentials),
    })
}

fn returned(status: u16, body: &[u8]) -> OllamaHubError {
    OllamaHubError::Message(format!(
        "Ollama returned {}: {}",
        lettuce_network::status_text(status),
        String::from_utf8_lossy(body)
    ))
}

fn text(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}

pub(crate) fn parse_inventory(payload: &Value) -> Vec<OllamaInstalledModel> {
    payload
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let details = item.get("details");
            Some(OllamaInstalledModel {
                name: item.get("name")?.as_str()?.to_owned(),
                size: item.get("size").and_then(Value::as_u64),
                modified_at: text(item.get("modified_at")),
                digest: text(item.get("digest")),
                parameter_size: text(details.and_then(|details| details.get("parameter_size"))),
                quantization_level: text(
                    details.and_then(|details| details.get("quantization_level")),
                ),
                family: text(details.and_then(|details| details.get("family"))),
            })
        })
        .collect()
}

pub(crate) async fn inventory<S: SecretStore + ?Sized>(
    secret_store: &S,
    network: &JsonClient,
    account: &ProviderAccount,
) -> Result<Vec<OllamaInstalledModel>, OllamaHubError> {
    let connection = connect(secret_store, account).await?;
    let response = network
        .get_json(
            &connection.base,
            "/api/tags",
            &STANDARD_HEADERS,
            connection.auth,
            connection.secret_headers,
            connection.policy,
        )
        .await
        .map_err(|error| OllamaHubError::Message(error.to_string()))?;
    if !(200..300).contains(&response.status) {
        return Err(returned(response.status, &response.body));
    }
    let payload: Value = serde_json::from_slice(&response.body)
        .map_err(|error| OllamaHubError::Message(error.to_string()))?;
    Ok(parse_inventory(&payload))
}

pub(crate) async fn delete<S: SecretStore + ?Sized>(
    secret_store: &S,
    network: &JsonClient,
    account: &ProviderAccount,
    model_name: &str,
) -> Result<(), OllamaHubError> {
    let connection = connect(secret_store, account).await?;
    let response = network
        .delete_json(
            &connection.base,
            "/api/delete",
            json!({ "name": model_name }).to_string().into_bytes(),
            &STANDARD_HEADERS,
            connection.auth,
            connection.secret_headers,
            connection.policy,
        )
        .await
        .map_err(|error| {
            OllamaHubError::Message(format!("Ollama delete request failed: {error}"))
        })?;
    if !(200..300).contains(&response.status) {
        return Err(returned(response.status, &response.body));
    }
    Ok(())
}

/// Folds pull lines into progress the way the old queue showed it.
pub(crate) struct PullProgressReader {
    buffer: Vec<u8>,
    completed: u64,
    total: u64,
    status: String,
    speed_anchor: std::time::Instant,
    speed_anchor_bytes: u64,
    speed_bytes_per_sec: u64,
}

pub(crate) enum PullLine {
    Progress(OllamaPullProgress),
    Done,
    Failed(String),
    TooLong,
}

const MAX_PULL_LINE_BYTES: usize = 1024 * 1024;

impl PullProgressReader {
    pub(crate) fn new() -> Self {
        Self {
            buffer: Vec::new(),
            completed: 0,
            total: 0,
            status: "downloading".to_owned(),
            speed_anchor: std::time::Instant::now(),
            speed_anchor_bytes: 0,
            speed_bytes_per_sec: 0,
        }
    }

    fn line(&mut self, value: &Value) -> PullLine {
        if let Some(error) = value.get("error").and_then(Value::as_str) {
            return PullLine::Failed(error.to_owned());
        }
        let status = value
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or(&self.status)
            .to_owned();
        let completed = value
            .get("completed")
            .and_then(Value::as_u64)
            .unwrap_or(self.completed);
        let total = value
            .get("total")
            .and_then(Value::as_u64)
            .unwrap_or(self.total);
        if completed >= self.speed_anchor_bytes {
            let elapsed =
                u64::try_from(self.speed_anchor.elapsed().as_millis()).unwrap_or(u64::MAX);
            if elapsed >= 1_000 {
                self.speed_bytes_per_sec =
                    completed.saturating_sub(self.speed_anchor_bytes) * 1_000 / elapsed.max(1);
                self.speed_anchor = std::time::Instant::now();
                self.speed_anchor_bytes = completed;
            }
        } else {
            self.speed_anchor = std::time::Instant::now();
            self.speed_anchor_bytes = completed;
            self.speed_bytes_per_sec = 0;
        }
        self.completed = completed;
        self.total = total;
        self.status.clone_from(&status);
        if status.eq_ignore_ascii_case("success") {
            return PullLine::Done;
        }
        PullLine::Progress(OllamaPullProgress {
            status: "downloading",
            completed,
            total,
            speed_bytes_per_sec: self.speed_bytes_per_sec,
        })
    }

    /// The complete lines in `chunk`, in order.
    pub(crate) fn feed(&mut self, chunk: &[u8]) -> Vec<PullLine> {
        self.buffer.extend_from_slice(chunk);
        let mut lines = Vec::new();
        while let Some(index) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let line = self.buffer.drain(..=index).collect::<Vec<_>>();
            if let Ok(value) = serde_json::from_str::<Value>(String::from_utf8_lossy(&line).trim())
            {
                lines.push(self.line(&value));
            }
        }
        if self.buffer.len() > MAX_PULL_LINE_BYTES {
            lines.push(PullLine::TooLong);
        }
        lines
    }

    /// What the unterminated last line says, if anything final.
    pub(crate) fn finish(&self) -> Option<PullLine> {
        let value =
            serde_json::from_str::<Value>(String::from_utf8_lossy(&self.buffer).trim()).ok()?;
        if let Some(error) = value.get("error").and_then(Value::as_str) {
            return Some(PullLine::Failed(error.to_owned()));
        }
        value
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|status| status.eq_ignore_ascii_case("success"))
            .then_some(PullLine::Done)
    }

    fn complete(&self) -> OllamaPullProgress {
        OllamaPullProgress {
            status: "complete",
            completed: self.completed,
            total: self.total,
            speed_bytes_per_sec: 0,
        }
    }
}

pub(crate) async fn pull<S: SecretStore + ?Sized>(
    secret_store: &S,
    network: &JsonClient,
    account: &ProviderAccount,
    model_ref: &str,
    on_progress: &mut (dyn FnMut(OllamaPullProgress) + Send),
) -> Result<(), OllamaHubError> {
    let model_ref = model_ref.trim();
    if model_ref.is_empty() {
        return Err(OllamaHubError::EmptyReference);
    }
    let connection = connect(secret_store, account).await?;
    let mut stream = network
        .post_json_stream(
            &connection.base,
            "/api/pull",
            json!({ "model": model_ref, "stream": true })
                .to_string()
                .into_bytes(),
            &STANDARD_HEADERS,
            connection.auth,
            connection.secret_headers,
            RequestPolicy {
                timeout: RequestTimeout::Transfer,
                ..connection.policy
            },
        )
        .await
        .map_err(|error| OllamaHubError::Message(format!("Ollama pull request failed: {error}")))?;
    if !(200..300).contains(&stream.status) {
        let mut body = Vec::new();
        while let Ok(Some(chunk)) = stream.next_chunk().await {
            body.extend(chunk);
        }
        return Err(returned(stream.status, &body));
    }
    let mut reader = PullProgressReader::new();
    while let Some(chunk) = stream
        .next_chunk()
        .await
        .map_err(|error| OllamaHubError::Message(format!("Ollama pull stream error: {error}")))?
    {
        for line in reader.feed(&chunk) {
            match line {
                PullLine::TooLong => {
                    return Err(OllamaHubError::Message(
                        "Ollama pull stream error: a progress line is too long".to_owned(),
                    ));
                }
                PullLine::Progress(progress) => on_progress(progress),
                PullLine::Done => {
                    on_progress(reader.complete());
                    return Ok(());
                }
                PullLine::Failed(error) => return Err(OllamaHubError::Message(error)),
            }
        }
    }
    match reader.finish() {
        Some(PullLine::Failed(error)) => Err(OllamaHubError::Message(error)),
        Some(PullLine::Done) => {
            on_progress(reader.complete());
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_entries_carry_their_details() {
        let models = parse_inventory(&json!({"models": [
            {"name": "llama3:8b", "size": 5, "digest": "d", "details": {
                "parameter_size": "8B", "quantization_level": "Q4_0", "family": "llama"}},
            {"size": 1}
        ]}));
        assert_eq!(
            models,
            vec![OllamaInstalledModel {
                name: "llama3:8b".to_owned(),
                size: Some(5),
                modified_at: None,
                digest: Some("d".to_owned()),
                parameter_size: Some("8B".to_owned()),
                quantization_level: Some("Q4_0".to_owned()),
                family: Some("llama".to_owned()),
            }]
        );
    }

    #[test]
    fn pull_lines_report_progress_until_success_or_an_error() {
        let mut reader = PullProgressReader::new();
        let lines = reader.feed(
            b"{\"status\":\"pulling manifest\"}\n{\"status\":\"pulling ab\",\"completed\":5,\"total\":9}\nnot json\n{\"status\":\"verifying\"}\n{\"status\":\"succ",
        );
        let progress = lines
            .iter()
            .map(|line| match line {
                PullLine::Progress(progress) => (progress.completed, progress.total),
                _ => panic!("progress only"),
            })
            .collect::<Vec<_>>();
        assert_eq!(progress, [(0, 0), (5, 9), (5, 9)]);
        assert!(matches!(
            reader.feed(b"ess\"}\n").as_slice(),
            [PullLine::Done]
        ));
        assert_eq!(reader.complete().status, "complete");
        let mut split = PullProgressReader::new();
        let text = "{\"error\":\"café\"}\n".as_bytes();
        let middle = text.iter().position(|byte| *byte == 0xc3).expect("é") + 1;
        assert!(split.feed(&text[..middle]).is_empty());
        assert!(matches!(
            split.feed(&text[middle..]).as_slice(),
            [PullLine::Failed(error)] if error == "café"
        ));
        let mut failed = PullProgressReader::new();
        failed.feed(b"{\"error\":\"pull model manifest: file does not exist\"}");
        assert!(matches!(
            failed.finish(),
            Some(PullLine::Failed(error)) if error == "pull model manifest: file does not exist"
        ));
    }
}
