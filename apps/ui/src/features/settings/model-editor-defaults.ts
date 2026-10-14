import type { ImageBundleProfile, SettingsModelSettingsLayer } from "@/api/generated/bindings";
import { effectiveField, type Layer, type ModelConfig } from "@/entities/model/model-config";
import type { EditorKind } from "@/entities/model/model-editor-sections";
import type { FieldId } from "@/entities/model/model-editor-fields";

const profiles = {
  balanced: { temperature: 0.8, topP: 0.95, topK: 40, llamaMinP: 0.05, llamaTypicalP: 1, frequencyPenalty: 0.15, presencePenalty: 0 },
  creative: { temperature: 0.95, topP: 0.98, topK: 80, llamaMinP: 0.02, llamaTypicalP: 1, frequencyPenalty: 0, presencePenalty: 0.25 },
  stable: { temperature: 0.55, topP: 0.9, topK: 32, llamaMinP: 0.08, llamaTypicalP: 0.97, frequencyPenalty: 0.2, presencePenalty: 0 },
  reasoning: { temperature: 0.35, topP: 0.9, topK: 24, llamaMinP: 0, llamaTypicalP: 0.95, frequencyPenalty: 0.1, presencePenalty: 0 },
};

const local: Partial<Record<FieldId, unknown>> = {
  maxOutputTokens: 512, llamaSamplerProfile: "balanced", llamaRepeatPenalty: 1, llamaNPenRange: -1,
  llamaDryMultiplier: 0, llamaDryBase: 1.75, llamaDryAllowedLength: 2, llamaDryPenaltyLastN: -1,
  llamaDrySequenceBreakers: ["\n", ":", "\"", "*"], llamaXtcProbability: 0, llamaXtcThreshold: 0.1,
  llamaAdaptiveTarget: 0, llamaAdaptiveDecay: 0.95,
  samplerOrder: ["penalties", "grammar", "top_k", "top_p", "min_p", "dry", "typical", "xtc", "temp", "adaptive_p"],
  llamaKvPlacement: "auto", llamaFlashAttention: "auto", llamaSwaFull: true, llamaOffloadKqv: true,
  llamaKvType: "f16", llamaKvTypeK: "f16", llamaKvTypeV: "f16", llamaBatchSize: 512,
  llamaMtpEnabled: false, llamaDflashEnabled: false, llamaMtpDraftTokens: 4, llamaDflashDraftTokens: 4,
  llamaDflashMinProbability: 0.55, llamaMtpPlacement: "auto", llamaMultiGpuEnabled: false,
  llamaGpuDistributionMode: "balanced", llamaCpuLayers: 0, llamaStreamingEnabled: true, forceGemma4Reasoning: false, llamaRawCompletionFallback: false, llamaStrictMode: false,
};

export function runtimeFieldDefault(kind: EditorKind, id: FieldId, config: ModelConfig, defaults: SettingsModelSettingsLayer | undefined, layer: Layer, imageProfile?: ImageBundleProfile): unknown {
  if (id === "promptCachingEnabled") return "disabled";
  if (id === "forceSendThinkingState") return false;
  if (kind === "local") {
    const name = effectiveField(config, defaults, layer, { store: "sampler", key: "profile" });
    const profile = profiles[(typeof name === "string" && name in profiles ? name : "balanced") as keyof typeof profiles];
    if (id in profile) return profile[id as keyof typeof profile];
    if (id === "llamaUbatchSize") return effectiveField(config, defaults, layer, { store: "llama", key: "batch_size" }) ?? 512;
    return local[id];
  }
  if (kind === "cloud" || kind === "ollama") {
    if (id === "maxOutputTokens") return 4096;
    if (id === "ollamaNumPredict") return effectiveField(config, defaults, layer, { store: "chat", key: "max_output_tokens" }) ?? 4096;
  }
  if (kind === "image") {
    if (id === "sdSize") return imageProfile ? `${imageProfile.default_width}x${imageProfile.default_height}` : undefined;
    if (id === "sdSteps") return imageProfile?.default_steps ?? 20;
    if (id === "sdCfgScale") return imageProfile?.default_cfg ?? 7;
    if (id === "sdVaeTilingEnabled") return true;
    if (id === "sdHiresEnabled" || id === "sdIncreaseRefIndex") return false;
    if (id === "sdAutoResizeRefImages") return true;
    if (id === "sdSlgScale") return 0;
  }
  return undefined;
}
