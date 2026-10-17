import { useTranslation } from "react-i18next";
import { Chip, Field, Textarea } from "@lettuceai/crisp";
import { wordCount } from "@/features/characters";
import type { StepProps } from "./step-props";

export function PersonaDescription({ form, update }: StepProps) {
  const { t, i18n } = useTranslation("personas");
  const { description } = form.draft;
  const words = wordCount(description);
  const setDescription = (value: string) => update((current) => ({ ...current, draft: { ...current.draft, description: value } }));

  return <div className="space-y-3">
    <Field label={t("about.description")} required hint={
      <span className="flex flex-wrap justify-between gap-x-4">
        <span>{t("about.descriptionHint", { char: "{{char}}", user: "{{user}}" })}</span>
        <span className="tabular-nums">{t("about.words", { count: words, formattedCount: new Intl.NumberFormat(i18n.language).format(words) })}</span>
      </span>
    }>
      <Textarea value={description} rows={8} autoGrow maxRows={24} placeholder={t("about.descriptionPlaceholder")} onChange={(event) => setDescription(event.target.value)} />
    </Field>
    <div className="flex flex-wrap gap-2">
      {["{{user}}", "{{char}}"].map((placeholder) => <Chip key={placeholder} onClick={() => setDescription(`${description}${description && !description.endsWith(" ") ? " " : ""}${placeholder}`)}><span className="font-mono text-xs">{placeholder}</span></Chip>)}
    </div>
  </div>;
}
