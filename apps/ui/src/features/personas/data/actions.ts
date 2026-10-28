import type { QueryClient } from "@tanstack/react-query";
import { ApiFailure, type ApiClient } from "@/api/client";
import type { PersonaDefaultView, PersonaView_Serialize } from "@/api/generated/bindings";
import { lorebookIds, syncLorebooks } from "@/features/characters";
import { personaChanged } from "./edit";
import { draftToSave, type PersonaForm } from "./form";

export interface PersonaServices {
  api: ApiClient;
  queryClient: QueryClient;
}

export interface CreatedPersona {
  id: string;
  /** Lorebooks that could not be attached; the persona itself was saved. */
  unattached: string[];
  /** The persona was saved but could not be made the default. */
  defaultFailed: boolean;
}

/**
 * Saves the new persona, attaches the chosen lorebooks in order, then makes it the default
 * when asked. `operationId` identifies this attempt, so a retry does not create a second persona.
 */
export async function createPersona({ api }: PersonaServices, form: PersonaForm, operationId: string): Promise<CreatedPersona> {
  const created = await api.call("personaSave", { persona_id: null, draft: draftToSave(form), expected_revision: null, client_operation_id: operationId });
  let revision = created.revision;
  const unattached: string[] = [];
  for (const [index, lorebookId] of form.lorebookIds.entries()) {
    try {
      const bound = await api.call("personaLorebooksBind", {
        persona_id: created.id,
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
  let defaultFailed = false;
  if (form.makeDefault) {
    try {
      const slot = await readDefaultSlot(api);
      await api.call("personaDefaultSet", { persona_id: created.id, expected_revision: slot.revision, client_operation_id: `${operationId}:default` });
    } catch {
      defaultFailed = true;
    }
  }
  return { id: created.id, unattached, defaultFailed };
}

/** Imports a persona file; false when the picker was cancelled. Other kinds of file are refused. */
export async function importPersona({ api }: PersonaServices): Promise<boolean> {
  const picked = await api.call("filesPickOpen", { kinds: ["character_card", "json"], multiple: false });
  const source = picked.sources[0];
  if (!source) return false;
  const preview = await api.call("importPreview", { source });
  if (preview.content.kind !== "persona") throw new ApiFailure("unsupported", "not a persona file", null);
  await api.call("importCommit", { source, digest: preview.digest, options: {}, client_operation_id: crypto.randomUUID() });
  return true;
}

/** The default-persona slot, whose revision guards every change to it. */
export async function readDefaultSlot(api: ApiClient): Promise<PersonaDefaultView> {
  const page = await api.call("personasList", { cursor: null, limit: 1, query: null, sort: null });
  return page.default_persona;
}

export async function loadPersona(api: ApiClient, personaId: string): Promise<{ view: PersonaView_Serialize; lorebooks: string[] }> {
  const [view, bindings] = await Promise.all([api.call("personaGet", { persona_id: personaId }), api.call("personaLorebooksList", { persona_id: personaId })]);
  return { view, lorebooks: lorebookIds(bindings) };
}

export interface EditBaseline {
  form: PersonaForm;
  revision: number;
}

export interface EditSaved {
  view: PersonaView_Serialize | null;
  revision: number;
  lorebooks: string[];
  lorebooksFailed: boolean;
}

/** `operationId` makes a retry after a lost reply idempotent; each lorebook step derives its own id from it. */
export async function savePersonaEdits(api: ApiClient, personaId: string, baseline: EditBaseline, draft: PersonaForm, operationId: string): Promise<EditSaved> {
  let view: PersonaView_Serialize | null = null;
  let revision = baseline.revision;
  if (personaChanged(draft, baseline.form)) {
    view = await api.call("personaSave", { persona_id: personaId, draft: draftToSave(draft), expected_revision: revision, client_operation_id: operationId });
    revision = view.revision;
  }
  let order = baseline.form.lorebookIds;
  try {
    const synced = await syncLorebooks(
      {
        unbind: (id, expectedRevision, clientOperationId) => api.call("personaLorebooksUnbind", { persona_id: personaId, lorebook_id: id, expected_revision: expectedRevision, client_operation_id: clientOperationId }),
        bind: (id, expectedRevision, clientOperationId, targetOrdinal) => api.call("personaLorebooksBind", { persona_id: personaId, lorebook_id: id, expected_revision: expectedRevision, client_operation_id: clientOperationId, target_ordinal: targetOrdinal }),
        reorder: (id, expectedRevision, clientOperationId, targetOrdinal) => api.call("personaLorebooksReorder", { persona_id: personaId, lorebook_id: id, expected_revision: expectedRevision, client_operation_id: clientOperationId, target_ordinal: targetOrdinal }),
      },
      baseline.form.lorebookIds,
      draft.lorebookIds,
      revision,
      operationId,
    );
    revision = synced.revision;
    order = synced.order;
  } catch {
    const current = await loadPersona(api, personaId);
    return { view: current.view, revision: current.view.revision, lorebooks: current.lorebooks, lorebooksFailed: true };
  }
  return { view, revision, lorebooks: order, lorebooksFailed: false };
}

export function setPersonaDefault(api: ApiClient, personaId: string, on: boolean, slotRevision: number): Promise<PersonaDefaultView> {
  return on
    ? api.call("personaDefaultSet", { persona_id: personaId, expected_revision: slotRevision, client_operation_id: crypto.randomUUID() })
    : api.call("personaDefaultClear", { expected_revision: slotRevision, client_operation_id: crypto.randomUUID() });
}

export function deletePersona(api: ApiClient, personaId: string, revision: number): Promise<null> {
  return api.call("personaDelete", { persona_id: personaId, expected_revision: revision, client_operation_id: crypto.randomUUID() });
}
