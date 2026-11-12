const MAX_MEMORY_TEXT_CHARS: usize = 280;

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

/// Transcript role labels; they only count at the start of a line, since a memory may quote someone speaking to the
/// user.
const ROLE_LABELS: [&str; 3] = ["assistant:", "user:", "system:"];

const META_MARKERS: [&str; 10] = [
    "as an ai",
    "as a language model",
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

/// Why a model-written memory text was not kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTextProblem {
    Empty,
    TooLong,
    Refusal,
    Meta,
}

/// Normalization of model output: a surrounding code fence and any thinking
/// section are removed and the text is trimmed.
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
/// and within a 280-byte limit, rejecting refusals and meta output.
pub fn normalize_memory_text(raw: &str) -> Result<String, MemoryTextProblem> {
    let normalized = collapse_whitespace(&normalize_llm_output_text(raw));
    if normalized.is_empty() {
        return Err(MemoryTextProblem::Empty);
    }
    if normalized.chars().count() > MAX_MEMORY_TEXT_CHARS {
        return Err(MemoryTextProblem::TooLong);
    }
    let lower = normalized.to_lowercase();
    let opening = lower.trim_start_matches(|character: char| {
        character.is_whitespace() || matches!(character, '"' | '\'' | '*' | '`' | '“' | '‘')
    });
    if REFUSAL_MARKERS
        .iter()
        .any(|marker| opening.starts_with(marker))
    {
        return Err(MemoryTextProblem::Refusal);
    }
    if META_MARKERS.iter().any(|marker| lower.contains(marker))
        || lower.lines().any(|line| {
            let line = line.trim_start();
            ROLE_LABELS.iter().any(|label| line.starts_with(label))
        })
    {
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
            normalize_memory_text("I'm sorry, but I can't help with that."),
            Err(MemoryTextProblem::Refusal)
        );
        assert_eq!(
            normalize_memory_text("\"I cannot assist with this request.\""),
            Err(MemoryTextProblem::Refusal)
        );
        assert_eq!(
            normalize_memory_text("User: likes tea"),
            Err(MemoryTextProblem::Meta)
        );
        assert_eq!(
            normalize_memory_text("As an AI, I have no memories."),
            Err(MemoryTextProblem::Meta)
        );
    }

    #[test]
    fn roleplay_memories_that_quote_refusals_or_address_the_user_are_kept() {
        for text in [
            "Ari cannot swim",
            "Mira told Alex \"I can't go back there\" and left the tavern",
            "Cassian said he is sorry about the fire; I'm sorry, he repeated twice",
            "Lyra warned the group: user: beware the tide (a note she carved into the pier)",
            "Mira told the user: stay away from the harbor",
        ] {
            assert!(normalize_memory_text(text).is_ok(), "{text}");
        }
    }

    #[test]
    fn the_length_limit_counts_characters() {
        let accented = "é".repeat(280);
        assert!(accented.len() > 280);
        assert!(normalize_memory_text(&accented).is_ok());
        assert_eq!(
            normalize_memory_text(&"é".repeat(281)),
            Err(MemoryTextProblem::TooLong)
        );
    }
}
