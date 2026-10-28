import { useMemo, useRef, useState, type ReactNode } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { useNavigate, useParams, useRouteContext, useSearch } from "@tanstack/react-router";
import { Brain, Sprout } from "lucide-react";
import { useTranslation } from "react-i18next";
import { AppBar, Avatar, Button, CrispStringsProvider, EmptyState, icon, ListGroup, LoadingState, relativeTime, Sheet, Skeleton, Tab, TabList, Tabs, Text, toast, Tooltip, useCompactLayout } from "@lettuceai/crisp";
import type { MemoryCycleView, MemoryItemView, SoulFactView, TimelineMessage } from "@/api/generated/bindings";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { FailureState } from "@/shared/ui/FailureState";
import { characterParticipant } from "../data/messages";
import { memoryCyclesQuery, memoryQuery, settingsQuery, timelineQuery } from "../data/queries";
import { characterQuery } from "../data/settings-queries";
import { ActivityTab } from "../memory/ActivityTab";
import { MemoriesTab } from "../memory/MemoryPage";
import { MemoryRow, MemorySheet, type MemoryDraft } from "../memory/MemoryItems";
import { saveMemoryDraft } from "../memory/save-memory";
import { StatusCard } from "../memory/StatusCard";
import { SummaryCard, SummarySheet } from "../memory/SummaryParts";
import { useMemoryWrite } from "../memory/use-memory-write";
import { BondTiles, Drivers, EmotionGroup, SectionHead } from "./presentation";
import { companionSoulQuery, companionStateQuery, emotionStatusQuery } from "./queries";
import { RelationshipTab } from "./RelationshipTab";
import { SoulTab } from "./SoulTab";
import { useOverlayBack } from "./use-overlay-back";
import { EffectSheet } from "./EffectSheet";

type Target = { kind: "add" } | { kind: "edit"; item: MemoryItemView };
type CompanionTab = "relationship" | "memories" | "soul" | "activity";

export function CompanionPage() {
  const { conversationId } = useParams({ strict: false });
  return conversationId ? <CompanionScreen key={conversationId} conversationId={conversationId} /> : null;
}

