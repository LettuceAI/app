import type { ChatParameters, LlamaCpp, LlamaSampler, StableDiffusion } from "./model-config";

type Stores = { chat: ChatParameters; ollama: NonNullable<ChatParameters["ollama"]>; openrouter: NonNullable<ChatParameters["openrouter"]>; sampler: LlamaSampler; llama: LlamaCpp; sd: StableDiffusion; cpp: NonNullable<StableDiffusion["cpp"]> };
export type EditorField = { [S in keyof Stores]: { store: S; key: keyof Stores[S] } }[keyof Stores] & {
  type: "slider" | "number" | "select" | "switch" | "text" | "file" | "strings" | "order" | "reasoning" | "caching";
  min?: number;
  max?: number;
  step?: number;
  options?: readonly string[];
  multiline?: boolean;
};

export const editorFields = {
  "temperature": {
    "store": "chat",
    "key": "temperature",
    "type": "slider",
    "min": 0,
    "max": 2,
    "step": 0.05
  },
  "topP": {
    "store": "chat",
    "key": "top_p",
    "type": "slider",
    "min": 0,
    "max": 1,
    "step": 0.01
  },
  "topK": {
    "store": "chat",
    "key": "top_k",
    "type": "number",
    "min": 1,
    "max": 2147483647,
    "step": 1
  },
  "maxOutputTokens": {
    "store": "chat",
    "key": "max_output_tokens",
    "type": "number",
    "min": 1,
    "max": 2147483647,
    "step": 1
  },
  "frequencyPenalty": {
    "store": "chat",
    "key": "frequency_penalty",
    "type": "slider",
    "min": -2,
    "max": 2,
    "step": 0.05
  },
  "presencePenalty": {
    "store": "chat",
    "key": "presence_penalty",
    "type": "slider",
    "min": -2,
    "max": 2,
    "step": 0.05
  },
  "llamaSamplerProfile": {
    "store": "sampler",
    "key": "profile",
    "type": "select",
    "options": [
      "balanced",
      "creative",
      "stable",
      "reasoning"
    ]
  },
  "llamaMinP": {
    "store": "sampler",
    "key": "min_p",
    "type": "slider",
    "min": 0,
    "max": 1,
    "step": 0.01
  },
  "llamaTypicalP": {
    "store": "sampler",
    "key": "typical_p",
    "type": "slider",
    "min": 0,
    "max": 1,
    "step": 0.01
  },
  "llamaSeed": {
    "store": "sampler",
    "key": "seed",
    "type": "number",
    "min": 0,
    "max": 2147483647,
    "step": 1
  },
  "llamaRepeatPenalty": {
    "store": "sampler",
    "key": "repeat_penalty",
    "type": "slider",
    "min": 0,
    "max": 2,
    "step": 0.01
  },
  "llamaNPenRange": {
    "store": "sampler",
    "key": "n_pen_range",
    "type": "number",
    "min": -1,
    "max": 262144,
    "step": 1
  },
  "llamaDryMultiplier": {
    "store": "sampler",
    "key": "dry_multiplier",
    "type": "slider",
    "min": 0,
    "max": 10,
    "step": 0.1
  },
  "llamaDryBase": {
    "store": "sampler",
    "key": "dry_base",
    "type": "slider",
    "min": 0,
    "max": 10,
    "step": 0.05
  },
  "llamaDryAllowedLength": {
    "store": "sampler",
    "key": "dry_allowed_length",
    "type": "number",
    "min": 0,
    "max": 128,
    "step": 1
  },
  "llamaDryPenaltyLastN": {
    "store": "sampler",
    "key": "dry_penalty_last_n",
    "type": "number",
    "min": -1,
    "max": 262144,
    "step": 1
  },
  "llamaDrySequenceBreakers": {
    "store": "sampler",
    "key": "dry_sequence_breakers",
    "type": "strings"
  },
  "llamaXtcProbability": {
    "store": "sampler",
    "key": "xtc_probability",
    "type": "slider",
    "min": 0,
    "max": 1,
    "step": 0.01
  },
  "llamaXtcThreshold": {
    "store": "sampler",
    "key": "xtc_threshold",
    "type": "slider",
    "min": 0,
    "max": 1,
    "step": 0.01
  },
  "llamaAdaptiveTarget": {
    "store": "sampler",
    "key": "adaptive_target",
    "type": "slider",
    "min": 0,
    "max": 1,
    "step": 0.01
  },
  "llamaAdaptiveDecay": {
    "store": "sampler",
    "key": "adaptive_decay",
    "type": "slider",
    "min": 0,
    "max": 0.99,
    "step": 0.01
  },
  "reasoningEnabled": {
    "store": "chat",
    "key": "reasoning_mode",
    "type": "reasoning"
  },
  "reasoningEffort": {
    "store": "chat",
    "key": "reasoning_effort",
    "type": "select",
    "options": [
      "low",
      "medium",
      "high"
    ]
  },
  "reasoningBudgetTokens": {
    "store": "chat",
    "key": "reasoning_budget_tokens",
    "type": "number",
    "min": 1,
    "max": 2147483647,
    "step": 1
  },
  "forceSendThinkingState": {
    "store": "chat",
    "key": "send_thinking_state",
    "type": "switch"
  },
  "promptCachingEnabled": {
    "store": "chat",
    "key": "prompt_caching",
    "type": "caching",
    "options": [
      "disabled",
      "in_memory",
      "five_minutes",
      "one_hour",
      "twenty_four_hours"
    ]
  },
  "contextLength": {
    "store": "chat",
    "key": "context_length",
    "type": "number",
    "min": 1,
    "max": 4294967295,
    "step": 1
  },
  "llamaKvType": {
    "store": "llama",
    "key": "kv_type",
    "type": "select",
    "options": [
      "f16",
      "f32",
      "q8_0",
      "q8_1",
      "q6_k",
      "q5_k",
      "q5_1",
      "q5_0",
      "q4_k",
      "q4_1",
      "q4_0",
      "q3_k",
      "q2_k",
      "iq4_nl",
      "iq3_s",
      "iq3_xxs",
      "iq2_xs",
      "iq2_xxs",
      "iq1_s"
    ]
  },
  "llamaKvPlacement": {
    "store": "llama",
    "key": "kv_placement",
    "type": "select",
    "options": [
      "auto",
      "split",
      "system_ram",
      "pin"
    ]
  },
  "llamaSwaFull": {
    "store": "llama",
    "key": "swa_full",
    "type": "switch"
  },
  "llamaFlashAttention": {
    "store": "llama",
    "key": "flash_attention",
    "type": "select",
    "options": [
      "auto",
      "enabled",
      "disabled"
    ]
  },
  "llamaGpuLayers": {
    "store": "llama",
    "key": "gpu_layers",
    "type": "number",
    "min": 0,
    "max": 512,
    "step": 1
  },
  "llamaCpuLayers": {
    "store": "llama",
    "key": "cpu_layers",
    "type": "number",
    "min": 0,
    "max": 512,
    "step": 1
  },
  "llamaMultiGpuEnabled": {
    "store": "llama",
    "key": "multi_gpu_enabled",
    "type": "switch"
  },
  "llamaGpuDistributionMode": {
    "store": "llama",
    "key": "gpu_distribution_mode",
    "type": "select",
    "options": [
      "balanced",
      "proportional",
      "priority",
      "manual"
    ]
  },
  "llamaMainGpu": {
    "store": "llama",
    "key": "main_gpu",
    "type": "number",
    "min": 0,
    "max": 2147483647,
    "step": 1
  },
  "llamaPriorityVramLimitBytes": {
    "store": "llama",
    "key": "priority_vram_limit_bytes",
    "type": "number",
    "min": 0,
    "max": 2147483647,
    "step": 1
  },
  "llamaSingleGpuDeviceId": {
    "store": "llama",
    "key": "single_gpu_device_id",
    "type": "number",
    "min": 0,
    "max": 2147483647,
    "step": 1
  },
  "llamaThreads": {
    "store": "llama",
    "key": "threads",
    "type": "number",
    "min": 1,
    "max": 256,
    "step": 1
  },
  "llamaThreadsBatch": {
    "store": "llama",
    "key": "threads_batch",
    "type": "number",
    "min": 1,
    "max": 256,
    "step": 1
  },
  "llamaBatchSize": {
    "store": "llama",
    "key": "batch_size",
    "type": "number",
    "min": 1,
    "max": 8192,
    "step": 1
  },
  "llamaUbatchSize": {
    "store": "llama",
    "key": "ubatch_size",
    "type": "number",
    "min": 1,
    "max": 8192,
    "step": 1
  },
  "llamaRopeFreqBase": {
    "store": "llama",
    "key": "rope_freq_base",
    "type": "number",
    "min": 0,
    "max": 1000000,
    "step": 0.01
  },
  "llamaRopeFreqScale": {
    "store": "llama",
    "key": "rope_freq_scale",
    "type": "number",
    "min": 0,
    "max": 10,
    "step": 0.01
  },
  "llamaMtpEnabled": {
    "store": "llama",
    "key": "mtp_enabled",
    "type": "switch"
  },
  "llamaMtpModelPath": {
    "store": "llama",
    "key": "mtp_model_path",
    "type": "file"
  },
  "llamaMtpDraftTokens": {
    "store": "llama",
    "key": "mtp_draft_tokens",
    "type": "slider",
    "min": 1,
    "max": 8,
    "step": 1
  },
  "llamaMtpPlacement": {
    "store": "llama",
    "key": "mtp_placement",
    "type": "select",
    "options": [
      "auto",
      "gpu",
      "cpu"
    ]
  },
  "llamaDflashEnabled": {
    "store": "llama",
    "key": "dflash_enabled",
    "type": "switch"
  },
  "llamaDflashModelPath": {
    "store": "llama",
    "key": "dflash_model_path",
    "type": "file"
  },
  "llamaDflashDraftTokens": {
    "store": "llama",
    "key": "dflash_draft_tokens",
    "type": "slider",
    "min": 1,
    "max": 15,
    "step": 1
  },
  "llamaDflashMinProbability": {
    "store": "llama",
    "key": "dflash_min_probability",
    "type": "slider",
    "min": 0,
    "max": 1,
    "step": 0.01
  },
  "llamaChatTemplatePreset": {
    "store": "llama",
    "key": "chat_template_preset",
    "type": "select",
    "options": [
      "",
      "chatml",
      "llama3",
      "gemma",
      "mistral",
      "custom"
    ]
  },
  "llamaChatTemplateOverride": {
    "store": "llama",
    "key": "chat_template_override",
    "type": "text",
    "multiline": true
  },
  "llamaMmprojPath": {
    "store": "llama",
    "key": "mmproj_path",
    "type": "file"
  },
  "forceGemma4Reasoning": {
    "store": "llama",
    "key": "force_gemma4_reasoning",
    "type": "switch"
  },
  "llamaStreamingEnabled": {
    "store": "llama",
    "key": "streaming_enabled",
    "type": "switch"
  },
  "llamaRawCompletionFallback": {
    "store": "llama",
    "key": "raw_completion_fallback",
    "type": "switch"
  },
  "llamaStrictMode": {
    "store": "llama",
    "key": "strict_mode",
    "type": "switch"
  },
  "sdSize": {
    "store": "sd",
    "key": "size",
    "type": "text",
    "options": [
      "1024x1024",
      "832x1216",
      "1216x832",
      "768x1344",
      "1344x768"
    ]
  },
  "sdSteps": {
    "store": "sd",
    "key": "steps",
    "type": "slider",
    "min": 1,
    "max": 150,
    "step": 1
  },
  "sdCfgScale": {
    "store": "sd",
    "key": "cfg_scale",
    "type": "slider",
    "min": 0,
    "max": 30,
    "step": 0.5
  },
  "sdSampler": {
    "store": "sd",
    "key": "sampler",
    "type": "text",
    "options": [
      "euler",
      "euler_a",
      "heun",
      "dpm++2m",
      "dpm++2s_a",
      "lcm"
    ]
  },
  "sdScheduler": {
    "store": "sd",
    "key": "scheduler",
    "type": "text",
    "options": [
      "discrete",
      "karras",
      "exponential",
      "sgm_uniform",
      "simple"
    ]
  },
  "sdSeed": {
    "store": "sd",
    "key": "seed",
    "type": "number",
    "min": 0,
    "max": 2147483647,
    "step": 1
  },
  "sdDistilledGuidance": {
    "store": "sd",
    "key": "distilled_guidance",
    "type": "slider",
    "min": 0,
    "max": 30,
    "step": 0.5
  },
  "sdImageCfgScale": {
    "store": "sd",
    "key": "image_cfg_scale",
    "type": "slider",
    "min": 0,
    "max": 30,
    "step": 0.5
  },
  "sdDenoisingStrength": {
    "store": "sd",
    "key": "denoising_strength",
    "type": "slider",
    "min": 0,
    "max": 1,
    "step": 0.01
  },
  "sdEta": {
    "store": "sd",
    "key": "eta",
    "type": "number",
    "min": 0,
    "max": 10,
    "step": 0.01
  },
  "sdFlowShift": {
    "store": "sd",
    "key": "flow_shift",
    "type": "number",
    "min": -100,
    "max": 100,
    "step": 0.01
  },
  "sdNegativePrompt": {
    "store": "sd",
    "key": "negative_prompt",
    "type": "text",
    "multiline": true
  },
  "sdExtraPrompt": {
    "store": "sd",
    "key": "extra_prompt",
    "type": "text",
    "multiline": true
  },
  "sdPromptWriterInstructions": {
    "store": "sd",
    "key": "prompt_writer_instructions",
    "type": "text",
    "multiline": true
  },
  "sdHiresEnabled": {
    "store": "sd",
    "key": "hires_enabled",
    "type": "switch"
  },
  "sdHiresUpscaler": {
    "store": "sd",
    "key": "hires_upscaler",
    "type": "text"
  },
  "sdHiresScale": {
    "store": "sd",
    "key": "hires_scale",
    "type": "slider",
    "min": 1,
    "max": 8,
    "step": 0.25
  },
  "sdHiresWidth": {
    "store": "sd",
    "key": "hires_width",
    "type": "number",
    "min": 0,
    "max": 16384,
    "step": 1
  },
  "sdHiresHeight": {
    "store": "sd",
    "key": "hires_height",
    "type": "number",
    "min": 0,
    "max": 16384,
    "step": 1
  },
  "sdHiresSteps": {
    "store": "sd",
    "key": "hires_steps",
    "type": "number",
    "min": 0,
    "max": 150,
    "step": 1
  },
  "sdHiresDenoisingStrength": {
    "store": "sd",
    "key": "hires_denoising_strength",
    "type": "slider",
    "min": 0.01,
    "max": 1,
    "step": 0.01
  },
  "sdCacheMode": {
    "store": "sd",
    "key": "cache_mode",
    "type": "select",
    "options": [
      "disabled",
      "easycache",
      "ucache",
      "dbcache",
      "taylorseer",
      "cache_dit",
      "spectrum"
    ]
  },
  "sdCacheOption": {
    "store": "sd",
    "key": "cache_option",
    "type": "text"
  },
  "sdVaeTilingEnabled": {
    "store": "sd",
    "key": "vae_tiling_enabled",
    "type": "switch"
  },
  "sdVaeTileSizeX": {
    "store": "sd",
    "key": "vae_tile_size_x",
    "type": "number",
    "min": 1,
    "max": 8192,
    "step": 1
  },
  "sdVaeTileSizeY": {
    "store": "sd",
    "key": "vae_tile_size_y",
    "type": "number",
    "min": 1,
    "max": 8192,
    "step": 1
  },
  "sdVaeTileOverlap": {
    "store": "sd",
    "key": "vae_tile_overlap",
    "type": "slider",
    "min": 0,
    "max": 1,
    "step": 0.05
  },
  "sdSlgScale": {
    "store": "sd",
    "key": "slg_scale",
    "type": "slider",
    "min": 0,
    "max": 30,
    "step": 0.5
  },
  "sdSlgLayers": {
    "store": "sd",
    "key": "slg_layers",
    "type": "text"
  },
  "sdSlgLayerStart": {
    "store": "sd",
    "key": "slg_layer_start",
    "type": "slider",
    "min": 0,
    "max": 1,
    "step": 0.01
  },
  "sdSlgLayerEnd": {
    "store": "sd",
    "key": "slg_layer_end",
    "type": "slider",
    "min": 0,
    "max": 1,
    "step": 0.01
  },
  "sdAutoResizeRefImages": {
    "store": "sd",
    "key": "auto_resize_reference_images",
    "type": "switch"
  },
  "sdIncreaseRefIndex": {
    "store": "sd",
    "key": "increase_reference_index",
    "type": "switch"
  },
  "ollamaMinP": {
    "store": "ollama",
    "key": "min_p",
    "type": "slider",
    "min": 0,
    "max": 1,
    "step": 0.01
  },
  "ollamaTypicalP": {
    "store": "ollama",
    "key": "typical_p",
    "type": "slider",
    "min": 0,
    "max": 1,
    "step": 0.01
  },
  "ollamaTfsZ": {
    "store": "ollama",
    "key": "tfs_z",
    "type": "slider",
    "min": 0,
    "max": 1,
    "step": 0.01
  },
  "ollamaRepeatPenalty": {
    "store": "chat",
    "key": "repetition_penalty",
    "type": "slider",
    "min": 0.01,
    "max": 2,
    "step": 0.01
  },
  "ollamaSeed": {
    "store": "ollama",
    "key": "seed",
    "type": "number",
    "min": 0,
    "max": 2147483647,
    "step": 1
  },
  "ollamaStop": {
    "store": "ollama",
    "key": "stop",
    "type": "strings"
  },
  "ollamaMirostat": {
    "store": "ollama",
    "key": "mirostat",
    "type": "select",
    "options": [
      "0",
      "1",
      "2"
    ]
  },
  "ollamaMirostatTau": {
    "store": "ollama",
    "key": "mirostat_tau",
    "type": "slider",
    "min": 0,
    "max": 10,
    "step": 0.1
  },
  "ollamaMirostatEta": {
    "store": "ollama",
    "key": "mirostat_eta",
    "type": "slider",
    "min": 0,
    "max": 1,
    "step": 0.01
  },
  "ollamaNumCtx": {
    "store": "ollama",
    "key": "num_ctx",
    "type": "number",
    "min": 1,
    "max": 262144,
    "step": 1
  },
  "ollamaNumPredict": {
    "store": "ollama",
    "key": "num_predict",
    "type": "number",
    "min": 1,
    "max": 131072,
    "step": 1
  },
  "ollamaNumKeep": {
    "store": "ollama",
    "key": "num_keep",
    "type": "number",
    "min": 0,
    "max": 32768,
    "step": 1
  },
  "ollamaNumGpu": {
    "store": "ollama",
    "key": "num_gpu",
    "type": "number",
    "min": 0,
    "max": 512,
    "step": 1
  },
  "ollamaNumThread": {
    "store": "ollama",
    "key": "num_thread",
    "type": "number",
    "min": 1,
    "max": 256,
    "step": 1
  },
  "ollamaNumBatch": {
    "store": "ollama",
    "key": "num_batch",
    "type": "number",
    "min": 1,
    "max": 16384,
    "step": 1
  },
  "samplerOrder": {
    "store": "sampler",
    "key": "order",
    "type": "order",
    "options": [
      "penalties",
      "grammar",
      "top_k",
      "top_p",
      "min_p",
      "dry",
      "typical",
      "xtc",
      "temp",
      "adaptive_p"
    ]
  },
  "pinnedProvider": {
    "store": "openrouter",
    "key": "pinned_provider",
    "type": "text"
  },
  "sdOffloadMode": {
    "store": "sd",
    "key": "offload_mode",
    "type": "select",
    "options": [
      "auto",
      "gpu",
      "mixed"
    ]
  },
  "textEncoder": {
    "store": "cpp",
    "key": "text_encoder_path",
    "type": "file"
  },
  "vae": {
    "store": "cpp",
    "key": "vae_path",
    "type": "file"
  },
  "visionEncoder": {
    "store": "cpp",
    "key": "vision_encoder_path",
    "type": "file"
  },
  "llamaKvTypeK": {
    "store": "llama",
    "key": "kv_type_k",
    "type": "select",
    "options": [
      "f16",
      "f32",
      "q8_0",
      "q8_1",
      "q6_k",
      "q5_k",
      "q5_1",
      "q5_0",
      "q4_k",
      "q4_1",
      "q4_0",
      "q3_k",
      "q2_k",
      "iq4_nl",
      "iq3_s",
      "iq3_xxs",
      "iq2_xs",
      "iq2_xxs",
      "iq1_s"
    ]
  },
  "llamaKvTypeV": {
    "store": "llama",
    "key": "kv_type_v",
    "type": "select",
    "options": [
      "f16",
      "f32",
      "q8_0",
      "q8_1",
      "q6_k",
      "q5_k",
      "q5_1",
      "q5_0",
      "q4_k",
      "q4_1",
      "q4_0",
      "q3_k",
      "q2_k",
      "iq4_nl",
      "iq3_s",
      "iq3_xxs",
      "iq2_xs",
      "iq2_xxs",
      "iq1_s"
    ]
  }
} as const satisfies Record<string, EditorField>;
export type FieldId = keyof typeof editorFields;
