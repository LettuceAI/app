import type { FieldId } from "./model-editor-fields";

export type EditorKind = "cloud" | "ollama" | "local" | "image";
export interface EditorGroup { label?: string; fields: FieldId[]; toggle?: FieldId }
export interface EditorSection { id: string; groups: EditorGroup[]; perFeature?: boolean }
const common: FieldId[] = ["temperature", "topP", "topK", "maxOutputTokens"];
const penalties: FieldId[] = ["frequencyPenalty", "presencePenalty"];
const group = (label: string, fields: FieldId[]): EditorGroup => ({ label, fields });
const section = (id: string, groups: EditorGroup[], perFeature = false): EditorSection => ({ id, groups, perFeature });

export function editorSections(kind: EditorKind, openRouter: boolean): EditorSection[] {
  if (kind === "image") return [
    section("generation", [
      { fields: ["sdSize", "sdSteps", "sdCfgScale", "sdSampler", "sdScheduler", "sdSeed"] },
      group("guidance", ["sdDistilledGuidance", "sdImageCfgScale", "sdDenoisingStrength", "sdEta", "sdFlowShift"]),
    ]),
    section("prompt", [{ fields: ["sdNegativePrompt", "sdExtraPrompt", "sdPromptWriterInstructions"] }]),
    section("loras", []),
    section("hires", [{ toggle: "sdHiresEnabled", fields: [ "sdHiresUpscaler", "sdHiresScale", "sdHiresWidth", "sdHiresHeight", "sdHiresSteps", "sdHiresDenoisingStrength"] }]),
    section("speed", [
      group("cache", ["sdCacheMode", "sdCacheOption"]),
      { label: "tiling", toggle: "sdVaeTilingEnabled", fields: ["sdVaeTileSizeX", "sdVaeTileSizeY", "sdVaeTileOverlap"] },
      group("slg", ["sdSlgScale", "sdSlgLayers", "sdSlgLayerStart", "sdSlgLayerEnd"]),
      group("references", ["sdAutoResizeRefImages", "sdIncreaseRefIndex"]),
    ]),
  ];
  if (kind === "ollama") return [
    section("generation", [
      { fields: common }, group("penalties", penalties),
      group("sampler", ["ollamaMinP", "ollamaTypicalP", "ollamaTfsZ", "ollamaRepeatPenalty", "ollamaSeed", "ollamaStop"]),
      group("mirostat", ["ollamaMirostat", "ollamaMirostatTau", "ollamaMirostatEta"]),
    ], true),
    section("server", [{ fields: ["ollamaNumCtx", "ollamaNumPredict", "ollamaNumKeep"] }, group("hardware", ["ollamaNumGpu", "ollamaNumThread", "ollamaNumBatch"])]),
  ];
  if (kind === "local") return [
    section("generation", [
      { fields: common },
      group("sampler", ["llamaSamplerProfile", "samplerOrder", "llamaMinP", "llamaTypicalP", "llamaSeed"]),
      group("repetition", ["llamaRepeatPenalty", "llamaNPenRange"]),
      group("dry", ["llamaDryMultiplier", "llamaDryBase", "llamaDryAllowedLength", "llamaDryPenaltyLastN", "llamaDrySequenceBreakers"]),
      group("xtc", ["llamaXtcProbability", "llamaXtcThreshold"]),
      group("adaptive", ["llamaAdaptiveTarget", "llamaAdaptiveDecay"]),
    ], true),
    section("memory", [{ fields: ["contextLength"] }, group("kv", ["llamaKvType", "llamaKvTypeK", "llamaKvTypeV", "llamaKvPlacement", "llamaOffloadKqv", "llamaSwaFull", "llamaFlashAttention"])]),
    section("gpu", [{ fields: ["llamaGpuLayers", "llamaCpuLayers"] }, { label: "multipleGpus", toggle: "llamaMultiGpuEnabled", fields: ["llamaGpuDistributionMode", "llamaMainGpu", "llamaPriorityVramLimitBytes"] }, group("singleGpu", ["llamaSingleGpuDeviceId"])]),
    section("performance", [group("cpu", ["llamaThreads", "llamaThreadsBatch"]), group("batching", ["llamaBatchSize", "llamaUbatchSize"]), group("rope", ["llamaRopeFreqBase", "llamaRopeFreqScale"])]),
    section("speculative", [{ label: "mtp", toggle: "llamaMtpEnabled", fields: ["llamaMtpModelPath", "llamaMtpDraftTokens", "llamaMtpPlacement"] }, { label: "dflash", toggle: "llamaDflashEnabled", fields: ["llamaDflashModelPath", "llamaDflashDraftTokens", "llamaDflashMinProbability"] }]),
    section("template", [group("template", ["llamaChatTemplatePreset", "llamaChatTemplateOverride"]), group("vision", ["llamaMmprojPath"]), group("reasoning", ["reasoningEnabled", "forceGemma4Reasoning"]), group("output", ["llamaStreamingEnabled", "llamaRawCompletionFallback", "llamaStrictMode"])]),
  ];
  return [
    section("generation", [{ fields: common }, group("penalties", penalties)], true),
    section("reasoning", [{ toggle: "reasoningEnabled", fields: ["reasoningEffort", "reasoningBudgetTokens"] }, group("compatibility", ["forceSendThinkingState"])]),
    section("caching", [{ fields: ["promptCachingEnabled"] }]),
    ...(openRouter ? [section("routing", [])] : []),
  ];
}
