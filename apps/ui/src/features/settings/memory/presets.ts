import type { SettingsDynamicMemorySettings, SettingsMemoryRunMode } from "@/api/generated/bindings";

export const presetIds = ["minimal", "balanced", "comprehensive"] as const;
export type PresetId = (typeof presetIds)[number];

export const tuningKeys = [
  "summary_message_interval",
  "max_entries",
  "retrieval_limit",
  "retrieval_strategy",
  "hot_memory_token_budget",
  "decay_rate_basis_points",
  "cold_threshold_basis_points",
] as const;

export type TuningKey = (typeof tuningKeys)[number];
export type Tuning = Pick<SettingsDynamicMemorySettings, TuningKey>;

export const presets: Record<PresetId, Tuning> = {
  minimal: { summary_message_interval: 30, max_entries: 100, retrieval_limit: 3, retrieval_strategy: "smart", hot_memory_token_budget: 3072, decay_rate_basis_points: 1500, cold_threshold_basis_points: 4000 },
  balanced: { summary_message_interval: 20, max_entries: 200, retrieval_limit: 5, retrieval_strategy: "smart", hot_memory_token_budget: 6144, decay_rate_basis_points: 800, cold_threshold_basis_points: 3000 },
  comprehensive: { summary_message_interval: 15, max_entries: 400, retrieval_limit: 8, retrieval_strategy: "smart", hot_memory_token_budget: 10240, decay_rate_basis_points: 500, cold_threshold_basis_points: 2000 },
};

/** The preset whose seven values all match, or "custom". The relevance threshold is not part of a profile. */
export function presetOf(value: SettingsDynamicMemorySettings): PresetId | "custom" {
  return presetIds.find((id) => tuningKeys.every((key) => presets[id][key] === value[key])) ?? "custom";
}

export const runModes: readonly SettingsMemoryRunMode[] = ["auto", "ask_first", "manual"];

export const basisPointsToDecimal = (basisPoints: number) => basisPoints / 10000;
export const decimalToBasisPoints = (decimal: number) => Math.round(decimal * 10000);

export const customThresholdBasisPoints = 3500;
export const capacities = [1024, 2048, 4096] as const;
export const defaultCapacity = 4096;
