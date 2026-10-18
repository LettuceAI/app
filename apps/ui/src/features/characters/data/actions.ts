import type { QueryClient } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { AssetIngestRole, AssetRef, CharacterSummary, CharacterView_Serialize, LibraryLorebookMutationResult } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { characterChanged, lorebookIds } from "./edit";
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

export interface EditBaseline {
  form: CharacterForm;
  revision: number;
}

export interface EditSaved {
  view: CharacterView_Serialize | null;
  revision: number;
  lorebooks: string[];
  lorebooksFailed: boolean;
}

/** `operationId` makes a retry after a lost reply idempotent; each lorebook step derives its own id from it. */
export async function saveCharacterEdits(api: ApiClient, characterId: string, baseline: EditBaseline, draft: CharacterForm, operationId: string): Promise<EditSaved> {
  let view: CharacterView_Serialize | null = null;
  let revision = baseline.revision;
  if (characterChanged(draft, baseline.form)) {
    view = await api.call("characterSave", { character_id: characterId, draft: draftToSave(draft), expected_revision: revision, client_operation_id: operationId });
    revision = view.revision;
  }
  let order = baseline.form.lorebookIds;
  const target = draft.lorebookIds;
  let step = 0;
  const apply = async (call: (clientOperationId: string) => Promise<LibraryLorebookMutationResult>) => {
    const result = await call(`${operationId}:lorebook:${step++}`);
    revision = result.owner_revision;
    order = lorebookIds(result.bindings);
  };
  try {
    for (const id of baseline.form.lorebookIds.filter((entry) => !target.includes(entry))) {
      await apply((client_operation_id) => api.call("characterLorebooksUnbind", { character_id: characterId, lorebook_id: id, expected_revision: revision, client_operation_id }));
    }
    for (const [index, id] of target.entries()) {
      if (order.includes(id)) continue;
      await apply((client_operation_id) => api.call("characterLorebooksBind", { character_id: characterId, lorebook_id: id, expected_revision: revision, client_operation_id, target_ordinal: Math.min(index, order.length) }));
    }
    for (const [index, id] of target.entries()) {
      if (order[index] === id) continue;
      await apply((client_operation_id) => api.call("characterLorebooksReorder", { character_id: characterId, lorebook_id: id, expected_revision: revision, client_operation_id, target_ordinal: index }));
    }
  } catch {
    const [current, bindings] = await Promise.all([api.call("characterGet", { character_id: characterId }), api.call("characterLorebooksList", { character_id: characterId })]);
    return { view: current, revision: current.revision, lorebooks: lorebookIds(bindings), lorebooksFailed: true };
  }
  return { view, revision, lorebooks: order, lorebooksFailed: false };
}

export function archiveCharacter(api: ApiClient, characterId: string, revision: number): Promise<CharacterView_Serialize> {
  return api.call("characterArchive", { character_id: characterId, expected_revision: revision, client_operation_id: crypto.randomUUID() });
}

export function restoreCharacter(api: ApiClient, characterId: string, revision: number): Promise<CharacterView_Serialize> {
  return api.call("characterRestore", { character_id: characterId, expected_revision: revision, client_operation_id: crypto.randomUUID() });
}

export function duplicateCharacter(api: ApiClient, characterId: string): Promise<CharacterSummary> {
  return api.call("characterDuplicate", { character_id: characterId, client_operation_id: crypto.randomUUID() });
}
