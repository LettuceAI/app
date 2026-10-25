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
type Cursors = {
  characters: string | null | false;
  personas: string | null | false;
  lorebooks: string | null | false;
};
export interface EntryPage {
  items: Entry[];
  next: Cursors | null;
  defaultPersona: PersonaDefaultView | undefined;
}
export const libraryPageSize = 24;
export const libraryImagePageSize = 32;

export function entriesQuery(api: ApiClient, filter: EntryFilter, query: string) {
  return infiniteQueryOptions({
    queryKey: [...queryKeys.library.all, "entries", filter, query] as const,
    initialPageParam: {
      characters: filter === "all" || filter === "characters" ? null : false,
      personas: filter === "all" || filter === "personas" ? null : false,
      lorebooks: filter === "all" || filter === "lorebooks" ? null : false,
    } as Cursors,
    queryFn: async ({ pageParam }): Promise<EntryPage> => {
      const limit = filter === "all" ? libraryPageSize / 3 : libraryPageSize;
      const [characters, personas, lorebooks] = await Promise.all([
        pageParam.characters === false
          ? null
          : api.call("charactersList", {
              cursor: pageParam.characters,
              limit,
              query: query || null,
              search_description: true,
            }),
        pageParam.personas === false
          ? null
          : api.call("personasList", {
              cursor: pageParam.personas,
              limit,
              query: query || null,
              sort: null,
              search_description: true,
              search_nickname: true,
            }),
        pageParam.lorebooks === false
          ? null
          : api.call("lorebooksList", {
              cursor: pageParam.lorebooks,
              limit,
              query: query || null,
              lifecycle: "active",
            }),
      ]);
      const next: Cursors = {
        characters: characters?.next_cursor ?? false,
        personas: personas?.next_cursor ?? false,
        lorebooks: lorebooks?.next_cursor ?? false,
      };
      return {
        items: [
          ...(characters?.items ?? []).map(
            (item): Entry => ({
              kind: "character",
              id: item.id,
              name: item.name,
              description: item.description ?? "",
              image: item.avatar?.preview_url ?? item.avatar?.url ?? null,
            }),
          ),
          ...(personas?.items ?? []).map(
            (item): Entry => ({
              kind: "persona",
              id: item.id,
              name: item.title,
              description: item.description,
              image: item.avatar?.preview_url ?? item.avatar?.url ?? null,
              isDefault: personas?.default_persona.persona_id === item.id,
            }),
          ),
          ...(lorebooks?.items ?? []).map(
            (item): Entry => ({
              kind: "lorebook",
              id: item.id,
              name: item.name,
              description: "",
              image: item.icon?.preview_url ?? item.icon?.url ?? null,
            }),
          ),
        ],
        next: Object.values(next).every((cursor) => cursor === false) ? null : next,
        defaultPersona: personas?.default_persona,
      };
    },
    getNextPageParam: (last) => last.next ?? undefined,
  });
}
export function mediaQuery(api: ApiClient, kind: "image" | "audio", query: string) {
  const limit = kind === "image" ? libraryImagePageSize : libraryPageSize;
  return infiniteQueryOptions({
    queryKey: [...queryKeys.media.library(), "metadata", kind, query, limit] as const,
    initialPageParam: null as string | null,
    queryFn: ({ pageParam }) =>
      api.call("mediaLibraryList", {
        cursor: pageParam,
        limit,
        kind,
        query: query || null,
        include_references: false,
      }),
    getNextPageParam: (last) => last.next_cursor ?? undefined,
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
