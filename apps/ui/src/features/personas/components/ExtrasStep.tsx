import { useState } from "react";
import { BookOpen } from "lucide-react";
import { useTranslation } from "react-i18next";
import { icon, ListGroup, ListRowButton } from "@lettuceai/crisp";
import { LorebookChoiceSheet } from "@/features/characters";
import { DesignReferences } from "@/shared/ui/DesignReferences";
import { withReferences } from "../data/form";
import type { StepProps } from "./step-props";

export function ExtrasStep({ api, form, update }: StepProps) {
  const { t, i18n } = useTranslation("personas");
  const [choosingLorebooks, setChoosingLorebooks] = useState(false);
  const count = form.lorebookIds.length;

  return <div className="space-y-6">
    <p className="text-base text-fg-2">{t("extras.body")}</p>
    <DesignReferences
      api={api}
      autoFocus
      note={form.draft.design_description ?? ""}
      onNoteChange={(design_description) => update((current) => ({ ...current, draft: { ...current.draft, design_description } }))}
      references={form.references}
      onReferencesChange={(change) => update((current) => withReferences(current, change(current.references)))}
    />
    <div className="grid gap-6 @4xl:grid-cols-2 @4xl:gap-x-10">
      <ListGroup>
        <ListRowButton
          icon={<BookOpen size={icon.lg} />}
          title={t("extras.lorebooks")}
          description={count === 0 ? t("extras.lorebooksNone") : t("extras.lorebooksCount", { count, formattedCount: new Intl.NumberFormat(i18n.language).format(count) })}
          onClick={() => setChoosingLorebooks(true)}
        />
      </ListGroup>
    </div>
    <LorebookChoiceSheet api={api} open={choosingLorebooks} value={form.lorebookIds} onChange={(lorebookIds) => update((current) => ({ ...current, lorebookIds }))} onClose={() => setChoosingLorebooks(false)} />
  </div>;
}
