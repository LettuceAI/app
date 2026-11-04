import { useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useBlocker, useNavigate, useRouteContext } from "@tanstack/react-router";
import { History, LayoutGrid, List, MessageSquare, Pencil, Plus, Search, Trash2, Users } from "lucide-react";
import { useTranslation } from "react-i18next";
import { AvatarGroup, Button, cn, confirm, dismissTopLayer, EmptyState, icon, IconButton, InlineMarkdown, ListGroup, ListRowButton, MediaCardSkeleton, Menu, MenuContent, MenuItem, MenuTrigger, relativeTime, Sheet, Skeleton, SkeletonRow, toast, useCompactLayout, useLongPress } from "@lettuceai/crisp";
import { Page } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import { isApiFailure } from "@/api/client";
import type { ConversationSummary } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { ChatMenu, type ChatMenuTarget } from "@/features/home";
import { FailureState } from "@/shared/ui/FailureState";
import { MobileSettingsButton } from "@/shared/ui/MobileSettingsButton";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { GroupCard, type GroupEntry } from "./components/GroupCards";
import { castQuery, groupChatsQuery, groupsQuery } from "./data/queries";
import { faces } from "./data/faces";
import { launchGroup, refreshGroups } from "./data/actions";

const layouts = ["list", "gallery", "recent"] as const;
type Layout = typeof layouts[number];
const layoutIcons = { list: List, gallery: LayoutGrid, recent: History };

function RecentRow({ entry, chat, held, onOpen, onMenu }: { entry: GroupEntry | undefined; chat: ConversationSummary; held: boolean; onOpen: () => void; onMenu: () => void }) {
  const { t, i18n } = useTranslation("groups");
  const press = useLongPress(onMenu);
  return <button type="button" aria-haspopup="dialog" {...press.handlers} onClick={press.guardClick(onOpen)} className={cn("touch-target flex w-full items-start gap-3 rounded-2xl px-3 py-2.5 text-left transition-colors motion-instant", held ? "bg-fill-2" : "hover:bg-fill")}>
    <AvatarGroup people={entry ? faces(entry.members) : chat.avatars.map((avatar, index) => ({ name: `${chat.title} ${index + 1}`, src: avatar.url }))} max={3} size="sm" />
    <span className="min-w-0 flex-1">
      <span className="flex min-w-0 items-baseline gap-2">
        <span className="min-w-0 truncate text-base font-semibold text-fg">{entry?.group.draft.name ?? chat.title}</span>
        {entry && <span className="min-w-0 flex-1 truncate text-sm text-fg-3">{chat.title}</span>}
        <span className="ml-auto shrink-0 text-2xs tabular-nums text-fg-3">{relativeTime(chat.updated_at, undefined, i18n.language)}</span>
      </span>
      <span className="mt-0.5 line-clamp-2 wrap-anywhere text-sm leading-snug text-fg-3">{chat.last_message_preview ? <InlineMarkdown>{chat.last_message_preview}</InlineMarkdown> : t("noMessages")}</span>
    </span>
  </button>;
}

function GroupRowSkeleton({ compact }: { compact: boolean }) {
  return <div className="flex w-full items-center gap-4 rounded-2xl border border-line bg-surface-1 p-3.5 @xl:p-4">
    <span className="flex shrink-0 items-center">{[0, 1, 2].map((i) => <Skeleton key={i} shape="circle" className={cn(compact ? "h-10 w-10" : "h-14 w-14", i > 0 && "-ml-3")} />)}</span>
    <span className="min-w-0 flex-1 space-y-2"><Skeleton width="40%" /><Skeleton width="55%" className="h-3" /><Skeleton width="80%" className="h-3" /></span>
  </div>;
}

