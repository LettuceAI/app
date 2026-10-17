import type { QueryClient } from "@tanstack/react-query";
import { ApiFailure, type ApiClient } from "@/api/client";
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
      const page = await api.call("personasList", { cursor: null, limit: 1, query: null, sort: null });
      await api.call("personaDefaultSet", { persona_id: created.id, expected_revision: page.default_persona.revision, client_operation_id: `${operationId}:default` });
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
