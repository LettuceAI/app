import { useState } from "react";
import { BookOpen } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Field, icon, Input, ListGroup, ListRowButton, TagInput, Textarea } from "@lettuceai/crisp";
import { DesignReferences } from "@/shared/ui/DesignReferences";
import { withReferences } from "../data/form";
import { LorebookChoiceSheet } from "./Pickers";
import type { StepProps } from "./step-props";

export function ExtrasStep({ api, form, update }: StepProps) {
  const { t, i18n } = useTranslation("characters");
  const [picking, setPicking] = useState(false);
  const { profile, provenance } = form.draft;
  const setProfile = (patch: Partial<typeof profile>) => update((current) => ({ ...current, draft: { ...current.draft, profile: { ...current.draft.profile, ...patch } } }));
  const setProvenance = (patch: Partial<typeof provenance>) => update((current) => ({ ...current, draft: { ...current.draft, provenance: { ...current.draft.provenance, ...patch } } }));
  const count = form.lorebookIds.length;

  return (
    <div className="space-y-6">
      <p className="text-base text-fg-2">{t("extras.body")}</p>
      <div className="grid gap-6 @4xl:grid-cols-2 @4xl:gap-x-10">
        <div className="space-y-5">
        <Field label={t("extras.nickname")} hint={t("extras.nicknameHint")}>
          <Input value={profile.nickname ?? ""} placeholder={t("extras.nicknamePlaceholder")} onChange={(event) => setProfile({ nickname: event.target.value })} data-autofocus />
        </Field>
        <Field label={t("extras.creator")}>
          <Input value={provenance.creator ?? ""} placeholder={t("extras.creatorPlaceholder")} onChange={(event) => setProvenance({ creator: event.target.value })} />
        </Field>
      <Field label={t("extras.tags")}>
        <TagInput value={provenance.tags} placeholder={t("extras.tagsPlaceholder")} onChange={(tags) => setProvenance({ tags })} />
      </Field>
        </div>
        <div className="space-y-5">
      <ListGroup>
        <ListRowButton
          icon={<BookOpen size={icon.lg} />}
          title={t("extras.lorebooks")}
          description={count === 0 ? t("extras.lorebooksNone") : t("extras.lorebooksCount", { count, formattedCount: new Intl.NumberFormat(i18n.language).format(count) })}
          onClick={() => setPicking(true)}
        />
      </ListGroup>
      <Field label={t("extras.creatorNotes")}>
        <Textarea value={provenance.creator_notes ?? ""} rows={6} autoGrow maxRows={12} placeholder={t("extras.creatorNotesPlaceholder")} onChange={(event) => setProvenance({ creator_notes: event.target.value })} />
      </Field>
        </div>
      </div>
      <DesignReferences
        api={api}
        note={profile.design_description ?? ""}
        onNoteChange={(design_description) => setProfile({ design_description })}
        references={form.references}
        onReferencesChange={(change) => update((current) => withReferences(current, change(current.references)))}
      />
      <LorebookChoiceSheet api={api} open={picking} value={form.lorebookIds} onChange={(lorebookIds) => update((current) => ({ ...current, lorebookIds }))} onClose={() => setPicking(false)} />
    </div>
  );
}
