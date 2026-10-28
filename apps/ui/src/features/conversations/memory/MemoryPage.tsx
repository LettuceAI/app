import { useMemo, useState } from "react";
import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query";
import { useCanGoBack, useNavigate, useParams, useRouteContext, useRouter } from "@tanstack/react-router";
import { Brain, Plus, Search } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, Chip, EmptyState, icon, ListGroup, LoadingState, Page, SearchField, Tab, TabList, Tabs, toast, useCompactLayout } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import type { MemoryCycleView, MemoryItemView, MemoryView, TimelineMessage } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { FailureState } from "@/shared/ui/FailureState";
import { characterParticipant } from "../data/messages";
import { memoryCyclesQuery, memoryQuery, pinnedMessagesQuery, settingsQuery, timelineQuery } from "../data/queries";
import { ActivityTab } from "./ActivityTab";
import { useMemoryError } from "./errors";
import { MemoryRow, MemorySheet, type MemoryDraft } from "./MemoryItems";
import { createOperationIds } from "./operation-ids";
import { PinnedTab } from "./PinnedTab";
import { StatusCard } from "./StatusCard";
import { SummaryCard, SummarySheet } from "./SummaryParts";
import { useMemoryWrite } from "./use-memory-write";
import { saveMemoryDraft } from "./save-memory";

type Tab = "memories" | "activity" | "pinned";
type SheetTarget = { kind: "edit"; item: MemoryItemView } | { kind: "add" };

export function MemoryPage() {
  const { conversationId } = useParams({ strict: false });
  return conversationId ? <MemoryScreen key={conversationId} conversationId={conversationId} /> : null;
}

