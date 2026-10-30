import { queryOptions } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { CharacterPage, CharacterSummary, ConversationPage, ConversationSummary, GroupListItem, GroupPage } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

export function groupsQuery(api: ApiClient) {
  return queryOptions({
    queryKey: queryKeys.groups.list(),
    queryFn: async () => {
      const items: GroupListItem[] = [];
      let cursor: string | null = null;
      do {
        const page: GroupPage = await api.call("groupsList", { query: null, sort: "updated_desc", cursor, limit: 100 });
        items.push(...page.items);
        cursor = page.next_cursor;
      } while (cursor !== null);
      return items.sort((a, b) => b.last_activity_at - a.last_activity_at);
    },
  });
}

export function castQuery(api: ApiClient) {
  return queryOptions({
    queryKey: queryKeys.characters.list(),
    queryFn: async () => {
      const items: CharacterSummary[] = [];
      let cursor: string | null = null;
      do {
        const page: CharacterPage = await api.call("charactersList", { cursor, limit: null });
        items.push(...page.items);
        cursor = page.next_cursor;
      } while (cursor !== null);
      return items;
    },
  });
}

export function groupChatsQuery(api: ApiClient) {
  return queryOptions({
    queryKey: [...queryKeys.conversations.lists(), "groups"],
    queryFn: async () => {
      const items: ConversationSummary[] = [];
      let cursor: string | null = null;
      do {
        const page: ConversationPage = await api.call("conversationsList", { kind: "group", source_group_id: null, character_id: null, lifecycle: "active", cursor, limit: null });
        items.push(...page.items);
        cursor = page.next_cursor;
      } while (cursor !== null);
      return items.sort((a, b) => b.updated_at - a.updated_at);
    },
  });
}

export { personaChoicesQuery as groupPersonasQuery } from "@/entities/persona";

export async function loadGroupEdit(api: ApiClient, id: string) {
  const view = await api.call("groupGet", { group_id: id });
  const [bindings, members] = await Promise.all([
    api.call("groupLorebooksList", { group_id: id }),
    Promise.all(view.draft.members.map((member) => api.call("characterGet", { character_id: member.character_id }))),
  ]);
  return { view, members, lorebookIds: bindings.sort((a, b) => a.ordinal - b.ordinal).map((binding) => binding.lorebook_id) };
}

export function groupEditQuery(api: ApiClient, id: string) {
  return queryOptions({
    queryKey: [...queryKeys.groups.all, "edit", id],
    queryFn: () => loadGroupEdit(api, id),
    staleTime: Infinity,
    gcTime: 0,
  });
}
