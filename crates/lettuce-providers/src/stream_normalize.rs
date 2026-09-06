use std::collections::{BTreeMap, BTreeSet};

use lettuce_conversations::{
    FinishReason, InferenceCandidate, InferenceOutcome, InferenceUsage, InferenceWarningCode,
    MAX_TOOL_ARGUMENT_BYTES, MAX_TOOL_CALLS_PER_RESPONSE, MessagePart, ProposedToolCall,
};
use serde_json::Value;

use crate::stream_framing::StreamRecord;

const MAX_TEXT_BYTES: usize = 1024 * 1024;
const MAX_REASONING_BYTES: usize = 256 * 1024;
const MAX_ERROR_CODE_BYTES: usize = 128;
const MAX_ERROR_MESSAGE_BYTES: usize = 2 * 1024;
const MAX_PROVIDER_REPLAY_BYTES: usize = 4 * 1024 * 1024;

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

pub(crate) struct StreamCompletion {
    pub tail: Vec<StreamDelta>,
    pub outcome: InferenceOutcome,
    pub provider_replay: Option<Vec<u8>>,
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
    cached_input_tokens: Option<u64>,
    reasoning_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
    web_search_requests: Option<u64>,
    finish_reason: FinishReason,
    provider_finish_reason: Option<String>,
    warning_codes: Vec<InferenceWarningCode>,
    provider_request_id: Option<String>,
    openai_tool_calls: BTreeMap<u64, PendingOpenAiToolCall>,
    anthropic_tool_calls: BTreeMap<u64, PendingAnthropicToolCall>,
    anthropic_replay_blocks: BTreeMap<u64, AnthropicReplayBlock>,
    anthropic_server_tool_blocks: BTreeSet<u64>,
    anthropic_unreplayable_block: bool,
    gemini_tool_calls: Vec<ProposedToolCall>,
    gemini_replay_parts: Vec<Value>,
    gemini_replay_bytes: usize,
    ollama_tool_calls: Vec<ProposedToolCall>,
    gemini_has_thought_signature: bool,
    requires_openai_tool_calls: bool,
    requires_anthropic_tool_calls: bool,
    terminal: bool,
}

#[derive(Debug, Default)]
struct PendingOpenAiToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

#[derive(Debug)]
struct PendingAnthropicToolCall {
    id: String,
    name: String,
    initial_input: Value,
    partial_json: String,
    closed: bool,
}

#[derive(Debug)]
enum AnthropicReplayBlock {
    Thinking {
        thinking: String,
        signature: String,
        closed: bool,
    },
    RedactedThinking {
        data: String,
        closed: bool,
    },
    Text {
        text: String,
        closed: bool,
    },
    ToolUse,
}

