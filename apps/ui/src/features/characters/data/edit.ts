import type { AssetRef, CharacterView_Serialize, LibraryCharacterMediaSlot, LibraryLorebookBinding } from "@/api/generated/bindings";
import { draftToSave, enteredDraft, type CharacterForm } from "./form";

export type EditTab = "profile" | "scenes" | "chat" | "look" | "soul";

export const editTabs: readonly EditTab[] = ["profile", "scenes", "chat", "look", "soul"];

export function lorebookIds(bindings: LibraryLorebookBinding[]): string[] {
  return [...bindings].sort((a, b) => a.ordinal - b.ordinal).map((binding) => binding.lorebook_id);
}

export function formFromView(view: CharacterView_Serialize, lorebooks: string[]): CharacterForm {
  const assets = new Map<string, AssetRef>(view.assets.map((asset) => [asset.asset_id, asset]));
  const images: Partial<Record<LibraryCharacterMediaSlot, AssetRef>> = {};
  const references: AssetRef[] = [];
  for (const link of [...view.draft.media].sort((a, b) => a.ordinal - b.ordinal)) {
    const asset = assets.get(link.asset_id);
    if (!asset) continue;
    if (link.slot === "design_reference") references.push(asset);
    else images[link.slot] = asset;
  }
  return { draft: view.draft, images, references, lorebookIds: lorebooks };
}

function characterPart(form: CharacterForm): string {
  const draft = { ...draftToSave(form), scenes: enteredDraft(form.draft).scenes };
  const media = [...draft.media].sort((a, b) => a.slot.localeCompare(b.slot) || a.ordinal - b.ordinal);
  return JSON.stringify({ ...draft, media });
}

export function characterChanged(draft: CharacterForm, baseline: CharacterForm): boolean {
  return characterPart(draft) !== characterPart(baseline);
}

export function lorebooksChanged(draft: CharacterForm, baseline: CharacterForm): boolean {
  return draft.lorebookIds.join("\n") !== baseline.lorebookIds.join("\n");
}

export function isDirty(draft: CharacterForm, baseline: CharacterForm): boolean {
  return characterChanged(draft, baseline) || lorebooksChanged(draft, baseline);
}
