import { useTranslation } from "react-i18next";
import { ListGroup, ListRowButton } from "@lettuceai/crisp";
import { LorebookChoiceSheet } from "@/entities/lorebook";
import type { PanelCtx } from "./context";

export function LorebooksSheet({ ctx, open, onClose }: { ctx: PanelCtx; open: boolean; onClose: () => void }) {
  const { t } = useTranslation("conversations");
  return (
    <LorebookChoiceSheet
      api={ctx.api}
      open={open}
      value={ctx.settings?.lorebooks.ids ?? []}
      busy={ctx.busy}
      header={ctx.settings?.lorebooks.source === "conversation" && (
        <ListGroup>
          <ListRowButton title={t("settings.lorebooks.useDefault")} description={t("settings.lorebooks.useDefaultHint")} chevron={false} disabled={ctx.busy} onClick={() => void ctx.save({ lorebooks: { type: "reset" } })} />
        </ListGroup>
      )}
      onChange={(ids) => void ctx.save({ lorebooks: { type: "set", ids } })}
      onClose={onClose}
    />
  );
}
