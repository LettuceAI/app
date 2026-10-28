import { DesignReferences } from "@/shared/ui/DesignReferences";
import { withReferences } from "../data/form";
import type { StepProps } from "./step-props";

export function PersonaLook({ api, form, update, autoFocus }: StepProps & { autoFocus?: boolean }) {
  return <DesignReferences
    api={api}
    {...(autoFocus ? { autoFocus } : {})}
    note={form.draft.design_description ?? ""}
    onNoteChange={(design_description) => update((current) => ({ ...current, draft: { ...current.draft, design_description } }))}
    references={form.references}
    onReferencesChange={(change) => update((current) => withReferences(current, change(current.references)))}
  />;
}