function CompanionScreen({ conversationId }: { conversationId: string }) {
  const { t, i18n } = useTranslation("conversations");
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const navigate = useNavigate();
  const { tab = "relationship" } = useSearch({ from: "/chat/$conversationId/companion" });
  const phone = useCompactLayout();
  const main = useRef<HTMLElement>(null);
  const writing = useRef(false);
  const timeline = useInfiniteQuery(timelineQuery(api, conversationId));
  const view = timeline.data?.pages[0]?.view;
  const characterId = view?.source.type === "direct" ? view.source.character_id : null;
  const character = useQuery(characterQuery(api, characterId));
  const companion = view?.kind === "direct" && character.data?.draft.defaults.interaction_mode === "companion";
  const settings = useQuery(settingsQuery(api, conversationId));
  const memoryOn = companion && settings.data !== undefined && settings.data.memory.mode !== "disabled";
  const dynamic = memoryOn && settings.data?.memory.mode === "dynamic";
  const state = useQuery({ ...companionStateQuery(api, conversationId), enabled: companion });
  const soul = useQuery({ ...companionSoulQuery(api, conversationId, characterId), enabled: companion && characterId !== null });
  const emotion = useQuery({ ...emotionStatusQuery(api), enabled: companion });
  const memory = useQuery({ ...memoryQuery(api, conversationId), enabled: memoryOn });
  const cycles = useInfiniteQuery({ ...memoryCyclesQuery(api, conversationId), enabled: dynamic });
  const cycleItems = useMemo(() => cycles.data?.pages.flatMap((page) => page.items) ?? [], [cycles.data]);
  const write = useMemoryWrite(api, conversationId);
  const [target, setTarget] = useState<Target | null>(null);
  const [sources, setSources] = useState<SoulFactView | null>(null);
  const [summaryOpen, setSummaryOpen] = useState(false);
  const [summarySession, setSummarySession] = useState(0);
  const [saving, setSaving] = useState(false);
  const [effectMessage, setEffectMessage] = useState<TimelineMessage | null>(null);
  const { shown: shownEffect } = useClosingValue(effectMessage);
  const { shown, session } = useClosingValue(target);
  const { shown: shownSources } = useClosingValue(sources);
  const name = character.data?.draft.profile.name ?? characterParticipant(view ?? undefined)?.name ?? view?.title ?? "";
  const participant = characterParticipant(view ?? undefined);
  const avatar = participant?.avatar?.url ?? null;

  const closeOverlay = () => {
    setTarget(null);
    setSources(null);
    setSummaryOpen(false);
    setEffectMessage(null);
  };
  useOverlayBack(target !== null || sources !== null || summaryOpen, closeOverlay);
  const openChat = () => void navigate({ to: "/chat/$conversationId", params: { conversationId }, replace: true });
  const changeTab = (next: CompanionTab) => {
    closeOverlay();
    void navigate({ to: "/chat/$conversationId/companion", params: { conversationId }, search: { tab: next }, resetScroll: false });
  };
  const edit = (item: MemoryItemView) => {
    setSources(null);
    setTarget({ kind: "edit", item });
  };
  const perform = async (action: () => Promise<boolean>) => {
    if (writing.current) return;
    writing.current = true;
    setSaving(true);
    try {
      if (await action()) setTarget(null);
    } finally {
      writing.current = false;
      setSaving(false);
    }
  };
  const save = (item: MemoryItemView | null, draft: MemoryDraft) => perform(async () => {
    const ok = await saveMemoryDraft(api, conversationId, write, item, draft, session);
    if (ok) toast.success(t(item === null ? "memory.sheet.added" : "memory.sheet.saved"));
    return ok;
  });
  const remove = (item: MemoryItemView) => perform(async () => {
    const ok = await write(`delete:${session}:${item.id}`, (current, operationId) => api.call("memoryDelete", { conversation_id: conversationId, memory_id: item.id, expected_revision: current.revision, client_operation_id: operationId }));
    if (ok) toast.success(t("memory.sheet.deleted"));
    return ok;
  });
  const saveSummary = (text: string | null) => perform(async () => {
    const ok = await write(`summary:${summarySession}:${text ?? ""}`, (current, operationId) => api.call("memorySummaryUpdate", { conversation_id: conversationId, summary: text === null ? { type: "clear" } : { type: "set", text }, expected_revision: current.revision, client_operation_id: operationId }));
    if (ok) {
      setSummaryOpen(false);
      toast.success(t(text === null ? "memory.summary.cleared" : "memory.summary.saved"));
    }
    return ok;
  });
  const revert = async (cycle: MemoryCycleView) => {
    const ok = await write(`revert:${cycle.run_id}`, (current, operationId) => api.call("memoryCycleRevert", { conversation_id: conversationId, run_id: cycle.run_id, expected_revision: current.revision, client_operation_id: operationId }));
    if (ok) toast.success(t("memory.activity.undone"));
    return ok;
  };
  const openSummary = () => {
    setSummarySession((value) => value + 1);
    setSummaryOpen(true);
  };
  const initialMessages = view?.messages;
  const latestReply = initialMessages?.items.findLast((message) => message.role === "assistant");
  const firstMessageAt = initialMessages?.next_cursor === null ? initialMessages.items.find((message) => message.role !== "system")?.created_at ?? null : null;
  const counts: Record<CompanionTab, number | undefined> = {
    relationship: undefined,
    memories: memory.data?.items.length,
    soul: soul.data?.growth.active_count,
    activity: dynamic && cycles.isSuccess && !cycles.hasNextPage ? cycleItems.length : undefined,
  };

  let body: ReactNode;
  let side: ReactNode = null;
  if (timeline.isPending || (characterId !== null && character.isPending)) body = <LoadingState />;
  else if (timeline.isError) body = <FailureState error={timeline.error} onRetry={() => void timeline.refetch()} />;
  else if (character.isError && character.data === undefined) body = <FailureState error={character.error} onRetry={() => void character.refetch()} />;
  else if (!companion) body = <EmptyState icon={<Sprout size={icon.xl} />} title={t("companion.unavailable")} description={t("companion.unavailableBody")} action={<Button onClick={openChat}>{t("companion.back")}</Button>} />;
  else if (settings.isPending) body = <LoadingState />;
  else if (settings.isError && settings.data === undefined) body = <FailureState error={settings.error} onRetry={() => void settings.refetch()} />;
  else if ((tab === "memories" || tab === "activity") && !memoryOn) body = <EmptyState icon={<Brain size={icon.xl} />} title={t("memory.off.title")} description={t("memory.off.description")} action={<Button onClick={() => void navigate({ to: "/chat/$conversationId", params: { conversationId }, search: { panel: "settings" }, state: { panelPushed: true } })}>{t("memory.off.action")}</Button>} />;
  else if (tab === "soul") body = soul.isPending ? <LoadingState /> : soul.isError && soul.data === undefined ? <FailureState error={soul.error} onRetry={() => void soul.refetch()} /> : soul.data ? <SoulTab api={api} conversationId={conversationId} soul={soul.data} onSources={setSources} /> : null;
  else if (tab === "activity") body = <div className="space-y-4">{phone && dynamic && memory.data && <StatusCard api={api} conversationId={conversationId} status={memory.data.status} />}<ActivityTab companion cycles={cycleItems} hasMore={cycles.hasNextPage} loadingMore={cycles.isFetchingNextPage} loading={dynamic && cycles.isPending} error={dynamic ? cycles.error : null} onRetry={() => void (cycles.isFetchNextPageError ? cycles.fetchNextPage() : cycles.refetch())} onLoadMore={() => void cycles.fetchNextPage()} onRevert={revert} /></div>;
  else if (tab === "memories" && memoryOn && memory.isPending) body = <LoadingState />;
  else if (tab === "memories" && memoryOn && memory.isError && memory.data === undefined) body = <FailureState error={memory.error} onRetry={() => void memory.refetch()} />;
  else if (tab === "memories") body = memory.data ? <MemoriesTab companion items={memory.data.items} dynamic={dynamic} onOpen={edit} onAdd={() => setTarget({ kind: "add" })} /> : null;
  else if (state.isError && state.data === undefined) body = <FailureState error={state.error} onRetry={() => void state.refetch()} />;
  else if (soul.isError && soul.data === undefined) body = <FailureState error={soul.error} onRetry={() => void soul.refetch()} />;
  else if (emotion.isError && emotion.data === undefined) body = <FailureState error={emotion.error} onRetry={() => void emotion.refetch()} />;
  else if (state.isPending || soul.isPending || emotion.isPending) body = <div className="grid grid-cols-2 gap-2 @2xl:grid-cols-3 @6xl:grid-cols-6" aria-hidden="true">{[0, 1, 2, 3, 4, 5].map((index) => <Skeleton key={index} className="h-28 w-full" />)}</div>;
  else if (soul.data && emotion.data && state.data !== undefined) body = <RelationshipTab api={api} state={state.data} soul={soul.data} emotion={emotion.data} items={memory.data?.items ?? null} timelineFallback={memoryOn && memory.isPending ? <LoadingState /> : memoryOn && memory.isError && memory.data === undefined ? <FailureState error={memory.error} onRetry={() => void memory.refetch()} /> : null} name={name} phone={phone} firstMessageAt={firstMessageAt} onOpen={edit} />;

  if (memory.data && (tab === "memories" || tab === "activity")) side = <div className="space-y-4">
    {dynamic && <StatusCard api={api} conversationId={conversationId} status={memory.data.status} />}
    {tab === "memories" && <>
      {state.isPending || soul.isPending ? <Skeleton className="h-28 w-full" /> : state.isError && state.data === undefined ? <FailureState error={state.error} onRetry={() => void state.refetch()} size="sm" /> : soul.isError && soul.data === undefined ? <FailureState error={soul.error} onRetry={() => void soul.refetch()} size="sm" /> : state.data && soul.data ? <section><SectionHead title={t("companion.currentState")} right={t("companion.updated", { when: relativeTime(state.data.updated_at, undefined, i18n.language).toLowerCase() })} /><div className="grid grid-cols-2 gap-2"><BondTiles state={state.data} defaults={soul.data.config.relationship_defaults} compact /></div>{emotion.data?.type === "installed" && <><div className="mt-2"><EmotionGroup group="felt" values={state.data.felt} limit={3} /></div><Drivers values={state.data.active_drivers} /></>}</section> : null}
      <SummaryCard companion summary={memory.data.summary} onOpen={openSummary} />
    </>}
  </div>;
  const sourceItems = shownSources ? (memory.data?.items ?? []).filter((item) => shownSources.source_memory_ids.includes(item.id) || shownSources.source_memory_ids.includes(item.short_id)) : [];
  const missingSources = shownSources ? shownSources.source_memory_ids.length - sourceItems.length : 0;

  return <CrispStringsProvider locale={i18n.language} strings={{ sheetClose: t("crisp.sheetClose"), dialogClose: t("companion.crisp.close"), searchFieldClear: t("companion.crisp.clearSearch") }}>
    <div className="relative flex h-full flex-col overflow-clip">
      <main ref={main} className="min-h-0 flex-1 overflow-y-auto">
        <AppBar title={<span className="flex min-w-0 items-center gap-3"><Avatar src={avatar} name={name} size={phone ? "sm" : "md"} /><Text as="span" variant="title" className="min-w-0 truncate">{name}</Text></span>} {...(phone ? {} : { meta: t("companion.title") })} onBack={openChat} backLabel={t("companion.back")} scrollRef={main} actions={companion && latestReply ? <Tooltip placement="left" content={t("companion.replyHint")}><Button size="sm" variant="ghost" onClick={() => setEffectMessage(latestReply)}>{t("companion.lastReply")}</Button></Tooltip> : undefined} filters={<Tabs value={tab} onChange={changeTab}><TabList>{(["relationship", "memories", "soul", "activity"] as const).map((id) => <Tab key={id} value={id} {...(!phone && counts[id] !== undefined ? { count: counts[id] } : {})}>{t(phone && id === "relationship" ? "companion.tabs.bond" : `companion.tabs.${id}`)}</Tab>)}</TabList></Tabs>} />
        <div className="@container px-4 pb-10 pt-4 @xl:px-8">
          {[character, settings, state, soul, emotion, memory].filter((query) => query.isError && query.data !== undefined).map((query, index) => <FailureState key={index} error={query.error} onRetry={() => void query.refetch()} size="sm" />)}
          {side && !phone ? <div className="grid gap-6 @4xl:grid-cols-[21rem_minmax(0,1fr)]"><aside className="hidden space-y-4 @4xl:sticky @4xl:top-24 @4xl:block @4xl:self-start">{side}</aside><div className="min-w-0">{body}</div></div> : body}
        </div>
      </main>
    </div>
    {memory.data && shown && <MemorySheet key={session} open={target !== null} item={shown.kind === "edit" ? shown.item : null} companionName={name} categories={memory.data.allowed_categories} busy={saving} onClose={() => setTarget(null)} onSave={(draft) => void save(shown.kind === "edit" ? shown.item : null, draft)} onDelete={() => { if (shown.kind === "edit") void remove(shown.item); }} />}
    {memory.data && <SummarySheet key={summarySession} open={summaryOpen} text={memory.data.summary?.text ?? ""} dynamic={dynamic} busy={saving} onClose={() => setSummaryOpen(false)} onSave={(text) => void saveSummary(text)} onClear={() => void saveSummary(null)} />}
    <Sheet open={sources !== null} onClose={() => setSources(null)} placement="auto" size="md" title={t("companion.soul.sourcesTitle")} description={shownSources ? t("companion.soul.sourceDescription", { category: t(`companion.soul.categories.${shownSources.category}`), text: shownSources.value }) : undefined}>
      <div className="space-y-3 pb-2">{!memoryOn ? <Text variant="hint">{t("memory.off.description")}</Text> : memory.isPending ? <LoadingState /> : memory.isError && memory.data === undefined ? <FailureState error={memory.error} onRetry={() => void memory.refetch()} /> : sourceItems.length > 0 ? <ListGroup label={t("companion.memories.total", { count: sourceItems.length })}>{sourceItems.map((item) => <MemoryRow key={item.id} companion item={item} onOpen={() => edit(item)} />)}</ListGroup> : <Text variant="hint">{t("companion.soul.sourcesEmpty")}</Text>}{sourceItems.length > 0 && missingSources > 0 && <Text variant="hint">{t("companion.soul.sourcesMissing", { count: missingSources })}</Text>}</div>
    </Sheet>
    {shownEffect && <EffectSheet key={shownEffect.id} api={api} conversationId={conversationId} message={shownEffect} open={effectMessage !== null} onClose={() => setEffectMessage(null)} />}
  </CrispStringsProvider>;
}
