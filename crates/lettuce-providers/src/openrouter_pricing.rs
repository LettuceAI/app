use lettuce_models::{ProviderAccount, ProviderConfig, ProviderProtocol};
use lettuce_network::{JsonQueryParameter, JsonResponse};
use lettuce_settings::SecretStore;
use lettuce_usage::{ModelPricing, OpenRouterEndpointPricing, OpenRouterGenerationDetails};
use serde::Deserialize;

use crate::common::{
    ACCEPT_ONLY, AdapterError, AuthPlan, Credentials, load_auth, load_secret_headers, probe_policy,
};
use crate::{ProviderRequestError, RemoteProviders};

impl<S: SecretStore + ?Sized> RemoteProviders<S> {
    pub async fn openrouter_endpoint_pricing(
        &self,
        account: &ProviderAccount,
        model: &str,
    ) -> Result<Vec<OpenRouterEndpointPricing>, ProviderRequestError> {
        let Some((author, slug)) = model.split_once('/') else {
            return Err(ProviderRequestError::Unsupported);
        };
        if [author, slug].iter().any(|part| {
            part.is_empty()
                || matches!(*part, "." | "..")
                || part.contains(['/', '\\', '?', '#', '%'])
                || part.chars().any(|c| c.is_control() || c.is_whitespace())
        }) {
            return Err(ProviderRequestError::Unsupported);
        }
        let response = self
            .openrouter_get(account, &format!("models/{model}/endpoints"), &[])
            .await?;
        parse_endpoints(&response.body, model).map_err(Into::into)
    }

    pub async fn openrouter_generation_details(
        &self,
        account: &ProviderAccount,
        generation_id: &str,
    ) -> Result<Option<OpenRouterGenerationDetails>, ProviderRequestError> {
        if generation_id.trim().is_empty() {
            return Err(ProviderRequestError::Unsupported);
        }
        let response = self
            .openrouter_get(
                account,
                "generation",
                &[JsonQueryParameter {
                    name: "id",
                    value: generation_id,
                }],
            )
            .await;
        let response = match response {
            Err(AdapterError::Provider(failure)) if failure.status == 404 => return Ok(None),
            result => result?,
        };
        parse_generation(&response.body, generation_id)
            .map(Some)
            .map_err(Into::into)
    }

