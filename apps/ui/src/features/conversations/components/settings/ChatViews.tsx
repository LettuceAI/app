import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { ListGroup, ListRowButton, LoadingState } from "@lettuceai/crisp";
import { FailureState } from "@/shared/ui/FailureState";
import { characterQuery } from "../../data/settings-queries";
import { startDirectChat } from "../../data/actions";
import { ChatHistoryList } from "../../history/ChatHistoryList";
import type { PanelCtx } from "./context";

export function HistoryView({ ctx }: { ctx: PanelCtx }) {
  return (
    <ChatHistoryList
      api={ctx.api}
      characterId={ctx.characterId}
      groupId={ctx.view.source.type === "group" ? ctx.view.source.group_id : null}
      currentConversationId={ctx.conversationId}
      busy={ctx.busy}
      run={ctx.run}
      actionsOpen={ctx.activeSheet === "history_actions"}
      onActionsOpenChange={(open) => open ? ctx.openSheet("history_actions") : ctx.closeSheet()}
      onOpen={(id) => id === ctx.conversationId ? ctx.back() : ctx.goTo(id, false)}
      onRemoveCurrent={ctx.home}
    />
  );
}

export function StartersView({ ctx }: { ctx: PanelCtx }) {
  const { t } = useTranslation("conversations");
  const character = useQuery(characterQuery(ctx.api, ctx.characterId));
  const start = async (starterId: string | null) => {
    if (!ctx.characterId) return;
    const id = await ctx.run(() => startDirectChat(ctx.api, ctx.queryClient, ctx.characterId ?? "", starterId));
    if (id) ctx.goTo(id, true);
  };
  if (character.isPending) return <LoadingState />;
  if (character.isError) return <FailureState error={character.error} onRetry={() => void character.refetch()} />;
  return (
    <div className="space-y-3 pb-1">
      <p className="text-sm text-fg-muted">{t("settings.starters.hint")}</p>
      <ListGroup>
        <ListRowButton title={t("settings.starters.none")} chevron={false} disabled={ctx.busy} onClick={() => void start(null)} />
        {character.data.draft.starters.map((starter) => (
          <ListRowButton key={starter.id} title={starter.name} chevron={false} disabled={ctx.busy} onClick={() => void start(starter.id)} />
        ))}
      </ListGroup>
    </div>
  );
}
