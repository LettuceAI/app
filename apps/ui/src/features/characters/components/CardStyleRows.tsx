import { useTranslation } from "react-i18next";
import { Segmented } from "@lettuceai/crisp";
import { EditorRow } from "@/shared/ui/EditorRow";
import { GradientSettings } from "./GradientSettings";
import type { StepProps } from "./step-props";

export function CardStyleRows({ api, form, update, inline = false }: StepProps & { inline?: boolean }) {
  const { t } = useTranslation("characters");
  const { card_style } = form.draft.presentation;
  return <>
    <EditorRow label={t("identity.cardStyle")} hint={t("identity.cardStyleBody")} inline={inline}>
      <Segmented aria-label={t("identity.cardStyle")} block={!inline} size={inline ? "sm" : "md"} value={card_style} options={[{ value: "circle", label: t("identity.cardCircle") }, { value: "banner", label: t("identity.cardBanner") }]} onChange={(next) => update((current) => ({ ...current, draft: { ...current.draft, presentation: { ...current.draft.presentation, card_style: next } } }))} />
    </EditorRow>
    <GradientSettings api={api} form={form} update={update} inline={inline} />
  </>;
}
