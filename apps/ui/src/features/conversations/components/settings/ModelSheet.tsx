import { useTranslation } from "react-i18next";
import { confirm } from "@lettuceai/crisp";
import { ModelChoiceSheet } from "@/entities/model";
import { useChatModels } from "../../data/use-models";
import type { PanelCtx } from "./context";

/** What the model picker needs of the settings panel, so a chat widget can open it without a panel around it. */
export type ModelCtx = Pick<PanelCtx, "api" | "settings" | "busy" | "save">;

export function ModelSheet({ ctx, open, onClose }: { ctx: ModelCtx; open: boolean; onClose: () => void }) {
  const { t } = useTranslation("conversations");
  const chat = useChatModels(ctx.api);
  const chosen = ctx.settings?.model.source === "conversation";
  const choose = async (id: string | null) => {
    if (id === null) return ctx.save({ model: { type: "reset" } });
    const model = chat.find(id);
    if (model && !model.local) {
      const sure = await confirm({
        title: t("settings.model.remoteTitle"),
        message: t("settings.model.remoteBody", { provider: model.provider || model.name }),
        confirmLabel: t("settings.model.use"),
        cancelLabel: t("confirm.cancel"),
      });
      if (!sure) return false;
    }
    return ctx.save({ model: { type: "set", id } });
  };
  return (
    <ModelChoiceSheet
      api={ctx.api}
      open={open}
      onClose={onClose}
      title={t("settings.model.title")}
      value={chosen ? (ctx.settings?.model.id ?? null) : null}
      requirement={{ kind: "chat" }}
      nullChoice={{
        title: t("settings.model.useDefault"),
        description: t("settings.model.useDefaultHint"),
        selected: !chosen,
      }}
      busy={ctx.busy}
      onChange={choose}
    />
  );
}
