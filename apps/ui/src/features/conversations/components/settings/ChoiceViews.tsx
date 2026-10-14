import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Check, Cpu } from "lucide-react";
import { useTranslation } from "react-i18next";
import { confirm, icon, ListGroup, ListRow, ListRowButton, LoadingState, SearchField, Switch } from "@lettuceai/crisp";
import { ModelMark } from "@/entities/provider";
import { FailureState } from "@/shared/ui/FailureState";
import { modelsQuery } from "../../data/queries";
import { lorebooksQuery } from "../../data/settings-queries";
import { useChatModels, type ChatModel } from "../../data/use-models";
import type { PanelCtx } from "./context";

const mark = <Check size={icon.md} />;

export function ModelView({ ctx, onDone }: { ctx: PanelCtx; onDone: () => void }) {
  const { t } = useTranslation("conversations");
  const status = useQuery(modelsQuery(ctx.api));
  const chat = useChatModels(ctx.api);
  const [query, setQuery] = useState("");
  const selectedId = ctx.settings?.model.id ?? null;
  const needle = query.trim().toLowerCase();
  const shown = chat.models.filter((model) => !needle || model.name.toLowerCase().includes(needle));
  const choose = async (model: ChatModel) => {
    if (!model.local) {
      const sure = await confirm({
        title: t("settings.model.remoteTitle"),
        message: t("settings.model.remoteBody", { provider: model.provider || model.name }),
        confirmLabel: t("settings.model.use"),
        cancelLabel: t("confirm.cancel"),
      });
      if (!sure) return;
    }
    if (await ctx.save({ model: { type: "set", id: model.id } })) onDone();
  };
  if (status.isPending) return <LoadingState />;
  if (status.isError) return <FailureState error={status.error} onRetry={() => void status.refetch()} />;
  return (
    <div className="space-y-3 pb-1">
      {chat.models.length > 5 && <SearchField value={query} onChange={setQuery} placeholder={t("settings.model.search")} aria-label={t("settings.model.search")} />}
      <ListGroup>
        <ListRowButton
          icon={<Cpu size={icon.lg} />}
          title={t("settings.model.useDefault")}
          description={t("settings.model.useDefaultHint")}
          chevron={false}
          disabled={ctx.busy}
          onClick={() => void ctx.save({ model: { type: "reset" } }).then((ok) => ok && onDone())}
        />
        {shown.map((model) => (
          <ListRowButton
            key={model.id}
            icon={<ModelMark modelId={model.externalId} providerKind={model.providerKind ?? ""} name={model.name} />}
            title={model.name}
            description={model.local ? t("header.onDevice") : model.provider}
            chevron={false}
            disabled={ctx.busy}
            trailing={model.id === selectedId ? mark : undefined}
            onClick={() => void choose(model)}
          />
        ))}
      </ListGroup>
      {chat.models.length === 0 && <p className="py-4 text-center text-sm text-fg-muted">{t("settings.model.none")}</p>}
    </div>
  );
}

export function LorebooksView({ ctx }: { ctx: PanelCtx }) {
  const { t } = useTranslation("conversations");
  const books = useQuery(lorebooksQuery(ctx.api, ctx.conversationId));
  const chosen = ctx.settings?.lorebooks.ids ?? [];
  const toggle = (id: string) => {
    const next = chosen.includes(id) ? chosen.filter((entry) => entry !== id) : [...chosen, id];
    void ctx.save({ lorebooks: { type: "set", ids: next } });
  };
  if (books.isPending) return <LoadingState />;
  if (books.isError) return <FailureState error={books.error} onRetry={() => void books.refetch()} />;
  return (
    <div className="space-y-4 pb-1">
      {ctx.settings?.lorebooks.source === "conversation" && (
        <ListGroup>
          <ListRowButton title={t("settings.lorebooks.useDefault")} description={t("settings.lorebooks.useDefaultHint")} chevron={false} disabled={ctx.busy} onClick={() => void ctx.save({ lorebooks: { type: "reset" } })} />
        </ListGroup>
      )}
      {books.data.length === 0 ? (
        <p className="py-4 text-center text-sm text-fg-muted">{t("settings.lorebooks.none")}</p>
      ) : (
        <ListGroup>
          {books.data.map((book) => (
            <ListRow
              key={book.id}
              title={book.name}
              trailing={<Switch checked={chosen.includes(book.id)} disabled={ctx.busy} onChange={() => toggle(book.id)} aria-label={book.name} />}
            />
          ))}
        </ListGroup>
      )}
    </div>
  );
}
