import { useTranslation } from "react-i18next";
import { Button, Sheet } from "@lettuceai/crisp";
import { ActionError } from "../components/ActionError";
import { ActionButton } from "../components/ui";

export interface SkipWarningSheetProps {
  open: boolean;
  onClose: () => void;
  onConnect: () => void;
  onSkip: () => void;
  skipping: boolean;
  error: unknown;
}

export function SkipWarningSheet({ open, onClose, onConnect, onSkip, skipping, error }: SkipWarningSheetProps) {
  const { t } = useTranslation("onboarding");
  return (
    <Sheet
      open={open}
      onClose={onClose}
      title={t("other.warning.title")}
      footer={
        <div className="flex flex-col gap-2">
          <ActionButton label={t("other.warning.connect")} block onClick={onConnect} />
          <Button variant="ghost" size="lg" block disabled={skipping} onClick={onSkip}>
            {t("other.warning.skipAnyway")}
          </Button>
        </div>
      }
    >
      <div className="space-y-3">
        <p className="text-base leading-relaxed text-fg-2">{t("other.warning.body")}</p>
        <ActionError error={error} busy={skipping} onRetry={onSkip} />
      </div>
    </Sheet>
  );
}