    async fn openrouter_get(
        &self,
        account: &ProviderAccount,
        path: &str,
        query: &[JsonQueryParameter<'_>],
    ) -> Result<JsonResponse, AdapterError> {
        if !account.provider_kind.eq_ignore_ascii_case("openrouter")
            || account.protocol != ProviderProtocol::OpenAiCompatible
            || !matches!(account.config, ProviderConfig::Standard)
        {
            return Err(AdapterError::Rejected);
        }
        let endpoint = account
            .endpoint
            .as_deref()
            .or(crate::openrouter::DESCRIPTOR.default_endpoint)
            .ok_or(AdapterError::Rejected)?;
        let path = if endpoint.trim_end_matches('/').ends_with("/v1") {
            format!("/{path}")
        } else {
            format!("/v1/{path}")
        };
        let credentials = Credentials::from(account);
        let auth = load_auth(AuthPlan::Bearer, &*self.secret_store, &credentials).await?;
        let headers = load_secret_headers(&*self.secret_store, &credentials).await?;
        let response = self
            .network
            .get_json_with_query(
                endpoint,
                &path,
                query,
                &ACCEPT_ONLY,
                auth,
                headers,
                probe_policy(&credentials),
            )
            .await?;
        if let Some(error) = AdapterError::from_response(&response) {
            return Err(error);
        }
        Ok(response)
    }
}

#[derive(Deserialize)]
struct Envelope<T> {
    data: T,
}

#[derive(Deserialize)]
struct Endpoints {
    id: String,
    endpoints: Vec<Endpoint>,
}

#[derive(Deserialize)]
struct Endpoint {
    provider_name: String,
    provider_display_name: Option<String>,
    tag: Option<String>,
    pricing: serde_json::Value,
}

fn parse_endpoints(
    body: &[u8],
    model: &str,
) -> Result<Vec<OpenRouterEndpointPricing>, AdapterError> {
    let parsed: Envelope<Endpoints> =
        serde_json::from_slice(body).map_err(|_| AdapterError::MalformedResponse)?;
    if parsed.data.id != model {
        return Err(AdapterError::MalformedResponse);
    }
    parsed
        .data
        .endpoints
        .into_iter()
        .map(|endpoint| {
            if endpoint.provider_name.trim().is_empty() {
                return Err(AdapterError::MalformedResponse);
            }
            let price = |name: &str, required: bool| -> Result<String, AdapterError> {
                match endpoint.pricing.get(name) {
                    Some(serde_json::Value::String(value)) => Ok(value.clone()),
                    Some(serde_json::Value::Number(value)) => Ok(value.to_string()),
                    None | Some(serde_json::Value::Null) if !required => Ok(String::new()),
                    _ => Err(AdapterError::MalformedResponse),
                }
            };
            let pricing = ModelPricing {
                prompt: price("prompt", true)?,
                completion: price("completion", true)?,
                request: price("request", false)?,
                image: price("image", false)?,
                image_output: price("image_output", false)?,
                web_search: price("web_search", false)?,
                internal_reasoning: price("internal_reasoning", false)?,
                input_cache_read: price("input_cache_read", false)?,
                input_cache_write: price("input_cache_write", false)?,
            };
            for value in [&pricing.prompt, &pricing.completion] {
                if !value
                    .parse::<f64>()
                    .is_ok_and(|v| v.is_finite() && v >= 0.0)
                {
                    return Err(AdapterError::MalformedResponse);
                }
            }
            Ok(OpenRouterEndpointPricing {
                provider_name: endpoint.provider_name,
                provider_display_name: endpoint.provider_display_name,
                tag: endpoint.tag,
                pricing,
            })
        })
        .collect()
}

#[derive(Deserialize)]
struct Generation {
    id: String,
    model: String,
    provider_name: Option<String>,
    native_tokens_prompt: Option<u64>,
    native_tokens_completion: Option<u64>,
    tokens_prompt: Option<u64>,
    tokens_completion: Option<u64>,
    native_tokens_cached: Option<u64>,
    native_tokens_reasoning: Option<u64>,
    total_cost: Option<lettuce_conversations::ProviderReportedCost>,
}

fn parse_generation(
    body: &[u8],
    generation_id: &str,
) -> Result<OpenRouterGenerationDetails, AdapterError> {
    let parsed: Envelope<Generation> =
        serde_json::from_slice(body).map_err(|_| AdapterError::MalformedResponse)?;
    let data = parsed.data;
    if data.id != generation_id || data.model.trim().is_empty() {
        return Err(AdapterError::MalformedResponse);
    }
    Ok(OpenRouterGenerationDetails {
        generation_id: data.id,
        model: data.model,
        provider_name: data.provider_name,
        native_prompt_tokens: data.native_tokens_prompt,
        native_completion_tokens: data.native_tokens_completion,
        normalized_prompt_tokens: data.tokens_prompt,
        normalized_completion_tokens: data.tokens_completion,
        native_cached_tokens: data.native_tokens_cached,
        native_reasoning_tokens: data.native_tokens_reasoning,
        total_cost: data.total_cost,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn billing_evidence_rejects_wrong_identity_and_malformed_counters() {
        let base = serde_json::json!({"data":{"id":"gen-one","model":"author/model"}});
        let details = parse_generation(&serde_json::to_vec(&base).expect("JSON"), "gen-one")
            .expect("absent evidence");
        assert_eq!(details.native_prompt_tokens, None);
        assert_eq!(details.total_cost, None);
        for (key, value) in [
            ("id", serde_json::json!("gen-other")),
            ("model", serde_json::json!("")),
            ("native_tokens_prompt", serde_json::json!(-1)),
            ("native_tokens_completion", serde_json::json!(1.5)),
            ("total_cost", serde_json::json!(-0.1)),
        ] {
            let mut body = base.clone();
            body["data"][key] = value;
            assert!(
                parse_generation(&serde_json::to_vec(&body).expect("JSON"), "gen-one").is_err()
            );
        }
    }

    #[test]
    fn endpoint_prices_reject_wrong_models_and_unusable_required_prices() {
        let base = serde_json::json!({"data":{"id":"author/model","endpoints":[
            {"provider_name":"Provider","pricing":{"prompt":"0.001","completion":"0.002"}}
        ]}});
        let bytes = serde_json::to_vec(&base).expect("JSON");
        assert!(parse_endpoints(&bytes, "different/model").is_err());
        for price in [
            serde_json::json!("NaN"),
            serde_json::json!(-1),
            serde_json::Value::Null,
        ] {
            let mut body = base.clone();
            body["data"]["endpoints"][0]["pricing"]["prompt"] = price;
            assert!(
                parse_endpoints(&serde_json::to_vec(&body).expect("JSON"), "author/model").is_err()
            );
        }
        let prices = parse_endpoints(&bytes, "author/model").expect("prices");
        assert!(prices[0].pricing.input_cache_write.is_empty());
    }
}
