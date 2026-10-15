import { useTranslation } from "react-i18next";
import { Sheet } from "@lettuceai/crisp";
import { LorebooksView } from "./ChoiceViews";
import type { PanelCtx } from "./context";

export function LorebooksSheet({ ctx, open, onClose }: { ctx: PanelCtx; open: boolean; onClose: () => void }) {
  const { t } = useTranslation("conversations");
  return (
    <Sheet open={open} onClose={onClose} size="sm" title={t("settings.lorebooks.title")} dismissible={!ctx.busy}>
      <LorebooksView ctx={ctx} />
    </Sheet>
  );
}
