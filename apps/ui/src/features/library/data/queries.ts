import { infiniteQueryOptions, queryOptions } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type {
  CharacterView_Serialize,
  LorebookView,
  LorebookProjectSummary,
  PersonaDefaultView,
  PersonaView_Serialize,
} from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

export type EntryKind = "character" | "persona" | "lorebook";
export type Filter = "all" | "characters" | "personas" | "lorebooks" | "images" | "audio";
export type EntryFilter = Exclude<Filter, "images" | "audio">;
export type Entry = {
  kind: EntryKind;
  id: string;
  name: string;
  description: string;
  image: string | null;
  isDefault?: boolean;
  draft?: LorebookProjectSummary;
};
export type EntryView =
  | { kind: "character"; view: CharacterView_Serialize }
  | { kind: "persona"; view: PersonaView_Serialize }
  | { kind: "lorebook"; view: LorebookView };
export interface EntryPage {
  items: Entry[];
  next: string | null;
  defaultPersona: PersonaDefaultView | undefined;
}
const libraryPageSize = 24;

export function kindEntriesQuery(api: ApiClient, kind: EntryKind, query: string) {
  return infiniteQueryOptions({
    queryKey: [...queryKeys.library.all, "entries", kind, query] as const,
    initialPageParam: null as string | null,
    queryFn: async ({ pageParam }): Promise<EntryPage> => {
      if (kind === "character") {
        const characters = await api.call("charactersList", {
          cursor: pageParam,
          limit: libraryPageSize,
          query: query || null,
          search_description: true,
        });
        return {
          items: characters.items.map(
            (item): Entry => ({
              kind: "character",
              id: item.id,
              name: item.name,
              description: item.description ?? "",
              image: item.avatar?.preview_url ?? item.avatar?.url ?? null,
            }),
          ),
          next: characters.next_cursor ?? null,
          defaultPersona: undefined,
        };
      }
      if (kind === "persona") {
        const personas = await api.call("personasList", {
          cursor: pageParam,
          limit: libraryPageSize,
          query: query || null,
          sort: null,
          search_description: true,
          search_nickname: true,
        });
        return {
          items: personas.items.map(
            (item): Entry => ({
              kind: "persona",
              id: item.id,
              name: item.title,
              description: item.intro?.trim() || item.description,
              image: item.avatar?.preview_url ?? item.avatar?.url ?? null,
              isDefault: personas.default_persona.persona_id === item.id,
            }),
          ),
          next: personas.next_cursor ?? null,
          defaultPersona: personas.default_persona,
        };
      }
      const lorebooks = await api.call("lorebooksList", {
        cursor: pageParam,
        limit: libraryPageSize,
        query: query || null,
        lifecycle: "active",
      });
      return {
        items: lorebooks.items.map(
          (item): Entry => ({
            kind: "lorebook",
            id: item.id,
            name: item.name,
            description: "",
            image: item.icon?.preview_url ?? item.icon?.url ?? null,
          }),
        ),
        next: lorebooks.next_cursor ?? null,
        defaultPersona: undefined,
      };
    },
    getNextPageParam: (last) => last.next ?? undefined,
  });
}
export function entryQuery(api: ApiClient, entry: Entry) {
  return queryOptions({
    queryKey: [
      entry.kind === "character"
        ? "characters"
        : entry.kind === "persona"
          ? "personas"
          : "lorebooks",
      "detail",
      entry.id,
      "library",
    ] as const,
    queryFn: async (): Promise<EntryView> => {
      if (entry.kind === "character")
        return {
          kind: "character",
          view: await api.call("characterGet", { character_id: entry.id }),
        };
      if (entry.kind === "persona")
        return { kind: "persona", view: await api.call("personaGet", { persona_id: entry.id }) };
      return { kind: "lorebook", view: await api.call("lorebookGet", { lorebook_id: entry.id }) };
    },
  });
}
export type LibrarySearch = {
  create?: EntryKind;
  item?: string;
  kind?: EntryKind;
  media?: string;
  draft?: string;
};
export function validateLibrarySearch(search: Record<string, unknown>): LibrarySearch {
  const kind = (value: unknown): value is EntryKind =>
    value === "character" || value === "persona" || value === "lorebook";
  if (typeof search.draft === "string" && search.draft) return { draft: search.draft };
  if (kind(search.create)) return { create: search.create };
  if (typeof search.item === "string" && kind(search.kind))
    return { item: search.item, kind: search.kind };
  if (typeof search.media === "string") return { media: search.media };
  return {};
}

export function countsQuery(api: ApiClient) {
  return queryOptions({
    queryKey: queryKeys.library.counts(),
    queryFn: () => api.call("libraryCounts"),
  });
}
