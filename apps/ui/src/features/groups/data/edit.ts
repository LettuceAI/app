import type { GroupView_Serialize } from "@/api/generated/bindings";
import type { GroupForm } from "./form";

export const editSections = ["group", "members", "scene", "chat", "appearance"] as const;
export type EditSection = typeof editSections[number];

export interface GroupBaseline {
  id: string;
  revision: number;
  form: GroupForm;
}

export const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

export function formFromGroup(view: GroupView_Serialize, lorebookIds: string[]): GroupForm {
  return {
    draft: view.draft,
    background: view.assets.find((asset) => asset.asset_id === view.draft.background_asset_id) ?? null,
    lorebookIds,
    sceneAssets: view.assets,
    sceneSource: view.draft.starting_scene ? "custom" : "none",
  };
}

export function rebaseGroup(draft: GroupForm, old: GroupForm, fresh: GroupForm): GroupForm {
  const next = { ...fresh.draft };
  for (const key of Object.keys(draft.draft) as (keyof GroupForm["draft"])[]) {
    if (!same(draft.draft[key], old.draft[key])) Object.assign(next, { [key]: draft.draft[key] });
  }
  const sceneChanged = !same(draft.draft.starting_scene, old.draft.starting_scene) || draft.sceneSource !== old.sceneSource;
  return {
    draft: next,
    background: draft.draft.background_asset_id !== old.draft.background_asset_id ? draft.background : fresh.background,
    lorebookIds: same(draft.lorebookIds, old.lorebookIds) ? fresh.lorebookIds : draft.lorebookIds,
    sceneSource: sceneChanged ? draft.sceneSource : fresh.sceneSource,
    sceneAssets: sceneChanged ? [...(fresh.sceneAssets ?? []), ...(draft.sceneAssets ?? [])] : fresh.sceneAssets ?? [],
  };
}
