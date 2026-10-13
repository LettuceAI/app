import { queryOptions } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { CharacterPage, CharacterSummary, ConversationSummary, LatestConversationPage } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

export interface HomeCharacter {
  character: CharacterSummary;
  /** The character's newest one-to-one chat, archived included. */
  latest: ConversationSummary | null;
  /** When the character was last active: its newest chat, else its own last change. */
  activeAt: number;
}

async function allCharacters(api: ApiClient): Promise<CharacterSummary[]> {
  const characters: CharacterSummary[] = [];
  let cursor: string | null = null;
  do {
    const page: CharacterPage = await api.call("charactersList", { cursor, limit: null });
    characters.push(...page.items);
    cursor = page.next_cursor;
  } while (cursor !== null);
  return characters;
}

async function latestByCharacter(api: ApiClient): Promise<Map<string, ConversationSummary>> {
  const latest = new Map<string, ConversationSummary>();
  let cursor: string | null = null;
  do {
    const page: LatestConversationPage = await api.call("conversationsLatestByCharacter", { cursor, limit: null });
    for (const item of page.items) latest.set(item.source_id, item.conversation);
    cursor = page.next_cursor;
  } while (cursor !== null);
  return latest;
}

/** The characters home shows, the most recently active first; one whose newest chat is archived is hidden. */
export function sortByActivity(characters: readonly CharacterSummary[], latest: ReadonlyMap<string, ConversationSummary>): HomeCharacter[] {
  return characters
    .filter((character) => !latest.get(character.id)?.archived)
    .map((character) => {
      const chat = latest.get(character.id) ?? null;
      return { character, latest: chat, activeAt: chat?.updated_at ?? character.updated_at };
    })
    .sort((a, b) => b.activeAt - a.activeAt);
}

export function homeCharactersQuery(api: ApiClient) {
  return queryOptions({
    queryKey: queryKeys.home.characters(),
    queryFn: async () => {
      const [characters, latest] = await Promise.all([allCharacters(api), latestByCharacter(api)]);
      return sortByActivity(characters, latest);
    },
  });
}

/** Whether `home` matches the search: its name or description contains the trimmed text, ignoring case. */
export function matchesSearch(home: HomeCharacter, search: string): boolean {
  const needle = search.trim().toLowerCase();
  if (!needle) return true;
  return home.character.name.toLowerCase().includes(needle) || (home.character.description ?? "").toLowerCase().includes(needle);
}