export function GroupsPage() {
  const { t, i18n } = useTranslation("groups");
  const { t: common } = useTranslation();
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const status = useQuery(appStatusQuery(api));
  const phone = useCompactLayout() || status.data?.platform === "android" || status.data?.platform === "ios";
  const groups = useQuery(groupsQuery(api));
  const cast = useQuery(castQuery(api));
  const chats = useQuery(groupChatsQuery(api));
  const stored = status.data?.ui_state.groupHomeLayout;
  const [now] = useState(() => Date.now());
  const [chosen, setChosen] = useState<Layout | null>(null);
  const layout = chosen ?? layouts.find((id) => id === stored) ?? "list";
  const [query, setQuery] = useState("");
  const [layoutOpen, setLayoutOpen] = useState(false);
  const [menu, setMenu] = useState<GroupEntry | null>(null);
  const [chatMenu, setChatMenu] = useState<ChatMenuTarget | null>(null);
  const { shown } = useClosingValue(menu);
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const launchAttempts = useRef(new Map<string, string>());
  const mainRef = useRef<HTMLElement>(null);
  useBlocker({ shouldBlockFn: ({ action }) => action === "BACK" && dismissTopLayer(), enableBeforeUnload: false });
  const entries = useMemo<GroupEntry[]>(() => (groups.data ?? []).map(({ group }) => ({ group, members: [...group.draft.members].sort((a, b) => a.ordinal - b.ordinal).flatMap((member) => cast.data?.filter((item) => item.id === member.character_id) ?? []), chats: (chats.data ?? []).filter((chat) => chat.source.type === "group" && chat.source.group_id === group.id) })), [groups.data, cast.data, chats.data]);
  const needle = query.trim().toLocaleLowerCase();
  const matches = (entry: GroupEntry) => [entry.group.draft.name, ...entry.members.map((member) => member.name)].some((text) => text.toLocaleLowerCase().includes(needle));
  const visible = entries.filter(matches);
  const byId = new Map(entries.map((entry) => [entry.group.id, entry]));
  const recent = (chats.data ?? []).filter((chat) => {
    const entry = chat.source.type === "group" ? byId.get(chat.source.group_id) : undefined;
    return !needle || (entry && matches(entry)) || [chat.title, chat.last_message_preview].some((text) => text?.toLocaleLowerCase().includes(needle));
  });
  const count = layout === "recent" ? recent.length : visible.length;
  const total = layout === "recent" ? chats.data?.length : entries.length;
  const run = async (task: () => Promise<void>) => {
    if (busyRef.current) return;
    busyRef.current = true; setBusy(true);
    try { await task(); }
    catch (error) { toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`)); }
    finally { busyRef.current = false; setBusy(false); }
  };
  const openChat = (id: string) => void navigate({ to: "/chat/$conversationId", params: { conversationId: id } });
  const startChat = (entry: GroupEntry) => void run(async () => {
    let operationId = launchAttempts.current.get(entry.group.id);
    if (!operationId) { operationId = crypto.randomUUID(); launchAttempts.current.set(entry.group.id, operationId); }
    const id = await launchGroup(api, queryClient, entry.group.id, operationId);
    launchAttempts.current.delete(entry.group.id); setMenu(null); openChat(id);
  });
  const openGroup = (entry: GroupEntry) => entry.chats[0] ? openChat(entry.chats[0].id) : startChat(entry);
  const chooseLayout = (next: Layout) => {
    const previous = layout; setChosen(next); setLayoutOpen(false); mainRef.current?.scrollTo({ top: 0 });
    void api.call("appUiStateUpdate", { patch: { groupHomeLayout: next } }).then(() => queryClient.invalidateQueries({ queryKey: queryKeys.app.status() }), (error: unknown) => { setChosen(previous); toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`)); });
  };
  const LayoutIcon = layoutIcons[layout];
  const failed = [groups, cast, chats].find((query) => query.isError);
  const loading = groups.isPending || cast.isPending || chats.isPending;
  const grid = "grid grid-cols-2 gap-3 @3xl:grid-cols-3 @5xl:grid-cols-4 @6xl:grid-cols-5";
  return <Page title={t("title")} meta={loading ? undefined : needle ? t("count", { shown: new Intl.NumberFormat(i18n.language).format(count), total: new Intl.NumberFormat(i18n.language).format(total ?? 0) }) : total ? new Intl.NumberFormat(i18n.language).format(total) : undefined} searchValue={query} onSearchChange={setQuery} searchPlaceholder={t(layout === "recent" ? "searchChats" : "search")} mainRef={mainRef} padding="none" contentClassName={cn("pb-10 pt-1", phone ? "px-3" : "px-6")} actions={<>{phone ? <IconButton label={t("layout.current", { layout: t(`layout.${layout}`) })} onClick={() => setLayoutOpen(true)}><LayoutIcon size={icon.lg} /></IconButton> : <Menu><MenuTrigger><IconButton label={t("layout.current", { layout: t(`layout.${layout}`) })}><LayoutIcon size={icon.lg} /></IconButton></MenuTrigger><MenuContent align="end">{layouts.map((id) => { const Icon = layoutIcons[id]; return <MenuItem key={id} icon={<Icon size={icon.lg} />} selected={id === layout} onSelect={() => chooseLayout(id)}>{t(`layout.${id}`)}</MenuItem>; })}</MenuContent></Menu>}<MobileSettingsButton /></>}>
    {failed ? <FailureState error={failed.error} onRetry={() => void failed.refetch()} /> : loading ? layout === "gallery" ? <div className={grid}>{Array.from({ length: 10 }, (_, i) => <MediaCardSkeleton key={i} />)}</div> : layout === "recent" ? <div className={cn("space-y-1 px-3", !phone && "mx-auto max-w-5xl")}>{Array.from({ length: 8 }, (_, i) => <SkeletonRow key={i} />)}</div> : <div className={cn("grid gap-3", !phone && "grid-cols-2")}>{Array.from({ length: 6 }, (_, i) => <GroupRowSkeleton key={i} compact={phone} />)}</div> : entries.length === 0 ? <EmptyState icon={<Users size={icon.xl} />} title={t("empty.title")} description={t((cast.data?.length ?? 0) >= 2 ? "empty.body" : "empty.needCharacters")} action={<Button variant="tonal" onClick={() => void navigate({ to: (cast.data?.length ?? 0) >= 2 ? "/groups/new" : "/characters/new" })}>{t((cast.data?.length ?? 0) >= 2 ? "create.submit" : "members.create")}</Button>} /> : needle && count === 0 ? <EmptyState icon={<Search size={icon.xl} />} title={t("noMatch", { query: query.trim() })} description={t("noMatchBody")} action={<Button variant="secondary" onClick={() => setQuery("")}>{t("clear")}</Button>} /> : layout === "recent" ? recent.length === 0 ? <EmptyState icon={<MessageSquare size={icon.xl} />} title={t("empty.chatsTitle")} description={t("empty.chatsBody")} action={<Button variant="secondary" onClick={() => chooseLayout("list")}>{t("empty.showGroups")}</Button>} /> : <div className={cn("space-y-5", !phone && "mx-auto max-w-5xl")}>{(["today", "week", "earlier"] as const).map((bucket) => {
      const day = 24 * 60 * 60 * 1000;
      const items = recent.filter((chat) => { const age = now - chat.updated_at; return (age < day ? "today" : age < 7 * day ? "week" : "earlier") === bucket; });
      return items.length > 0 && <section key={bucket} aria-label={t(`recent.${bucket}`)}><h2 className="px-3 pb-1 text-sm font-medium text-fg-3">{t(`recent.${bucket}`)}</h2>{items.map((chat) => <RecentRow key={chat.id} chat={chat} entry={chat.source.type === "group" ? byId.get(chat.source.group_id) : undefined} held={chatMenu?.chat.id === chat.id} onOpen={() => openChat(chat.id)} onMenu={() => setChatMenu({ chat, character: null })} />)}</section>;
    })}</div> : <div className={layout === "gallery" ? grid : cn("grid gap-3", !phone && "grid-cols-2")}>{visible.map((entry) => <GroupCard key={entry.group.id} entry={entry} layout={layout} compact={phone} held={menu?.group.id === entry.group.id} onOpen={() => openGroup(entry)} onMenu={() => setMenu(entry)} />)}</div>}
    <Sheet open={layoutOpen} onClose={() => setLayoutOpen(false)} size="sm" title={t("layout.title")}><ListGroup>{layouts.map((id) => { const Icon = layoutIcons[id]; return <ListRowButton key={id} icon={<Icon size={icon.lg} />} title={t(`layout.${id}`)} description={t(`layout.${id}Body`)} chevron={false} aria-pressed={layout === id} onClick={() => chooseLayout(id)} />; })}</ListGroup></Sheet>
    <Sheet open={Boolean(menu)} onClose={() => { if (!busy) setMenu(null); }} dismissible={!busy} size="sm" title={shown?.group.draft.name} leftAction={shown && <AvatarGroup people={faces(shown.members)} max={2} size="xs" />}>
      {shown && <div className="space-y-4 pb-1"><ListGroup>{shown.chats[0] && <ListRowButton icon={<MessageSquare size={icon.lg} />} title={t("menu.continue")} description={shown.chats[0].title} chevron={false} disabled={busy} onClick={() => { setMenu(null); openChat(shown.chats[0]!.id); }} />}<ListRowButton icon={<Plus size={icon.lg} />} title={t("menu.newChat")} chevron={false} disabled={busy} onClick={() => startChat(shown)} /></ListGroup>
        {shown.chats.length > 1 && <ListGroup label={t("menu.chats")}>{shown.chats.map((chat) => <ListRowButton key={chat.id} title={chat.title} description={relativeTime(chat.updated_at, undefined, i18n.language)} disabled={busy} onClick={() => { setMenu(null); openChat(chat.id); }} />)}</ListGroup>}
        <ListGroup><ListRowButton icon={<Pencil size={icon.lg} />} title={t("menu.edit")} disabled={busy} onClick={() => { setMenu(null); void navigate({ to: "/groups/$groupId/edit", params: { groupId: shown.group.id } }); }} /></ListGroup>
        <ListGroup><ListRowButton icon={<Trash2 size={icon.lg} />} title={t("menu.delete")} tone="danger" disabled={busy} chevron={false} onClick={() => void run(async () => {
          if (!await confirm({ title: t("menu.deleteTitle", { name: shown.group.draft.name }), message: t("menu.deleteBody"), confirmLabel: t("menu.delete"), cancelLabel: common("actions.cancel"), destructive: true })) return;
          await api.call("groupDelete", { group_id: shown.group.id, expected_revision: shown.group.revision, client_operation_id: crypto.randomUUID() });
          setMenu(null); await refreshGroups(queryClient);
        })} /></ListGroup>
      </div>}
    </Sheet>
    <ChatMenu api={api} target={chatMenu} onClose={() => setChatMenu(null)} />
  </Page>;
}
