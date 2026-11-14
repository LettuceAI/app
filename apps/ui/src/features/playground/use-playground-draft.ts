import { useState } from "react";
import type { ModelView } from "@/api/generated/bindings";
import type { ModelChoice } from "@/entities/model";
import { parseModelConfig } from "@/entities/model/model-config";
import type { PlaygroundDraft } from "./playground-data";

const lastModelKey = "playground:lastModel";
const draftKey = (modelId: string) => `playground:settings:${modelId}`;

const emptyDraft: PlaygroundDraft = {
  size: null,
  steps: null,
  cfg_scale: null,
  sampler: null,
  scheduler: null,
  seed: null,
  count: null,
  quality: null,
  style: null,
  hires_enabled: false,
  hires_upscaler: null,
  hires_scale: null,
  hires_steps: null,
  hires_denoising_strength: null,
  loras: [],
};

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    return;
  }
}

function modelDraft(model: ModelView): PlaygroundDraft {
  const { stable_diffusion: sd } = parseModelConfig(model.config);
  return {
    ...emptyDraft,
    size: sd.size ?? null,
    steps: sd.steps ?? null,
    cfg_scale: sd.cfg_scale ?? null,
    sampler: sd.sampler ?? null,
    scheduler: sd.scheduler ?? null,
    hires_enabled: sd.hires_enabled ?? false,
    hires_upscaler: sd.hires_upscaler ?? null,
    hires_scale: sd.hires_scale ?? null,
    hires_steps: sd.hires_steps ?? null,
    hires_denoising_strength: sd.hires_denoising_strength ?? null,
    loras: (sd.base_loras ?? []).map((lora) => ({
      path: lora.path,
      multiplier: lora.multiplier,
      is_high_noise: lora.is_high_noise,
      keywords: lora.keywords,
    })),
  };
}

/** The model's own values with the stored draft on top; an empty stored LoRA list falls back to the model's. */
function initialDraft(model: ModelView): PlaygroundDraft {
  const base = modelDraft(model);
  const raw = read(draftKey(model.id));
  if (raw === null) return base;
  let stored: Partial<PlaygroundDraft>;
  try {
    stored = JSON.parse(raw);
  } catch {
    return base;
  }
  if (typeof stored !== "object" || stored === null || Array.isArray(stored)) return base;
  const merged = { ...base, ...stored };
  if (!Array.isArray(stored.loras) || stored.loras.length === 0) merged.loras = base.loras;
  return merged;
}

export function usePlaygroundDraft(choices: readonly ModelChoice[]) {
  const [selectedId, setSelectedId] = useState<string | null>(() => read(lastModelKey));
  const selected = choices.find((choice) => choice.model.id === selectedId) ?? choices[0] ?? null;
  const modelId = selected?.model.id ?? null;
  const [state, setState] = useState<{ modelId: string | null; draft: PlaygroundDraft }>({ modelId: null, draft: emptyDraft });
  if (state.modelId !== modelId) setState({ modelId, draft: selected ? initialDraft(selected.model) : emptyDraft });

  const selectModel = (id: string) => {
    setSelectedId(id);
    write(lastModelKey, id);
  };
  const update = (patch: Partial<PlaygroundDraft>) => {
    if (modelId === null) return;
    const draft = { ...state.draft, ...patch };
    setState({ modelId, draft });
    write(draftKey(modelId), JSON.stringify(draft));
  };
  const base = selected ? modelDraft(selected.model) : emptyDraft;
  return { selected, draft: state.draft, base, selectModel, update };
}
