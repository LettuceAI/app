//! Splits reasoning written inline between thinking tags (`<think>`,
//! `<thinking>`, `<reason>`, `<reasoning>`, Gemma's channel markers) from
//! visible text, both for streamed chunks and complete responses.

#[derive(Debug, Default)]
pub struct ThinkingTagParser {
    in_think: bool,
    close_tag: Option<&'static str>,
    pending: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ThinkingSplit {
    pub content: String,
    pub reasoning: String,
}

const TAG_PAIRS: [(&str, &str); 6] = [
    ("<think>", "</think>"),
    ("<thinking>", "</thinking>"),
    ("<reason>", "</reason>"),
    ("<reasoning>", "</reasoning>"),
    ("<|channel>thought", "<channel|>"),
    ("<|channel>", "<channel|>"),
];

impl ThinkingTagParser {
    /// A parser that starts inside reasoning ending at `close_tag`, for a
    /// reply prefilled with the reasoning opener.
    #[must_use]
    pub fn starting_in_reasoning(close_tag: &'static str) -> Self {
        Self {
            in_think: true,
            close_tag: Some(close_tag),
            pending: String::new(),
        }
    }

    /// Feeds one chunk; a possible partial tag at the end is held back.
    pub fn feed(&mut self, chunk: &str) -> ThinkingSplit {
        self.pending.push_str(chunk);
        let mut split = ThinkingSplit::default();
        loop {
            if let Some(close) = self.close_tag.filter(|_| self.in_think) {
                let lower = self.pending.to_ascii_lowercase();
                if let Some(index) = lower.find(close) {
                    split.reasoning.push_str(&self.pending[..index]);
                    self.pending.drain(..index + close.len());
                    self.in_think = false;
                    self.close_tag = None;
                    continue;
                }
                let keep = partial_suffix_len(&self.pending, close);
                let emit = self.pending.len().saturating_sub(keep);
                if emit > 0 {
                    split.reasoning.push_str(&self.pending[..emit]);
                    self.pending.drain(..emit);
                }
                break;
            }
            if let Some((index, open, close)) = earliest_open_tag(&self.pending) {
                split.content.push_str(&self.pending[..index]);
                self.pending.drain(..index + open.len());
                self.in_think = true;
                self.close_tag = Some(close);
                continue;
            }
            let opens = TAG_PAIRS.map(|(open, _)| open);
            let keep = opens
                .iter()
                .map(|open| partial_suffix_len(&self.pending, open))
                .max()
                .unwrap_or(0);
            let emit = self.pending.len().saturating_sub(keep);
            if emit > 0 {
                split.content.push_str(&self.pending[..emit]);
                self.pending.drain(..emit);
            }
            break;
        }
        split
    }

