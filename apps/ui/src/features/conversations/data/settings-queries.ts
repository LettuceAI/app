import { queryOptions } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { CompanionNoteView, ConversationSummary, SettingChoice } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

async function allPages<T>(fetchPage: (cursor: string | null) => Promise<{ items: T[]; next_cursor: string | null }>): Promise<T[]> {
  const all: T[] = [];
  let cursor: string | null = null;
  do {
    const page: { items: T[]; next_cursor: string | null } = await fetchPage(cursor);
    all.push(...page.items);
    cursor = page.next_cursor;
  } while (cursor !== null);
  return all;
}

export function characterQuery(api: ApiClient, characterId: string | null) {
  return queryOptions({
    queryKey: [...queryKeys.characters.all, "detail", characterId] as const,
    queryFn: () => api.call("characterGet", { character_id: characterId ?? "" }),
    enabled: characterId !== null,
  });
}

export function messageCountQuery(api: ApiClient, conversationId: string) {
  return queryOptions({
    queryKey: [...queryKeys.conversations.detail(conversationId), "count"] as const,
    queryFn: async () => (await api.call("conversationMessageCount", { conversation_id: conversationId })).count,
  });
}

export function sourceChatsQuery(api: ApiClient, characterId: string | null, groupId: string | null) {
  return queryOptions({
    queryKey: [...queryKeys.conversations.lists(), groupId ? "group" : "character", groupId ?? characterId] as const,
    queryFn: (): Promise<ConversationSummary[]> =>
      allPages((cursor) =>
        api.call("conversationsList", groupId
          ? { kind: "group", character_id: null, source_group_id: groupId, lifecycle: "all", cursor, limit: null }
          : { kind: "direct", character_id: characterId, source_group_id: null, lifecycle: "all", cursor, limit: null }),
      ),
    enabled: characterId !== null || groupId !== null,
  });
}

export function libraryCastQuery(api: ApiClient) {
  return queryOptions({
    queryKey: queryKeys.characters.list(),
    queryFn: () => allPages((cursor) => api.call("charactersList", { cursor, limit: null })),
  });
}

export function participationQuery(api: ApiClient, conversationId: string, enabled: boolean) {
  return queryOptions({
    queryKey: [...queryKeys.conversations.detail(conversationId), "participation"] as const,
    queryFn: () => api.call("conversationParticipationStats", { conversation_id: conversationId }),
    enabled,
  });
}

export function characterChatsQuery(api: ApiClient, characterId: string | null) {
  return sourceChatsQuery(api, characterId, null);
}

export function notesQuery(api: ApiClient, characterId: string | null) {
  return queryOptions({
    queryKey: [...queryKeys.characters.all, "notes", characterId] as const,
    queryFn: (): Promise<CompanionNoteView[]> => api.call("companionNotesList", { character_id: characterId ?? "" }),
    enabled: characterId !== null,
  });
}

export function effectivePersonaId(choice: SettingChoice, defaultId: string | null): string | null {
  return choice.id ?? (choice.source === "app_default" ? defaultId : null);
}
