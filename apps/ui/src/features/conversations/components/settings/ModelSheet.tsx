import { useTranslation } from "react-i18next";
import { Sheet } from "@lettuceai/crisp";
import { ModelView } from "./ChoiceViews";
import type { PanelCtx } from "./context";

export function ModelSheet({ ctx, open, onClose }: { ctx: PanelCtx; open: boolean; onClose: () => void }) {
  const { t } = useTranslation("conversations");
  return (
    <Sheet open={open} onClose={onClose} size="sm" title={t("settings.model.title")} dismissible={!ctx.busy}>
      <ModelView ctx={ctx} onDone={onClose} />
    </Sheet>
  );
}
