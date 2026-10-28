import { useState } from "react";
import { BookOpen } from "lucide-react";
import { useTranslation } from "react-i18next";
import { icon, ListGroup, ListRowButton } from "@lettuceai/crisp";
import { LorebookChoiceSheet } from "@/entities/lorebook";
import type { StepProps } from "./step-props";

export function PersonaLorebooks({ api, form, update }: StepProps) {
  const { t, i18n } = useTranslation("personas");
  const [choosing, setChoosing] = useState(false);
  const count = form.lorebookIds.length;

  return <>
    <ListGroup>
      <ListRowButton
        icon={<BookOpen size={icon.lg} />}
        title={t("extras.lorebooks")}
        description={count === 0 ? t("extras.lorebooksNone") : t("extras.lorebooksCount", { count, formattedCount: new Intl.NumberFormat(i18n.language).format(count) })}
        onClick={() => setChoosing(true)}
      />
    </ListGroup>
    <LorebookChoiceSheet api={api} open={choosing} value={form.lorebookIds} onChange={(lorebookIds) => update((current) => ({ ...current, lorebookIds }))} onClose={() => setChoosing(false)} />
  </>;
}
