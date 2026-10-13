import type { QueryClient } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { CharacterExportFormat, CharacterSummary, ConversationSummary, EntityExportResult } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

export interface HomeServices {
  api: ApiClient;
  queryClient: QueryClient;
}

async function refresh({ queryClient }: HomeServices): Promise<void> {
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: queryKeys.home.characters() }),
    queryClient.invalidateQueries({ queryKey: queryKeys.conversations.lists() }),
    queryClient.invalidateQueries({ queryKey: queryKeys.characters.all }),
  ]);
}

/** Switches the card between the row and the banner shape. */
export async function toggleCardStyle(services: HomeServices, character: CharacterSummary): Promise<void> {
  const { api } = services;
  const view = await api.call("characterGet", { character_id: character.id });
  const draft = view.draft;
  const card_style = draft.presentation.card_style === "banner" ? "circle" : "banner";
  await api.call("characterSave", {
    character_id: character.id,
    draft: { ...draft, presentation: { ...draft.presentation, card_style } },
    expected_revision: view.revision,
    client_operation_id: crypto.randomUUID(),
  });
  await refresh(services);
}

/** Writes the character card to the app's export folder. */
export function exportCharacter({ api }: HomeServices, character: CharacterSummary, format: CharacterExportFormat): Promise<EntityExportResult> {
  return api.call("characterExport", { id: character.id, format, target: null });
}

/** Takes a character off the home screen by archiving its newest chat; it stays in the library. */
export async function hideCharacter(services: HomeServices, latest: ConversationSummary): Promise<void> {
  await services.api.call("conversationArchive", { conversation_id: latest.id });
  await refresh(services);
}

export async function deleteCharacter(services: HomeServices, character: CharacterSummary): Promise<void> {
  await services.api.call("characterDelete", {
    character_id: character.id,
    expected_revision: character.revision,
    client_operation_id: crypto.randomUUID(),
  });
  await refresh(services);
}

export async function renameChat(services: HomeServices, chat: ConversationSummary, title: string): Promise<void> {
  await services.api.call("conversationRename", { conversation_id: chat.id, expected_revision: chat.revision, title: title.trim() });
  await refresh(services);
}

export async function deleteChat(services: HomeServices, chat: ConversationSummary): Promise<void> {
  await services.api.call("conversationDelete", { conversation_id: chat.id });
  await refresh(services);
}
