import { queryOptions } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { LorebookSummary } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

export function lorebooksQuery(api: ApiClient) {
  return queryOptions({
    queryKey: queryKeys.lorebooks.list(),
    queryFn: async (): Promise<LorebookSummary[]> => {
      const items: LorebookSummary[] = [];
      let cursor: string | null = null;
      do {
        const page: { items: LorebookSummary[]; next_cursor: string | null } =
          await api.call("lorebooksList", {
            query: null,
            lifecycle: "active",
            cursor,
            limit: null,
          });
        items.push(...page.items);
        cursor = page.next_cursor;
      } while (cursor !== null);
      return items;
    },
  });
}
