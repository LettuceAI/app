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

export function audioProvidersQuery(api: ApiClient) {
  return queryOptions({ queryKey: queryKeys.audioProviders.all, queryFn: () => api.call("audioProvidersList") });
}

export function audioVoicesQuery(api: ApiClient, providerId: string) {
  return queryOptions({ queryKey: [...queryKeys.audioProviders.all, "voices", providerId] as const, queryFn: () => api.call("audioProviderVoices", { provider_id: providerId }) });
}

/** The character as the edit page loads it: the stored view and the lorebooks bound to it. */
export function characterEditQuery(api: ApiClient, characterId: string) {
  return queryOptions({
    queryKey: [...queryKeys.characters.all, "edit", characterId] as const,
    queryFn: async () => {
      const [view, bindings] = await Promise.all([api.call("characterGet", { character_id: characterId }), api.call("characterLorebooksList", { character_id: characterId })]);
      return { view, bindings };
    },
    staleTime: Infinity,
    gcTime: 0,
    refetchOnWindowFocus: false,
  });
}

export function companionNotesQuery(api: ApiClient, characterId: string) {
  return queryOptions({ queryKey: queryKeys.companionNotes.list(characterId), queryFn: () => api.call("companionNotesList", { character_id: characterId }) });
}

export function soulWriterModelQuery(api: ApiClient) {
  return queryOptions({
    queryKey: queryKeys.settings.detail(),
    queryFn: () => api.call("settingsGet"),
    select: (settings) => settings.global.companion_soul_writer.model_profile_id,
  });
}
