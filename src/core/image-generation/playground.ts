import { invoke } from "@tauri-apps/api/core";
import type { AdvancedModelSettings } from "../storage/schemas";

export type PlaygroundGenerationStatus = "pending" | "complete" | "failed" | "cancelled";

export type PlaygroundGenerationImage = {
  assetId: string;
  filePath: string;
  mimeType?: string;
  url?: string | null;
  width?: number | null;
  height?: number | null;
};

export type PlaygroundLoraSelection = {
  path: string;
  multiplier: number;
  isHighNoise?: boolean;
  keywords?: string[];
};

export type PlaygroundGenerationParams = {
  advancedModelSettings?: AdvancedModelSettings | null;
  loras?: PlaygroundLoraSelection[];
  size?: string | null;
  n?: number | null;
  quality?: string | null;
  style?: string | null;
  initImageAssetId?: string | null;
  denoisingStrength?: number | null;
  upscaleOf?: string | null;
};

export type PlaygroundGenerationEntry = {
  id: string;
  createdAt: number;
  providerId: string;
  modelId: string;
  modelName: string;
  prompt: string;
  negativePrompt: string | null;
  seed: number | null;
  params: PlaygroundGenerationParams;
  status: PlaygroundGenerationStatus;
  error: string | null;
  images: PlaygroundGenerationImage[];
};

type PlaygroundGenerationRecord = {
  id: string;
  createdAt: number;
  providerId: string;
  modelId: string;
  modelName: string;
  prompt: string;
  negativePrompt: string | null;
  seed: number | null;
  paramsJson: string;
  status: PlaygroundGenerationStatus;
  error: string | null;
  imagesJson: string;
};

function parseJson<T>(raw: string, fallback: T): T {
  try {
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

function entryFromRecord(record: PlaygroundGenerationRecord): PlaygroundGenerationEntry {
  return {
    id: record.id,
    createdAt: record.createdAt,
    providerId: record.providerId,
    modelId: record.modelId,
    modelName: record.modelName,
    prompt: record.prompt,
    negativePrompt: record.negativePrompt,
    seed: record.seed,
    params: parseJson<PlaygroundGenerationParams>(record.paramsJson, {}),
    status: record.status,
    error: record.error,
    images: parseJson<PlaygroundGenerationImage[]>(record.imagesJson, []),
  };
}

function recordFromEntry(entry: PlaygroundGenerationEntry): PlaygroundGenerationRecord {
  return {
    id: entry.id,
    createdAt: entry.createdAt,
    providerId: entry.providerId,
    modelId: entry.modelId,
    modelName: entry.modelName,
    prompt: entry.prompt,
    negativePrompt: entry.negativePrompt,
    seed: entry.seed,
    paramsJson: JSON.stringify(entry.params ?? {}),
    status: entry.status,
    error: entry.error,
    imagesJson: JSON.stringify(entry.images ?? []),
  };
}

export async function listPlaygroundHistory(
  limit?: number,
  before?: number,
): Promise<PlaygroundGenerationEntry[]> {
  const records = await invoke<PlaygroundGenerationRecord[]>("playground_history_list", {
    limit: limit ?? null,
    before: before ?? null,
  });
  return records.map(entryFromRecord);
}

export async function savePlaygroundHistoryEntry(entry: PlaygroundGenerationEntry): Promise<void> {
  await invoke("playground_history_save", { entry: recordFromEntry(entry) });
}

export async function deletePlaygroundHistoryEntry(
  id: string,
  deleteImages: boolean,
): Promise<void> {
  await invoke("playground_history_delete", { id, deleteImages });
}

export const PLAYGROUND_SEED_MAX = 2_147_483_647;

export function randomPlaygroundSeed(): number {
  const buffer = new Uint32Array(1);
  crypto.getRandomValues(buffer);
  return buffer[0] % (PLAYGROUND_SEED_MAX + 1);
}

export function isLocalSdProvider(providerId: string | null | undefined): boolean {
  return providerId === "sdcpp";
}

export type PlaygroundModelDraft = {
  size?: string | null;
  steps?: number | null;
  cfgScale?: number | null;
  sampler?: string | null;
  scheduler?: string | null;
  seed?: number | null;
  n?: number | null;
  quality?: string | null;
  style?: string | null;
  denoisingStrength?: number | null;
  hiresEnabled?: boolean;
  hiresUpscaler?: string | null;
  hiresScale?: number | null;
  hiresSteps?: number | null;
  hiresDenoisingStrength?: number | null;
  loras?: PlaygroundLoraSelection[];
};

const DRAFT_KEY_PREFIX = "playground:settings:";
export const PLAYGROUND_LAST_MODEL_KEY = "playground:settings:last-model";

export function loadPlaygroundDraft(modelId: string): PlaygroundModelDraft | null {
  try {
    const raw = localStorage.getItem(`${DRAFT_KEY_PREFIX}${modelId}`);
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" ? (parsed as PlaygroundModelDraft) : null;
  } catch {
    return null;
  }
}

export function savePlaygroundDraft(modelId: string, draft: PlaygroundModelDraft): void {
  try {
    localStorage.setItem(`${DRAFT_KEY_PREFIX}${modelId}`, JSON.stringify(draft));
  } catch {
    return;
  }
}
