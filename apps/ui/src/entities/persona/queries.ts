import { queryOptions } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { PersonaPage, PersonaSummary } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

export function personaChoicesQuery(api: ApiClient) {
  return queryOptions({
    queryKey: queryKeys.personas.list(),
    queryFn: async () => {
      const items: PersonaSummary[] = [];
      let cursor: string | null = null;
      let defaultId: string | null = null;
      do {
        const page: PersonaPage = await api.call("personasList", { cursor, limit: null, query: null, sort: "name" });
        items.push(...page.items);
        defaultId = page.default_persona.persona_id;
        cursor = page.next_cursor;
      } while (cursor !== null);
      return { items, defaultId };
    },
  });
}
