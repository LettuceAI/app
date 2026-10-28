import type { AssetRef, PersonaView_Serialize } from "@/api/generated/bindings";
import { draftToSave, withAvatar, withReferences, type PersonaForm } from "./form";

export function formFromView(view: PersonaView_Serialize, lorebooks: string[]): PersonaForm {
  const assets = new Map<string, AssetRef>(view.assets.map((asset) => [asset.asset_id, asset]));
  let avatar: AssetRef | null = null;
  const references: AssetRef[] = [];
  for (const link of [...view.draft.media].sort((a, b) => a.ordinal - b.ordinal)) {
    const asset = assets.get(link.asset_id);
    if (!asset) continue;
    if (link.slot === "design_reference") references.push(asset);
    else avatar = asset;
  }
  return { draft: view.draft, avatar, references, lorebookIds: lorebooks, makeDefault: false };
}

function personaPart(form: PersonaForm): string {
  const draft = draftToSave(form);
  const media = [...draft.media].sort((a, b) => a.slot.localeCompare(b.slot) || a.ordinal - b.ordinal);
  return JSON.stringify({ ...draft, media });
}

export function personaChanged(draft: PersonaForm, baseline: PersonaForm): boolean {
  return personaPart(draft) !== personaPart(baseline);
}

export function lorebooksChanged(draft: PersonaForm, baseline: PersonaForm): boolean {
  return draft.lorebookIds.join("\n") !== baseline.lorebookIds.join("\n");
}

export function isDirty(draft: PersonaForm, baseline: PersonaForm): boolean {
  return personaChanged(draft, baseline) || lorebooksChanged(draft, baseline);
}

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

/** `fresh` with the fields the user changed since `old` laid over it; every other field takes the fresh value. */
export function rebase(draft: PersonaForm, old: PersonaForm, fresh: PersonaForm): PersonaForm {
  const oldDraft: Record<string, unknown> = { ...old.draft };
  const kept = Object.fromEntries(Object.entries(draft.draft).filter(([key, value]) => key !== "media" && !same(value, oldDraft[key])));
  let next: PersonaForm = { ...fresh, draft: { ...fresh.draft, ...kept } };
  if (!same(draft.avatar?.asset_id ?? null, old.avatar?.asset_id ?? null)) next = withAvatar(next, draft.avatar);
  if (!same(draft.references.map((asset) => asset.asset_id), old.references.map((asset) => asset.asset_id))) next = withReferences(next, draft.references);
  if (lorebooksChanged(draft, old)) next = { ...next, lorebookIds: draft.lorebookIds };
  return next;
}
