import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Archive, ArchiveRestore, Download, MoreHorizontal, Pencil, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  Badge, Button, confirm, icon, IconButton, Input, ListGroup, ListRowButton, LoadingState, Menu, MenuContent, MenuItem, MenuTrigger, SearchField, toast,
} from "@lettuceai/crisp";
import type { ConversationSummary } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { exportChat, removeChat, renameChat, setArchived } from "../../data/chat-actions";
import { formatTime } from "../../data/messages";
import { characterChatsQuery, characterQuery } from "../../data/settings-queries";
import { startDirectChat } from "../../data/actions";
import type { PanelCtx } from "./context";

function RenameRow({ initial, onSave, onCancel, busy }: { initial: string; onSave: (title: string) => void; onCancel: () => void; busy: boolean }) {
  const { t } = useTranslation("conversations");
  const [text, setText] = useState(initial);
  return (
    <div className="flex items-center gap-2 px-4 py-3">
      <Input
        data-autofocus
        value={text}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && text.trim()) onSave(text);
          if (event.key === "Escape") onCancel();
        }}
        aria-label={t("settings.rename.field")}
      />
      <Button variant="primary" size="sm" disabled={busy || !text.trim()} onClick={() => onSave(text)}>{t("settings.rename.save")}</Button>
      <Button variant="ghost" size="sm" onClick={onCancel}>{t("confirm.cancel")}</Button>
    </div>
  );
}

export function HistoryView({ ctx }: { ctx: PanelCtx }) {
  const { t, i18n } = useTranslation("conversations");
  const chats = useQuery(characterChatsQuery(ctx.api, ctx.conversationId, ctx.characterId));
  const [query, setQuery] = useState("");
  const [renaming, setRenaming] = useState<string | null>(null);
  const needle = query.trim().toLowerCase();
  if (chats.isPending) return <LoadingState />;
  if (chats.isError) return <FailureState error={chats.error} onRetry={() => void chats.refetch()} />;
  const shown = chats.data
    .filter((chat) => !needle || chat.title.toLowerCase().includes(needle) || (chat.last_message_preview ?? "").toLowerCase().includes(needle))
    .sort((a, b) => b.updated_at - a.updated_at);

  const remove = async (chat: ConversationSummary) => {
    const sure = await confirm({
      title: t("settings.delete.title"),
      message: t("settings.delete.body", { title: chat.title || t("settings.history.untitled") }),
      confirmLabel: t("settings.delete.confirm"),
      cancelLabel: t("confirm.cancel"),
      destructive: true,
    });
    if (!sure) return;
    const done = await ctx.run(() => removeChat(ctx.api, ctx.queryClient, chat.id).then(() => true));
    if (done && chat.id === ctx.conversationId) ctx.home();
  };
  const exportOne = async (chat: ConversationSummary) => {
    const uri = await ctx.run(() => exportChat(ctx.api, chat.id, chat.title));
    if (uri) toast.success(t("settings.export.done"));
  };

  return (
    <div className="space-y-3 pb-1">
      {chats.data.length > 4 && <SearchField value={query} onChange={setQuery} placeholder={t("settings.history.search")} aria-label={t("settings.history.search")} />}
      {shown.length === 0 ? (
        <p className="py-6 text-center text-sm text-fg-muted">{needle ? t("settings.history.noMatch") : t("settings.history.empty")}</p>
      ) : (
        <ListGroup>
          {shown.map((chat) =>
            renaming === chat.id ? (
              <RenameRow
                key={chat.id}
                initial={chat.title}
                busy={ctx.busy}
                onCancel={() => setRenaming(null)}
                onSave={(title) => void ctx.run(() => renameChat(ctx.api, ctx.queryClient, chat.id, chat.revision, title).then(() => true)).then((ok) => ok && setRenaming(null))}
              />
            ) : (
              <div key={chat.id} className="flex items-center">
                <ListRowButton
                  title={chat.title || t("settings.history.untitled")}
                  description={
                    <span className="block">
                      {chat.last_message_preview && <span className="block truncate">{chat.last_message_preview}</span>}
                      <span className="block text-xs text-fg-3">{t("settings.history.meta", { count: chat.message_count, date: formatTime(chat.updated_at, i18n.language) })}</span>
                    </span>
                  }
                  trailing={
                    <span className="flex items-center gap-1.5">
                      {chat.id === ctx.conversationId && <Badge tone="accent">{t("settings.history.current")}</Badge>}
                      {chat.archived && <Badge tone="neutral">{t("settings.history.archived")}</Badge>}
                    </span>
                  }
                  chevron={false}
                  className="min-w-0 flex-1"
                  onClick={() => (chat.id === ctx.conversationId ? ctx.back() : ctx.goTo(chat.id, false))}
                />
                <Menu>
                  <MenuTrigger>
                    <IconButton label={t("settings.history.actions", { title: chat.title })} className="mr-2 shrink-0"><MoreHorizontal size={icon.md} /></IconButton>
                  </MenuTrigger>
                  <MenuContent align="end">
                    <MenuItem icon={<Pencil size={icon.md} />} onSelect={() => setRenaming(chat.id)}>{t("settings.history.rename")}</MenuItem>
                    <MenuItem icon={<Download size={icon.md} />} onSelect={() => void exportOne(chat)}>{t("settings.history.export")}</MenuItem>
                    <MenuItem
                      icon={chat.archived ? <ArchiveRestore size={icon.md} /> : <Archive size={icon.md} />}
                      onSelect={() => void ctx.run(() => setArchived(ctx.api, ctx.queryClient, chat.id, !chat.archived))}
                    >
                      {chat.archived ? t("settings.history.restore") : t("settings.history.archive")}
                    </MenuItem>
                    <MenuItem danger icon={<Trash2 size={icon.md} />} onSelect={() => void remove(chat)}>{t("settings.history.delete")}</MenuItem>
                  </MenuContent>
                </Menu>
              </div>
            ),
          )}
        </ListGroup>
      )}
    </div>
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
