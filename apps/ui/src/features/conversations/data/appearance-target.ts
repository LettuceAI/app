import type { ApiClient } from "@/api/client";
import type { AssetRef_Serialize, CharacterView_Serialize, GroupView_Serialize } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { characterAppearance, withCharacterAppearance, type AppearanceDraft } from "@/features/groups";

/** The saved appearance of whatever the open chat takes its look from, plus the way to write a new one back. */
export interface AppearanceSnapshot {
  revision: number;
  value: AppearanceDraft;
  assets: AssetRef_Serialize[];
  save: (next: AppearanceDraft, operationId: string) => Promise<AppearanceSnapshot>;
}

export interface AppearanceTarget {
  kind: "group" | "character";
  key: readonly unknown[];
  load: () => Promise<AppearanceSnapshot>;
}

function groupSnapshot(api: ApiClient, view: GroupView_Serialize): AppearanceSnapshot {
  return {
    revision: view.revision,
    assets: view.assets,
    value: { presentation: view.draft.presentation, presentation_overrides: view.draft.presentation_overrides ?? null },
    save: async (next, operationId) =>
      groupSnapshot(api, await api.call("groupSave", {
        group_id: view.id,
        expected_revision: view.revision,
        draft: { ...view.draft, presentation: next.presentation, presentation_overrides: next.presentation_overrides ?? null },
        client_operation_id: operationId,
      })),
  };
}

function characterSnapshot(api: ApiClient, view: CharacterView_Serialize): AppearanceSnapshot {
  return {
    revision: view.revision,
    assets: view.assets,
    value: characterAppearance(view.draft.presentation),
    save: async (next, operationId) =>
      characterSnapshot(api, await api.call("characterSave", {
        character_id: view.id,
        expected_revision: view.revision,
        draft: { ...view.draft, presentation: withCharacterAppearance(view.draft.presentation, next) },
        client_operation_id: operationId,
      })),
  };
}

export function groupAppearanceTarget(api: ApiClient, groupId: string): AppearanceTarget {
  return {
    kind: "group",
    key: [...queryKeys.groups.all, "appearance", groupId],
    load: async () => groupSnapshot(api, await api.call("groupGet", { group_id: groupId })),
  };
}

export function characterAppearanceTarget(api: ApiClient, characterId: string): AppearanceTarget {
  return {
    kind: "character",
    key: [...queryKeys.characters.all, "appearance", characterId],
    load: async () => characterSnapshot(api, await api.call("characterGet", { character_id: characterId })),
  };
}
