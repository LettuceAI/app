import type { QueryClient } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { AssetIngestRole, AssetRef } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { draftToSave, type CharacterForm } from "./form";

export interface CharacterServices {
  api: ApiClient;
  queryClient: QueryClient;
}

/** Asks for an image and stores it; null when the picker was cancelled. */
export async function pickImage({ api }: CharacterServices, role: AssetIngestRole): Promise<AssetRef | null> {
  const picked = await api.call("filesPickOpen", { kinds: ["image"], multiple: false });
  const source = picked.sources[0];
  if (!source) return null;
  return api.call("assetsIngest", { source, role });
}

export interface Created {
  id: string;
  /** Lorebooks that could not be attached; the character itself was saved. */
  unattached: string[];
}

/**
 * Saves the new character, then attaches the chosen lorebooks in order. `operationId`
 * identifies this attempt, so retrying after a lost reply does not create a second character.
 */
export async function createCharacter(services: CharacterServices, form: CharacterForm, operationId: string): Promise<Created> {
  const { api, queryClient } = services;
  const created = await api.call("characterSave", { character_id: null, draft: draftToSave(form), expected_revision: null, client_operation_id: operationId });
  let revision = created.revision;
  const unattached: string[] = [];
  for (const [index, lorebookId] of form.lorebookIds.entries()) {
    try {
      const bound = await api.call("characterLorebooksBind", {
        character_id: created.id,
        lorebook_id: lorebookId,
        expected_revision: revision,
        client_operation_id: `${operationId}:lorebook:${index}`,
        target_ordinal: null,
      });
      revision = bound.owner_revision;
    } catch {
      unattached.push(lorebookId);
    }
  }
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: queryKeys.characters.all }),
    queryClient.invalidateQueries({ queryKey: queryKeys.home.characters() }),
  ]);
  return { id: created.id, unattached };
}
