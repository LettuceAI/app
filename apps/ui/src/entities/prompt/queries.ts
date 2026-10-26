import { queryOptions } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { PromptKind, PromptPage, PromptSummary } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

export function promptsQuery(api: ApiClient, kind: PromptKind) {
  return queryOptions({
    queryKey: queryKeys.prompts.list(kind),
    queryFn: async (): Promise<PromptSummary[]> => {
      const items: PromptSummary[] = [];
      let cursor: string | null = null;
      do {
        const page: PromptPage = await api.call("promptsList", {
          kind,
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
