import { useEffect, useState, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Archive, ArchiveRestore, ChevronDown, Download, MoreHorizontal, Pencil, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  Badge, Button, confirm, EmptyState, FilterBar, icon, IconButton, Input, ListGroup, ListRowButton, LoadingState, Menu, MenuContent, MenuItem, MenuTrigger, Pagination, SearchField, Sheet, toast, useCompactLayout,
} from "@lettuceai/crisp";
import type { ConversationSummary, LifecycleFilter } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { exportChat, removeChat, renameChat, setArchived } from "../data/chat-actions";
import { characterChatsQuery } from "../data/settings-queries";
import type { ApiClient } from "@/api/client";

interface ChatHistoryListProps {
  api: ApiClient;
  characterId: string | null;
  currentConversationId?: string;
  busy: boolean;
  run: <T>(task: () => Promise<T>) => Promise<T | undefined>;
  actionsOpen: boolean;
  onActionsOpenChange: (open: boolean) => void;
  onOpen: (id: string) => void;
  onRemoveCurrent?: () => void;
  emptyAction?: ReactNode;
}

const historyPageSize = 10;
const historyFilters: readonly LifecycleFilter[] = ["all", "active", "archived"];
const historySorts = ["recent", "oldest"] as const;

export function RenameRow({ initial, onSave, onCancel, busy, label, saveLabel }: { initial: string; onSave: (title: string) => void; onCancel: () => void; busy: boolean; label?: string; saveLabel?: string }) {
  const { t } = useTranslation("conversations");
  const [text, setText] = useState(initial);
  return (
    <div className="flex items-center gap-2 px-4 py-3">
      <Input
        data-autofocus
        value={text}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && !busy && text.trim()) onSave(text);
          if (event.key === "Escape") onCancel();
        }}
        aria-label={label ?? t("settings.rename.field")}
      />
      <Button variant="primary" size="sm" disabled={busy || !text.trim()} onClick={() => onSave(text)}>{saveLabel ?? t("settings.rename.save")}</Button>
      <Button variant="ghost" size="sm" onClick={onCancel}>{t("confirm.cancel")}</Button>
    </div>
  );
}

