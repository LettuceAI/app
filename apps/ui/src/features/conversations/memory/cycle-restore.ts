import type { ApiClient } from "@/api/client";
import type { MemoryCategory, MemoryCycleAction, MemoryItemView, MemorySkipReason } from "@/api/generated/bindings";
import type { MemoryWrite } from "./use-memory-write";

export type RestorePlan =
  | { label: "addAnyway" | "addBack"; type: "add"; text: string; category: MemoryCategory | null; observedAt: number | null }
  | { label: "addBack"; type: "warm"; memoryId: string };

const unaddable: readonly MemorySkipReason[] = ["missing_text", "empty_text", "invalid_text"];

const same = (left: string, right: string) => left.trim().toLowerCase() === right.trim().toLowerCase();

/** How a removed or skipped memory from a cycle can come back, or null when it is already in the memories or never had usable text. */
export function restorePlan(action: MemoryCycleAction, items: readonly MemoryItemView[]): RestorePlan | null {
  const present = (text: string) => items.some((item) => same(item.text, text));
  switch (action.kind) {
    case "skipped":
    case "duplicate_skipped": {
      const text = action.proposed_text;
      if (!text?.trim() || present(text)) return null;
      if (action.skip_reason !== null && unaddable.includes(action.skip_reason)) return null;
      return { label: "addAnyway", type: "add", text, category: action.category, observedAt: null };
    }
    case "deleted": {
      const text = action.text;
      if (!text?.trim() || present(text)) return null;
      return { label: "addBack", type: "add", text, category: action.category, observedAt: action.observed_at };
    }
    case "soft_deleted": {
      const item = items.find((entry) => entry.id === action.memory_id);
      if (item) return item.temperature === "cold" ? { label: "addBack", type: "warm", memoryId: item.id } : null;
      const text = action.text;
      if (!text?.trim() || present(text)) return null;
      return { label: "addBack", type: "add", text, category: action.category, observedAt: null };
    }
    default:
      return null;
  }
}

export function restoreCycleAction(api: ApiClient, conversationId: string, write: MemoryWrite, key: string, plan: RestorePlan): Promise<boolean> {
  if (plan.type === "warm") {
    return write(key, (current, operationId) =>
      api.call("memorySetTemperature", { conversation_id: conversationId, memory_id: plan.memoryId, temperature: "hot", expected_revision: current.revision, client_operation_id: operationId }),
    );
  }
  return write(key, (current, operationId) =>
    api.call("memoryAdd", { conversation_id: conversationId, text: plan.text, category: plan.category, observed_at: plan.observedAt, expected_revision: current.revision, client_operation_id: operationId }),
  );
}
