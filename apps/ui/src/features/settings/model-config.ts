import type {
  SettingsChatParameterProfile,
  SettingsLlamaCppSettings,
  SettingsLlamaSamplerSettings,
  SettingsModelSettingsLayer,
  SettingsOllamaOptions,
  SettingsOpenRouterOptions,
  SettingsPromptCacheRetention,
  SettingsReasoningEffort,
  SettingsReasoningMode,
  SettingsStableDiffusionCppBinding,
  SettingsStableDiffusionSettings,
  SettingsStableDiffusionLora,
} from "@/api/generated/bindings";

type Sparse<T> = { [K in keyof T]?: T[K] | null };

export type ChatParameters = Sparse<Omit<SettingsChatParameterProfile, "ollama" | "openrouter">> & {
  ollama?: Sparse<SettingsOllamaOptions>;
  openrouter?: Sparse<SettingsOpenRouterOptions>;
};
export type LlamaSampler = Sparse<SettingsLlamaSamplerSettings>;
export type LlamaCpp = Sparse<Omit<SettingsLlamaCppSettings, "sampler">> & { sampler?: LlamaSampler };
export type ModelLora = Omit<SettingsStableDiffusionLora, "multiplier"> & { multiplier: number };
export type StableDiffusion = Sparse<Omit<SettingsStableDiffusionSettings, "cpp" | "base_loras">> & {
  base_loras?: ModelLora[] | null;
  cpp?: Sparse<SettingsStableDiffusionCppBinding>;
};

export type Override<T> = { kind: "inherit" } | { kind: "set"; value: T } | { kind: "clear" };

export type ChatOverrides = {
  [K in keyof Omit<SettingsChatParameterProfile, "ollama" | "openrouter">]?: Override<
    NonNullable<SettingsChatParameterProfile[K]>
  >;
} & {
  ollama?: { [K in keyof SettingsOllamaOptions]?: Override<NonNullable<SettingsOllamaOptions[K]>> };
};

export interface FeatureSlot {
  parameters?: ChatOverrides;
  llama_sampler?: LlamaSampler;
}

export const featureKeys = [
  "dynamic_memory",
  "companion_memory",
  "companion_soul_writer",
  "lorebook_entry_generator",
  "lorebook_generator",
  "scene_writer",
  "help_me_reply",
  "group_speaker_selection",
  "creation_helper",
] as const;
export type FeatureKey = (typeof featureKeys)[number];
export type FeatureParameters = { [K in FeatureKey]?: FeatureSlot };

export type CapabilityStatus = "supported" | "unsupported" | "unknown";
export interface ModelCapabilities {
  format_version: number;
  evidence: { source: "unspecified" | "provider_reported" | "catalog" | "probed" | "user_override"; source_version: number; observed_at: number };
  input_modalities: Record<"text" | "image" | "audio", CapabilityStatus>;
  output_modalities: Record<"text" | "image" | "audio", CapabilityStatus>;
  streaming: CapabilityStatus;
  tools: CapabilityStatus;
  structured_output: CapabilityStatus;
  reasoning: CapabilityStatus;
  prompt_cache: CapabilityStatus;
  context_length: number | null;
  max_visible_output_tokens: number | null;
  max_total_completion_tokens: number | null;
  parameter_support: Record<"temperature" | "top_p" | "top_k" | "frequency_penalty" | "presence_penalty" | "repetition_penalty", CapabilityStatus>;
}

export interface ModelConfig {
  chat_parameters: ChatParameters;
  feature_parameters: FeatureParameters;
  capabilities: ModelCapabilities;
  llama_cpp: LlamaCpp;
  stable_diffusion: StableDiffusion;
  [unknown: string]: unknown;
}

export type Layer = "chat" | FeatureKey;
export type Store = "chat" | "ollama" | "sampler" | "llama" | "sd" | "cpp" | "openrouter";

export type PromptCaching = "disabled" | { enabled: { retention: SettingsPromptCacheRetention } };
export type { SettingsReasoningEffort, SettingsReasoningMode };

function record(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value) ? (value as Record<string, unknown>) : {};
}

export function parseModelConfig(raw: Record<string, unknown>): ModelConfig {
  const copy = structuredClone(raw);
  return {
    ...copy,
    chat_parameters: record(copy.chat_parameters) as ChatParameters,
    feature_parameters: record(copy.feature_parameters) as FeatureParameters,
    capabilities: copy.capabilities as ModelCapabilities,
    llama_cpp: record(copy.llama_cpp) as LlamaCpp,
    stable_diffusion: record(copy.stable_diffusion) as StableDiffusion,
  };
}

export function serializeModelConfig(config: ModelConfig): Record<string, unknown> {
  return structuredClone(config);
}

function isOverride(value: unknown): value is Override<unknown> {
  const kind = record(value).kind;
  return kind === "inherit" || kind === "set" || kind === "clear";
}

function container(config: ModelConfig, layer: Layer, store: Store): Record<string, unknown> | null {
  if (layer === "chat") {
    switch (store) {
      case "chat": return config.chat_parameters;
      case "ollama": return record(config.chat_parameters.ollama);
      case "openrouter": return record(config.chat_parameters.openrouter);
      case "sampler": return record(config.llama_cpp.sampler);
      case "llama": return config.llama_cpp;
      case "sd": return config.stable_diffusion;
      case "cpp": return record(config.stable_diffusion.cpp);
    }
  }
  const slot = config.feature_parameters[layer];
  if (store === "chat") return record(slot?.parameters);
  if (store === "ollama") return record(record(slot?.parameters).ollama);
  if (store === "sampler") return record(slot?.llama_sampler);
  return null;
}

export interface FieldState {
  value: unknown;
  overridden: boolean;
}

