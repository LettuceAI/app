import { useTranslation } from "react-i18next";
import { Chip, Field, Input, TagInput, Textarea } from "@lettuceai/crisp";
import { wordCount } from "../data/form";
import { draftSetters } from "./draft-setters";
import type { FormProps } from "./step-props";

export function NameField({ form, update, focus }: FormProps & { focus?: boolean }) {
  const { t } = useTranslation("characters");
  const { setProfile } = draftSetters(update);
  return <Field label={t("identity.name")} required hint={t("identity.nameHint")}>
    <Input value={form.draft.profile.name} maxLength={200} placeholder={t("identity.namePlaceholder")} onChange={(event) => setProfile({ name: event.target.value })} {...(focus ? { "data-autofocus": true } : {})} />
  </Field>;
}

export function NicknameField({ form, update, focus }: FormProps & { focus?: boolean }) {
  const { t } = useTranslation("characters");
  const { setProfile } = draftSetters(update);
  return <Field label={t("extras.nickname")} hint={t("extras.nicknameHint")}>
    <Input value={form.draft.profile.nickname ?? ""} placeholder={t("extras.nicknamePlaceholder")} onChange={(event) => setProfile({ nickname: event.target.value })} {...(focus ? { "data-autofocus": true } : {})} />
  </Field>;
}

export function DefinitionField({ form, update, focus }: FormProps & { focus?: boolean }) {
  const { t, i18n } = useTranslation("characters");
  const { setProfile } = draftSetters(update);
  const { profile } = form.draft;
  const words = wordCount(profile.definition ?? "");
  return <div className="space-y-3">
    <Field label={t("details.definition")} required hint={
      <span className="flex flex-wrap justify-between gap-x-4">
        <span>{t("details.definitionHint", { char: "{{char}}", user: "{{user}}" })}</span>
        <span className="tabular-nums">{t("details.words", { count: words, formattedCount: new Intl.NumberFormat(i18n.language).format(words) })}</span>
      </span>
    }>
      <Textarea value={profile.definition ?? ""} rows={8} className="@4xl:min-h-96" autoGrow maxRows={24} placeholder={t("details.definitionPlaceholder")} onChange={(event) => setProfile({ definition: event.target.value })} {...(focus ? { "data-autofocus": true } : {})} />
    </Field>
    <div className="flex flex-wrap gap-2">
      {["{{char}}", "{{user}}"].map((placeholder) => <Chip key={placeholder} onClick={() => setProfile({ definition: `${profile.definition ?? ""}${profile.definition && !profile.definition.endsWith(" ") ? " " : ""}${placeholder}` })}><span className="font-mono text-xs">{placeholder}</span></Chip>)}
    </div>
  </div>;
}

export function DescriptionField({ form, update }: FormProps) {
  const { t } = useTranslation("characters");
  const { setProfile } = draftSetters(update);
  return <Field label={t("details.description")} hint={t("details.descriptionHint")}>
    <Textarea value={form.draft.profile.description ?? ""} rows={3} autoGrow maxRows={12} placeholder={t("details.descriptionPlaceholder")} onChange={(event) => setProfile({ description: event.target.value })} />
  </Field>;
}

export function CreatorField({ form, update }: FormProps) {
  const { t } = useTranslation("characters");
  const { setProvenance } = draftSetters(update);
  return <Field label={t("extras.creator")}>
    <Input value={form.draft.provenance.creator ?? ""} placeholder={t("extras.creatorPlaceholder")} onChange={(event) => setProvenance({ creator: event.target.value })} />
  </Field>;
}

export function TagsField({ form, update }: FormProps) {
  const { t } = useTranslation("characters");
  const { setProvenance } = draftSetters(update);
  return <Field label={t("extras.tags")}>
    <TagInput value={form.draft.provenance.tags} placeholder={t("extras.tagsPlaceholder")} onChange={(tags) => setProvenance({ tags })} />
  </Field>;
}

export function CreatorNotesField({ form, update }: FormProps) {
  const { t } = useTranslation("characters");
  const { setProvenance } = draftSetters(update);
  return <Field label={t("extras.creatorNotes")}>
    <Textarea value={form.draft.provenance.creator_notes ?? ""} rows={6} autoGrow maxRows={12} placeholder={t("extras.creatorNotesPlaceholder")} onChange={(event) => setProvenance({ creator_notes: event.target.value })} />
  </Field>;
}
