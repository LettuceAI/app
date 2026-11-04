import { useTranslation } from "react-i18next";
import { Sheet } from "@lettuceai/crisp";
import { ActionButton } from "../components/ui";

export interface SkipWarningSheetProps {
  open: boolean;
  onClose: () => void;
  onConnect: () => void;
}

export function SkipWarningSheet({ open, onClose, onConnect }: SkipWarningSheetProps) {
  const { t } = useTranslation("onboarding");
  return (
    <Sheet
      open={open}
      onClose={onClose}
      title={t("other.warning.title")}
      footer={
        <div className="flex flex-col gap-2">
          <ActionButton label={t("other.warning.connect")} block onClick={onConnect} />
        </div>
      }
    >
      <div className="space-y-3">
        <p className="text-base leading-relaxed text-fg-2">{t("other.warning.body")}</p>
      </div>
    </Sheet>
  );
}
