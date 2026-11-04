/// Validates an API graph while accepting legacy unquoted numeric tokens.
/// Workflow editor JSON (nodes/links arrays) is deliberately not an API graph.
pub fn valid_comfyui_workflow(template: &str) -> bool {
    let mut normalized = String::with_capacity(template.len());
    let mut quoted = false;
    let mut escaped = false;
    let mut rest = template;
    while let Some(character) = rest.chars().next() {
        if !quoted && character == '%' {
            let tail = &rest[1..];
            if let Some(end) = tail.find('%')
                && matches!(&tail[..end], "WIDTH" | "HEIGHT" | "STEPS" | "CFG" | "SEED" | "DENOISE" | "IMAGE_COUNT" | "BATCH_SIZE")
            {
                normalized.push('0');
                rest = &tail[end + 1..];
                continue;
            }
        }
        normalized.push(character);
        if escaped { escaped = false; }
        else if quoted && character == '\\' { escaped = true; }
        else if character == '"' { quoted = !quoted; }
        rest = &rest[character.len_utf8()..];
    }
    let Ok(serde_json::Value::Object(nodes)) = serde_json::from_str(&normalized) else { return false; };
    !nodes.is_empty() && nodes.values().all(|node| {
        node.get("class_type").and_then(serde_json::Value::as_str).is_some_and(|kind| !kind.trim().is_empty())
            && node.get("inputs").is_some_and(serde_json::Value::is_object)
    })
}

#[cfg(test)]
mod tests {
    use super::valid_comfyui_workflow;

    #[test]
    fn api_graphs_accept_numeric_tokens_and_escaped_text() {
        assert!(valid_comfyui_workflow(r#"{"3":{"class_type":"KSampler","inputs":{"seed":%SEED%,"steps":"%STEPS%","text":"a \"猫\" %PROMPT%"}}}"#));
        for invalid in ["{}", "[]", "null", r#"{"nodes":[],"links":[]}"#, r#"{"3":{"inputs":{}}}"#, r#"{"3":{"class_type":"KSampler","inputs":{"seed":%UNKNOWN%}}}"#] {
            assert!(!valid_comfyui_workflow(invalid), "{invalid}");
        }
    }
}