export function ChatHistoryList({ api, characterId, currentConversationId, busy, run, actionsOpen, onActionsOpenChange, onOpen, onRemoveCurrent, emptyAction }: ChatHistoryListProps) {
  const { t, i18n } = useTranslation("conversations");
  const queryClient = useQueryClient();
  const phone = useCompactLayout();
  const chats = useQuery(characterChatsQuery(api, characterId));
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<LifecycleFilter>("all");
  const [sort, setSort] = useState<(typeof historySorts)[number]>("recent");
  const [page, setPage] = useState(1);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [actions, setActions] = useState<ConversationSummary | null>(null);
  useEffect(() => {
    if (!phone && actionsOpen) onActionsOpenChange(false);
  }, [phone, actionsOpen, onActionsOpenChange]);
  const needle = query.trim().toLowerCase();
  const shown = (chats.data ?? [])
    .filter((chat) => !needle || chat.title.toLowerCase().includes(needle) || (chat.last_message_preview ?? "").toLowerCase().includes(needle))
    .filter((chat) => filter === "all" || chat.archived === (filter === "archived"))
    .sort((a, b) => (sort === "recent" ? b.updated_at - a.updated_at : a.updated_at - b.updated_at) || a.id.localeCompare(b.id));
  const pageCount = Math.max(1, Math.ceil(shown.length / historyPageSize));
  const currentPage = Math.min(page, pageCount);
  const visible = shown.slice((currentPage - 1) * historyPageSize, currentPage * historyPageSize);
  if (chats.isSuccess && page !== currentPage) setPage(currentPage);
  if (chats.isPending) return <LoadingState />;
  if (chats.isError) return <FailureState error={chats.error} onRetry={() => void chats.refetch()} />;
  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" });

  const remove = async (chat: ConversationSummary) => {
    const sure = await confirm({
      title: t("settings.delete.title"),
      message: t("settings.delete.body", { title: chat.title || t("settings.history.untitled") }),
      confirmLabel: t("settings.delete.confirm"),
      cancelLabel: t("confirm.cancel"),
      destructive: true,
    });
    if (!sure) return;
    const done = await run(() => removeChat(api, queryClient, chat.id).then(() => true));
    if (done && chat.id === currentConversationId) onRemoveCurrent?.();
  };
  const exportOne = async (chat: ConversationSummary) => {
    const uri = await run(() => exportChat(api, chat.id, chat.title));
    if (uri) toast.success(t("settings.export.done"));
  };

  return (
    <div className="space-y-3 pb-1">
      <SearchField value={query} onChange={(value) => { setQuery(value); setPage(1); }} placeholder={t("settings.history.search")} aria-label={t("settings.history.search")} />
      <div className="flex flex-wrap items-center justify-between gap-3">
        <FilterBar
          aria-label={t("settings.history.filter")}
          options={historyFilters.map((value) => ({ value, label: t(`settings.history.filters.${value}`) }))}
          value={filter}
          onChange={(value) => { setFilter(value); setPage(1); }}
        />
        <Menu>
          <MenuTrigger>
            <Button variant="secondary" size="sm" aria-label={t("settings.history.sort")} trailing={<ChevronDown size={icon.sm} />}>{t(`settings.history.sorts.${sort}`)}</Button>
          </MenuTrigger>
          <MenuContent align="end">
            {historySorts.map((value) => <MenuItem key={value} selected={sort === value} onSelect={() => { setSort(value); setPage(1); }}>{t(`settings.history.sorts.${value}`)}</MenuItem>)}
          </MenuContent>
        </Menu>
      </div>
      <p role="status" className="text-xs text-fg-3">{t("settings.history.results", { count: shown.length, formattedCount: new Intl.NumberFormat(i18n.language).format(shown.length) })}</p>
      {shown.length === 0 ? (
        <EmptyState size="sm" title={needle || filter !== "all" ? t("settings.history.noMatch") : t("settings.history.empty")} action={!needle && filter === "all" ? emptyAction : undefined} />
      ) : (
        <ListGroup>
          {visible.map((chat) =>
            renaming === chat.id ? (
              <RenameRow
                key={chat.id}
                initial={chat.title}
                busy={busy}
                onCancel={() => setRenaming(null)}
                onSave={(title) => void run(() => renameChat(api, queryClient, chat.id, chat.revision, title).then(() => true)).then((ok) => ok && setRenaming(null))}
              />
            ) : (
              <div key={chat.id} className="flex items-center">
                <ListRowButton
                  title={chat.title || t("settings.history.untitled")}
                  description={
                    <span className="block">
                      {chat.last_message_preview && <span className="block truncate">{chat.last_message_preview}</span>}
                      <span className="block text-xs text-fg-3">{t("settings.history.meta", { count: chat.message_count, date: date.format(chat.updated_at) })}</span>
                    </span>
                  }
                  trailing={
                    <span className="flex items-center gap-1.5">
                      {chat.id === currentConversationId && <Badge tone="accent">{t("settings.history.current")}</Badge>}
                      {chat.archived && <Badge tone="neutral">{t("settings.history.archived")}</Badge>}
                    </span>
                  }
                  chevron={false}
                  className="min-w-0 flex-1"
                  onClick={() => onOpen(chat.id)}
                />
                {phone ? <IconButton
                  label={t("settings.history.actions", { title: chat.title })}
                  className="mr-2 shrink-0"
                  disabled={busy}
                  onClick={() => {
                    setActions(chat);
                    onActionsOpenChange(true);
                  }}
                ><MoreHorizontal size={icon.md} /></IconButton> : (
                  <Menu>
                    <MenuTrigger>
                      <IconButton label={t("settings.history.actions", { title: chat.title })} disabled={busy} className="mr-2 shrink-0"><MoreHorizontal size={icon.md} /></IconButton>
                    </MenuTrigger>
                    <MenuContent align="end">
                      <MenuItem icon={<Pencil size={icon.md} />} onSelect={() => setRenaming(chat.id)}>{t("settings.history.rename")}</MenuItem>
                      <MenuItem icon={<Download size={icon.md} />} onSelect={() => void exportOne(chat)}>{t("settings.history.export")}</MenuItem>
                      <MenuItem
                        icon={chat.archived ? <ArchiveRestore size={icon.md} /> : <Archive size={icon.md} />}
                        onSelect={() => void run(() => setArchived(api, queryClient, chat.id, !chat.archived))}
                      >
                        {chat.archived ? t("settings.history.restore") : t("settings.history.archive")}
                      </MenuItem>
                      <MenuItem danger icon={<Trash2 size={icon.md} />} onSelect={() => void remove(chat)}>{t("settings.history.delete")}</MenuItem>
                    </MenuContent>
                  </Menu>
                )}
              </div>
            ),
          )}
        </ListGroup>
      )}
      <Pagination page={currentPage} pageCount={pageCount} onChange={setPage} className="flex-wrap justify-center" />
      {phone && actions && (
        <Sheet open={actionsOpen} onClose={() => onActionsOpenChange(false)} size="sm" title={t("settings.history.actions", { title: actions.title || t("settings.history.untitled") })} dismissible={!busy}>
          <ListGroup>
            <ListRowButton
              icon={<Pencil size={icon.md} />}
              title={t("settings.history.rename")}
              chevron={false}
              disabled={busy}
              onClick={() => {
                onActionsOpenChange(false);
                setRenaming(actions.id);
              }}
            />
            <ListRowButton
              icon={<Download size={icon.md} />}
              title={t("settings.history.export")}
              chevron={false}
              disabled={busy}
              onClick={() => {
                onActionsOpenChange(false);
                void exportOne(actions);
              }}
            />
            <ListRowButton
              icon={actions.archived ? <ArchiveRestore size={icon.md} /> : <Archive size={icon.md} />}
              title={actions.archived ? t("settings.history.restore") : t("settings.history.archive")}
              chevron={false}
              disabled={busy}
              onClick={() => {
                onActionsOpenChange(false);
                void run(() => setArchived(api, queryClient, actions.id, !actions.archived));
              }}
            />
            <ListRowButton
              icon={<Trash2 size={icon.md} />}
              title={t("settings.history.delete")}
              tone="danger"
              chevron={false}
              disabled={busy}
              onClick={() => {
                onActionsOpenChange(false);
                void remove(actions);
              }}
            />
          </ListGroup>
        </Sheet>
      )}
    </div>
  );
}
