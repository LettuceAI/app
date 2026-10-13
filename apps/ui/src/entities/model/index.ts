const unknown = "unknown";

export const minimalModelConfig = {
  capabilities: {
    format_version: 1,
    evidence: { source: "unspecified", source_version: 0, observed_at: 0 },
    input_modalities: { text: unknown, image: unknown, audio: unknown },
    output_modalities: { text: unknown, image: unknown, audio: unknown },
    streaming: unknown,
    tools: unknown,
    structured_output: unknown,
    reasoning: unknown,
    prompt_cache: unknown,
    context_length: null,
    max_visible_output_tokens: null,
    max_total_completion_tokens: null,
    parameter_support: {
      temperature: unknown,
      top_p: unknown,
      top_k: unknown,
      frequency_penalty: unknown,
      presence_penalty: unknown,
      repetition_penalty: unknown,
    },
  },
};