export function readField(config: ModelConfig, layer: Layer, store: Store, key: string): FieldState {
  const source = container(config, layer, store);
  const raw = source?.[key];
  if (layer !== "chat" && store !== "sampler") {
    if (!isOverride(raw)) return { value: undefined, overridden: false };
    return { value: raw.kind === "set" ? raw.value : undefined, overridden: raw.kind !== "inherit" };
  }
  return { value: raw ?? undefined, overridden: raw !== undefined && raw !== null };
}

function assign(target: Record<string, unknown>, key: string, value: unknown) {
  if (value === undefined) delete target[key];
  else target[key] = value;
}

export function resolvedKvSettings(config: ModelConfig, defaults: SettingsModelSettingsLayer | undefined): Pick<LlamaCpp, "kv_type" | "kv_type_k" | "kv_type_v"> {
  const layer = [config.llama_cpp, defaults?.llama_cpp].find((value) => value && (value.kv_type != null || value.kv_type_k != null || value.kv_type_v != null));
  return { kv_type: layer?.kv_type ?? null, kv_type_k: layer?.kv_type_k ?? null, kv_type_v: layer?.kv_type_v ?? null };
}

export function writeField(config: ModelConfig, layer: Layer, store: Store, key: string, value: unknown, defaults?: SettingsModelSettingsLayer): ModelConfig {
  const next = structuredClone(config);
  if (layer === "chat") {
    if (store === "chat") assign(next.chat_parameters, key, value);
    else if (store === "ollama") assign((next.chat_parameters.ollama ??= {}), key, value);
    else if (store === "openrouter") assign((next.chat_parameters.openrouter ??= {}), key, value);
    else if (store === "sampler") assign((next.llama_cpp.sampler ??= {}), key, value);
    else if (store === "llama") {
      if (key === "kv_type" && value === "split") {
        const inherited = resolvedKvSettings(config, defaults);
        delete next.llama_cpp.kv_type;
        next.llama_cpp.kv_type_k = inherited.kv_type_k ?? inherited.kv_type ?? "f16";
        next.llama_cpp.kv_type_v = inherited.kv_type_v ?? inherited.kv_type ?? "f16";
        return next;
      }
      assign(next.llama_cpp, key, value);
      if (key === "kv_type") { delete next.llama_cpp.kv_type_k; delete next.llama_cpp.kv_type_v; }
      if (key === "kv_type_k" || key === "kv_type_v") {
        delete next.llama_cpp.kv_type;
        const other = key === "kv_type_k" ? "kv_type_v" : "kv_type_k";
        if (value === undefined) delete next.llama_cpp[other];
        else if (next.llama_cpp[other] == null) {
          const inherited = resolvedKvSettings(config, defaults);
          assign(next.llama_cpp, other, inherited[other] ?? inherited.kv_type ?? "f16");
        }
      }
    }
    else if (store === "sd") assign(next.stable_diffusion, key, value);
    else assign((next.stable_diffusion.cpp ??= {}), key, value);
    return next;
  }
  const slot = (next.feature_parameters[layer] ??= {});
  if (store === "sampler") {
    assign((slot.llama_sampler ??= {}), key, value);
    return next;
  }
  const parameters = (slot.parameters ??= {});
  const target = store === "ollama" ? (parameters.ollama ??= {}) : parameters;
  assign(target as Record<string, unknown>, key, value === undefined ? undefined : { kind: "set", value });
  return next;
}

export function resetFeature(config: ModelConfig, layer: FeatureKey): ModelConfig {
  const next = structuredClone(config);
  delete next.feature_parameters[layer];
  return next;
}

export function changedCount(config: ModelConfig, layer: FeatureKey): number {
  const slot = config.feature_parameters[layer];
  if (!slot) return 0;
  const parameters = record(slot.parameters);
  const direct = Object.entries(parameters).filter(([key, item]) => key !== "ollama" && isOverride(item) && item.kind !== "inherit").length;
  const ollama = Object.values(record(parameters.ollama)).filter((item) => isOverride(item) && item.kind !== "inherit").length;
  const sampler = Object.values(record(slot.llama_sampler)).filter((item) => item !== null && item !== undefined).length;
  return direct + ollama + sampler;
}

export function defaultOf(defaults: SettingsModelSettingsLayer | undefined, store: Store, key: string): unknown {
  if (!defaults) return undefined;
  const source: Record<string, unknown> | null =
    store === "chat" ? record(defaults.chat_parameters)
    : store === "ollama" ? record(defaults.chat_parameters.ollama)
    : store === "openrouter" ? record(defaults.chat_parameters.openrouter)
    : store === "sampler" ? record(defaults.llama_cpp.sampler)
    : store === "llama" ? record(defaults.llama_cpp)
    : store === "sd" ? record(defaults.stable_diffusion)
    : record(defaults.stable_diffusion.cpp);
  return source[key] ?? undefined;
}

export function effectiveField(config: ModelConfig, defaults: SettingsModelSettingsLayer | undefined, layer: Layer, field: { store: Store; key: string }) {
  if (field.store === "llama" && ["kv_type", "kv_type_k", "kv_type_v"].includes(field.key)) {
    const kv = resolvedKvSettings(config, defaults);
    const k = kv.kv_type_k ?? kv.kv_type ?? "f16";
    const v = kv.kv_type_v ?? kv.kv_type ?? "f16";
    return field.key === "kv_type_k" ? k : field.key === "kv_type_v" ? v : kv.kv_type_k != null || kv.kv_type_v != null ? "split" : k;
  }
  const own = readField(config, layer, field.store, field.key);
  if (own.overridden) return own.value;
  if (layer !== "chat") {
    const chat = readField(config, "chat", field.store, field.key);
    if (chat.overridden) return chat.value;
  }
  return defaultOf(defaults, field.store, field.key);
}
