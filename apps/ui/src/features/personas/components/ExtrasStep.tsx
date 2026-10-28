import { useTranslation } from "react-i18next";
import { PersonaLook } from "./Look";
import { PersonaLorebooks } from "./Lorebooks";
import type { StepProps } from "./step-props";

export function ExtrasStep(props: StepProps) {
  const { t } = useTranslation("personas");

  return <div className="space-y-6">
    <p className="text-base text-fg-2">{t("extras.body")}</p>
    <PersonaLook {...props} autoFocus />
    <div className="grid grid-cols-1 gap-6 @4xl:grid-cols-2 @4xl:gap-x-10">
      <PersonaLorebooks {...props} />
    </div>
  </div>;
}
