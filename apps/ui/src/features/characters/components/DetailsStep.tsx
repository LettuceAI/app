import { useState } from "react";
import { ChevronDown } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, Field, icon } from "@lettuceai/crisp";
import { draftSetters } from "./draft-setters";
import { MemoryGroup, PromptsGroup } from "./ChatSettings";
import { useModelName } from "./use-model-name";
import { ModelChoiceSheet } from "./Pickers";
import { DefinitionField, DescriptionField } from "./ProfileFields";
import type { StepProps } from "./step-props";

export function DetailsStep({ api, form, update }: StepProps) {
  const { t } = useTranslation("characters");
  const [sheet, setSheet] = useState<"model" | null>(null);
  const { setDefaults } = draftSetters(update);
  const { defaults } = form.draft;
  const modelName = useModelName(api, defaults.model_profile_id);

  return <>
    <div className="grid grid-cols-1 gap-6 @4xl:grid-cols-[3fr_2fr] @4xl:gap-x-10">
      <div className="min-w-0 space-y-6">
        <DefinitionField form={form} update={update} focus />
        <DescriptionField form={form} update={update} />
      </div>
      <div className="min-w-0 space-y-5">
        <Field label={t("details.model")} hint={t("details.modelHint")}>
          <Button block variant="secondary" aria-label={t("details.model")} className="grid grid-cols-1 text-left" trailing={<ChevronDown size={icon.sm} className="shrink-0" />} onClick={() => setSheet("model")}><span className="min-w-0 flex-1 truncate">{modelName}</span></Button>
        </Field>
        <PromptsGroup api={api} form={form} update={update} />
        <MemoryGroup api={api} form={form} update={update} />
      </div>
    </div>
    <ModelChoiceSheet api={api} open={sheet === "model"} value={defaults.model_profile_id} onChange={(model_profile_id) => setDefaults({ model_profile_id })} onClose={() => setSheet(null)} />
  </>;
}
