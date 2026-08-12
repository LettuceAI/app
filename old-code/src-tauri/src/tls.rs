use reqwest::{Certificate, ClientBuilder};
use serde_json::Value;

pub fn apply_trusted_certificates(
    app: &tauri::AppHandle,
    mut builder: ClientBuilder,
) -> ClientBuilder {
    let Some(raw) = crate::storage_manager::settings::internal_read_settings(app)
        .ok()
        .flatten()
    else {
        return builder;
    };

    let Ok(json) = serde_json::from_str::<Value>(&raw) else {
        return builder;
    };

    let certificates = json
        .get("appState")
        .and_then(|app_state| app_state.get("trustedCertificates"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for certificate in certificates {
        let Some(pem) = certificate.get("pem").and_then(Value::as_str) else {
            continue;
        };
        match Certificate::from_pem(pem.as_bytes()) {
            Ok(cert) => {
                builder = builder.add_root_certificate(cert);
            }
            Err(error) => {
                crate::utils::log_warn(
                    app,
                    "tls",
                    format!("Skipping invalid trusted certificate: {}", error),
                );
            }
        }
    }

    builder
}

pub fn allow_invalid_tls_for_request(
    app: &tauri::AppHandle,
    provider_id: Option<&str>,
    request_url: &str,
) -> bool {
    let Some(provider_id) = provider_id else {
        return false;
    };
    if !matches!(
        provider_id,
        "ollama" | "lmstudio" | "intenserp" | "automatic1111" | "custom" | "custom-anthropic"
    ) {
        return false;
    }

    let Some(raw) = crate::storage_manager::settings::internal_read_settings(app)
        .ok()
        .flatten()
    else {
        return false;
    };
    let Ok(json) = serde_json::from_str::<Value>(&raw) else {
        return false;
    };
    let Some(credentials) = json.get("providerCredentials").and_then(Value::as_array) else {
        return false;
    };
    let request_base = normalize_base_url(request_url);

    credentials.iter().any(|credential| {
        let credential_provider_id = credential.get("providerId").and_then(Value::as_str);
        if credential_provider_id != Some(provider_id) {
            return false;
        }
        let allow_invalid = credential
            .get("config")
            .and_then(|config| config.get("allowInvalidTls"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !allow_invalid {
            return false;
        }
        let base_url = credential
            .get("baseUrl")
            .and_then(Value::as_str)
            .unwrap_or("");
        normalize_base_url(base_url) == request_base
    })
}

fn normalize_base_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut url = reqwest::Url::parse(trimmed).ok()?;
    let path = url.path().trim_end_matches('/');
    let normalized_path = path
        .strip_suffix("/api/chat")
        .or_else(|| path.strip_suffix("/api/tags"))
        .or_else(|| path.strip_suffix("/v1"))
        .unwrap_or(path)
        .trim_end_matches('/');
    if normalized_path.is_empty() {
        url.set_path("/");
    } else {
        url.set_path(&format!("{}/", normalized_path));
    }
    url.set_query(None);
    url.set_fragment(None);
    Some(url.to_string())
}
