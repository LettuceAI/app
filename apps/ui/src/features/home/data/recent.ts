import { infiniteQueryOptions } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { ConversationSummary } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

export function recentChatsQuery(api: ApiClient) {
  return infiniteQueryOptions({
    queryKey: [...queryKeys.conversations.lists(), "recent"] as const,
    queryFn: ({ pageParam }) =>
      api.call("conversationsList", { kind: null, character_id: null, source_group_id: null, lifecycle: "active", cursor: pageParam, limit: null }),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.next_cursor,
  });
}

export type RecencyGroup = "today" | "week" | "earlier";

const day = 24 * 60 * 60 * 1000;

/** Chats split into today, this week and earlier, newest first, empty groups left out. */
export function byRecency(chats: readonly ConversationSummary[], now: number): { group: RecencyGroup; chats: ConversationSummary[] }[] {
  const groups: { group: RecencyGroup; chats: ConversationSummary[] }[] = [
    { group: "today", chats: [] },
    { group: "week", chats: [] },
    { group: "earlier", chats: [] },
  ];
  for (const chat of chats) {
    const age = now - chat.updated_at;
    groups[age < day ? 0 : age < 7 * day ? 1 : 2]!.chats.push(chat);
  }
  return groups.filter((entry) => entry.chats.length > 0);
}