impl StreamNormalizer {
    pub(crate) fn new(protocol: StreamProtocol, provider_request_id: Option<String>) -> Self {
        Self {
            protocol,
            thinking: ThinkingTagParser::default(),
            text: String::new(),
            reasoning: String::new(),
            input_tokens: None,
            cached_input_tokens: None,
            reasoning_tokens: None,
            cache_write_tokens: None,
            web_search_requests: None,
            output_tokens: None,
            finish_reason: FinishReason::Stop,
            provider_finish_reason: None,
            warning_codes: Vec::new(),
            provider_request_id,
            openai_tool_calls: BTreeMap::new(),
            anthropic_tool_calls: BTreeMap::new(),
            anthropic_replay_blocks: BTreeMap::new(),
            anthropic_server_tool_blocks: BTreeSet::new(),
            anthropic_unreplayable_block: false,
            gemini_tool_calls: Vec::new(),
            gemini_replay_parts: Vec::new(),
            gemini_replay_bytes: 0,
            ollama_tool_calls: Vec::new(),
            gemini_has_thought_signature: false,
            requires_openai_tool_calls: false,
            requires_anthropic_tool_calls: false,
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

    #[cfg(test)]
    pub(crate) fn finish(
        self,
    ) -> Result<(Vec<StreamDelta>, InferenceOutcome), StreamNormalizeError> {
        let completion = self.finish_with_provider_replay()?;
        Ok((completion.tail, completion.outcome))
    }

    pub(crate) fn finish_with_provider_replay(
        mut self,
    ) -> Result<StreamCompletion, StreamNormalizeError> {
        if !self.terminal {
            return Err(StreamNormalizeError::PrematureEof);
        }
        let mut tail = Vec::new();
        let split = self.thinking.finish();
        self.append_split(split, &mut tail)?;
        if !self.anthropic_server_tool_blocks.is_empty() {
            return Err(StreamNormalizeError::MalformedJson);
        }
        let provider_replay = if self.protocol == StreamProtocol::Anthropic
            && !self.anthropic_tool_calls.is_empty()
        {
            self.build_anthropic_replay()?
        } else if self.protocol == StreamProtocol::Gemini
            && self.gemini_has_thought_signature
            && !self.gemini_tool_calls.is_empty()
        {
            Some(
                serde_json::to_vec(&self.gemini_replay_parts)
                    .map_err(|_| StreamNormalizeError::MalformedJson)?,
            )
        } else {
            None
        };
        let tool_calls = match self.protocol {
            StreamProtocol::OpenAi => {
                finish_openai_tool_calls(std::mem::take(&mut self.openai_tool_calls))?
            }
            StreamProtocol::Anthropic => {
                finish_anthropic_tool_calls(std::mem::take(&mut self.anthropic_tool_calls))?
            }
            StreamProtocol::Gemini => {
                if self.gemini_tool_calls.len() > 1
                    && self
                        .gemini_tool_calls
                        .iter()
                        .any(|call| call.provider_call_id.is_none())
                {
                    return Err(StreamNormalizeError::MalformedJson);
                }
                std::mem::take(&mut self.gemini_tool_calls)
            }
            StreamProtocol::Ollama => std::mem::take(&mut self.ollama_tool_calls),
        };
        if self.requires_openai_tool_calls && tool_calls.is_empty() {
            return Err(StreamNormalizeError::MalformedJson);
        }
        if self.requires_anthropic_tool_calls && tool_calls.is_empty() {
            return Err(StreamNormalizeError::MalformedJson);
        }
        if self.protocol == StreamProtocol::Anthropic
            && !tool_calls.is_empty()
            && self.provider_finish_reason.as_deref() != Some("tool_use")
        {
            return Err(StreamNormalizeError::MalformedJson);
        }
        if self.text.trim().is_empty()
            && self.reasoning.trim().is_empty()
            && tool_calls.is_empty()
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
                cache_write_tokens: self.cache_write_tokens,
                web_search_requests: self.web_search_requests,
                cached_input_tokens: self.cached_input_tokens,
                reasoning_tokens: self.reasoning_tokens,
                input_tokens,
                output_tokens,
            }),
            _ => None,
        };
        let outcome = InferenceOutcome {
            candidates: vec![InferenceCandidate {
                ordinal: 0,
                parts,
                tool_calls,
                provider_replay: None,
            }],
            usage,
            finish_reason: self.finish_reason,
            provider_finish_reason: self.provider_finish_reason,
            provider_request_id: self.provider_request_id,
            warning_codes: self.warning_codes,
        };
        outcome
            .validate()
            .map_err(|_| StreamNormalizeError::MalformedJson)?;
        Ok(StreamCompletion {
            tail,
            outcome,
            provider_replay,
        })
    }

    fn build_anthropic_replay(&self) -> Result<Option<Vec<u8>>, StreamNormalizeError> {
        let mut has_signed_thinking = false;
        let mut blocks = Vec::with_capacity(self.anthropic_replay_blocks.len());
        for (index, block) in &self.anthropic_replay_blocks {
            let value = match block {
                AnthropicReplayBlock::Thinking {
                    thinking,
                    signature,
                    closed,
                } => {
                    if !closed || thinking.is_empty() || signature.is_empty() {
                        return Err(StreamNormalizeError::MalformedJson);
                    }
                    has_signed_thinking = true;
                    serde_json::json!({
                        "type": "thinking",
                        "thinking": thinking,
                        "signature": signature,
                    })
                }
                AnthropicReplayBlock::RedactedThinking { data, closed } => {
                    if !closed || data.is_empty() {
                        return Err(StreamNormalizeError::MalformedJson);
                    }
                    has_signed_thinking = true;
                    serde_json::json!({"type": "redacted_thinking", "data": data})
                }
                AnthropicReplayBlock::Text { text, closed } => {
                    if !closed {
                        return Err(StreamNormalizeError::MalformedJson);
                    }
                    serde_json::json!({"type": "text", "text": text})
                }
                AnthropicReplayBlock::ToolUse => {
                    let pending = self
                        .anthropic_tool_calls
                        .get(index)
                        .ok_or(StreamNormalizeError::MalformedJson)?;
                    if !pending.closed {
                        return Err(StreamNormalizeError::MalformedJson);
                    }
                    let input = if pending.partial_json.is_empty() {
                        pending.initial_input.clone()
                    } else {
                        serde_json::from_str::<Value>(&pending.partial_json)
                            .map_err(|_| StreamNormalizeError::MalformedJson)?
                    };
                    if !input.is_object() {
                        return Err(StreamNormalizeError::MalformedJson);
                    }
                    serde_json::json!({
                        "type": "tool_use",
                        "id": pending.id,
                        "name": pending.name,
                        "input": input,
                    })
                }
            };
            blocks.push(value);
        }
        if !has_signed_thinking {
            return Ok(None);
        }
        if self.anthropic_unreplayable_block {
            return Err(StreamNormalizeError::MalformedJson);
        }
        serde_json::to_vec(&blocks)
            .map(Some)
            .map_err(|_| StreamNormalizeError::MalformedJson)
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
            (self.cache_write_tokens, self.web_search_requests) = usage
                .as_object()
                .map(crate::common::openai_usage_extras)
                .unwrap_or_default();
            self.input_tokens = token(usage, &["prompt_tokens", "input_tokens"]);
            (self.cached_input_tokens, self.reasoning_tokens) = usage
                .as_object()
                .map(crate::common::openai_usage_details)
                .unwrap_or_default();
            self.output_tokens = token(usage, &["completion_tokens", "output_tokens"]);
        }
        let Some(choices) = value.get("choices").and_then(Value::as_array) else {
            return Ok(Vec::new());
        };
        if choices.len() > 1 {
            return Err(StreamNormalizeError::MalformedJson);
        }
        let Some(choice) = choices.first() else {
            return Ok(Vec::new());
        };
        if choice
            .get("index")
            .and_then(Value::as_u64)
            .is_some_and(|index| index != 0)
        {
            return Err(StreamNormalizeError::MalformedJson);
        }
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
        if let Some(calls) = delta
            .and_then(|delta| delta.get("tool_calls"))
            .and_then(Value::as_array)
        {
            for call in calls {
                self.append_openai_tool_call(call)?;
            }
        }
        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            self.set_finish_reason(reason, FinishFamily::OpenAi);
        }
        Ok(deltas)
    }

    fn append_openai_tool_call(&mut self, value: &Value) -> Result<(), StreamNormalizeError> {
        if value
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind != "function")
        {
            return Err(StreamNormalizeError::MalformedJson);
        }
        let index = value
            .get("index")
            .and_then(Value::as_u64)
            .ok_or(StreamNormalizeError::MalformedJson)?;
        if !self.openai_tool_calls.contains_key(&index)
            && self.openai_tool_calls.len() >= MAX_TOOL_CALLS_PER_RESPONSE
        {
            return Err(StreamNormalizeError::OutputTooLarge {
                field: "tool_calls",
            });
        }
        let pending = self.openai_tool_calls.entry(index).or_default();
        merge_fragment(&mut pending.id, value.get("id").and_then(Value::as_str))?;
        let function = value.get("function");
        merge_fragment(
            &mut pending.name,
            function
                .and_then(|function| function.get("name"))
                .and_then(Value::as_str),
        )?;
        if let Some(arguments) = function
            .and_then(|function| function.get("arguments"))
            .and_then(Value::as_str)
        {
            if pending.arguments.len().saturating_add(arguments.len()) > MAX_TOOL_ARGUMENT_BYTES {
                return Err(StreamNormalizeError::OutputTooLarge {
                    field: "tool_arguments",
                });
            }
            pending.arguments.push_str(arguments);
        }
        Ok(())
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
                    self.cached_input_tokens = token(usage, &["cache_read_input_tokens"]);
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
                            if let Some(AnthropicReplayBlock::Text {
                                text: replay_text,
                                closed: false,
                            }) = value
                                .get("index")
                                .and_then(Value::as_u64)
                                .and_then(|index| self.anthropic_replay_blocks.get_mut(&index))
                            {
                                replay_text.push_str(text);
                            }
                        }
                    }
                    Some("thinking_delta") => {
                        if let Some(reasoning) = delta
                            .and_then(|delta| delta.get("thinking"))
                            .and_then(Value::as_str)
                        {
                            self.append_reasoning(reasoning, &mut deltas)?;
                            if let Some(AnthropicReplayBlock::Thinking {
                                thinking,
                                closed: false,
                                ..
                            }) = value
                                .get("index")
                                .and_then(Value::as_u64)
                                .and_then(|index| self.anthropic_replay_blocks.get_mut(&index))
                            {
                                thinking.push_str(reasoning);
                            }
                        }
                    }
                    Some("input_json_delta") => {
                        let index = anthropic_index(&value)?;
                        let partial = delta
                            .and_then(|delta| delta.get("partial_json"))
                            .and_then(Value::as_str)
                            .ok_or(StreamNormalizeError::MalformedJson)?;
                        let Some(pending) = self.anthropic_tool_calls.get_mut(&index) else {
                            return if self.anthropic_server_tool_blocks.contains(&index) {
                                Ok(deltas)
                            } else {
                                Err(StreamNormalizeError::MalformedJson)
                            };
                        };
                        if pending.closed
                            || pending
                                .initial_input
                                .as_object()
                                .is_none_or(|v| !v.is_empty())
                        {
                            return Err(StreamNormalizeError::MalformedJson);
                        }
                        if pending.partial_json.len().saturating_add(partial.len())
                            > MAX_TOOL_ARGUMENT_BYTES
                        {
                            return Err(StreamNormalizeError::OutputTooLarge {
                                field: "tool_arguments",
                            });
                        }
                        pending.partial_json.push_str(partial);
                    }
                    Some("signature_delta") => {
                        let signature = delta
                            .and_then(|delta| delta.get("signature"))
                            .and_then(Value::as_str)
                            .ok_or(StreamNormalizeError::MalformedJson)?;
                        let index = anthropic_index(&value)?;
                        let Some(AnthropicReplayBlock::Thinking {
                            signature: replay_signature,
                            closed: false,
                            ..
                        }) = self.anthropic_replay_blocks.get_mut(&index)
                        else {
                            return Err(StreamNormalizeError::MalformedJson);
                        };
                        if replay_signature.len().saturating_add(signature.len())
                            > MAX_REASONING_BYTES
                        {
                            return Err(StreamNormalizeError::OutputTooLarge {
                                field: "thinking_signature",
                            });
                        }
                        replay_signature.push_str(signature);
                    }
                    None => {}
                    Some(_) => self.push_warning(InferenceWarningCode::ProviderDegraded),
                }
            }
            Some("message_delta") => {
                if let Some(reason) = value
                    .get("delta")
                    .and_then(|delta| delta.get("stop_reason"))
                    .and_then(Value::as_str)
                {
                    if reason == "tool_use" {
                        self.requires_anthropic_tool_calls = true;
                    }
                    self.set_finish_reason(reason, FinishFamily::Anthropic);
                }
                if let Some(usage) = value.get("usage") {
                    self.output_tokens = token(usage, &["output_tokens"]);
                }
            }
            Some("message_stop") => self.terminal = true,
            Some("content_block_start") => {
                let Some(block) = value.get("content_block") else {
                    return Err(StreamNormalizeError::MalformedJson);
                };
                if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                    let index = anthropic_index(&value)?;
                    if self.anthropic_tool_calls.len() >= MAX_TOOL_CALLS_PER_RESPONSE
                        || self.anthropic_tool_calls.contains_key(&index)
                        || self.anthropic_server_tool_blocks.contains(&index)
                        || self.anthropic_replay_blocks.contains_key(&index)
                    {
                        return Err(StreamNormalizeError::MalformedJson);
                    }
                    let id = required_nonempty(block, "id")?;
                    let name = required_nonempty(block, "name")?;
                    let initial_input = block
                        .get("input")
                        .cloned()
                        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
                    if !initial_input.is_object() {
                        return Err(StreamNormalizeError::MalformedJson);
                    }
                    self.anthropic_tool_calls.insert(
                        index,
                        PendingAnthropicToolCall {
                            id,
                            name,
                            initial_input,
                            partial_json: String::new(),
                            closed: false,
                        },
                    );
                    self.anthropic_replay_blocks
                        .insert(index, AnthropicReplayBlock::ToolUse);
                } else if block.get("type").and_then(Value::as_str) == Some("server_tool_use") {
                    let index = anthropic_index(&value)?;
                    if self.anthropic_server_tool_blocks.len() >= MAX_TOOL_CALLS_PER_RESPONSE
                        || self.anthropic_tool_calls.contains_key(&index)
                        || !self.anthropic_server_tool_blocks.insert(index)
                    {
                        return Err(StreamNormalizeError::MalformedJson);
                    }
                    self.anthropic_unreplayable_block = true;
                    self.push_warning(InferenceWarningCode::ProviderDegraded);
                } else {
                    let index = anthropic_index(&value)?;
                    if self.anthropic_replay_blocks.contains_key(&index)
                        || self.anthropic_tool_calls.contains_key(&index)
                        || self.anthropic_server_tool_blocks.contains(&index)
                    {
                        return Err(StreamNormalizeError::MalformedJson);
                    }
                    let replay = match block.get("type").and_then(Value::as_str) {
                        Some("thinking") => AnthropicReplayBlock::Thinking {
                            thinking: block
                                .get("thinking")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_owned(),
                            signature: block
                                .get("signature")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_owned(),
                            closed: false,
                        },
                        Some("redacted_thinking") => AnthropicReplayBlock::RedactedThinking {
                            data: required_nonempty(block, "data")?,
                            closed: false,
                        },
                        Some("text") => AnthropicReplayBlock::Text {
                            text: block
                                .get("text")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_owned(),
                            closed: false,
                        },
                        Some(_) | None => {
                            self.anthropic_unreplayable_block = true;
                            return Ok(deltas);
                        }
                    };
                    self.anthropic_replay_blocks.insert(index, replay);
                }
            }
            Some("content_block_stop") => {
                let index = anthropic_index(&value)?;
                if let Some(pending) = self.anthropic_tool_calls.get_mut(&index) {
                    if pending.closed {
                        return Err(StreamNormalizeError::MalformedJson);
                    }
                    pending.closed = true;
                } else {
                    self.anthropic_server_tool_blocks.remove(&index);
                    match self.anthropic_replay_blocks.get_mut(&index) {
                        Some(
                            AnthropicReplayBlock::Thinking { closed, .. }
                            | AnthropicReplayBlock::RedactedThinking { closed, .. }
                            | AnthropicReplayBlock::Text { closed, .. },
                        ) if !*closed => {
                            *closed = true;
                        }
                        Some(AnthropicReplayBlock::ToolUse) => {
                            return Err(StreamNormalizeError::MalformedJson);
                        }
                        Some(_) => return Err(StreamNormalizeError::MalformedJson),
                        None => {}
                    }
                }
            }
            Some("ping") | None => {}
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
            self.cached_input_tokens = token(usage, &["cachedContentTokenCount"]);
            self.reasoning_tokens = token(usage, &["thoughtsTokenCount"]);
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
                    if (part.get("text").is_some() && part.get("functionCall").is_some())
                        || part.get("functionResponse").is_some()
                        || part.get("toolCall").is_some()
                        || part.get("toolResponse").is_some()
                    {
                        return Err(StreamNormalizeError::MalformedJson);
                    }
                    if part
                        .get("thoughtSignature")
                        .and_then(Value::as_str)
                        .is_some_and(|signature| !signature.is_empty())
                    {
                        self.gemini_has_thought_signature = true;
                    }
                    let replay_bytes = serde_json::to_vec(part)
                        .map_err(|_| StreamNormalizeError::MalformedJson)?
                        .len();
                    self.gemini_replay_bytes = self
                        .gemini_replay_bytes
                        .checked_add(replay_bytes)
                        .ok_or(StreamNormalizeError::OutputTooLarge {
                        field: "provider_replay",
                    })?;
                    if self.gemini_replay_bytes > MAX_PROVIDER_REPLAY_BYTES {
                        return Err(StreamNormalizeError::OutputTooLarge {
                            field: "provider_replay",
                        });
                    }
                    self.gemini_replay_parts.push(part.clone());
                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                        if part.get("thought").and_then(Value::as_bool) == Some(true) {
                            self.append_reasoning(text, &mut deltas)?;
                        } else {
                            let split = self.thinking.feed(text);
                            self.append_split(split, &mut deltas)?;
                        }
                    }
                    if let Some(call) = part.get("functionCall") {
                        self.append_gemini_tool_call(call)?;
                    }
                }
            }
            if let Some(reason) = candidate.get("finishReason").and_then(Value::as_str) {
                if !self.gemini_tool_calls.is_empty()
                    && !matches!(reason, "STOP" | "FINISH_REASON_UNSPECIFIED")
                {
                    return Err(StreamNormalizeError::MalformedJson);
                }
                self.set_finish_reason(reason, FinishFamily::Gemini);
                self.terminal = true;
            }
        }
        Ok(deltas)
    }

    fn append_gemini_tool_call(&mut self, call: &Value) -> Result<(), StreamNormalizeError> {
        if self.gemini_tool_calls.len() >= MAX_TOOL_CALLS_PER_RESPONSE {
            return Err(StreamNormalizeError::OutputTooLarge {
                field: "tool_calls",
            });
        }
        let name = call
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .ok_or(StreamNormalizeError::MalformedJson)?;
        let id = match call.get("id") {
            Some(Value::String(id)) if !id.is_empty() => Some(id.clone()),
            Some(_) => return Err(StreamNormalizeError::MalformedJson),
            None => None,
        };
        let arguments = call
            .get("args")
            .cloned()
            .unwrap_or_else(|| Value::Object(Default::default()));
        if !arguments.is_object()
            || serde_json::to_vec(&arguments)
                .map_err(|_| StreamNormalizeError::MalformedJson)?
                .len()
                > MAX_TOOL_ARGUMENT_BYTES
        {
            return Err(StreamNormalizeError::MalformedJson);
        }
        self.gemini_tool_calls.push(ProposedToolCall {
            provider_call_id: id,
            name: name.to_owned(),
            arguments,
            raw_arguments: None,
            provider_replay: None,
        });
        Ok(())
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
            if let Some(calls) = message.get("tool_calls") {
                let calls = calls
                    .as_array()
                    .ok_or(StreamNormalizeError::MalformedJson)?;
                for call in calls {
                    if self.ollama_tool_calls.len() >= MAX_TOOL_CALLS_PER_RESPONSE {
                        return Err(StreamNormalizeError::OutputTooLarge {
                            field: "tool_calls",
                        });
                    }
                    let function = call
                        .get("function")
                        .and_then(Value::as_object)
                        .ok_or(StreamNormalizeError::MalformedJson)?;
                    if call
                        .get("type")
                        .is_some_and(|kind| kind.as_str() != Some("function"))
                    {
                        return Err(StreamNormalizeError::MalformedJson);
                    }
                    let name = function
                        .get("name")
                        .and_then(Value::as_str)
                        .ok_or(StreamNormalizeError::MalformedJson)?;
                    let arguments = function
                        .get("arguments")
                        .filter(|value| value.is_object())
                        .cloned()
                        .ok_or(StreamNormalizeError::MalformedJson)?;
                    let provider_call_id = match call.get("id") {
                        Some(id) => Some(
                            id.as_str()
                                .ok_or(StreamNormalizeError::MalformedJson)?
                                .to_owned(),
                        ),
                        None => None,
                    };
                    let proposal = ProposedToolCall {
                        provider_call_id,
                        name: name.to_owned(),
                        arguments,
                        raw_arguments: None,
                        provider_replay: None,
                    };
                    proposal
                        .validate()
                        .map_err(|_| StreamNormalizeError::MalformedJson)?;
                    self.ollama_tool_calls.push(proposal);
                }
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
                "tool_calls" | "function_call" => self.requires_openai_tool_calls = true,
                _ => self.push_warning(InferenceWarningCode::ProviderDegraded),
            },
            FinishFamily::Anthropic => match reason {
                "end_turn" | "stop_sequence" => {}
                "max_tokens" | "model_context_window_exceeded" => self.mark_length(),
                "refusal" => self.push_warning(InferenceWarningCode::SafetyTransformed),
                "tool_use" => {}
                "pause_turn" => {
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

fn merge_fragment(
    target: &mut Option<String>,
    fragment: Option<&str>,
) -> Result<(), StreamNormalizeError> {
    let Some(fragment) = fragment.filter(|fragment| !fragment.is_empty()) else {
        return Ok(());
    };
    match target {
        Some(current) if current != fragment => Err(StreamNormalizeError::MalformedJson),
        Some(_) => Ok(()),
        None => {
            *target = Some(fragment.to_owned());
            Ok(())
        }
    }
}

fn finish_openai_tool_calls(
    pending: BTreeMap<u64, PendingOpenAiToolCall>,
) -> Result<Vec<ProposedToolCall>, StreamNormalizeError> {
    pending
        .into_values()
        .map(|pending| {
            let raw_arguments = pending.arguments;
            let arguments = serde_json::from_str(&raw_arguments)
                .map_err(|_| StreamNormalizeError::MalformedJson)?;
            let call = ProposedToolCall {
                provider_call_id: pending.id,
                name: pending.name.ok_or(StreamNormalizeError::MalformedJson)?,
                arguments,
                raw_arguments: Some(raw_arguments),
                provider_replay: None,
            };
            if call.provider_call_id.is_none() {
                return Err(StreamNormalizeError::MalformedJson);
            }
            call.validate()
                .map_err(|_| StreamNormalizeError::MalformedJson)?;
            Ok(call)
        })
        .collect()
}

fn finish_anthropic_tool_calls(
    pending: BTreeMap<u64, PendingAnthropicToolCall>,
) -> Result<Vec<ProposedToolCall>, StreamNormalizeError> {
    pending
        .into_values()
        .map(|pending| {
            if !pending.closed {
                return Err(StreamNormalizeError::MalformedJson);
            }
            let (arguments, raw_arguments) = if pending.partial_json.is_empty() {
                (pending.initial_input, None)
            } else {
                let arguments = serde_json::from_str(&pending.partial_json)
                    .map_err(|_| StreamNormalizeError::MalformedJson)?;
                (arguments, Some(pending.partial_json))
            };
            let call = ProposedToolCall {
                provider_call_id: Some(pending.id),
                name: pending.name,
                arguments,
                raw_arguments,
                provider_replay: None,
            };
            call.validate()
                .map_err(|_| StreamNormalizeError::MalformedJson)?;
            Ok(call)
        })
        .collect()
}

fn anthropic_index(value: &Value) -> Result<u64, StreamNormalizeError> {
    value
        .get("index")
        .and_then(Value::as_u64)
        .ok_or(StreamNormalizeError::MalformedJson)
}

fn required_nonempty(value: &Value, field: &str) -> Result<String, StreamNormalizeError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .ok_or(StreamNormalizeError::MalformedJson)
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

    use super::{
        MAX_TOOL_CALLS_PER_RESPONSE, StreamDelta, StreamNormalizeError, StreamNormalizer,
        StreamProtocol,
    };
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
                    r#"{"choices":[],"usage":{"prompt_tokens":3,"completion_tokens":2,"prompt_tokens_details":{"cached_tokens":0},"completion_tokens_details":{"reasoning_tokens":1}}}"#
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
        let usage = outcome.usage.unwrap();
        assert_eq!(usage.input_tokens, 3);
        assert_eq!(usage.cached_input_tokens, Some(0));
        assert_eq!(usage.reasoning_tokens, Some(1));
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
    fn openai_accumulates_interleaved_tool_calls_by_wire_index() {
        let mut normalizer = StreamNormalizer::new(StreamProtocol::OpenAi, None);
        normalizer.consume(&record(r#"{"choices":[{"delta":{"tool_calls":[{"index":7,"id":"call-b","type":"function","function":{"name":"delete_memory","arguments":"{\"id\":"}},{"index":2,"id":"call-a","type":"function","function":{"name":"create_memory","arguments":"{\"content\":"}}]}}]}"#)).unwrap();
        normalizer.consume(&record(r#"{"choices":[{"delta":{"tool_calls":[{"index":2,"function":{"arguments":"\"one\"}"}},{"index":7,"function":{"arguments":"\"two\"}"}}]},"finish_reason":"tool_calls"}]}"#)).unwrap();
        normalizer.consume(&record("[DONE]")).unwrap();
        let (_, outcome) = normalizer.finish().unwrap();
        let calls = &outcome.candidates[0].tool_calls;
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].provider_call_id.as_deref(), Some("call-a"));
        assert_eq!(calls[0].arguments, serde_json::json!({"content": "one"}));
        assert_eq!(calls[1].provider_call_id.as_deref(), Some("call-b"));
        assert_eq!(calls[1].arguments, serde_json::json!({"id": "two"}));
    }

    #[test]
    fn openai_rejects_conflicting_or_incomplete_tool_fragments() {
        let mut conflicting = StreamNormalizer::new(StreamProtocol::OpenAi, None);
        conflicting.consume(&record(r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"first","function":{"name":"one","arguments":"{"}}]}}]}"#)).unwrap();
        assert_eq!(
            conflicting
                .consume(&record(r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"second","function":{"arguments":"}"}}]}}]}"#))
                .unwrap_err(),
            StreamNormalizeError::MalformedJson
        );

        let mut incomplete = StreamNormalizer::new(StreamProtocol::OpenAi, None);
        incomplete.consume(&record(r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call","function":{"name":"one","arguments":"{"}}]},"finish_reason":"tool_calls"}]}"#)).unwrap();
        incomplete.consume(&record("[DONE]")).unwrap();
        assert_eq!(
            incomplete.finish().unwrap_err(),
            StreamNormalizeError::MalformedJson
        );

        let mut duplicate_id = StreamNormalizer::new(StreamProtocol::OpenAi, None);
        duplicate_id.consume(&record(r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"same","function":{"name":"one","arguments":"{}"}},{"index":1,"id":"same","function":{"name":"two","arguments":"{}"}}]},"finish_reason":"tool_calls"}]}"#)).unwrap();
        duplicate_id.consume(&record("[DONE]")).unwrap();
        assert_eq!(
            duplicate_id.finish().unwrap_err(),
            StreamNormalizeError::MalformedJson
        );
    }

    #[test]
    fn openai_stream_rejects_non_primary_or_multiple_choices() {
        let mut non_primary = StreamNormalizer::new(StreamProtocol::OpenAi, None);
        assert_eq!(
            non_primary
                .consume(&record(
                    r#"{"choices":[{"index":1,"delta":{"content":"wrong"}}]}"#
                ))
                .unwrap_err(),
            StreamNormalizeError::MalformedJson
        );

        let mut multiple = StreamNormalizer::new(StreamProtocol::OpenAi, None);
        assert_eq!(
            multiple
                .consume(&record(
                    r#"{"choices":[{"index":0,"delta":{"content":"one"}},{"index":1,"delta":{"content":"two"}}]}"#
                ))
                .unwrap_err(),
            StreamNormalizeError::MalformedJson
        );
    }

    #[test]
    fn anthropic_uses_event_types_and_cumulative_usage() {
        let mut normalizer = StreamNormalizer::new(StreamProtocol::Anthropic, None);
        normalizer.consume(&event("message_start", r#"{"type":"message_start","message":{"usage":{"input_tokens":7,"output_tokens":0,"cache_read_input_tokens":9}}}"#)).unwrap();
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
        let usage = outcome.usage.unwrap();
        assert_eq!(usage.output_tokens, 4);
        assert_eq!(usage.input_tokens, 7);
        assert_eq!(usage.cached_input_tokens, Some(9));
        assert_eq!(usage.reasoning_tokens, None);
        assert!(matches!(
            outcome.candidates[0].parts[0],
            MessagePart::ReasoningSummary { .. }
        ));
    }

    #[test]
    fn anthropic_accumulates_tool_input_only_after_block_stop() {
        let mut normalizer = StreamNormalizer::new(StreamProtocol::Anthropic, None);
        normalizer.consume(&event("message_start", r#"{"type":"message_start","message":{"usage":{"input_tokens":7,"output_tokens":0}}}"#)).unwrap();
        normalizer.consume(&event("content_block_start", r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu-1","name":"create_memory","input":{}}}"#)).unwrap();
        normalizer.consume(&event("content_block_delta", r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"content\":"}}"#)).unwrap();
        normalizer.consume(&event("content_block_delta", r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"\"one\"}"}}"#)).unwrap();
        normalizer
            .consume(&event(
                "content_block_stop",
                r#"{"type":"content_block_stop","index":1}"#,
            ))
            .unwrap();
        normalizer.consume(&event("message_delta", r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":4}}"#)).unwrap();
        normalizer
            .consume(&event("message_stop", r#"{"type":"message_stop"}"#))
            .unwrap();

        let (_, outcome) = normalizer.finish().expect("tool outcome");
        let call = &outcome.candidates[0].tool_calls[0];
        assert_eq!(call.provider_call_id.as_deref(), Some("toolu-1"));
        assert_eq!(call.arguments, serde_json::json!({"content": "one"}));
        assert_eq!(call.raw_arguments.as_deref(), Some(r#"{"content":"one"}"#));
        assert!(outcome.warning_codes.is_empty());
        assert_eq!(outcome.usage.unwrap().output_tokens, 4);
    }

    #[test]
    fn anthropic_stream_reconstructs_signed_native_replay_blocks() {
        let mut normalizer = StreamNormalizer::new(StreamProtocol::Anthropic, None);
        normalizer.consume(&event("content_block_start", r#"{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"","signature":""}}"#)).unwrap();
        normalizer.consume(&event("content_block_delta", r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"why"}}"#)).unwrap();
        normalizer.consume(&event("content_block_delta", r#"{"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"opaque"}}"#)).unwrap();
        normalizer
            .consume(&event(
                "content_block_stop",
                r#"{"type":"content_block_stop","index":0}"#,
            ))
            .unwrap();
        normalizer.consume(&event("content_block_start", r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu-1","name":"create_memory","input":{}}}"#)).unwrap();
        normalizer.consume(&event("content_block_delta", r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"content\":\"one\"}"}}"#)).unwrap();
        normalizer
            .consume(&event(
                "content_block_stop",
                r#"{"type":"content_block_stop","index":1}"#,
            ))
            .unwrap();
        normalizer
            .consume(&event(
                "message_delta",
                r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"}}"#,
            ))
            .unwrap();
        normalizer
            .consume(&event("message_stop", r#"{"type":"message_stop"}"#))
            .unwrap();

        let completion = normalizer
            .finish_with_provider_replay()
            .expect("signed replay");
        assert_eq!(completion.outcome.candidates[0].tool_calls.len(), 1);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(
                &completion.provider_replay.expect("replay bytes")
            )
            .expect("replay json"),
            serde_json::json!([
                {"type": "thinking", "thinking": "why", "signature": "opaque"},
                {"type": "tool_use", "id": "toolu-1", "name": "create_memory", "input": {"content": "one"}}
            ])
        );
    }

    #[test]
    fn anthropic_rejects_incomplete_or_duplicate_tool_calls() {
        let mut incomplete = StreamNormalizer::new(StreamProtocol::Anthropic, None);
        incomplete.consume(&event("content_block_start", r#"{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu-1","name":"x","input":{}}}"#)).unwrap();
        incomplete
            .consume(&event(
                "message_delta",
                r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"}}"#,
            ))
            .unwrap();
        incomplete
            .consume(&event("message_stop", r#"{"type":"message_stop"}"#))
            .unwrap();
        assert_eq!(
            incomplete.finish().unwrap_err(),
            StreamNormalizeError::MalformedJson
        );

        let mut duplicate = StreamNormalizer::new(StreamProtocol::Anthropic, None);
        for index in [0, 1] {
            duplicate.consume(&event("content_block_start", &format!(r#"{{"type":"content_block_start","index":{index},"content_block":{{"type":"tool_use","id":"same","name":"x","input":{{}}}}}}"#))).unwrap();
            duplicate
                .consume(&event(
                    "content_block_stop",
                    &format!(r#"{{"type":"content_block_stop","index":{index}}}"#),
                ))
                .unwrap();
        }
        duplicate
            .consume(&event(
                "message_delta",
                r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"}}"#,
            ))
            .unwrap();
        duplicate
            .consume(&event("message_stop", r#"{"type":"message_stop"}"#))
            .unwrap();
        assert_eq!(
            duplicate.finish().unwrap_err(),
            StreamNormalizeError::MalformedJson
        );

        let mut mismatched_stop = StreamNormalizer::new(StreamProtocol::Anthropic, None);
        mismatched_stop.consume(&event("content_block_start", r#"{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu-1","name":"x","input":{}}}"#)).unwrap();
        mismatched_stop
            .consume(&event(
                "content_block_stop",
                r#"{"type":"content_block_stop","index":0}"#,
            ))
            .unwrap();
        mismatched_stop
            .consume(&event(
                "message_delta",
                r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"}}"#,
            ))
            .unwrap();
        mismatched_stop
            .consume(&event("message_stop", r#"{"type":"message_stop"}"#))
            .unwrap();
        assert_eq!(
            mismatched_stop.finish().unwrap_err(),
            StreamNormalizeError::MalformedJson
        );

        let mut malformed_json = StreamNormalizer::new(StreamProtocol::Anthropic, None);
        malformed_json.consume(&event("content_block_start", r#"{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu-1","name":"x","input":{}}}"#)).unwrap();
        malformed_json.consume(&event("content_block_delta", r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{"}}"#)).unwrap();
        malformed_json
            .consume(&event(
                "content_block_stop",
                r#"{"type":"content_block_stop","index":0}"#,
            ))
            .unwrap();
        malformed_json
            .consume(&event(
                "message_delta",
                r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"}}"#,
            ))
            .unwrap();
        malformed_json
            .consume(&event("message_stop", r#"{"type":"message_stop"}"#))
            .unwrap();
        assert_eq!(
            malformed_json.finish().unwrap_err(),
            StreamNormalizeError::MalformedJson
        );
    }

    #[test]
    fn anthropic_bounds_server_tool_tracking() {
        let mut normalizer = StreamNormalizer::new(StreamProtocol::Anthropic, None);
        for index in 0..MAX_TOOL_CALLS_PER_RESPONSE {
            normalizer.consume(&event("content_block_start", &format!(r#"{{"type":"content_block_start","index":{index},"content_block":{{"type":"server_tool_use"}}}}"#))).unwrap();
        }
        assert_eq!(
            normalizer
                .consume(&event("content_block_start", &format!(r#"{{"type":"content_block_start","index":{},"content_block":{{"type":"server_tool_use"}}}}"#, MAX_TOOL_CALLS_PER_RESPONSE)))
                .unwrap_err(),
            StreamNormalizeError::MalformedJson
        );
    }

    #[test]
    fn gemini_uses_only_primary_candidate_and_native_thought_parts() {
        let mut normalizer = StreamNormalizer::new(StreamProtocol::Gemini, None);
        let deltas = normalizer.consume(&record(r#"{"candidates":[{"content":{"parts":[{"text":"why","thought":true},{"text":"answer"}]},"finishReason":"STOP"},{"content":{"parts":[{"text":"ignored"}]}}],"usageMetadata":{"promptTokenCount":2,"candidatesTokenCount":1,"cachedContentTokenCount":0,"thoughtsTokenCount":3}}"#)).unwrap();
        assert!(deltas.contains(&StreamDelta::Reasoning("why".to_owned())));
        assert!(!deltas.contains(&StreamDelta::Text("ignored".to_owned())));
        let (_, outcome) = normalizer.finish().unwrap();
        let usage = outcome.usage.unwrap();
        assert_eq!(usage.output_tokens, 1);
        assert_eq!(usage.cached_input_tokens, Some(0));
        assert_eq!(usage.reasoning_tokens, Some(3));
    }

    #[test]
    fn gemini_collects_unsigned_native_calls_across_chunks() {
        let mut normalizer = StreamNormalizer::new(StreamProtocol::Gemini, None);
        normalizer.consume(&record(r#"{"candidates":[{"content":{"parts":[{"text":"checking"},{"functionCall":{"id":"call-1","name":"lookup_weather","args":{"city":"Paris"}}}]}}]}"#)).unwrap();
        normalizer.consume(&record(r#"{"candidates":[{"content":{"parts":[{"functionCall":{"id":"call-2","name":"lookup_weather","args":{"city":"London"}}}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":4,"candidatesTokenCount":2}}"#)).unwrap();
        let (_, outcome) = normalizer.finish().expect("tool outcome");
        let calls = &outcome.candidates[0].tool_calls;
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].provider_call_id.as_deref(), Some("call-1"));
        assert_eq!(calls[1].provider_call_id.as_deref(), Some("call-2"));
        assert_eq!(calls[1].arguments, serde_json::json!({"city":"London"}));
    }

    #[test]
    fn gemini_preserves_signed_parts_and_rejects_malformed_stream_calls() {
        let mut signed = StreamNormalizer::new(StreamProtocol::Gemini, None);
        signed.consume(&record(r#"{"candidates":[{"content":{"parts":[{"functionCall":{"id":"call-1","name":"lookup_weather","args":{}},"thoughtSignature":"opaque"}]},"finishReason":"STOP"}]}"#)).unwrap();
        let completion = signed
            .finish_with_provider_replay()
            .expect("signed completion");
        assert_eq!(completion.outcome.candidates[0].tool_calls.len(), 1);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(
                &completion.provider_replay.expect("signed replay")
            )
            .expect("replay json"),
            serde_json::json!([{
                "functionCall": {"id": "call-1", "name": "lookup_weather", "args": {}},
                "thoughtSignature": "opaque"
            }])
        );

        let mut malformed = StreamNormalizer::new(StreamProtocol::Gemini, None);
        assert_eq!(
            malformed
                .consume(&record(
                    r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"lookup_weather","args":[]}}]}}]}"#,
                ))
                .unwrap_err(),
            StreamNormalizeError::MalformedJson
        );

        let mut incompatible = StreamNormalizer::new(StreamProtocol::Gemini, None);
        assert_eq!(
            incompatible
                .consume(&record(r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"lookup_weather","args":{}}}]},"finishReason":"MALFORMED_FUNCTION_CALL"}]}"#))
                .unwrap_err(),
            StreamNormalizeError::MalformedJson
        );

        let mut ambiguous = StreamNormalizer::new(StreamProtocol::Gemini, None);
        ambiguous.consume(&record(r#"{"candidates":[{"content":{"parts":[{"functionCall":{"id":"call-1","name":"lookup_weather","args":{}}},{"functionCall":{"name":"lookup_weather","args":{}}}]},"finishReason":"STOP"}]}"#)).unwrap();
        assert_eq!(
            ambiguous.finish().unwrap_err(),
            StreamNormalizeError::MalformedJson
        );
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
    fn ollama_accumulates_atomic_tool_calls_across_ndjson_chunks() {
        let mut normalizer = StreamNormalizer::new(StreamProtocol::Ollama, None);
        normalizer.consume(&record(r#"{"message":{"thinking":"step","tool_calls":[{"function":{"index":0,"name":"lookup_weather","arguments":{"city":"Paris"}}}]},"done":false}"#)).unwrap();
        normalizer.consume(&record(r#"{"message":{"content":"checking","tool_calls":[{"id":"call-2","type":"function","function":{"index":1,"name":"lookup_weather","arguments":{"city":"London"}}}]},"done":true,"done_reason":"stop"}"#)).unwrap();
        let (_, outcome) = normalizer.finish().expect("tool outcome");
        let calls = &outcome.candidates[0].tool_calls;
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].provider_call_id, None);
        assert_eq!(calls[1].provider_call_id.as_deref(), Some("call-2"));
        assert_eq!(calls[1].arguments, serde_json::json!({"city":"London"}));

        let mut malformed = StreamNormalizer::new(StreamProtocol::Ollama, None);
        assert_eq!(
            malformed.consume(&record(r#"{"message":{"tool_calls":[{"function":{"name":"lookup","arguments":[]}}]},"done":true}"#)).unwrap_err(),
            StreamNormalizeError::MalformedJson
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