function MemoryScreen({ conversationId }: { conversationId: string }) {
  const { t } = useTranslation("conversations");
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const router = useRouter();
  const canGoBack = useCanGoBack();
  const phone = useCompactLayout();
  const errorText = useMemoryError();
  const write = useMemoryWrite(api, conversationId);
  const [pinIds] = useState(createOperationIds);
  const [tab, setTab] = useState<Tab>("memories");
  const [target, setTarget] = useState<SheetTarget | null>(null);
  const [saving, setSaving] = useState(false);
  const [summaryOpen, setSummaryOpen] = useState(false);
  const [summarySession, setSummarySession] = useState(0);
  const [unpinning, setUnpinning] = useState<string | null>(null);
  const { shown, session } = useClosingValue(target);

  const settings = useQuery(settingsQuery(api, conversationId));
  const mode = settings.data?.memory.mode;
  const on = mode !== undefined && mode !== "disabled";
  const dynamic = mode === "dynamic";
  const memory = useQuery({ ...memoryQuery(api, conversationId), enabled: on });
  const cycles = useInfiniteQuery({ ...memoryCyclesQuery(api, conversationId), enabled: dynamic });
  const pinned = useInfiniteQuery({ ...pinnedMessagesQuery(api, conversationId), enabled: on });
  const timeline = useInfiniteQuery({ ...timelineQuery(api, conversationId), enabled: on });

  const view = timeline.data?.pages[0]?.view ?? undefined;
  const participants = view?.participants ?? [];
  const chatName = characterParticipant(view)?.name ?? view?.title;
  const cycleItems = useMemo(() => cycles.data?.pages.flatMap((page) => page.items) ?? [], [cycles.data]);
  const pinnedItems = useMemo(() => pinned.data?.pages.flatMap((page) => page.items) ?? [], [pinned.data]);

  const back = () => {
    if (canGoBack) router.history.back();
    else void navigate({ to: "/chat/$conversationId", params: { conversationId } });
  };

  const saveMemory = async (item: MemoryItemView | null, draft: MemoryDraft) => {
    setSaving(true);
    try {
      const ok = await saveMemoryDraft(api, conversationId, write, item, draft, session);
      if (ok) toast.success(t(item === null ? "memory.sheet.added" : "memory.sheet.saved"));
      if (ok) setTarget(null);
    } finally {
      setSaving(false);
    }
  };

  const deleteMemory = async (item: MemoryItemView) => {
    setSaving(true);
    try {
      const ok = await write(`delete:${session}:${item.id}`, (current, operationId) =>
        api.call("memoryDelete", { conversation_id: conversationId, memory_id: item.id, expected_revision: current.revision, client_operation_id: operationId }),
      );
      if (ok) {
        toast.success(t("memory.sheet.deleted"));
        setTarget(null);
      }
    } finally {
      setSaving(false);
    }
  };

  const saveSummary = async (text: string | null) => {
    setSaving(true);
    try {
      const ok = await write(`summary:${summarySession}:${text ?? ""}`, (current, operationId) =>
        api.call("memorySummaryUpdate", {
          conversation_id: conversationId,
          summary: text === null ? { type: "clear" } : { type: "set", text },
          expected_revision: current.revision,
          client_operation_id: operationId,
        }),
      );
      if (ok) {
        toast.success(text === null ? t("memory.summary.cleared") : t("memory.summary.saved"));
        setSummaryOpen(false);
      }
    } finally {
      setSaving(false);
    }
  };

  const revert = async (cycle: MemoryCycleView) => {
    const ok = await write(`revert:${cycle.run_id}`, (current, operationId) =>
      api.call("memoryCycleRevert", { conversation_id: conversationId, run_id: cycle.run_id, expected_revision: current.revision, client_operation_id: operationId }),
    );
    if (ok) toast.success(t("memory.activity.undone"));
    return ok;
  };

  const unpin = async (message: TimelineMessage) => {
    setUnpinning(message.id);
    try {
      const current = await queryClient.fetchQuery({ ...settingsQuery(api, conversationId), staleTime: 0 });
      await api.call("messagePin", {
        conversation_id: conversationId,
        message_id: message.id,
        expected_revision: current.revision,
        pinned: false,
        client_operation_id: pinIds.idFor(message.id),
      });
      pinIds.settle(message.id);
      toast.success(t("memory.pinnedTab.unpinned"));
      await queryClient.invalidateQueries({ queryKey: queryKeys.conversations.detail(conversationId) });
    } catch (error) {
      if (isApiFailure(error) && error.code === "conflict") pinIds.settle(message.id);
      toast.error(errorText(error));
    } finally {
      setUnpinning(null);
    }
  };

  const openMessage = (message: TimelineMessage) => {
    void navigate({ to: "/chat/$conversationId", params: { conversationId }, search: { message: message.id } });
  };
  const openChatSettings = () => {
    void navigate({ to: "/chat/$conversationId", params: { conversationId }, search: { panel: "settings" }, state: { panelPushed: true } });
  };
  const openSummary = () => {
    setSummarySession((current) => current + 1);
    setSummaryOpen(true);
  };

  const data: MemoryView | undefined = memory.data;
  const tabs: readonly Tab[] = dynamic ? ["memories", "activity", "pinned"] : ["memories", "pinned"];
  const active = tabs.includes(tab) ? tab : "memories";
  const complete = (query: { isSuccess: boolean; hasNextPage: boolean }, count: number) => (query.isSuccess && !query.hasNextPage ? count : undefined);
  const counts: Record<Tab, number | undefined> = {
    memories: data?.items.length,
    activity: complete(cycles, cycleItems.length),
    pinned: complete(pinned, pinnedItems.length),
  };
  const tabLabel: Record<Tab, string> = { memories: t("memory.tabs.memories"), activity: t("memory.tabs.activity"), pinned: t("memory.tabs.pinned") };

  let body;
  if (settings.isPending) body = <LoadingState />;
  else if (settings.isError) body = <FailureState error={settings.error} onRetry={() => void settings.refetch()} />;
  else if (!on) {
    body = (
      <EmptyState
        icon={<Brain size={icon.xl} />}
        title={t("memory.off.title")}
        description={t("memory.off.description")}
        action={<Button onClick={openChatSettings}>{t("memory.off.action")}</Button>}
      />
    );
  } else if (memory.isPending) body = <LoadingState />;
  else if (memory.isError) body = <FailureState error={memory.error} onRetry={() => void memory.refetch()} />;
  else if (data) {
    const main =
      active === "memories" ? (
        <MemoriesTab items={data.items} dynamic={dynamic} onOpen={(item) => setTarget({ kind: "edit", item })} onAdd={() => setTarget({ kind: "add" })} />
      ) : active === "activity" ? (
        <ActivityTab
          cycles={cycleItems}
          hasMore={cycles.hasNextPage}
          loadingMore={cycles.isFetchingNextPage}
          loading={cycles.isPending}
          error={cycles.error}
          onRetry={() => void (cycles.isFetchNextPageError ? cycles.fetchNextPage() : cycles.refetch())}
          onLoadMore={() => void cycles.fetchNextPage()}
          onRevert={revert}
        />
      ) : pinned.isError ? (
        <FailureState error={pinned.error} onRetry={() => void pinned.refetch()} />
      ) : pinned.isPending ? (
        <LoadingState />
      ) : (
        <PinnedTab
          messages={pinnedItems}
          participants={participants}
          hasMore={pinned.hasNextPage}
          loadingMore={pinned.isFetchingNextPage}
          unpinning={unpinning}
          onLoadMore={() => void pinned.fetchNextPage()}
          onOpen={openMessage}
          onUnpin={(message) => void unpin(message)}
        />
      );
    body = (
      <div className="@container px-4 pb-10 pt-4 @xl:px-8">
        <div className="grid grid-cols-1 gap-6 @4xl:grid-cols-[minmax(0,1fr)_21rem]">
          <aside className="order-1 space-y-4 @4xl:sticky @4xl:top-[calc(var(--page-bar-height)+1rem)] @4xl:order-2 @4xl:self-start">
            {dynamic && <StatusCard api={api} conversationId={conversationId} status={data.status} />}
            <SummaryCard summary={data.summary} onOpen={openSummary} />
          </aside>
          <div className="order-2 min-w-0 @4xl:order-1">{main}</div>
        </div>
      </div>
    );
  }

  return (
    <>
      <Page
        title={t("memory.title")}
        {...(chatName && !phone ? { meta: chatName } : {})}
        onBack={back}
        backLabel={t("memory.back")}
        padding="none"
        {...(on && data
          ? {
              filters: (
                <Tabs value={active} onChange={setTab}>
                  <TabList>
                    {tabs.map((id) => (
                      <Tab key={id} value={id} {...(counts[id] === undefined ? {} : { count: counts[id] })}>
                        {tabLabel[id]}
                      </Tab>
                    ))}
                  </TabList>
                </Tabs>
              ),
            }
          : {})}
      >
        {body}
      </Page>
      {data && shown !== null && (
        <MemorySheet
          key={session}
          open={target !== null}
          item={shown.kind === "edit" ? shown.item : null}
          categories={data.allowed_categories}
          busy={saving}
          onClose={() => setTarget(null)}
          onSave={(draft) => void saveMemory(shown.kind === "edit" ? shown.item : null, draft)}
          onDelete={() => {
            if (shown.kind === "edit") void deleteMemory(shown.item);
          }}
        />
      )}
      {data && (
        <SummarySheet
          key={summarySession}
          open={summaryOpen}
          text={data.summary?.text ?? ""}
          dynamic={dynamic}
          busy={saving}
          onClose={() => setSummaryOpen(false)}
          onSave={(text) => void saveSummary(text)}
          onClear={() => void saveSummary(null)}
        />
      )}
    </>
  );
}

