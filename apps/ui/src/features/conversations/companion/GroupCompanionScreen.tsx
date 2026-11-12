import { useMemo, useRef, useState, type ReactNode } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Brain, Check, Sprout } from "lucide-react";
import { useTranslation } from "react-i18next";
import { AppBar, Avatar, AvatarGroup, Button, CrispStringsProvider, EmptyState, FilterBar, icon, ListGroup, ListRow, ListRowButton, LoadingState, relativeTime, Sheet, Skeleton, Tab, TabList, Tabs, Text, toast, useCompactLayout } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { ConversationView, GroupCompanionMemberView, GroupMemberRelationshipView, MemoryCycleView, MemoryItemView } from "@/api/generated/bindings";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { FailureState } from "@/shared/ui/FailureState";
import { memoryCyclesQuery, memoryPerspectiveQuery, memoryQuery, settingsQuery } from "../data/queries";
import { ActivityTab } from "../memory/ActivityTab";
import { MemoriesTab } from "../memory/MemoryPage";
import { MemorySheet, type MemoryDraft } from "../memory/MemoryItems";
import { saveMemoryDraft } from "../memory/save-memory";
import { StatusCard } from "../memory/StatusCard";
import { SummaryCard, SummarySheet } from "../memory/SummaryParts";
import { useMemoryWrite } from "../memory/use-memory-write";
import { BondTiles, EmotionGroup, SectionHead } from "./presentation";
import { companionSoulQuery, emotionStatusQuery, groupCompanionStateQuery } from "./queries";
import { EmotionInstallNotice } from "./EmotionInstallNotice";
import { SoulTab } from "./SoulTab";
import { closenessBand, signed, stanceAxes } from "./values";
import { useOverlayBack } from "@/shared/hooks/useOverlayBack";

export type GroupCompanionTab = "relationship" | "memories" | "soul" | "activity";

type Target = { kind: "add" } | { kind: "edit"; item: MemoryItemView };
const SHARED = "shared";

interface Stance {
  from: GroupCompanionMemberView;
  to: GroupCompanionMemberView;
  value: GroupMemberRelationshipView;
}

function stanceOf(from: GroupCompanionMemberView, to: GroupCompanionMemberView): GroupMemberRelationshipView | null {
  return from.member_relationships.find((entry) => entry.target_character_id === to.character_id) ?? null;
}

