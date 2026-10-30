import type { AssetRef, GroupDraft_Serialize, LibraryChatAppearanceV1_Serialize, LibraryGroupStartingSceneDraft_Serialize } from "@/api/generated/bindings";

export interface GroupForm {
  sceneAssets?: AssetRef[];
  draft: GroupDraft_Serialize;
  background: AssetRef | null;
  lorebookIds: string[];
  sceneSource: "none" | "custom" | "member";
}

export function newGroupForm(presentation: LibraryChatAppearanceV1_Serialize, dynamicMemory: boolean): GroupForm {
  return {
    draft: {
      name: "", chat_mode: "conversation", speaker_selection: "llm", memory_policy: dynamicMemory ? "dynamic" : "manual",
      persona: { kind: "inherit" }, disable_character_lorebooks: false,
      group_conversation_prompt_id: null, group_roleplay_prompt_id: null,
      presentation, presentation_overrides: [], members: [], background_asset_id: null, starting_scene: null,
    },
    background: null, lorebookIds: [], sceneSource: "none",
  };
}

export function stepsFor(mode: GroupDraft_Serialize["chat_mode"]) {
  return mode === "roleplay" ? ["members", "setup", "scene", "extras"] as const : ["members", "setup", "extras"] as const;
}

export function sceneFromText(text: string, id: string): LibraryGroupStartingSceneDraft_Serialize {
  return { scene: { id, ordinal: 0, content: { format_version: 1, parts: [{ kind: "text", text }] }, direction: null, selected_variant_id: null, assets: [] }, variants: [] };
}

export function sceneText(form: GroupForm): string {
  return form.draft.starting_scene?.scene.content.parts.flatMap((part) => part.kind === "text" ? [part.text] : []).join("") ?? "";
}

export function validStep(step: string, form: GroupForm): boolean {
  if (step === "members") return form.draft.members.length >= 2;
  if (step === "scene" && form.sceneSource !== "none") return form.draft.starting_scene !== null && (form.draft.starting_scene.scene.content.parts.some((part) => part.kind !== "text" || part.text.trim()) || form.draft.starting_scene.scene.assets.length > 0);
  return true;
}

export function draftToSave(form: GroupForm, fallbackName: string): GroupDraft_Serialize {
  return { ...form.draft, name: form.draft.name.trim() || fallbackName, members: form.draft.members.map((member, ordinal) => ({ ...member, ordinal })), starting_scene: form.draft.chat_mode === "roleplay" && form.sceneSource !== "none" ? form.draft.starting_scene : null };
}
