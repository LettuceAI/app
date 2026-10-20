import type { MemoryCategory, MemoryCycleActionKind, MemoryFailureCode, MemoryGateReason, MemoryOrigin, MemoryRunMode } from "@/api/generated/bindings";

export const memoryCategories: readonly MemoryCategory[] = [
  "character_trait", "relationship", "plot_event", "world_detail", "preference", "other",
  "milestone", "boundary", "profile", "routine", "episodic", "emotional_snapshot",
];

export const memoryOrigins: readonly MemoryOrigin[] = ["model", "user", "import"];

export const memoryRunModes: readonly MemoryRunMode[] = ["auto", "ask_first", "manual"];

export const memoryFailureCodes: readonly MemoryFailureCode[] = [
  "embedding_unavailable", "model_missing", "model_invalid", "prompt_missing", "settings_invalid", "provider_unavailable",
  "provider_rejected", "empty_response", "timed_out", "round_limit", "tool_failed", "storage_failure", "lease_lost", "internal",
];

export const memoryGateReasons: readonly MemoryGateReason[] = ["not_dynamic", "disabled", "nothing_to_summarise", "cycle_running"];

export const memoryActionKinds: readonly MemoryCycleActionKind[] = [
  "created", "duplicate_skipped", "deleted", "soft_deleted", "pinned", "unpinned", "target_not_found", "done", "rejected", "skipped", "stopped_after_done",
];

/** The message window a cycle label names ("61-80"), or null when the label is some other text. */
export function windowOf(label: string): { from: number; to: number } | null {
  const match = /^(\d+)-(\d+)$/.exec(label.trim());
  return match ? { from: Number(match[1]), to: Number(match[2]) } : null;
}

export function dateOf(milliseconds: number | null): Date | null {
  return milliseconds === null ? null : new Date(milliseconds);
}

export function sameInstant(left: Date | null, right: Date | null): boolean {
  return (left?.getTime() ?? null) === (right?.getTime() ?? null);
}
