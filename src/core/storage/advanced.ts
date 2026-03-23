import { storageBridge } from "./files";
import { readSettings } from "./repo";

export interface AdvancedSettings {
  summarisationModelId?: string;
  creationHelperEnabled?: boolean;
  creationHelperModelId?: string;
  sceneWriterModelId?: string;
  helpMeReplyEnabled?: boolean;
  manualModeContextWindow?: number;
  embeddingMaxTokens?: number; // 1024, 2048, or 4096
  accessibility?: {
    send: { enabled: boolean; volume: number };
    success: { enabled: boolean; volume: number };
    failure: { enabled: boolean; volume: number };
  };
  dynamicMemory?: {
    enabled: boolean;
    summaryMessageInterval: number;
    maxEntries: number;
    minSimilarityThreshold: number;
    retrievalLimit: number;
    retrievalStrategy: "smart" | "cosine";
    hotMemoryTokenBudget: number;
    decayRate: number;
    coldThreshold: number;
    contextEnrichmentEnabled?: boolean;
  };
  groupDynamicMemory?: {
    enabled: boolean;
    summaryMessageInterval: number;
    maxEntries: number;
    minSimilarityThreshold: number;
    retrievalLimit: number;
    retrievalStrategy: "smart" | "cosine";
    hotMemoryTokenBudget: number;
    decayRate: number;
    coldThreshold: number;
    contextEnrichmentEnabled?: boolean;
  };
}

/**
 * Read the current advanced settings
 */
export async function readAdvancedSettings(): Promise<AdvancedSettings> {
  const settings = await readSettings();
  return settings.advancedSettings || {};
}

/**
 * Update the entire advanced settings object
 */
export async function updateAdvancedSettings(settings: AdvancedSettings): Promise<void> {
  await storageBridge.settingsSetAdvanced(settings);
}

/**
 * Update a single field in advanced settings
 */
export async function updateAdvancedSetting<K extends keyof AdvancedSettings>(
  key: K,
  value: AdvancedSettings[K],
): Promise<void> {
  const current = await readAdvancedSettings();
  const newSettings: AdvancedSettings = {
    ...current,
    [key]: value,
  };
  await storageBridge.settingsSetAdvanced(newSettings);
}
