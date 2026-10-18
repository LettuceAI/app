import type { CharacterDraft } from "../data/form";
import type { StepProps } from "./step-props";

type Update = StepProps["update"];

export function draftSetters(update: Update) {
  return {
    setProfile: (patch: Partial<CharacterDraft["profile"]>) => update((current) => ({ ...current, draft: { ...current.draft, profile: { ...current.draft.profile, ...patch } } })),
    setProvenance: (patch: Partial<CharacterDraft["provenance"]>) => update((current) => ({ ...current, draft: { ...current.draft, provenance: { ...current.draft.provenance, ...patch } } })),
    setDefaults: (patch: Partial<CharacterDraft["defaults"]>) => update((current) => ({ ...current, draft: { ...current.draft, defaults: { ...current.draft.defaults, ...patch } } })),
  };
}
