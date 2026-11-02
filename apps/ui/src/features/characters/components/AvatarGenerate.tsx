import { AvatarGeneration } from "@/entities/image-input";
import { withImage } from "../data/form";
import type { StepProps } from "./step-props";

/** Generate avatar for a character form; Use only changes the draft, the page's own Save or Create commits it. */
export function AvatarGenerate({ api, form, update, characterId, disabled }: StepProps & { characterId?: string; disabled: boolean }) {
  const { profile } = form.draft;
  return <AvatarGeneration
    api={api}
    disabled={disabled}
    readContext={() => ({ subjectName: profile.name.trim(), subjectDescription: (profile.description ?? profile.definition ?? "").trim(), currentAvatar: form.images.avatar_original ?? null, characterId: characterId ?? null })}
    onUse={(asset) => update((current) => withImage(current, "avatar_original", asset))}
  />;
}
