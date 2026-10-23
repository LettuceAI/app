import { queryOptions, useMutation, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import type { ApiClient } from "@/api/client";
import type { PromptInput, PromptPage, PromptSummary, PromptView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { rulesFromCondition, conditionFromRules } from "./rules";
import type { Entry, Kind, Prompt } from "./types";

export const promptListQuery = (api: ApiClient) => queryOptions({ queryKey: [...queryKeys.prompts.all, "management"], queryFn: async () => {
  const items: PromptSummary[] = [];
  let cursor: string | null = null;
  do {
    const page: PromptPage = await api.call("promptsList", { kind: null, lifecycle: "active", limit: 100, cursor });
    items.push(...page.items.filter((item) => item.kind !== "runtime_text"));
    cursor = page.next_cursor;
  } while (cursor);
  return items;
} });
export const promptQuery = (api: ApiClient, id: string) => queryOptions({ queryKey: [...queryKeys.prompts.all, "detail", id], queryFn: () => api.call("promptGet", { prompt_id: id }), enabled: id !== "new" });
export const placeholdersQuery = (api: ApiClient, kind: Kind) => queryOptions({ queryKey: [...queryKeys.prompts.all, "placeholders", kind], queryFn: () => api.call("promptPlaceholders", { kind }), staleTime: Infinity });

export function blankEntry(position: Entry["position"] = "in_chat"): Entry {
  const rules = { include: [], exclude: [] };
  return { id: crypto.randomUUID(), name: "", role: "system", content: "", enabled: true, position, depth: 0, minMessages: null, everyTurns: null, required: false, rules, imageSlot: null, persisted: false, systemPrompt: false, originalCondition: null, originalRules: JSON.stringify(rules) };
}
export function fromView(view: PromptView): Prompt {
  return { id: view.prompt.id, name: view.prompt.name, kind: view.prompt.kind as Kind, origin: view.prompt.origin, condense: view.condense, behavior: view.behavior, revision: view.prompt.revision, sourceId: null,
    entries: view.entries.map((entry) => {
      const rules = rulesFromCondition(entry.condition);
      return { id: entry.id, name: entry.name, role: entry.role, content: entry.content, enabled: entry.enabled, position: entry.position, depth: entry.depth, minMessages: entry.conditional_min_messages, everyTurns: entry.interval_turns, required: entry.system_prompt && view.behavior === "legacy_v1", rules, imageSlot: entry.image_slot, persisted: true, systemPrompt: entry.system_prompt, originalCondition: entry.condition, originalRules: JSON.stringify(rules) };
    }),
  };
}
export function newPrompt(source?: PromptView, name = ""): Prompt {
  const id = crypto.randomUUID();
  if (!source) return { id, name, kind: "direct_chat", origin: { type: "user" }, condense: false, behavior: "legacy_v1", revision: null, sourceId: null, entries: [blankEntry("relative")] };
  const draft = fromView(source);
  return { ...draft, id, name, revision: null, sourceId: source.prompt.id, origin: { type: "derived", source_id: source.prompt.id, source_name: source.prompt.name, source_deleted: false }, entries: draft.entries.map((entry) => ({ ...entry, id: crypto.randomUUID(), persisted: false, enabled: entry.enabled || entry.required, required: false, systemPrompt: false })) };
}
export function toInput(draft: Prompt, preview = false): PromptInput {
  return { name: draft.name.trim(), kind: draft.kind, condense: draft.condense, behavior: draft.behavior, entries: draft.entries.map((entry) => ({
    entry_id: preview || entry.persisted ? entry.id : null, name: entry.name, role: entry.role, content: entry.content, enabled: entry.enabled,
    position: entry.position, depth: entry.depth, conditional_min_messages: entry.minMessages, interval_turns: entry.everyTurns,
    system_prompt: entry.systemPrompt, condition: JSON.stringify(entry.rules) === entry.originalRules ? entry.originalCondition : conditionFromRules(entry.rules), image_slot: entry.imageSlot,
  })) };
}
export function usePromptAction() {
  const client = useQueryClient();
  const operations = useRef(new Map<string, string>());
  const operation = (key: string) => {
    let id = operations.current.get(key);
    if (!id) { id = crypto.randomUUID(); operations.current.set(key, id); }
    return id;
  };
  const action = useMutation({ mutationFn: (run: () => Promise<unknown>) => run(), onSuccess: async () => {
    operations.current.clear();
    await Promise.all([queryKeys.prompts.all, queryKeys.settings.all, queryKeys.characters.all, queryKeys.conversations.all].map((queryKey) => client.invalidateQueries({ queryKey })));
  } });
  return { ...action, operation };
}

export function usePromptDraft<T>(value: T) {
  const [settled, setSettled] = useState(value);
  useEffect(() => { const timer = setTimeout(() => setSettled(value), 180); return () => clearTimeout(timer); }, [value]);
  return settled;
}
