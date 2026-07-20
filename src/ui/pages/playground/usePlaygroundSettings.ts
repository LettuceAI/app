import { useCallback, useEffect, useMemo, useState } from "react";

import { readSettings, SETTINGS_UPDATED_EVENT } from "../../../core/storage/repo";
import type { AdvancedModelSettings, Model, ProviderCredential } from "../../../core/storage/schemas";
import {
  resolveImageGenerationOptions,
  resolveProviderCredential,
} from "../../../core/image-generation";
import {
  isLocalSdProvider,
  loadPlaygroundDraft,
  PLAYGROUND_LAST_MODEL_KEY,
  savePlaygroundDraft,
  type PlaygroundModelDraft,
} from "../../../core/image-generation/playground";

export type PlaygroundRequestBase = {
  model: string;
  providerId: string;
  credentialId: string;
  advancedModelSettings: AdvancedModelSettings;
  size?: string;
  n?: number;
  quality?: string;
  style?: string;
};

export type PlaygroundSettingsController = {
  loading: boolean;
  models: Model[];
  providers: ProviderCredential[];
  selectedModel: Model | null;
  selectedCredential: ProviderCredential | null;
  isLocal: boolean;
  selectModel: (modelId: string) => void;
  draft: PlaygroundModelDraft;
  updateDraft: (updates: Partial<PlaygroundModelDraft>) => void;
  buildRequestBase: () => PlaygroundRequestBase | null;
};

function draftFromModel(model: Model): PlaygroundModelDraft {
  const advanced = model.advancedModelSettings ?? {};
  const baseLoras = (advanced.sdBaseLoras ?? []).map((lora) => ({
    path: lora.path,
    multiplier: lora.multiplier,
    isHighNoise: lora.isHighNoise ?? undefined,
    keywords: lora.keywords ?? undefined,
  }));
  return {
    size: advanced.sdSize ?? null,
    steps: advanced.sdSteps ?? null,
    cfgScale: advanced.sdCfgScale ?? null,
    sampler: advanced.sdSampler ?? null,
    scheduler: advanced.sdScheduler ?? null,
    seed: null,
    n: null,
    hiresEnabled: advanced.sdHiresEnabled ?? false,
    hiresUpscaler: advanced.sdHiresUpscaler ?? null,
    hiresScale: advanced.sdHiresScale ?? null,
    hiresSteps: advanced.sdHiresSteps ?? null,
    hiresDenoisingStrength: advanced.sdHiresDenoisingStrength ?? null,
    loras: baseLoras,
  };
}

export function usePlaygroundSettings(): PlaygroundSettingsController {
  const [loading, setLoading] = useState(true);
  const [models, setModels] = useState<Model[]>([]);
  const [providers, setProviders] = useState<ProviderCredential[]>([]);
  const [selectedModelId, setSelectedModelId] = useState<string | null>(() => {
    try {
      return localStorage.getItem(PLAYGROUND_LAST_MODEL_KEY);
    } catch {
      return null;
    }
  });
  const [draft, setDraft] = useState<PlaygroundModelDraft>({});

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const settings = await readSettings();
        if (cancelled) return;
        const options = resolveImageGenerationOptions(settings);
        setModels(options.models);
        setProviders(options.providers);
      } finally {
        if (!cancelled) setLoading(false);
      }
    };
    void load();
    const handler = () => void load();
    window.addEventListener(SETTINGS_UPDATED_EVENT, handler);
    return () => {
      cancelled = true;
      window.removeEventListener(SETTINGS_UPDATED_EVENT, handler);
    };
  }, []);

  const selectedModel = useMemo(() => {
    if (models.length === 0) return null;
    return models.find((model) => model.id === selectedModelId) ?? models[0];
  }, [models, selectedModelId]);

  useEffect(() => {
    if (!selectedModel) {
      setDraft({});
      return;
    }
    const base = draftFromModel(selectedModel);
    const stored = loadPlaygroundDraft(selectedModel.id);
    if (!stored) {
      setDraft(base);
      return;
    }
    const merged = { ...base, ...stored };
    if (!stored.loras || stored.loras.length === 0) {
      merged.loras = base.loras;
    }
    setDraft(merged);
  }, [selectedModel?.id]);

  const selectModel = useCallback((modelId: string) => {
    setSelectedModelId(modelId);
    try {
      localStorage.setItem(PLAYGROUND_LAST_MODEL_KEY, modelId);
    } catch {
      return;
    }
  }, []);

  const updateDraft = useCallback(
    (updates: Partial<PlaygroundModelDraft>) => {
      setDraft((current) => {
        const next = { ...current, ...updates };
        if (selectedModel) savePlaygroundDraft(selectedModel.id, next);
        return next;
      });
    },
    [selectedModel?.id],
  );

  const selectedCredential = useMemo(() => {
    if (!selectedModel) return null;
    return resolveProviderCredential(
      providers,
      selectedModel.providerId,
      selectedModel.providerLabel,
    );
  }, [providers, selectedModel]);

  const isLocal = isLocalSdProvider(selectedModel?.providerId);

  const buildRequestBase = useCallback((): PlaygroundRequestBase | null => {
    if (!selectedModel || !selectedCredential) return null;
    const advanced: AdvancedModelSettings = {
      ...(selectedModel.advancedModelSettings ?? {}),
    };
    advanced.sdBaseLoras = null;
    if (draft.size?.trim()) advanced.sdSize = draft.size.trim();
    if (draft.steps != null) advanced.sdSteps = draft.steps;
    if (draft.cfgScale != null) advanced.sdCfgScale = draft.cfgScale;
    if (draft.sampler?.trim()) advanced.sdSampler = draft.sampler.trim();
    if (draft.scheduler?.trim()) advanced.sdScheduler = draft.scheduler.trim();
    if (draft.seed != null) advanced.sdSeed = draft.seed;
    advanced.sdHiresEnabled = draft.hiresEnabled ?? false;
    if (draft.hiresUpscaler?.trim()) advanced.sdHiresUpscaler = draft.hiresUpscaler.trim();
    if (draft.hiresScale != null) advanced.sdHiresScale = draft.hiresScale;
    if (draft.hiresSteps != null) advanced.sdHiresSteps = draft.hiresSteps;
    if (draft.hiresDenoisingStrength != null) {
      advanced.sdHiresDenoisingStrength = draft.hiresDenoisingStrength;
    }
    return {
      model: selectedModel.name,
      providerId: selectedModel.providerId,
      credentialId: selectedCredential.id,
      advancedModelSettings: advanced,
      size: draft.size?.trim() || undefined,
      n: draft.n ?? undefined,
      quality: draft.quality ?? undefined,
      style: draft.style ?? undefined,
    };
  }, [selectedModel, selectedCredential, draft]);

  return {
    loading,
    models,
    providers,
    selectedModel,
    selectedCredential,
    isLocal,
    selectModel,
    draft,
    updateDraft,
    buildRequestBase,
  };
}