export function MemoriesTab({ items, dynamic, onOpen, onAdd, companion = false }: { items: readonly MemoryItemView[]; dynamic: boolean; onOpen: (item: MemoryItemView) => void; onAdd: () => void; companion?: boolean }) {
  const { t } = useTranslation("conversations");
  const [query, setQuery] = useState("");
  const [picked, setPicked] = useState<MemoryItemView["category"]>(null);
  const categories = useMemo(() => [...new Set(items.flatMap((item) => (item.category ? [item.category] : [])))], [items]);
  const category = picked !== null && categories.includes(picked) ? picked : null;
  const needle = query.trim().toLowerCase();
  const shown = items.filter((item) => (category === null || item.category === category) && item.text.toLowerCase().includes(needle));
  const byModel = items.filter((item) => item.origin === "model").length;
  const byUser = items.filter((item) => item.origin === "user").length;
  const imported = items.length - byModel - byUser;
  const origins = [
    ...(byModel > 0 ? [t("memory.list.byModel", { count: byModel })] : []),
    ...(byUser > 0 ? [t("memory.list.byUser", { count: byUser })] : []),
    ...(imported > 0 ? [t("memory.list.imported", { count: imported })] : []),
  ].join(", ");
  const total = t("memory.list.total", { count: items.length });
  const label = companion ? t(needle || category !== null ? "companion.memories.found" : "companion.memories.total", { count: shown.length }) : needle || category !== null ? t("memory.list.found", { count: shown.length }) : origins ? t("memory.list.summary", { total, origins }) : total;

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        <div className="min-w-0 flex-1">
          <SearchField value={query} onChange={setQuery} placeholder={t("memory.search")} aria-label={t("memory.search")} />
        </div>
        <Button variant="tonal" leading={<Plus size={icon.sm} />} onClick={onAdd}>
          {t("memory.add")}
        </Button>
      </div>
      {categories.length > 0 && (
        <div className={`-mx-4 flex gap-2 overflow-x-auto px-4 pb-1 [scrollbar-width:none] @xl:-mx-8 @xl:px-8 @4xl:mx-0 @4xl:px-0 ${companion ? "@4xl:flex-wrap" : ""}`}>
          <Chip className="shrink-0 whitespace-nowrap" selected={category === null} onClick={() => setPicked(null)}>
            {t("memory.all")}
          </Chip>
          {categories.map((id) => (
            <Chip key={id} className="shrink-0 whitespace-nowrap" selected={category === id} onClick={() => setPicked(category === id ? null : id)}>
              {t(`memory.categories.${id}`)}
            </Chip>
          ))}
        </div>
      )}
      {shown.length === 0 ? (
        items.length === 0 ? (
          <EmptyState
            icon={<Brain size={icon.xl} />}
            title={t(companion ? "companion.memories.emptyTitle" : "memory.empty.title")}
            description={companion ? t("companion.memories.emptyBody") : dynamic ? t("memory.empty.dynamic") : t("memory.empty.manual")}
            action={
              <Button leading={<Plus size={icon.sm} />} onClick={onAdd}>
                {t("memory.empty.action")}
              </Button>
            }
          />
        ) : (
          <EmptyState
            icon={<Search size={icon.xl} />}
            title={needle ? t(companion ? "companion.memories.noMatch" : "memory.empty.noMatch", { query: query.trim() }) : t("memory.empty.noCategory")}
            {...(needle ? { description: t(companion ? "companion.memories.noMatchBody" : "memory.empty.noMatchBody") } : {})}
          />
        )
      ) : (
        <ListGroup label={label}>
          {shown.map((item) => (
            <MemoryRow key={item.id} companion={companion} item={item} onOpen={() => onOpen(item)} />
          ))}
        </ListGroup>
      )}
    </div>
  );
}
