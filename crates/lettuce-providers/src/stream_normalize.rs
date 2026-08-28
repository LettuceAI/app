use lettuce_conversations::{
    FinishReason, InferenceCandidate, InferenceOutcome, InferenceUsage, InferenceWarningCode,
    MessagePart,
};
use serde_json::Value;

use crate::stream_framing::StreamRecord;

const MAX_TEXT_BYTES: usize = 1024 * 1024;
const MAX_REASONING_BYTES: usize = 256 * 1024;
const MAX_ERROR_CODE_BYTES: usize = 128;
const MAX_ERROR_MESSAGE_BYTES: usize = 2 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StreamProtocol {
    OpenAi,
    Anthropic,
    Gemini,
    Ollama,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StreamDelta {
    Text(String),
    Reasoning(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum StreamNormalizeError {
    #[error("provider stream contains malformed JSON")]
    MalformedJson,
    #[error("provider stream contains data after its terminal event")]
    DataAfterTerminal,
    #[error("provider stream ended before its terminal event")]
    PrematureEof,
    #[error("provider stream exceeded its {field} limit")]
    OutputTooLarge { field: &'static str },
    #[error("provider returned an in-band error")]
    Provider {
        status: Option<u16>,
        code: Option<String>,
        message: Option<String>,
    },
    #[error("provider stream completed without content")]
    EmptyResponse,
}

#[derive(Debug)]
pub(crate) struct StreamNormalizer {
    protocol: StreamProtocol,
    thinking: ThinkingTagParser,
    text: String,
    reasoning: String,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    finish_reason: FinishReason,
    provider_finish_reason: Option<String>,
    warning_codes: Vec<InferenceWarningCode>,
    provider_request_id: Option<String>,
    terminal: bool,
}

impl StreamNormalizer {
    pub(crate) fn new(protocol: StreamProtocol, provider_request_id: Option<String>) -> Self {
        Self {
            protocol,
            thinking: ThinkingTagParser::default(),
            text: String::new(),
            reasoning: String::new(),
            input_tokens: None,
            output_tokens: None,
            finish_reason: FinishReason::Stop,
            provider_finish_reason: None,
            warning_codes: Vec::new(),
            provider_request_id,
            terminal: false,
        }
    }

    pub(crate) fn consume(
        &mut self,
        record: &StreamRecord,
    ) -> Result<Vec<StreamDelta>, StreamNormalizeError> {
        if self.terminal {
            return Err(StreamNormalizeError::DataAfterTerminal);
        }
        match self.protocol {
            StreamProtocol::OpenAi => self.consume_openai(record),
            StreamProtocol::Anthropic => self.consume_anthropic(record),
            StreamProtocol::Gemini => self.consume_gemini(record),
            StreamProtocol::Ollama => self.consume_ollama(record),
        }
    }

    pub(crate) fn finish(
        mut self,
    ) -> Result<(Vec<StreamDelta>, InferenceOutcome), StreamNormalizeError> {
        if !self.terminal {
            return Err(StreamNormalizeError::PrematureEof);
        }
        let mut tail = Vec::new();
        let split = self.thinking.finish();
        self.append_split(split, &mut tail)?;
        if self.text.trim().is_empty()
            && self.reasoning.trim().is_empty()
            && !self
                .warning_codes
                .contains(&InferenceWarningCode::SafetyTransformed)
            && !self
                .warning_codes
                .contains(&InferenceWarningCode::ProviderDegraded)
        {
            return Err(StreamNormalizeError::EmptyResponse);
        }

        let mut parts = Vec::with_capacity(2);
        if !self.reasoning.is_empty() {
            parts.push(MessagePart::ReasoningSummary {
                text: self.reasoning,
            });
        }
        if !self.text.is_empty() {
            parts.push(MessagePart::Text { text: self.text });
        }
        let usage = match (self.input_tokens, self.output_tokens) {
            (Some(input_tokens), Some(output_tokens)) => Some(InferenceUsage {
                input_tokens,
                output_tokens,
            }),
            _ => None,
        };
        Ok((
            tail,
            InferenceOutcome {
                candidates: vec![InferenceCandidate {
                    ordinal: 0,
                    parts,
                    provider_replay: None,
                }],
                usage,
                finish_reason: self.finish_reason,
                provider_finish_reason: self.provider_finish_reason,
                provider_request_id: self.provider_request_id,
                warning_codes: self.warning_codes,
            },
        ))
    }

    fn consume_openai(
        &mut self,
        record: &StreamRecord,
    ) -> Result<Vec<StreamDelta>, StreamNormalizeError> {
        if record.data.trim() == "[DONE]" {
            if self.provider_finish_reason.is_none() {
                self.push_warning(InferenceWarningCode::ProviderDegraded);
            }
            self.terminal = true;
            return Ok(Vec::new());
        }
        let value = parse_json(&record.data)?;
        if let Some(error) = provider_error(&value) {
            return Err(error);
        }
        if let Some(usage) = value.get("usage") {
            self.input_tokens = token(usage, &["prompt_tokens", "input_tokens"]);
            self.output_tokens = token(usage, &["completion_tokens", "output_tokens"]);
        }
        let Some(choice) = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
        else {
            return Ok(Vec::new());
        };
        let delta = choice.get("delta").or_else(|| choice.get("message"));
        let mut deltas = Vec::new();
        if let Some(text) = delta
            .and_then(|delta| delta.get("content"))
            .and_then(Value::as_str)
        {
            let split = self.thinking.feed(text);
            self.append_split(split, &mut deltas)?;
        }
        if let Some(reasoning) = delta
            .and_then(|delta| {
                delta
                    .get("reasoning")
                    .or_else(|| delta.get("reasoning_content"))
            })
            .and_then(Value::as_str)
        {
            self.append_reasoning(reasoning, &mut deltas)?;
        }
        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            self.set_finish_reason(reason, FinishFamily::OpenAi);
        }
        Ok(deltas)
    }

    fn consume_anthropic(
        &mut self,
        record: &StreamRecord,
    ) -> Result<Vec<StreamDelta>, StreamNormalizeError> {
        let value = parse_json(&record.data)?;
        let json_kind = value.get("type").and_then(Value::as_str);
        if let (Some(event_kind), Some(json_kind)) = (record.event.as_deref(), json_kind)
            && event_kind != json_kind
        {
            return Err(StreamNormalizeError::MalformedJson);
        }
        let kind = record.event.as_deref().or(json_kind);
        if kind == Some("error") {
            return Err(
                provider_error(&value).unwrap_or_else(|| StreamNormalizeError::Provider {
                    status: None,
                    code: Some("anthropic_stream_error".to_owned()),
                    message: None,
                }),
            );
        }
        let mut deltas = Vec::new();
        match kind {
            Some("message_start") => {
                if let Some(usage) = value
                    .get("message")
                    .and_then(|message| message.get("usage"))
                {
                    self.input_tokens = token(usage, &["input_tokens"]);
                    self.output_tokens = token(usage, &["output_tokens"]);
                }
            }
            Some("content_block_delta") => {
                let delta = value.get("delta");
                match delta
                    .and_then(|delta| delta.get("type"))
                    .and_then(Value::as_str)
                {
                    Some("text_delta") => {
                        if let Some(text) = delta
                            .and_then(|delta| delta.get("text"))
                            .and_then(Value::as_str)
                        {
                            let split = self.thinking.feed(text);
                            self.append_split(split, &mut deltas)?;
                        }
                    }
                    Some("thinking_delta") => {
                        if let Some(reasoning) = delta
                            .and_then(|delta| delta.get("thinking"))
                            .and_then(Value::as_str)
                        {
                            self.append_reasoning(reasoning, &mut deltas)?;
                        }
                    }
                    Some("input_json_delta") | Some("signature_delta") | None => {}
                    Some(_) => self.push_warning(InferenceWarningCode::ProviderDegraded),
                }
            }
            Some("message_delta") => {
                if let Some(reason) = value
                    .get("delta")
                    .and_then(|delta| delta.get("stop_reason"))
                    .and_then(Value::as_str)
                {
                    self.set_finish_reason(reason, FinishFamily::Anthropic);
                }
                if let Some(usage) = value.get("usage") {
                    self.output_tokens = token(usage, &["output_tokens"]);
                }
            }
            Some("message_stop") => self.terminal = true,
            Some("content_block_start") | Some("content_block_stop") | Some("ping") | None => {}
            Some(_) => {}
        }
        Ok(deltas)
    }

    fn consume_gemini(
        &mut self,
        record: &StreamRecord,
    ) -> Result<Vec<StreamDelta>, StreamNormalizeError> {
        let value = parse_json(&record.data)?;
        if let Some(error) = provider_error(&value) {
            return Err(error);
        }
        if value
            .get("promptFeedback")
            .and_then(|feedback| feedback.get("blockReason"))
            .is_some()
        {
            self.push_warning(InferenceWarningCode::SafetyTransformed);
            self.provider_finish_reason = value
                .get("promptFeedback")
                .and_then(|feedback| feedback.get("blockReason"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            self.terminal = true;
        }
        if let Some(usage) = value.get("usageMetadata") {
            self.input_tokens = token(usage, &["promptTokenCount"]);
            self.output_tokens = token(usage, &["candidatesTokenCount"]);
        }
        let mut deltas = Vec::new();
        if let Some(candidate) = value
            .get("candidates")
            .and_then(Value::as_array)
            .and_then(|candidates| candidates.first())
        {
            if let Some(parts) = candidate
                .get("content")
                .and_then(|content| content.get("parts"))
                .and_then(Value::as_array)
            {
                for part in parts {
                    let Some(text) = part.get("text").and_then(Value::as_str) else {
                        continue;
                    };
                    if part.get("thought").and_then(Value::as_bool) == Some(true) {
                        self.append_reasoning(text, &mut deltas)?;
                    } else {
                        let split = self.thinking.feed(text);
                        self.append_split(split, &mut deltas)?;
                    }
                }
            }
            if let Some(reason) = candidate.get("finishReason").and_then(Value::as_str) {
                self.set_finish_reason(reason, FinishFamily::Gemini);
                self.terminal = true;
            }
        }
        Ok(deltas)
    }

    fn consume_ollama(
        &mut self,
        record: &StreamRecord,
    ) -> Result<Vec<StreamDelta>, StreamNormalizeError> {
        let value = parse_json(&record.data)?;
        if let Some(error) = provider_error(&value) {
            return Err(error);
        }
        let mut deltas = Vec::new();
        if let Some(message) = value.get("message") {
            if let Some(text) = message.get("content").and_then(Value::as_str) {
                let split = self.thinking.feed(text);
                self.append_split(split, &mut deltas)?;
            }
            if let Some(reasoning) = ["thinking", "reasoning", "reasoning_content"]
                .into_iter()
                .find_map(|field| message.get(field).and_then(Value::as_str))
            {
                self.append_reasoning(reasoning, &mut deltas)?;
            }
        }
        if value.get("done").and_then(Value::as_bool) == Some(true) {
            self.input_tokens = token(&value, &["prompt_eval_count"]);
            self.output_tokens = token(&value, &["eval_count"]);
            if let Some(reason) = value.get("done_reason").and_then(Value::as_str) {
                self.set_finish_reason(reason, FinishFamily::Ollama);
            }
            self.terminal = true;
        }
        Ok(deltas)
    }

    fn append_split(
        &mut self,
        split: ThinkingSplit,
        deltas: &mut Vec<StreamDelta>,
    ) -> Result<(), StreamNormalizeError> {
        self.append_text(&split.content, deltas)?;
        self.append_reasoning(&split.reasoning, deltas)
    }

    fn append_text(
        &mut self,
        value: &str,
        deltas: &mut Vec<StreamDelta>,
    ) -> Result<(), StreamNormalizeError> {
        append_bounded(&mut self.text, value, MAX_TEXT_BYTES, "text")?;
        if !value.is_empty() {
            deltas.push(StreamDelta::Text(value.to_owned()));
        }
        Ok(())
    }

    fn append_reasoning(
        &mut self,
        value: &str,
        deltas: &mut Vec<StreamDelta>,
    ) -> Result<(), StreamNormalizeError> {
        append_bounded(&mut self.reasoning, value, MAX_REASONING_BYTES, "reasoning")?;
        if !value.is_empty() {
            deltas.push(StreamDelta::Reasoning(value.to_owned()));
        }
        Ok(())
    }

    fn set_finish_reason(&mut self, reason: &str, family: FinishFamily) {
        self.provider_finish_reason = Some(reason.to_owned());
        match family {
            FinishFamily::OpenAi => match reason {
                "stop" => {}
                "length" => self.mark_length(),
                "content_filter" => self.push_warning(InferenceWarningCode::SafetyTransformed),
                "tool_calls" | "function_call" => {
                    self.push_warning(InferenceWarningCode::ProviderDegraded);
                }
                _ => self.push_warning(InferenceWarningCode::ProviderDegraded),
            },
            FinishFamily::Anthropic => match reason {
                "end_turn" | "stop_sequence" => {}
                "max_tokens" | "model_context_window_exceeded" => self.mark_length(),
                "refusal" => self.push_warning(InferenceWarningCode::SafetyTransformed),
                "tool_use" | "pause_turn" => {
                    self.push_warning(InferenceWarningCode::ProviderDegraded);
                }
                _ => self.push_warning(InferenceWarningCode::ProviderDegraded),
            },
            FinishFamily::Gemini => match reason {
                "STOP" | "FINISH_REASON_UNSPECIFIED" => {}
                "MAX_TOKENS" => self.mark_length(),
                "SAFETY"
                | "RECITATION"
                | "LANGUAGE"
                | "BLOCKLIST"
                | "PROHIBITED_CONTENT"
                | "SPII"
                | "IMAGE_SAFETY"
                | "IMAGE_PROHIBITED_CONTENT"
                | "IMAGE_RECITATION"
                | "IMAGE_OTHER"
                | "OTHER" => {
                    self.push_warning(InferenceWarningCode::SafetyTransformed);
                }
                _ => self.push_warning(InferenceWarningCode::ProviderDegraded),
            },
            FinishFamily::Ollama => match reason {
                "stop" => {}
                "length" => self.mark_length(),
                _ => self.push_warning(InferenceWarningCode::ProviderDegraded),
            },
        }
    }

    fn mark_length(&mut self) {
        self.finish_reason = FinishReason::Length;
        self.push_warning(InferenceWarningCode::Truncated);
    }

    fn push_warning(&mut self, warning: InferenceWarningCode) {
        if !self.warning_codes.contains(&warning) {
            self.warning_codes.push(warning);
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum FinishFamily {
    OpenAi,
    Anthropic,
    Gemini,
    Ollama,
}

fn parse_json(data: &str) -> Result<Value, StreamNormalizeError> {
    serde_json::from_str(data).map_err(|_| StreamNormalizeError::MalformedJson)
}

fn token(value: &Value, fields: &[&str]) -> Option<u64> {
    fields
        .iter()
        .find_map(|field| value.get(*field).and_then(Value::as_u64))
}

fn provider_error(value: &Value) -> Option<StreamNormalizeError> {
    let error = value.get("error")?;
    let status = error
        .get("code")
        .and_then(Value::as_u64)
        .and_then(|status| u16::try_from(status).ok());
    let code = error
        .get("type")
        .or_else(|| error.get("status"))
        .or_else(|| error.get("code"))
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .or_else(|| value.as_u64().map(|v| v.to_string()))
        })
        .and_then(|value| bounded(&value, MAX_ERROR_CODE_BYTES));
    let message = error
        .as_str()
        .or_else(|| error.get("message").and_then(Value::as_str))
        .and_then(|value| bounded(value, MAX_ERROR_MESSAGE_BYTES));
    Some(StreamNormalizeError::Provider {
        status,
        code,
        message,
    })
}

fn bounded(value: &str, max_bytes: usize) -> Option<String> {
    let clean: String = value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .collect();
    let clean = clean.trim();
    if clean.is_empty() {
        return None;
    }
    let end = clean
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= max_bytes)
        .last()
        .unwrap_or(0);
    Some(
        clean[..clean.len().min(if clean.len() <= max_bytes {
            clean.len()
        } else {
            end
        })]
            .to_owned(),
    )
}

fn append_bounded(
    target: &mut String,
    value: &str,
    max_bytes: usize,
    field: &'static str,
) -> Result<(), StreamNormalizeError> {
    if target
        .len()
        .checked_add(value.len())
        .is_none_or(|length| length > max_bytes)
    {
        return Err(StreamNormalizeError::OutputTooLarge { field });
    }
    target.push_str(value);
    Ok(())
}

#[derive(Debug, Default)]
struct ThinkingTagParser {
    in_think: bool,
    close_tag: Option<&'static str>,
    pending: String,
}

#[derive(Debug, Default)]
struct ThinkingSplit {
    content: String,
    reasoning: String,
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
    fn feed(&mut self, chunk: &str) -> ThinkingSplit {
        self.pending.push_str(chunk);
        let mut split = ThinkingSplit::default();
        loop {
            if self.in_think {
                let close = self.close_tag.expect("thinking close tag must be present");
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

    fn finish(&mut self) -> ThinkingSplit {
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

pub(crate) fn split_complete_thinking(text: &str) -> (String, String) {
    let mut parser = ThinkingTagParser::default();
    let mut split = parser.feed(text);
    let tail = parser.finish();
    split.content.push_str(&tail.content);
    split.reasoning.push_str(&tail.reasoning);
    (split.content, split.reasoning)
}

pub(crate) fn merge_complete_reasoning<'a>(
    tagged: String,
    explicit: impl IntoIterator<Item = &'a str>,
) -> String {
    let mut merged = tagged.trim().to_owned();
    for value in explicit {
        let value = value.trim();
        if value.is_empty() || merged == value {
            continue;
        }
        if !merged.is_empty() {
            merged.push_str("\n\n");
        }
        merged.push_str(value);
    }
    merged
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
#[expect(
    clippy::unwrap_used,
    reason = "test fixtures assert exact normalization outcomes"
)]
mod tests {
    use lettuce_conversations::{FinishReason, InferenceWarningCode, MessagePart};

    use super::{StreamDelta, StreamNormalizeError, StreamNormalizer, StreamProtocol};
    use crate::stream_framing::StreamRecord;

    fn record(data: &str) -> StreamRecord {
        StreamRecord {
            event: None,
            data: data.to_owned(),
        }
    }

    fn event(event: &str, data: &str) -> StreamRecord {
        StreamRecord {
            event: Some(event.to_owned()),
            data: data.to_owned(),
        }
    }

    #[test]
    fn openai_preserves_split_thinking_and_usage_only_frames() {
        let mut normalizer = StreamNormalizer::new(StreamProtocol::OpenAi, Some("req".to_owned()));
        assert!(
            normalizer
                .consume(&record(
                    r#"{"choices":[],"usage":{"prompt_tokens":3,"completion_tokens":2}}"#
                ))
                .unwrap()
                .is_empty()
        );
        assert!(
            normalizer
                .consume(&record(r#"{"choices":[{"delta":{"content":"Hi<th"}}]}"#))
                .unwrap()
                .contains(&StreamDelta::Text("Hi".to_owned()))
        );
        let deltas = normalizer.consume(&record(r#"{"choices":[{"delta":{"content":"ink>secret</think>!","reasoning_content":"native"},"finish_reason":"length"}]}"#)).unwrap();
        assert!(deltas.contains(&StreamDelta::Reasoning("secret".to_owned())));
        assert!(deltas.contains(&StreamDelta::Reasoning("native".to_owned())));
        normalizer.consume(&record("[DONE]")).unwrap();
        let (_, outcome) = normalizer.finish().unwrap();
        assert_eq!(outcome.finish_reason, FinishReason::Length);
        assert_eq!(outcome.usage.unwrap().input_tokens, 3);
        assert!(
            outcome
                .warning_codes
                .contains(&InferenceWarningCode::Truncated)
        );
        assert_eq!(outcome.candidates[0].parts.len(), 2);
    }

    #[test]
    fn openai_requires_done_marker() {
        let mut normalizer = StreamNormalizer::new(StreamProtocol::OpenAi, None);
        normalizer
            .consume(&record(
                r#"{"choices":[{"delta":{"content":"hello"},"finish_reason":"stop"}]}"#,
            ))
            .unwrap();
        assert_eq!(
            normalizer.finish().unwrap_err(),
            StreamNormalizeError::PrematureEof
        );
    }

    #[test]
    fn anthropic_uses_event_types_and_cumulative_usage() {
        let mut normalizer = StreamNormalizer::new(StreamProtocol::Anthropic, None);
        normalizer.consume(&event("message_start", r#"{"type":"message_start","message":{"usage":{"input_tokens":7,"output_tokens":0}}}"#)).unwrap();
        assert_eq!(normalizer.consume(&event("content_block_delta", r#"{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"why"}}"#)).unwrap(), vec![StreamDelta::Reasoning("why".to_owned())]);
        normalizer
            .consume(&event(
                "content_block_delta",
                r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"answer"}}"#,
            ))
            .unwrap();
        normalizer.consume(&event("message_delta", r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":4}}"#)).unwrap();
        normalizer
            .consume(&event("message_stop", r#"{"type":"message_stop"}"#))
            .unwrap();
        let (_, outcome) = normalizer.finish().unwrap();
        assert_eq!(outcome.usage.unwrap().output_tokens, 4);
        assert!(matches!(
            outcome.candidates[0].parts[0],
            MessagePart::ReasoningSummary { .. }
        ));
    }

    #[test]
    fn gemini_uses_only_primary_candidate_and_native_thought_parts() {
        let mut normalizer = StreamNormalizer::new(StreamProtocol::Gemini, None);
        let deltas = normalizer.consume(&record(r#"{"candidates":[{"content":{"parts":[{"text":"why","thought":true},{"text":"answer"}]},"finishReason":"STOP"},{"content":{"parts":[{"text":"ignored"}]}}],"usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":1}}"#)).unwrap();
        assert!(deltas.contains(&StreamDelta::Reasoning("why".to_owned())));
        assert!(!deltas.contains(&StreamDelta::Text("ignored".to_owned())));
        let (_, outcome) = normalizer.finish().unwrap();
        assert_eq!(outcome.usage.unwrap().output_tokens, 1);
    }

    #[test]
    fn gemini_prompt_block_is_a_terminal_safety_outcome() {
        let mut normalizer = StreamNormalizer::new(StreamProtocol::Gemini, None);
        normalizer
            .consume(&record(r#"{"promptFeedback":{"blockReason":"SAFETY"}}"#))
            .unwrap();
        let (_, outcome) = normalizer.finish().expect("safety outcome");
        assert!(
            outcome
                .warning_codes
                .contains(&InferenceWarningCode::SafetyTransformed)
        );
    }

    #[test]
    fn anthropic_rejects_mismatched_sse_and_json_event_types() {
        let mut normalizer = StreamNormalizer::new(StreamProtocol::Anthropic, None);
        assert_eq!(
            normalizer
                .consume(&event(
                    "message_stop",
                    r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"wrong"}}"#,
                ))
                .expect_err("mismatch must fail"),
            StreamNormalizeError::MalformedJson
        );
    }

    #[test]
    fn ollama_requires_done_and_preserves_whitespace() {
        let mut normalizer = StreamNormalizer::new(StreamProtocol::Ollama, None);
        normalizer
            .consume(&record(
                r#"{"message":{"content":"hello","thinking":"step"},"done":false}"#,
            ))
            .unwrap();
        normalizer.consume(&record(r#"{"message":{"content":" world","thinking":" two"},"done":true,"done_reason":"stop","prompt_eval_count":8,"eval_count":3}"#)).unwrap();
        let (_, outcome) = normalizer.finish().unwrap();
        assert!(
            matches!(&outcome.candidates[0].parts[1], MessagePart::Text { text } if text == "hello world")
        );
    }

    #[test]
    fn malformed_json_and_data_after_terminal_are_errors() {
        let mut malformed = StreamNormalizer::new(StreamProtocol::OpenAi, None);
        assert_eq!(
            malformed.consume(&record("{")).unwrap_err(),
            StreamNormalizeError::MalformedJson
        );

        let mut done = StreamNormalizer::new(StreamProtocol::OpenAi, None);
        done.consume(&record("[DONE]")).unwrap();
        assert_eq!(
            done.consume(&record("[DONE]")).unwrap_err(),
            StreamNormalizeError::DataAfterTerminal
        );
    }

    #[test]
    fn in_band_errors_are_bounded_and_terminal() {
        let mut normalizer = StreamNormalizer::new(StreamProtocol::Gemini, None);
        let error = normalizer
            .consume(&record(&format!(
                r#"{{"error":{{"code":400,"status":"BAD_REQUEST","message":"{}"}}}}"#,
                "x".repeat(3000)
            )))
            .unwrap_err();
        let StreamNormalizeError::Provider {
            status,
            code,
            message,
        } = error
        else {
            panic!("provider error expected")
        };
        assert_eq!(status, Some(400));
        assert_eq!(code.as_deref(), Some("BAD_REQUEST"));
        assert!(message.unwrap().len() <= 2048);
    }
}
