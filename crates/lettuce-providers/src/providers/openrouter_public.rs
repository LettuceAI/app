use lettuce_network::{JsonAuth, JsonClient, RequestPolicy};
use serde::Deserialize;

use crate::{ProviderRequestError, RemoteProviders};
use lettuce_settings::SecretStore;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenRouterPublicEndpoint {
    pub id: String,
    pub name: String,
    pub prompt_price: String,
    pub completion_price: String,
    pub context_length: Option<u64>,
    pub uptime_last_30m: Option<f64>,
    pub supports_prompt_caching: bool,
    pub cache_read_price: Option<String>,
    pub cache_write_price: Option<String>,
}

impl<S: SecretStore + ?Sized> RemoteProviders<S> {
    pub async fn openrouter_public_endpoints(
        &self,
        model: &str,
    ) -> Result<Vec<OpenRouterPublicEndpoint>, ProviderRequestError> {
        public_endpoints(&self.network, "https://openrouter.ai/api", model).await
    }
}

fn model_path(model: &str) -> Result<String, ProviderRequestError> {
    let Some((author, slug)) = model.split_once('/') else {
        return Err(ProviderRequestError::Rejected);
    };
    if [author, slug].iter().any(|part| {
        part.is_empty()
            || matches!(*part, "." | "..")
            || part.contains(['/', '\\', '?', '#', '%'])
            || part.chars().any(|c| c.is_control() || c.is_whitespace())
    }) {
        return Err(ProviderRequestError::Rejected);
    }
    Ok(format!("/v1/models/{model}/endpoints"))
}

async fn public_endpoints(
    network: &JsonClient,
    endpoint: &str,
    model: &str,
) -> Result<Vec<OpenRouterPublicEndpoint>, ProviderRequestError> {
    let path = model_path(model)?;
    let response = network
        .get_json(
            endpoint,
            &path,
            &crate::common::ACCEPT_ONLY,
            JsonAuth::None,
            Vec::new(),
            RequestPolicy::BROWSE,
        )
        .await
        .map_err(|_| ProviderRequestError::Unavailable)?;
    if !(200..300).contains(&response.status) {
        return Err(ProviderRequestError::Unavailable);
    }
    parse_public_endpoints(&response.body)
}

#[derive(Deserialize)]
struct Envelope {
    data: Data,
}
#[derive(Deserialize)]
struct Data {
    endpoints: Vec<Endpoint>,
}
#[derive(Deserialize)]
struct Endpoint {
    tag: String,
    provider_name: String,
    pricing: Pricing,
    context_length: Option<u64>,
    uptime_last_30m: Option<f64>,
    #[serde(default)]
    supports_implicit_caching: bool,
}
#[derive(Deserialize)]
struct Pricing {
    prompt: String,
    completion: String,
    input_cache_read: Option<String>,
    input_cache_write: Option<String>,
}

fn parse_public_endpoints(
    body: &[u8],
) -> Result<Vec<OpenRouterPublicEndpoint>, ProviderRequestError> {
    let envelope: Envelope =
        serde_json::from_slice(body).map_err(|_| ProviderRequestError::Malformed)?;
    Ok(envelope
        .data
        .endpoints
        .into_iter()
        .map(|endpoint| OpenRouterPublicEndpoint {
            id: endpoint.tag,
            name: endpoint.provider_name,
            prompt_price: endpoint.pricing.prompt,
            completion_price: endpoint.pricing.completion,
            context_length: endpoint.context_length,
            uptime_last_30m: endpoint.uptime_last_30m,
            supports_prompt_caching: endpoint.supports_implicit_caching
                || endpoint.pricing.input_cache_read.is_some()
                || endpoint.pricing.input_cache_write.is_some(),
            cache_read_price: endpoint.pricing.input_cache_read,
            cache_write_price: endpoint.pricing.input_cache_write,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_rows_keep_uptime_context_and_caching_without_requiring_data_id() {
        let rows = parse_public_endpoints(br#"{"data":{"endpoints":[{"tag":"provider/region","provider_name":"Provider","pricing":{"prompt":"0.001","completion":"0.002","input_cache_read":"0"},"context_length":131072,"uptime_last_30m":99.5},{"tag":"implicit","provider_name":"Implicit","pricing":{"prompt":"0","completion":"0"},"supports_implicit_caching":true}]}}"#).expect("rows");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].context_length, Some(131072));
        assert_eq!(rows[0].uptime_last_30m, Some(99.5));
        assert_eq!(rows[0].cache_read_price.as_deref(), Some("0"));
        assert!(rows.iter().all(|row| row.supports_prompt_caching));
        assert!(parse_public_endpoints(br#"{"data":{"endpoints":[{}]}}"#).is_err());
    }

    #[test]
    fn model_identity_cannot_add_path_or_query_components() {
        assert_eq!(
            model_path("author/model:free").expect("model"),
            "/v1/models/author/model:free/endpoints"
        );
        for model in [
            "",
            "model",
            "author/../model",
            "author/model?token=secret",
            "author/%2f",
            "author/model#fragment",
        ] {
            assert_eq!(model_path(model), Err(ProviderRequestError::Rejected));
        }
    }
}

#[cfg(test)]
mod request_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn endpoint_discovery_is_public_and_does_not_require_an_account() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let endpoint = format!("http://{}/api", listener.local_addr().expect("address"));
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut bytes = vec![0; 8192];
            let count = socket.read(&mut bytes).await.expect("request");
            let request = String::from_utf8(bytes[..count].to_vec()).expect("HTTP");
            assert!(request.starts_with("GET /api/v1/models/author/model/endpoints HTTP/1.1"));
            assert!(!request.to_lowercase().contains("authorization:"));
            let body = r#"{"data":{"endpoints":[]}}"#;
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .expect("response");
        });
        assert!(
            public_endpoints(
                &JsonClient::new().expect("network"),
                &endpoint,
                "author/model"
            )
            .await
            .expect("endpoints")
            .is_empty()
        );
        server.await.expect("server");
    }
}
