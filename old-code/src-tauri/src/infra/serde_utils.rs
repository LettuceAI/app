use serde_json::Value;

pub fn json_value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

pub fn parse_body_to_value(text: &str) -> Value {
    if text.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string()))
    }
}

pub fn truncate_for_log(text: &str, max: usize) -> String {
    if text.len() <= max {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max).collect();
        format!("{}…", truncated)
    }
}

pub fn sanitize_header_value(key: &str, value: &str) -> String {
    let lowered = key.to_ascii_lowercase();
    if lowered.contains("authorization")
        || lowered.contains("api-key")
        || lowered.contains("apikey")
        || lowered.contains("secret")
        || lowered.contains("token")
        || lowered.contains("cookie")
    {
        "***".into()
    } else {
        truncate_for_log(value, 64)
    }
}

pub fn summarize_json(value: &Value) -> String {
    truncate_for_log(&value.to_string(), 512)
}
