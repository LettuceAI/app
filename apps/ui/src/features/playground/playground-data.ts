import { infiniteQueryOptions, queryOptions, skipToken } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { isApiFailure, type ApiClient } from "@/api/client";
import { queryKeys } from "@/api/query-keys";
import type {
  ImageCapabilities,
  ImageGenerateRequest_Deserialize,
  ImageLora_Deserialize,
  ImageSettings_Deserialize,
  PlaygroundEntry_Serialize,
} from "@/api/generated/bindings";

export type PlaygroundEntry = PlaygroundEntry_Serialize;
export type PlaygroundImage = PlaygroundEntry["images"][number];
export type PlaygroundAsset = NonNullable<PlaygroundImage["asset"]>;

export const historyPageSize = 30;
export const pickerPageSize = 60;

export const playgroundKeys = {
  history: ["playground", "history"] as const,
  picker: ["playground", "history", "picker"] as const,
  capabilities: (modelId: string | null) => ["playground", "capabilities", modelId] as const,
  upscalers: [...queryKeys.localImages.all, "upscaler"] as const,
};

export function historyQuery(api: ApiClient) {
  return infiniteQueryOptions({
    queryKey: playgroundKeys.history,
    initialPageParam: null as number | null,
    queryFn: ({ pageParam }) => api.call("playgroundHistoryList", { limit: historyPageSize, before: pageParam }),
    getNextPageParam: (page) => (page.entries.length === historyPageSize ? page.entries.at(-1)?.created_at : undefined),
  });
}

export function pickerQuery(api: ApiClient) {
  return queryOptions({
    queryKey: playgroundKeys.picker,
    queryFn: () => api.call("playgroundHistoryList", { limit: pickerPageSize, before: null }),
  });
}

export function capabilitiesQuery(api: ApiClient, modelId: string | null) {
  return queryOptions({
    queryKey: playgroundKeys.capabilities(modelId),
    queryFn: modelId === null ? skipToken : () => api.call("imageCapabilities", { target: { type: "model", model_id: modelId } }),
  });
}

export function upscalersQuery(api: ApiClient) {
  return queryOptions({ queryKey: playgroundKeys.upscalers, queryFn: () => api.call("sdUpscalersList") });
}

/** Loaded pages as one list, oldest first: each page is newest first, and a refetch can repeat an entry. */
export function historyEntries(pages: readonly { entries: readonly PlaygroundEntry[] }[] | undefined): PlaygroundEntry[] {
  const seen = new Set<string>();
  const entries: PlaygroundEntry[] = [];
  for (const page of pages ?? []) {
    for (const entry of page.entries) {
      if (seen.has(entry.id)) continue;
      seen.add(entry.id);
      entries.push(entry);
    }
  }
  return entries.reverse();
}

export interface PlaygroundDraft {
  size: string | null;
  steps: number | null;
  cfg_scale: number | null;
  sampler: string | null;
  scheduler: string | null;
  seed: number | null;
  count: number | null;
  quality: string | null;
  style: string | null;
  hires_enabled: boolean;
  hires_upscaler: string | null;
  hires_scale: number | null;
  hires_steps: number | null;
  hires_denoising_strength: number | null;
  loras: ImageLora_Deserialize[];
}

export const defaultStrength = 0.6;

export interface StartImage {
  assetId: string;
  url: string;
  strength: number;
}

type Present<T> = { [K in keyof T]?: NonNullable<T[K]> };

function present<T extends object>(value: T): Present<T> {
  const kept: Partial<Record<keyof T, unknown>> = {};
  for (const key of Object.keys(value) as (keyof T)[]) {
    const item = value[key];
    if (item !== null && item !== undefined && item !== "") kept[key] = item;
  }
  return kept as Present<T>;
}

export function buildGenerateRequest(input: {
  modelId: string;
  prompt: string;
  negative: string;
  capabilities: ImageCapabilities;
  draft: PlaygroundDraft;
  startImage: StartImage | null;
}): ImageGenerateRequest_Deserialize {
  const { capabilities, draft, startImage } = input;
  const local = capabilities.local;
  const size = draft.size ?? capabilities.default_size;
  return {
    request_id: crypto.randomUUID(),
    model_id: input.modelId,
    prompt: input.prompt.trim(),
    settings: present({
      ...(local
        ? {
            steps: draft.steps,
            cfg_scale: draft.cfg_scale,
            sampler: draft.sampler,
            scheduler: draft.scheduler,
            seed: draft.seed,
            hires_enabled: draft.hires_enabled,
            hires_upscaler: draft.hires_upscaler,
            hires_scale: draft.hires_scale,
            hires_steps: draft.hires_steps,
            hires_denoising_strength: draft.hires_denoising_strength,
          }
        : {}),
      size,
      negative_prompt: capabilities.negative_prompt ? input.negative.trim() : null,
      denoising_strength: startImage?.strength ?? null,
    }),
    input_images: startImage ? [startImage.assetId] : [],
    loras: local ? draft.loras : [],
    ...present({
      size,
      quality: capabilities.quality.length > 0 ? draft.quality : null,
      style: capabilities.styles.length > 0 ? draft.style : null,
      count: draft.count === null ? null : Math.min(draft.count, capabilities.max_count),
    }),
    source: "playground",
    output: { kind: "retained" },
  };
}