    /// Releases whatever was held back.
    pub fn finish(&mut self) -> ThinkingSplit {
        let mut split = ThinkingSplit::default();
        if self.in_think {
            split.reasoning.push_str(&self.pending);
        } else {
            split.content.push_str(&self.pending);
        }
        self.pending.clear();
        self.in_think = false;
        self.close_tag = None;
        split
    }
}

/// Splits a complete text into visible content and tagged reasoning.
#[must_use]
pub fn split_thinking_tags(text: &str) -> ThinkingSplit {
    let mut parser = ThinkingTagParser::default();
    let mut split = parser.feed(text);
    let tail = parser.finish();
    split.content.push_str(&tail.content);
    split.reasoning.push_str(&tail.reasoning);
    split
}

/// The complete-message normalization: tagged reasoning, then the explicit
/// reasoning appended unless it repeats it, both trimmed.
#[must_use]
pub fn normalize_thinking_content(
    content: Option<&str>,
    explicit_reasoning: Option<&str>,
) -> ThinkingSplit {
    merge_explicit_reasoning(
        content.map(split_thinking_tags).unwrap_or_default(),
        explicit_reasoning,
    )
}

/// [`normalize_thinking_content`] for a reply that starts inside reasoning
/// ending at `close_tag`.
#[must_use]
pub fn normalize_thinking_content_starting_in_reasoning(
    content: Option<&str>,
    explicit_reasoning: Option<&str>,
    close_tag: &'static str,
) -> ThinkingSplit {
    let split = content.map(|text| {
        let mut parser = ThinkingTagParser::starting_in_reasoning(close_tag);
        let mut split = parser.feed(text);
        let tail = parser.finish();
        split.content.push_str(&tail.content);
        split.reasoning.push_str(&tail.reasoning);
        split
    });
    merge_explicit_reasoning(split.unwrap_or_default(), explicit_reasoning)
}

fn merge_explicit_reasoning(
    mut split: ThinkingSplit,
    explicit_reasoning: Option<&str>,
) -> ThinkingSplit {
    if let Some(reasoning) = explicit_reasoning
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if split.reasoning.trim().is_empty() {
            split.reasoning = reasoning.to_string();
        } else if split.reasoning.trim() != reasoning {
            split.reasoning.push_str("\n\n");
            split.reasoning.push_str(reasoning);
        }
    }
    split.content = split.content.trim().to_string();
    split.reasoning = split.reasoning.trim().to_string();
    split
}

fn partial_suffix_len(buffer: &str, tag: &str) -> usize {
    let lower = buffer.to_ascii_lowercase();
    let max = lower.len().min(tag.len().saturating_sub(1));
    lower
        .char_indices()
        .map(|(index, _)| &lower[index..])
        .filter(|suffix| suffix.len() <= max && tag.starts_with(*suffix))
        .map(str::len)
        .max()
        .unwrap_or(0)
}

fn earliest_open_tag(buffer: &str) -> Option<(usize, &'static str, &'static str)> {
    let lower = buffer.to_ascii_lowercase();
    TAG_PAIRS
        .iter()
        .filter_map(|(open, close)| lower.find(open).map(|index| (index, *open, *close)))
        .min_by_key(|(index, _, _)| *index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prefilled_reply_starts_inside_reasoning() {
        let mut parser = ThinkingTagParser::starting_in_reasoning("<channel|>");
        let first = parser.feed("weighing it<chan");
        assert_eq!(first.reasoning, "weighing it");
        let second = parser.feed("nel|>Answer");
        assert_eq!(second.content, "Answer");
        let whole = normalize_thinking_content_starting_in_reasoning(
            Some(" plan <channel|> reply "),
            Some("plan"),
            "<channel|>",
        );
        assert_eq!(whole.reasoning, "plan");
        assert_eq!(whole.content, "reply");
    }

    #[test]
    fn streamed_tags_split_across_chunks() {
        let mut parser = ThinkingTagParser::default();
        let first = parser.feed("Hi <thi");
        assert_eq!(first.content, "Hi ");
        let second = parser.feed("nk>plan</think> done");
        assert_eq!(second.reasoning, "plan");
        assert_eq!(second.content, " done");
        assert_eq!(parser.finish(), ThinkingSplit::default());
    }

    #[test]
    fn unfinished_reasoning_is_released_on_finish() {
        let mut parser = ThinkingTagParser::default();
        parser.feed("<reasoning>still going</reas");
        assert_eq!(parser.finish().reasoning, "</reas");
    }

    #[test]
    fn normalization_merges_explicit_reasoning_like_legacy() {
        let split = normalize_thinking_content(Some(" <think>a  </think> text "), Some(" b "));
        assert_eq!(split.content, "text");
        assert_eq!(split.reasoning, "a  \n\nb");
        let repeated = normalize_thinking_content(Some("<think> b </think>x"), Some("b"));
        assert_eq!(repeated.reasoning, "b");
        let only_explicit = normalize_thinking_content(None, Some("why"));
        assert_eq!(only_explicit.reasoning, "why");
    }
}
