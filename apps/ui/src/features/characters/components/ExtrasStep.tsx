import { useTranslation } from "react-i18next";
import { DesignReferences } from "@/shared/ui/DesignReferences";
import { withReferences } from "../data/form";
import { draftSetters } from "./draft-setters";
import { LorebooksRow } from "./ChatSettings";
import { CreatorField, CreatorNotesField, NicknameField, TagsField } from "./ProfileFields";
import type { StepProps } from "./step-props";

export function ExtrasStep({ api, form, update }: StepProps) {
  const { t } = useTranslation("characters");
  const { setProfile } = draftSetters(update);
  const { profile } = form.draft;

  return (
    <div className="space-y-6">
      <p className="text-base text-fg-2">{t("extras.body")}</p>
      <div className="grid gap-6 @4xl:grid-cols-2 @4xl:gap-x-10">
        <div className="space-y-5">
          <NicknameField form={form} update={update} focus />
          <CreatorField form={form} update={update} />
          <TagsField form={form} update={update} />
        </div>
        <div className="space-y-5">
          <LorebooksRow api={api} form={form} update={update} />
          <CreatorNotesField form={form} update={update} />
        </div>
      </div>
      <DesignReferences
        api={api}
        note={profile.design_description ?? ""}
        onNoteChange={(design_description) => setProfile({ design_description })}
        references={form.references}
        onReferencesChange={(change) => update((current) => withReferences(current, change(current.references)))}
      />
    </div>
  );
}
