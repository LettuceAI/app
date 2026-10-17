import type { AssetRef, PersonaDraft_Serialize } from "@/api/generated/bindings";

export type PersonaDraft = PersonaDraft_Serialize;

export const personaSteps = ["identity", "extras"] as const;

export type PersonaStep = (typeof personaSteps)[number];

/** Everything the creation flow edits: the draft it saves, the images it shows, and what it applies after saving. */
export interface PersonaForm {
  draft: PersonaDraft;
  avatar: AssetRef | null;
  references: AssetRef[];
  lorebookIds: string[];
  makeDefault: boolean;
}

export function newPersonaForm(draft: PersonaDraft): PersonaForm {
  return { draft, avatar: null, references: [], lorebookIds: [], makeDefault: false };
}

export function withAvatar(form: PersonaForm, asset: AssetRef | null): PersonaForm {
  const media = form.draft.media.filter((link) => link.slot !== "avatar");
  if (asset) media.push({ asset_id: asset.asset_id, slot: "avatar", ordinal: 0 });
  return { ...form, avatar: asset, draft: { ...form.draft, media } };
}

export function withReferences(form: PersonaForm, references: AssetRef[]): PersonaForm {
  const media = [
    ...form.draft.media.filter((link) => link.slot !== "design_reference"),
    ...references.map((asset, ordinal) => ({ asset_id: asset.asset_id, slot: "design_reference" as const, ordinal })),
  ];
  return { ...form, references, draft: { ...form.draft, media } };
}

/** Whether the person may leave `step` forward: identity needs a name and a description. */
export function canLeave(step: PersonaStep, form: PersonaForm): boolean {
  if (step === "identity") return form.draft.title.trim().length > 0 && form.draft.description.trim().length > 0;
  return true;
}

const optional = (value: string | null) => (value?.trim() ? value.trim() : null);

export function draftToSave(form: PersonaForm): PersonaDraft {
  const { draft } = form;
  return { ...draft, title: draft.title.trim(), description: draft.description.trim(), nickname: optional(draft.nickname), design_description: optional(draft.design_description) };
}

/** Whether anything was entered, so leaving would lose work. */
export function isTouched(form: PersonaForm, start: PersonaDraft): boolean {
  return form.makeDefault || form.lorebookIds.length > 0 || JSON.stringify(form.draft) !== JSON.stringify(start);
}