function record(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value) ? (value as Record<string, unknown>) : {};
}

const text = (value: unknown) => (typeof value === "string" ? value : null);
const number = (value: unknown) => (typeof value === "number" ? value : null);
const flag = (value: unknown) => (typeof value === "boolean" ? value : null);

function parseParams(entry: PlaygroundEntry): Record<string, unknown> {
  try {
    return record(JSON.parse(entry.params_json));
  } catch {
    return {};
  }
}

export function entryShape(entry: PlaygroundEntry): { size: string | null; count: number } {
  const params = parseParams(entry);
  return {
    size: text(params.size) ?? text(record(params.settings).size) ?? text(record(params.advancedModelSettings).sdSize),
    count: number(params.count) ?? number(params.n) ?? 1,
  };
}

export function canRegenerate(entry: PlaygroundEntry): boolean {
  return entry.status !== "pending" && entry.upscale_of === null && parseParams(entry).format !== "upscale-v1";
}

const settingKeys = {
  steps: true,
  cfg_scale: true,
  sampler: true,
  scheduler: true,
  negative_prompt: true,
  denoising_strength: true,
  image_cfg_scale: true,
  distilled_guidance: true,
  eta: true,
  flow_shift: true,
  size: true,
  vae_tiling_enabled: true,
  vae_tile_size_x: true,
  vae_tile_size_y: true,
  vae_tile_overlap: true,
  auto_resize_reference_images: true,
  increase_reference_index: true,
  hires_enabled: true,
  hires_upscaler: true,
  hires_scale: true,
  hires_width: true,
  hires_height: true,
  hires_steps: true,
  hires_denoising_strength: true,
  slg_scale: true,
  slg_layers: true,
  slg_layer_start: true,
  slg_layer_end: true,
  cache_mode: true,
  cache_option: true,
  offload_mode: true,
  extra_prompt: true,
  prompt_writer_instructions: true,
} satisfies Record<Exclude<keyof ImageSettings_Deserialize, "seed" | "base_loras">, true>;

function generatedSettings(raw: unknown): ImageSettings_Deserialize {
  const source = record(raw);
  const kept: Record<string, unknown> = {};
  for (const key of Object.keys(settingKeys)) {
    const item = source[key];
    if (item !== null && item !== undefined) kept[key] = item;
  }
  return kept as ImageSettings_Deserialize;
}

function importedSettings(advanced: Record<string, unknown>, negative: string | null): ImageSettings_Deserialize {
  return present({
    size: text(advanced.sdSize),
    steps: number(advanced.sdSteps),
    cfg_scale: number(advanced.sdCfgScale),
    sampler: text(advanced.sdSampler),
    scheduler: text(advanced.sdScheduler),
    negative_prompt: negative ?? text(advanced.sdNegativePrompt),
    hires_enabled: flag(advanced.sdHiresEnabled),
    hires_upscaler: text(advanced.sdHiresUpscaler),
    hires_scale: number(advanced.sdHiresScale),
    hires_steps: number(advanced.sdHiresSteps),
    hires_denoising_strength: number(advanced.sdHiresDenoisingStrength),
  });
}

function lorasOf(value: unknown): ImageLora_Deserialize[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item): ImageLora_Deserialize[] => {
    const lora = record(item);
    if (typeof lora.path !== "string") return [];
    return [
      {
        path: lora.path,
        multiplier: number(lora.multiplier),
        is_high_noise: lora.is_high_noise === true || lora.isHighNoise === true,
        keywords: Array.isArray(lora.keywords) ? lora.keywords.filter((word): word is string => typeof word === "string") : [],
      },
    ];
  });
}

/** The entry's own settings and prompt as a new run on `modelId`, without its seed or starting image. */
export function regenerateRequest(entry: PlaygroundEntry, modelId: string): ImageGenerateRequest_Deserialize {
  const params = parseParams(entry);
  const settings =
    entry.origin === "imported"
      ? importedSettings(record(params.advancedModelSettings), entry.negative_prompt)
      : generatedSettings(params.settings);
  return {
    request_id: crypto.randomUUID(),
    model_id: modelId,
    prompt: entry.prompt,
    settings,
    input_images: [],
    loras: lorasOf(params.loras),
    ...present({
      size: text(params.size) ?? settings.size ?? null,
      quality: text(params.quality),
      style: text(params.style),
      count: number(params.count) ?? number(params.n),
    }),
    source: "playground",
    output: { kind: "retained" },
  };
}

export function useFailureText(): (error: unknown) => string {
  const { t } = useTranslation("common");
  return (error) => t(`errors.code.${isApiFailure(error) ? error.code : "internal"}`);
}