export function GroupCompanionScreen({ api, conversationId, view, tab, onTab, onBack }: { api: ApiClient; conversationId: string; view: ConversationView; tab: GroupCompanionTab; onTab: (tab: GroupCompanionTab) => void; onBack: () => void }) {
  const { t, i18n } = useTranslation("conversations");
  const navigate = useNavigate();
  const phone = useCompactLayout();
  const main = useRef<HTMLElement>(null);
  const state = useQuery(groupCompanionStateQuery(api, conversationId));
  const emotion = useQuery(emotionStatusQuery(api));
  const [picked, setPicked] = useState<string | null>(null);
  const [stance, setStance] = useState<Stance | null>(null);
  const { shown: shownStance } = useClosingValue(stance);
  useOverlayBack(stance !== null, () => setStance(null));
  const members = state.data?.members ?? [];
  const chosen = members.find((entry) => entry.character_id === picked) ?? null;
  const member = chosen ?? members[0] ?? null;
  const soul = useQuery(companionSoulQuery(api, conversationId, member?.character_id ?? null));
  const settings = useQuery(settingsQuery(api, conversationId));
  const memoryOn = settings.data !== undefined && settings.data.memory.mode !== "disabled";
  const dynamic = memoryOn && settings.data?.memory.mode === "dynamic";
  const scopeId = tab === "memories" ? (chosen?.character_id ?? null) : null;
  const shared = useQuery({ ...memoryQuery(api, conversationId), enabled: memoryOn });
  const perspective = useQuery({ ...memoryPerspectiveQuery(api, conversationId, scopeId), enabled: memoryOn && scopeId !== null });
  const memory = scopeId === null ? shared : perspective;
  const cycles = useInfiniteQuery({ ...memoryCyclesQuery(api, conversationId), enabled: dynamic });
  const cycleItems = useMemo(() => cycles.data?.pages.flatMap((page) => page.items) ?? [], [cycles.data]);
  const sharedWrite = useMemoryWrite(api, conversationId);
  const scopedWrite = useMemoryWrite(api, conversationId, scopeId);
  const writing = useRef(false);
  const [target, setTarget] = useState<Target | null>(null);
  const [saving, setSaving] = useState(false);
  const [summaryOpen, setSummaryOpen] = useState(false);
  const [summarySession, setSummarySession] = useState(0);
  const { shown, session } = useClosingValue(target);
  useOverlayBack(target !== null || summaryOpen, () => { setTarget(null); setSummaryOpen(false); });
  const changeTab = (next: GroupCompanionTab) => { setTarget(null); setSummaryOpen(false); onTab(next); };
  const pick = (next: string) => { setTarget(null); setPicked(next); };
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
    const ok = await saveMemoryDraft(api, conversationId, scopedWrite, item, draft, session, scopeId);
    if (ok) toast.success(t(item === null ? "memory.sheet.added" : "memory.sheet.saved"));
    return ok;
  });
  const remove = (item: MemoryItemView) => perform(async () => {
    const ok = await scopedWrite(`delete:${session}:${item.id}`, (current, operationId) => api.call("memoryDelete", { conversation_id: conversationId, ...(scopeId === null ? {} : { perspective_character_id: scopeId }), memory_id: item.id, expected_revision: current.revision, client_operation_id: operationId }));
    if (ok) toast.success(t("memory.sheet.deleted"));
    return ok;
  });
  const saveSummary = (text: string | null) => perform(async () => {
    const ok = await sharedWrite(`summary:${summarySession}:${text ?? ""}`, (current, operationId) => api.call("memorySummaryUpdate", { conversation_id: conversationId, summary: text === null ? { type: "clear" } : { type: "set", text }, expected_revision: current.revision, client_operation_id: operationId }));
    if (ok) {
      setSummaryOpen(false);
      toast.success(t(text === null ? "memory.summary.cleared" : "memory.summary.saved"));
    }
    return ok;
  });
  const revert = async (cycle: MemoryCycleView) => {
    const ok = await sharedWrite(`revert:${cycle.run_id}`, (current, operationId) => api.call("memoryCycleRevert", { conversation_id: conversationId, run_id: cycle.run_id, expected_revision: current.revision, client_operation_id: operationId }));
    if (ok) toast.success(t("memory.activity.undone"));
    return ok;
  };
  const openSummary = () => {
    setSummarySession((value) => value + 1);
    setSummaryOpen(true);
  };
  const avatarOf = (characterId: string) => view.participants.find((participant) => participant.character_id === characterId)?.avatar?.url ?? null;
  const band = (value: GroupMemberRelationshipView | null) => value === null ? t("companion.group.noStance") : t(`companion.axes.closeness.${closenessBand(value.closeness ?? 0)}`);

  const stanceRow = (other: GroupCompanionMemberView, from: GroupCompanionMemberView, to: GroupCompanionMemberView) => {
    const value = stanceOf(from, to);
    const face = <Avatar src={avatarOf(other.character_id)} name={other.display_name} size="sm" />;
    return value === null
      ? <ListRow key={other.character_id} icon={face} title={other.display_name} description={band(null)} />
      : <ListRowButton key={other.character_id} icon={face} title={other.display_name} description={band(value)} onClick={() => setStance({ from, to, value })} />;
  };

  let body;
  if (state.isPending) body = <LoadingState />;
  else if (state.isError && state.data === undefined) body = <FailureState error={state.error} onRetry={() => void state.refetch()} />;
  else if (state.data === null || member === null) body = <EmptyState icon={<Sprout size={icon.xl} />} title={t("companion.unavailable")} description={t("companion.unavailableBody")} action={<Button onClick={onBack}>{t("companion.back")}</Button>} />;
  else if ((tab === "memories" || tab === "activity") && settings.isPending) body = <LoadingState />;
  else if ((tab === "memories" || tab === "activity") && !memoryOn) body = <EmptyState icon={<Brain size={icon.xl} />} title={t("memory.off.title")} description={t("memory.off.description")} action={<Button onClick={() => void navigate({ to: "/chat/$conversationId", params: { conversationId }, search: { panel: "settings" }, state: { panelPushed: true } })}>{t("memory.off.action")}</Button>} />;
  else if (tab === "activity") body = <div className="space-y-4">{phone && dynamic && shared.data && <StatusCard api={api} conversationId={conversationId} status={shared.data.status} />}<ActivityTab companion companionGroup cycles={cycleItems} hasMore={cycles.hasNextPage} loadingMore={cycles.isFetchingNextPage} loading={dynamic && cycles.isPending} error={dynamic ? cycles.error : null} onRetry={() => void (cycles.isFetchNextPageError ? cycles.fetchNextPage() : cycles.refetch())} onLoadMore={() => void cycles.fetchNextPage()} onRevert={revert} /></div>;
  else if (tab === "memories") body = memory.isPending ? <LoadingState /> : memory.isError && memory.data === undefined ? <FailureState error={memory.error} onRetry={() => void memory.refetch()} /> : memory.data ? <MemoriesTab key={scopeId ?? SHARED} companion items={memory.data.items} dynamic={dynamic} {...(chosen ? { perspective: chosen.display_name } : {})} onOpen={(item) => setTarget({ kind: "edit", item })} onAdd={() => setTarget({ kind: "add" })} /> : null;
  else if (tab === "soul") body = soul.isPending ? <LoadingState /> : soul.isError && soul.data === undefined ? <FailureState error={soul.error} onRetry={() => void soul.refetch()} /> : soul.data ? <SoulTab key={member.character_id} api={api} conversationId={conversationId} soul={soul.data} onSources={() => undefined} /> : null;
  else {
    const others = members.filter((entry) => entry.character_id !== member.character_id);
    body = <div className="space-y-7">
      <section>
        <SectionHead title={t("companion.bond")} right={member.relationship.last_interaction_at > 0 ? t("companion.lastTalked", { when: relativeTime(member.relationship.last_interaction_at, undefined, i18n.language).toLowerCase() }) : undefined} />
        {soul.isPending ? <Skeleton className="h-28 w-full" /> : soul.isError && soul.data === undefined ? <FailureState error={soul.error} onRetry={() => void soul.refetch()} size="sm" /> : soul.data ? <div className="grid grid-cols-2 gap-2 @2xl:grid-cols-3 @6xl:grid-cols-5"><BondTiles state={member} defaults={soul.data.config.relationship_defaults} narrow={phone} /></div> : null}
        {!member.has_checkpoint && <Text variant="hint" className="mt-2 px-1">{t("companion.group.noCheckpoint", { name: member.display_name })}</Text>}
        {!member.following && <Text variant="hint" className="mt-2 px-1">{t("companion.group.notFollowing", { name: member.display_name })}</Text>}
      </section>
      <section>
        <SectionHead title={t("companion.engine")} />
        {emotion.isPending ? <Skeleton className="h-28 w-full" /> : emotion.isError && emotion.data === undefined ? <FailureState error={emotion.error} onRetry={() => void emotion.refetch()} size="sm" /> : emotion.data?.type !== "installed" ? <EmotionInstallNotice api={api} /> : <div className="grid gap-2 @2xl:grid-cols-2"><EmotionGroup group="expressed" values={member.expressed} /><EmotionGroup group="felt" values={member.felt} /></div>}
      </section>
      {others.length > 0 && <section className="space-y-4">
        <SectionHead title={t("companion.group.withOthers")} />
        <div className="grid gap-4 @3xl:grid-cols-2">
          <ListGroup label={t("companion.group.towardOthers", { name: member.display_name })}>{others.map((target) => stanceRow(target, member, target))}</ListGroup>
          <ListGroup label={t("companion.group.fromOthers", { name: member.display_name })}>{others.map((source) => stanceRow(source, source, member))}</ListGroup>
        </div>
      </section>}
    </div>;
  }

  const memoryScope = tab === "memories";
  const pickerOptions = [
    ...(memoryScope ? [{ value: SHARED, label: t("memory.perspective.shared"), icon: <Brain size={icon.sm} /> }] : []),
    ...members.map((entry) => ({ value: entry.character_id, label: entry.display_name, icon: <Avatar src={avatarOf(entry.character_id)} name={entry.display_name} size="xs" /> })),
  ];
  const pickerValue = memoryScope ? (chosen?.character_id ?? SHARED) : (member?.character_id ?? "");
  const showPicker = tab !== "activity" && members.length > 0 && (memoryScope || members.length > 1);
  const picker = showPicker ? <ListGroup label={t("companion.group.members")} role="radiogroup">{pickerOptions.map((option) => {
    const selected = option.value === pickerValue;
    const entry = members.find((item) => item.character_id === option.value);
    return <ListRowButton key={option.value} role="radio" aria-checked={selected} icon={option.value === SHARED ? <Brain size={icon.lg} /> : <Avatar src={avatarOf(option.value)} name={option.label} size="sm" />} title={option.label} description={entry && !entry.following ? t("companion.group.away") : undefined} chevron={false} trailing={selected ? <Check size={icon.md} /> : undefined} onClick={() => pick(option.value)} />;
  })}</ListGroup> : null;
  const counts: Record<GroupCompanionTab, number | undefined> = {
    relationship: undefined,
    memories: memory.data?.items.length,
    soul: soul.data?.growth.active_count,
    activity: dynamic && cycles.isSuccess && !cycles.hasNextPage ? cycleItems.length : undefined,
  };
  let side: ReactNode = null;
  if (state.data) side = <div className="space-y-4">
    {picker}
    {(tab === "activity" || (memoryScope && scopeId === null)) && dynamic && shared.data && <StatusCard api={api} conversationId={conversationId} status={shared.data.status} />}
    {memoryScope && scopeId === null && shared.data && <SummaryCard companion summary={shared.data.summary} onOpen={openSummary} />}
  </div>;

  return <CrispStringsProvider locale={i18n.language} strings={{ sheetClose: t("crisp.sheetClose"), dialogClose: t("companion.crisp.close"), searchFieldClear: t("companion.crisp.clearSearch") }}>
    <div className="relative flex h-full flex-col overflow-clip">
      <main ref={main} className="min-h-0 flex-1 overflow-y-auto">
        <AppBar title={<span className="flex min-w-0 items-center gap-3"><AvatarGroup people={members.map((entry) => ({ name: entry.display_name, src: avatarOf(entry.character_id) }))} max={3} size={phone ? "xs" : "sm"} /><Text as="span" variant="title" className="min-w-0 truncate">{view.title}</Text></span>} {...(phone ? {} : { meta: t("companion.title") })} onBack={onBack} backLabel={t("companion.back")} scrollRef={main} filters={<Tabs value={tab} onChange={changeTab}><TabList>{(["relationship", "memories", "soul", "activity"] as const).map((id) => <Tab key={id} value={id} {...(!phone && counts[id] !== undefined ? { count: counts[id] } : {})}>{t(phone && id === "relationship" ? "companion.tabs.bond" : `companion.tabs.${id}`)}</Tab>)}</TabList></Tabs>} />
        <div className="@container space-y-6 px-4 pb-10 pt-4 @xl:px-8">
          {phone && showPicker && <FilterBar aria-label={t("companion.group.members")} value={pickerValue} onChange={pick} options={pickerOptions} />}
          {side && !phone ? <div className="grid gap-6 @4xl:grid-cols-[21rem_minmax(0,1fr)]"><aside className="hidden space-y-4 @4xl:sticky @4xl:top-24 @4xl:block @4xl:self-start">{side}</aside><div className="min-w-0">{body}</div></div> : body}
        </div>
      </main>
    </div>
    {memory.data && shown && <MemorySheet key={session} open={target !== null} item={shown.kind === "edit" ? shown.item : null} {...(chosen ? { companionName: chosen.display_name } : {})} categories={(shared.data ?? memory.data).allowed_categories} busy={saving} onClose={() => setTarget(null)} onSave={(draft) => void save(shown.kind === "edit" ? shown.item : null, draft)} onDelete={() => { if (shown.kind === "edit") void remove(shown.item); }} />}
    {shared.data && <SummarySheet key={summarySession} open={summaryOpen} text={shared.data.summary?.text ?? ""} dynamic={dynamic} busy={saving} onClose={() => setSummaryOpen(false)} onSave={(text) => void saveSummary(text)} onClear={() => void saveSummary(null)} />}
    <Sheet open={stance !== null} onClose={() => setStance(null)} placement="auto" size="sm" title={shownStance ? t("companion.group.stanceTitle", { from: shownStance.from.display_name, to: shownStance.to.display_name }) : undefined} description={shownStance ? band(shownStance.value) : undefined}>
      {shownStance && <ListGroup>{stanceAxes.map((axis) => {
        const value = shownStance.value[axis] ?? 0;
        return <ListRow key={axis} title={t(`companion.axes.${axis}.label`)} className="py-2.5" trailing={<Text as="span" variant="body" className="tabular-nums text-fg">{axis === "tension" ? new Intl.NumberFormat(i18n.language).format(Math.round(value * 100)) : signed(value, i18n.language)}</Text>} />;
      })}</ListGroup>}
    </Sheet>
  </CrispStringsProvider>;
}
