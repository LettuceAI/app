#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ThinkingSplit {
    pub content: String,
    pub reasoning: String,
}

impl ThinkingSplit {
    pub fn merge_reasoning(mut self, explicit_reasoning: Option<&str>) -> Self {
        if let Some(reasoning) = explicit_reasoning
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if self.reasoning.trim().is_empty() {
                self.reasoning = reasoning.to_string();
            } else if self.reasoning.trim() != reasoning {
                self.reasoning.push_str("\n\n");
                self.reasoning.push_str(reasoning);
            }
        }
        self
    }
}

#[derive(Debug, Default, Clone)]
pub struct ThinkingTagStreamParser {
    in_think: bool,
    close_tag: Option<&'static str>,
    pending: String,
}

const TAG_PAIRS: [(&str, &str); 6] = [
    ("<think>", "</think>"),
    ("<thinking>", "</thinking>"),
    ("<reason>", "</reason>"),
    ("<reasoning>", "</reasoning>"),
    ("<|channel>thought", "<channel|>"),
    ("<|channel>", "<channel|>"),
];

fn partial_suffix_len(buffer: &str, tag: &str) -> usize {
    let buffer_lower = buffer.to_ascii_lowercase();
    let max_len = buffer_lower.len().min(tag.len().saturating_sub(1));
    let mut best = 0;

    for (start, _) in buffer_lower.char_indices() {
        let suffix = &buffer_lower[start..];
        let suffix_len = suffix.len();
        if suffix_len <= max_len && tag.starts_with(suffix) {
            best = best.max(suffix_len);
        }
    }

    best
}

fn partial_suffix_len_any(buffer: &str, tags: &[&str]) -> usize {
    tags.iter()
        .map(|tag| partial_suffix_len(buffer, tag))
        .max()
        .unwrap_or(0)
}

fn earliest_open_tag(buffer: &str) -> Option<(usize, &'static str, &'static str)> {
    let buffer_lower = buffer.to_ascii_lowercase();
    TAG_PAIRS
        .iter()
        .filter_map(|(open_tag, close_tag)| {
            buffer_lower
                .find(open_tag)
                .map(|index| (index, *open_tag, *close_tag))
        })
        .min_by_key(|(index, _, _)| *index)
}

impl ThinkingTagStreamParser {
    pub fn feed(&mut self, chunk: &str) -> ThinkingSplit {
        self.pending.push_str(chunk);
        let mut split = ThinkingSplit::default();

        loop {
            if self.in_think {
                let close_tag = self.close_tag.expect("close tag must exist when in_think");
                let pending_lower = self.pending.to_ascii_lowercase();
                if let Some(index) = pending_lower.find(close_tag) {
                    split.reasoning.push_str(&self.pending[..index]);
                    self.pending.drain(..index + close_tag.len());
                    self.in_think = false;
                    self.close_tag = None;
                    continue;
                }

                let keep = partial_suffix_len(&self.pending, close_tag);
                let emit_len = self.pending.len().saturating_sub(keep);
                if emit_len == 0 {
                    break;
                }
                split.reasoning.push_str(&self.pending[..emit_len]);
                self.pending.drain(..emit_len);
                break;
            }

            if let Some((index, open_tag, close_tag)) = earliest_open_tag(&self.pending) {
                split.content.push_str(&self.pending[..index]);
                self.pending.drain(..index + open_tag.len());
                self.in_think = true;
                self.close_tag = Some(close_tag);
                continue;
            }

            let open_tags = TAG_PAIRS.map(|(open_tag, _)| open_tag);
            let keep = partial_suffix_len_any(&self.pending, &open_tags);
            let emit_len = self.pending.len().saturating_sub(keep);
            if emit_len == 0 {
                break;
            }
            split.content.push_str(&self.pending[..emit_len]);
            self.pending.drain(..emit_len);
            break;
        }

        split
    }

    pub fn finish(&mut self) -> ThinkingSplit {
        let mut split = ThinkingSplit::default();
        if self.in_think {
            split.reasoning.push_str(&self.pending);
        } else {
            split.content.push_str(&self.pending);
        }
        self.close_tag = None;
        self.pending.clear();
        split
    }
}

pub fn split_thinking_tags(text: &str) -> ThinkingSplit {
    let mut parser = ThinkingTagStreamParser::default();
    let mut split = parser.feed(text);
    let tail = parser.finish();
    split.content.push_str(&tail.content);
    split.reasoning.push_str(&tail.reasoning);
    split
}

pub fn normalize_thinking_content(
    content: Option<&str>,
    explicit_reasoning: Option<&str>,
) -> ThinkingSplit {
    let mut split = content
        .map(split_thinking_tags)
        .unwrap_or_default()
        .merge_reasoning(explicit_reasoning);

    split.content = split.content.trim().to_string();
    split.reasoning = split.reasoning.trim().to_string();
    split
}
