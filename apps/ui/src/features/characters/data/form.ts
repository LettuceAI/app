import type { AssetRef, CharacterDraft_Serialize, LibraryCharacterMediaSlot, LibrarySceneDraft_Serialize } from "@/api/generated/bindings";

export type CharacterDraft = CharacterDraft_Serialize;
export type SceneDraft = LibrarySceneDraft_Serialize;

export const createSteps = ["identity", "scenes", "details", "extras"] as const;

export type CreateStep = (typeof createSteps)[number];

/** Everything the creation flow edits: the draft it saves, the images it shows, and the lorebooks it attaches after saving. */
export interface CharacterForm {
  draft: CharacterDraft;
  images: Partial<Record<LibraryCharacterMediaSlot, AssetRef>>;
  references: AssetRef[];
  lorebookIds: string[];
}

export function newForm(draft: CharacterDraft): CharacterForm {
  return { draft, images: {}, references: [], lorebookIds: [] };
}

/** Puts `asset` in `slot`, replacing what was there, or empties the slot. */
export function withImage(form: CharacterForm, slot: LibraryCharacterMediaSlot, asset: AssetRef | null): CharacterForm {
  const media = form.draft.media.filter((link) => link.slot !== slot);
  const images = { ...form.images };
  if (asset) {
    media.push({ asset_id: asset.asset_id, slot, ordinal: 0 });
    images[slot] = asset;
  } else {
    delete images[slot];
  }
  return { ...form, draft: { ...form.draft, media }, images };
}

export function withReferences(form: CharacterForm, references: AssetRef[]): CharacterForm {
  const media = [
    ...form.draft.media.filter((link) => link.slot !== "design_reference"),
    ...references.map((asset, ordinal) => ({ asset_id: asset.asset_id, slot: "design_reference" as const, ordinal })),
  ];
  return { ...form, references, draft: { ...form.draft, media } };
}

/** Empty strings become null, the way the backend stores an unset text. */
export function textOrNull(value: string): string | null {
  return value.trim() ? value : null;
}

export function sceneText(scene: SceneDraft): string {
  return scene.content.parts.map((part) => (part.kind === "text" ? part.text : "")).join("");
}

export interface SceneInput {
  text: string;
  direction: string;
}

/** Adds a scene at the end; the first scene becomes the default. */
export function withAddedScene(form: CharacterForm, input: SceneInput, id: string): CharacterForm {
  const { draft } = form;
  const scene: SceneDraft = {
    id,
    ordinal: draft.scenes.length,
    content: { format_version: 1, parts: [{ kind: "text", text: input.text }] },
    direction: textOrNull(input.direction),
    selected_variant_id: null,
    assets: [],
  };
  const defaults = draft.scenes.length === 0 && draft.defaults.default_scene_id === null ? { ...draft.defaults, default_scene_id: id } : draft.defaults;
  return { ...form, draft: { ...draft, scenes: [...draft.scenes, scene], defaults } };
}

export function withEditedScene(form: CharacterForm, id: string, input: SceneInput): CharacterForm {
  const scenes = form.draft.scenes.map((scene) =>
    scene.id === id ? { ...scene, content: { format_version: 1, parts: [{ kind: "text" as const, text: input.text }] }, direction: textOrNull(input.direction) } : scene,
  );
  return { ...form, draft: { ...form.draft, scenes } };
}

/** Removes a scene and closes the gap in the order; the default moves to the first scene left. */
export function withoutScene(form: CharacterForm, id: string): CharacterForm {
  const scenes = form.draft.scenes.filter((scene) => scene.id !== id).map((scene, ordinal) => ({ ...scene, ordinal }));
  const current = form.draft.defaults.default_scene_id;
  const default_scene_id = current === id ? (scenes[0]?.id ?? null) : current;
  return { ...form, draft: { ...retainScenes(form.draft, scenes), defaults: { ...form.draft.defaults, default_scene_id } } };
}

export function withDefaultScene(form: CharacterForm, id: string | null): CharacterForm {
  return { ...form, draft: { ...form.draft, defaults: { ...form.draft.defaults, default_scene_id: id } } };
}

export function wordCount(text: string): number {
  const trimmed = text.trim();
  return trimmed ? trimmed.split(/\s+/).length : 0;
}

/** Whether the person may leave `step` forward: a name first, then a definition. */
export function canLeave(step: CreateStep, form: CharacterForm): boolean {
  if (step === "identity") return form.draft.profile.name.trim().length > 0;
  if (step === "details") return (form.draft.profile.definition ?? "").trim().length > 0;
  return true;
}

export function hasSceneContent(scene: SceneDraft): boolean {
  return scene.content.parts.some((part) => part.kind !== "text" || part.text.trim().length > 0) || scene.assets.length > 0;
}

function retainScenes(draft: CharacterDraft, scenes: SceneDraft[]): CharacterDraft {
  const ids = new Set(scenes.map((scene) => scene.id));
  const defaultId = draft.defaults.default_scene_id;
  return {
    ...draft,
    scenes: scenes.map((scene, ordinal) => ({ ...scene, ordinal })),
    variants: draft.variants.filter((variant) => ids.has(variant.scene_id)),
    starters: draft.starters.map((starter) => starter.scene_id && !ids.has(starter.scene_id) ? { ...starter, scene_id: null } : starter),
    defaults: { ...draft.defaults, default_scene_id: defaultId && !ids.has(defaultId) ? scenes[0]?.id ?? null : defaultId },
  };
}

/** Omits empty scenes, repairs their references and normalizes optional profile text for saving. */
export function draftToSave(form: CharacterForm): CharacterDraft {
  const { profile, provenance } = form.draft;
  const optional = (value: string | null) => (value === null ? null : textOrNull(value));
  return {
    ...retainScenes(form.draft, form.draft.scenes.filter(hasSceneContent)),
    profile: {
      ...profile,
      name: profile.name.trim(),
      nickname: optional(profile.nickname),
      description: optional(profile.description),
      definition: optional(profile.definition),
      design_description: optional(profile.design_description),
      scenario: optional(profile.scenario),
    },
    provenance: { ...provenance, creator: optional(provenance.creator), creator_notes: optional(provenance.creator_notes) },
  };
}

export function enteredDraft(draft: CharacterDraft): CharacterDraft {
  return retainScenes(draft, draft.scenes.filter((scene) => hasSceneContent(scene) || Boolean(scene.direction?.trim())));
}

/** Whether anything was entered, so leaving would lose work. */
export function isTouched(form: CharacterForm, start: CharacterDraft): boolean {
  return form.lorebookIds.length > 0 || JSON.stringify(enteredDraft(form.draft)) !== JSON.stringify(enteredDraft(start));
}
