use serde_json::Value;

/// An error a provider reported inside a successful (HTTP 2xx) JSON body, as
/// OpenRouter does for upstream failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderBodyError {
    pub code: Option<u16>,
    pub message: String,
}

impl ProviderBodyError {
    pub(crate) const fn is_transient(&self) -> bool {
        matches!(self.code, Some(500 | 502 | 503 | 504 | 529))
    }

    pub(crate) fn describe(&self) -> String {
        match self.code {
            Some(code) => format!("Provider error {code}: {}", self.message),
            None => format!("Provider error: {}", self.message),
        }
    }
}

/// The `error` field as a non-blank string, or as an object with an optional
/// numeric or numeric-string `code` and a `message`/`msg` (the whole object
/// when neither is usable). Any other value is not an error.
pub(crate) fn extract_body_error(response: &Value) -> Option<ProviderBodyError> {
    match response.get("error")? {
        Value::String(message) => {
            let message = message.trim();
            (!message.is_empty()).then(|| ProviderBodyError {
                code: None,
                message: message.to_owned(),
            })
        }
        Value::Object(map) => {
            let code = map.get("code").and_then(|code| {
                code.as_u64()
                    .or_else(|| code.as_str().and_then(|value| value.parse().ok()))
                    .and_then(|value| u16::try_from(value).ok())
            });
            let message = map
                .get("message")
                .or_else(|| map.get("msg"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map_or_else(|| Value::Object(map.clone()).to_string(), str::to_owned);
            Some(ProviderBodyError { code, message })
        }
        _ => None,
    }
}
