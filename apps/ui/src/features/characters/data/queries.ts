import { queryOptions } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { LibraryInteractionMode, LorebookSummary, PromptKind, PromptSummary } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

async function allPages<T>(fetchPage: (cursor: string | null) => Promise<{ items: T[]; next_cursor: string | null }>): Promise<T[]> {
  const items: T[] = [];
  let cursor: string | null = null;
  do {
    const page = await fetchPage(cursor);
    items.push(...page.items);
    cursor = page.next_cursor;
  } while (cursor !== null);
  return items;
}

/** The draft a new character starts from, with the library's defaults filled in. */
export function draftDefaultQuery(api: ApiClient, mode: LibraryInteractionMode = "roleplay") {
  return queryOptions({ queryKey: queryKeys.characters.draftDefault(mode), queryFn: () => api.call("characterDraftDefault", mode) });
}

export function dynamicMemoryEnabledQuery(api: ApiClient) {
  return queryOptions({
    queryKey: queryKeys.settings.detail(),
    queryFn: () => api.call("settingsGet"),
    select: (settings) => settings.global.dynamic_memory.enabled,
  });
}

export function promptsQuery(api: ApiClient, kind: PromptKind) {
  return queryOptions({
    queryKey: queryKeys.prompts.list(kind),
    queryFn: (): Promise<PromptSummary[]> => allPages((cursor) => api.call("promptsList", { kind, lifecycle: "active", cursor, limit: null })),
  });
}

export function lorebooksQuery(api: ApiClient) {
  return queryOptions({
    queryKey: queryKeys.lorebooks.list(),
    queryFn: (): Promise<LorebookSummary[]> => allPages((cursor) => api.call("lorebooksList", { query: null, lifecycle: "active", cursor, limit: null })),
  });
}
