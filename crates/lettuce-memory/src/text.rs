const MAX_LEGACY_MEMORY_TEXT_BYTES: usize = 280;

const REFUSAL_MARKERS: [&str; 8] = [
    "i'm sorry",
    "i am sorry",
    "i can't",
    "i cannot",
    "i'm unable",
    "i am unable",
    "cannot comply",
    "i won't help",
];

const META_MARKERS: [&str; 13] = [
    "as an ai",
    "as a language model",
    "assistant:",
    "user:",
    "system:",
    "content policy",
    "safety policy",
    "cannot assist with",
    "here's a summary",
    "write_summary",
    "create_memory(",
    "\"operations\"",
    "\"items\"",
];

const THINKING_TAGS: [(&str, &str); 6] = [
    ("<think>", "</think>"),
    ("<thinking>", "</thinking>"),
    ("<reason>", "</reason>"),
    ("<reasoning>", "</reasoning>"),
    ("<|channel>thought", "<channel|>"),
    ("<|channel>", "<channel|>"),
];

/// Why a model-written memory text was not kept, as legacy checked it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTextProblem {
    Empty,
    TooLong,
    Refusal,
    Meta,
}

/// Legacy normalization of model output: a surrounding code fence and any
/// thinking section are removed and the text is trimmed.
#[must_use]
pub fn normalize_llm_output_text(raw: &str) -> String {
    let trimmed = raw.trim();
    let without_fences = if trimmed.starts_with("```") {
        let mut lines = trimmed.lines();
        let _ = lines.next();
        let mut body = lines.collect::<Vec<_>>();
        if body.last().is_some_and(|line| line.trim() == "```") {
            body.pop();
        }
        body.join("\n").trim().to_owned()
    } else {
        trimmed.to_owned()
    };
    strip_thinking_tags(&without_fences).trim().to_owned()
}

#[must_use]
pub fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The stored form of a model-written memory: normalized, whitespace collapsed
/// and within legacy's 280-byte limit, rejecting refusals and meta output.
pub fn normalize_memory_text(raw: &str) -> Result<String, MemoryTextProblem> {
    let normalized = collapse_whitespace(&normalize_llm_output_text(raw));
    if normalized.is_empty() {
        return Err(MemoryTextProblem::Empty);
    }
    if normalized.len() > MAX_LEGACY_MEMORY_TEXT_BYTES {
        return Err(MemoryTextProblem::TooLong);
    }
    let lower = normalized.to_ascii_lowercase();
    if REFUSAL_MARKERS.iter().any(|marker| lower.contains(marker)) {
        return Err(MemoryTextProblem::Refusal);
    }
    if META_MARKERS.iter().any(|marker| lower.contains(marker)) {
        return Err(MemoryTextProblem::Meta);
    }
    Ok(normalized)
}

fn strip_thinking_tags(text: &str) -> String {
    let mut content = text.to_owned();
    for (open, close) in THINKING_TAGS {
        loop {
            let lower = content.to_ascii_lowercase();
            let Some(start) = lower.find(open) else {
                break;
            };
            let tail = start + open.len();
            let end = lower[tail..]
                .find(close)
                .map_or(content.len(), |offset| tail + offset + close.len());
            content.replace_range(start..end, "");
        }
    }
    content
}

#[cfg(test)]
mod tests {
    use super::{MemoryTextProblem, normalize_memory_text};

    #[test]
    fn memory_text_follows_the_legacy_checks() {
        assert_eq!(
            normalize_memory_text("```\n  Mira   prefers\n tea \n```"),
            Ok("Mira prefers tea".to_owned())
        );
        assert_eq!(
            normalize_memory_text("<think>plan</think> Ari owns a boat"),
            Ok("Ari owns a boat".to_owned())
        );
        assert_eq!(normalize_memory_text("  "), Err(MemoryTextProblem::Empty));
        assert_eq!(
            normalize_memory_text(&"a".repeat(281)),
            Err(MemoryTextProblem::TooLong)
        );
        assert!(normalize_memory_text(&"a".repeat(280)).is_ok());
        assert_eq!(
            normalize_memory_text("Honestly I can't share that"),
            Err(MemoryTextProblem::Refusal)
        );
        assert_eq!(
            normalize_memory_text("User: likes tea"),
            Err(MemoryTextProblem::Meta)
        );
        assert_eq!(
            normalize_memory_text("Ari cannot swim"),
            Err(MemoryTextProblem::Refusal),
            "legacy matched refusal markers anywhere in the text"
        );
    }
}
