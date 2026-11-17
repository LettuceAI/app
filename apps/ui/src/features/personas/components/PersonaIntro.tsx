import { useTranslation } from "react-i18next";
import { Field, Textarea } from "@lettuceai/crisp";
import type { StepProps } from "./step-props";

export function PersonaIntro({ form, update, disabled }: Pick<StepProps, "form" | "update"> & { disabled?: boolean }) {
  const { t } = useTranslation("personas");
  return <Field label={t("about.intro")} hint={t("about.introHint")}>
    <Textarea value={form.draft.intro ?? ""} rows={3} autoGrow maxRows={12} disabled={disabled} onChange={(event) => {
      const intro = event.target.value;
      update((current) => ({ ...current, draft: { ...current.draft, intro } }));
    }} />
  </Field>;
}
