import { personaChoicesQuery } from "@/entities/persona";
import { infiniteQueryOptions, queryOptions } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { ConversationPage, ConversationSummary, ConversationView, MessageCandidatePage, MessageCandidateView, MessagePage, MessageWindow } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

export interface TimelinePage {
  view: ConversationView | null;
  page: MessagePage;
}

export function timelineQuery(api: ApiClient, conversationId: string) {
  return infiniteQueryOptions({
    queryKey: queryKeys.conversations.messages(conversationId),
    queryFn: async ({ pageParam }): Promise<TimelinePage> => {
      if (pageParam === null) {
        const view = await api.call("conversationOpen", { conversation_id: conversationId });
        return { view, page: view.messages };
      }
      const page = await api.call("conversationMessages", { conversation_id: conversationId, before_cursor: pageParam, limit: null });
      return { view: null, page };
    },
    initialPageParam: null as string | null,
    getNextPageParam: (last) => last.page.next_cursor,
  });
}

export function settingsKey(conversationId: string) {
  return [...queryKeys.conversations.detail(conversationId), "settings"] as const;
}

export function settingsQuery(api: ApiClient, conversationId: string) {
  return queryOptions({
    queryKey: settingsKey(conversationId),
    queryFn: () => api.call("conversationSettingsGet", { conversation_id: conversationId }),
  });
}

export function memoryQuery(api: ApiClient, conversationId: string) {
  return queryOptions({
    queryKey: queryKeys.memory.detail(conversationId),
    queryFn: () => api.call("memoryGet", { conversation_id: conversationId }),
  });
}

export function metricsQuery(api: ApiClient, conversationId: string, messageId: string) {
  return queryOptions({
    queryKey: [...queryKeys.conversations.detail(conversationId), "metrics", messageId] as const,
    queryFn: () => api.call("llmMetricsForMessage", { conversation_id: conversationId, message_id: messageId }),
  });
}

export function usageQuery(api: ApiClient, conversationId: string, messageId: string) {
  return queryOptions({
    queryKey: [...queryKeys.conversations.detail(conversationId), "usage", messageId] as const,
    queryFn: () => api.call("messageUsage", { conversation_id: conversationId, message_id: messageId }),
  });
}

export function promptSnapshotQuery(api: ApiClient, conversationId: string, messageId: string) {
  return queryOptions({
    queryKey: [...queryKeys.conversations.detail(conversationId), "prompt-snapshot", messageId] as const,
    queryFn: () => api.call("messagePromptSnapshot", { message_id: messageId }),
    retry: false,
  });
}

export function branchesQuery(api: ApiClient, conversationId: string) {
  return queryOptions({
    queryKey: [...queryKeys.conversations.detail(conversationId), "branches"] as const,
    queryFn: () => api.call("conversationBranches", { conversation_id: conversationId }),
  });
}

export function allChatsQuery(api: ApiClient) {
  return queryOptions({
    queryKey: [...queryKeys.conversations.lists(), "all"] as const,
    queryFn: async (): Promise<ConversationSummary[]> => {
      const all: ConversationSummary[] = [];
      let cursor: string | null = null;
      do {
        const page: ConversationPage = await api.call("conversationsList", { kind: null, character_id: null, source_group_id: null, lifecycle: null, cursor, limit: null });
        all.push(...page.items);
        cursor = page.next_cursor;
      } while (cursor !== null);
      return all;
    },
  });
}

export function windowQuery(api: ApiClient, conversationId: string, anchorId: string | null) {
  return queryOptions({
    queryKey: [...queryKeys.conversations.messages(conversationId), "window", anchorId] as const,
    queryFn: (): Promise<MessageWindow> =>
      api.call("conversationMessagesAround", { conversation_id: conversationId, message_id: anchorId ?? "", before: 20, after: 20 }),
    enabled: anchorId !== null,
  });
}

export function modelsQuery(api: ApiClient) {
  return queryOptions({ queryKey: queryKeys.models.list(), queryFn: () => api.call("modelsList") });
}

export function accountsQuery(api: ApiClient) {
  return queryOptions({ queryKey: queryKeys.providers.accounts(), queryFn: () => api.call("providerAccountsList") });
}

export function candidatesQuery(api: ApiClient, conversationId: string, messageId: string, count: number) {
  return queryOptions({
    queryKey: [...queryKeys.conversations.detail(conversationId), "candidates", messageId, count] as const,
    queryFn: async (): Promise<MessageCandidateView[]> => {
      const all: MessageCandidateView[] = [];
      let cursor: string | null = null;
      do {
        const page: MessageCandidatePage = await api.call("messageCandidates", { message_id: messageId, cursor });
        all.push(...page.items);
        cursor = page.next_cursor;
      } while (cursor !== null);
      return all;
    },
  });
}

export function personasQuery(api: ApiClient, _conversationId: string) {
  return personaChoicesQuery(api);
}

export function helpMeReplyEnabledQuery(api: ApiClient) {
  return queryOptions({
    queryKey: queryKeys.settings.detail(),
    queryFn: () => api.call("settingsGet"),
    select: (settings) => settings.global.help_me_reply.enabled,
  });
}

export function memoryCyclesQuery(api: ApiClient, conversationId: string) {
  return infiniteQueryOptions({
    queryKey: [...queryKeys.memory.detail(conversationId), "cycles"] as const,
    queryFn: ({ pageParam }) => api.call("memoryCycles", { conversation_id: conversationId, cursor: pageParam, limit: null }),
    initialPageParam: null as string | null,
    getNextPageParam: (last) => last.next_cursor,
  });
}

export function pinnedMessagesQuery(api: ApiClient, conversationId: string) {
  return infiniteQueryOptions({
    queryKey: [...queryKeys.conversations.detail(conversationId), "pinned"] as const,
    queryFn: ({ pageParam }) => api.call("conversationPinnedMessages", { conversation_id: conversationId, cursor: pageParam, limit: null }),
    initialPageParam: null as string | null,
    getNextPageParam: (last) => last.next_cursor,
  });
}
