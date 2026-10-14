import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCanGoBack, useNavigate, useParams, useRouter, useRouteContext, useSearch as useRouteSearch } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { cn, type ComposerAttachment, confirm, CrispStringsProvider, LoadingState, Sheet, Surface, toast, useCompactLayout } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import { queryKeys } from "@/api/query-keys";
import type { RuntimeNoticeCode, TimelineMessage } from "@/api/generated/bindings";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { FailureState } from "@/shared/ui/FailureState";
import { ChatBanners, ChatComposer, type ComposerHandle } from "./components/ChatComposer";
import { ChatTools, type ToolPanel } from "./components/ChatTools";
import { ChatHeader } from "./components/ChatHeader";
import { InlineAuthorNoteBar } from "./components/InlineAuthorNoteBar";
import { ChatSettingsPanel } from "./components/settings/ChatSettingsPanel";
import { EditSheet } from "./components/EditSheet";
import { MessageMenu } from "./components/MessageMenu";
import { Timeline } from "./components/Timeline";
import {
  deleteMessage, editMessage, pinMessage, rewindTo, selectCandidate, updateSettings, type ConversationServices,
} from "./data/actions";
import { characterParticipant, formatTime, textOf, userParticipant, type MediaItem } from "./data/messages";
import { candidatesQuery, memoryQuery, personasQuery, settingsQuery } from "./data/queries";
import { useGeneration } from "./data/use-generation";
import { useChatModels } from "./data/use-models";
import { useSearch } from "./data/use-search";
import { useTimeline } from "./data/use-timeline";
import { useInlineAuthorNote } from "./data/use-inline-author-note";

export function ConversationPage() {
  const { conversationId } = useParams({ strict: false });
  return conversationId ? <ConversationScreen key={conversationId} conversationId={conversationId} /> : null;
}

const crispKeys = [
  "composerPlaceholder", "composerMessage", "composerStop", "composerSend", "composerSending", "composerContinue", "messageScene",
  "composerAddAttachment", "composerAttachment", "composerFile", "composerCancelRecording", "composerRecordVoice", "composerStopAndTranscribe", "composerTranscribing",
  "messageSystem", "messagePinned", "messageMenuTitle", "messageReasoning", "messageTyping", "sheetClose", "bannerDismiss", "alertDismiss",
] as const;

function ConversationScreen({ conversationId }: { conversationId: string }) {
  const { t, i18n } = useTranslation("conversations");
  const { t: common } = useTranslation("common");
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const router = useRouter();
  const canGoBack = useCanGoBack();
  const phone = useCompactLayout();
  const inlineAuthorNote = useInlineAuthorNote();
  const panelOpen = useRouteSearch({ strict: false, select: (search) => (search as { panel?: string }).panel === "settings" });
  const homing = useRef(false);
  const stuck = useRef(true);
  const onStick = useCallback((value: boolean) => {
    stuck.current = value;
  }, []);
  const services = useMemo<ConversationServices>(() => ({ api, queryClient, conversationId, isStuck: () => stuck.current }), [api, queryClient, conversationId]);

  const timeline = useTimeline(api, conversationId);
  const latest = timeline.windowed ? undefined : timeline.messages[timeline.messages.length - 1];
  useQuery({
    ...candidatesQuery(api, conversationId, latest?.id ?? "", latest?.candidate_count ?? 0),
    enabled: latest?.role === "assistant" && latest.candidate_count > 1,
  });
  const { view, messages } = timeline;
  const settings = useQuery(settingsQuery(api, conversationId));
  const mode = settings.data?.memory.mode;
  const memoryOn = mode !== undefined && mode !== "disabled";
  const memory = useQuery({ ...memoryQuery(api, conversationId), enabled: memoryOn });
  const chat = useChatModels(api);
  const search = useSearch(api, conversationId);

  const [menuId, setMenuId] = useState<string | null>(null);
  const [editing, setEditing] = useState<TimelineMessage | null>(null);
  const [busy, setBusy] = useState(false);
  const [modelRequired, setModelRequired] = useState(false);
  const [panel, setPanel] = useState<ToolPanel | null>(null);
  const [swap, setSwap] = useState(false);
  const [pendingMedia, setPendingMedia] = useState<MediaItem[]>([]);
  const composer = useRef<ComposerHandle | null>(null);

  const fail = useCallback(
    (error: unknown) => {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    },
    [common],
  );

  const generation = useGeneration(api, conversationId, {
    onError: (failure) => {
      if (failure.code === "model_required") setModelRequired(true);
      else if (failure.code === "unsupported" && failure.details?.type === "invalid_field" && failure.details.field === "media.image") toast.error(t("toast.mediaImageUnsupported"));
      else if (failure.code === "unsupported" && failure.details?.type === "invalid_field" && failure.details.field === "media.audio") toast.error(t("toast.mediaAudioUnsupported"));
      else fail(failure);
    },
    onNotice: (code: RuntimeNoticeCode) => toast.info(t(`notice.${code}`)),
    isStuck: () => stuck.current,
  });
  const { store } = generation;
  const meta = useSyncExternalStore(store.subscribeMeta, store.getMeta);

  const run = useCallback(
    async (task: () => Promise<void>): Promise<boolean> => {
      setBusy(true);
      try {
        await task();
        return true;
      } catch (error) {
        fail(error);
        return false;
      } finally {
        setBusy(false);
      }
    },
    [fail],
  );

  const menuMessage = menuId ? (messages.find((message) => message.id === menuId) ?? null) : null;
  const { shown: shownMenu } = useClosingValue(menuMessage);

  const character = characterParticipant(view);
  const personaChoice = settings.data?.persona;
  const personas = useQuery({ ...personasQuery(api, conversationId), enabled: personaChoice !== undefined });
  const personaId = personaChoice ? (personaChoice.id ?? (personaChoice.source === "app_default" ? (personas.data?.defaultId ?? null) : null)) : null;
  const persona = personaId ? personas.data?.items.find((item) => item.id === personaId) : undefined;
  const participant = userParticipant(view);
  const user = useMemo(() => (participant && persona ? { ...participant, name: persona.title, avatar: persona.avatar } : participant), [participant, persona]);
  const participants = useMemo(
    () => (view ? view.participants.map((entry) => (user && entry.id === user.id ? user : entry)) : []),
    [view, user],
  );
  const name = character?.name ?? view?.title ?? t("header.untitled");
  const avatar = character?.avatar?.url ?? null;
  const pendingTurn = Boolean(view?.pending_turn_id) && !meta.active;
  const sending = meta.active || pendingTurn;
  const background = settings.data?.background.asset?.url ?? null;
  const currentModel = chat.find(settings.data?.model.id ?? null);

  const lastJump = useRef<string | null>(null);
  const hit = search.current?.message_id ?? null;
  const { jumpTo, jumpToLatest, windowed } = timeline;
  useEffect(() => {
    if (!hit) {
      lastJump.current = null;
      return;
    }
    if (messages.some((message) => message.id === hit) || lastJump.current === hit) return;
    lastJump.current = hit;
    jumpTo(hit);
  }, [hit, messages, jumpTo]);
  const { windowError } = timeline;
  useEffect(() => {
    if (!windowError) return;
    lastJump.current = null;
    jumpToLatest();
    fail(windowError);
  }, [windowError, jumpToLatest, fail]);
  const goHome = useCallback(() => {
    if (homing.current) return;
    homing.current = true;
    if (router.state.location.state.panelPushed === true) {
      const stop = router.subscribe("onResolved", () => {
        stop();
        if (router.state.location.pathname.startsWith("/chat/")) void navigate({ to: "/", replace: true });
      });
      router.history.back();
    } else {
      void navigate({ to: "/", replace: true });
    }
  }, [router, navigate]);
  useEffect(() => {
    let active = true;
    let stop: (() => void) | undefined;
    void api.subscribe((event) => {
      if (event.type === "conversation_removed" && event.conversation_id === conversationId) goHome();
    }).then((unsubscribe) => {
      if (active) stop = unsubscribe;
      else unsubscribe();
    });
    return () => {
      active = false;
      stop?.();
    };
  }, [api, conversationId, goHome]);
  useEffect(() => {
    if (!search.open && windowed) jumpToLatest();
  }, [search.open, windowed, jumpToLatest]);

  const openSettings = () => {
    void navigate({ to: "/chat/$conversationId", params: { conversationId }, search: { panel: "settings" }, state: { panelPushed: true } });
  };
  const closeSettings = () => {
    if (router.state.location.state.panelPushed === true) router.history.back();
    else void navigate({ to: "/chat/$conversationId", params: { conversationId }, search: {}, replace: true });
  };

  const goBack = () => {
    if (canGoBack) router.history.back();
    else void navigate({ to: "/" });
  };

  const onMenu = useCallback((message: TimelineMessage) => setMenuId(message.id), []);
  const busyRef = useRef(false);
  const onVariant = useCallback(
    (message: TimelineMessage, delta: -1 | 1) => {
      if (message.candidate_index === null || busyRef.current) return;
      const target = message.candidate_index + delta;
      busyRef.current = true;
      void run(async () => {
        await selectCandidate(services, message, target);
      }).finally(() => {
        busyRef.current = false;
      });
    },
    [services, run],
  );

  const send = async (text: string, attachments: ComposerAttachment[]) => {
    timeline.jumpToLatest();
    setPendingMedia(attachments.map((attachment) => ({ id: attachment.id, url: attachment.src ?? null, kind: attachment.src ? "image" : "audio", ...(attachment.name ? { name: attachment.name } : {}) })));
    const accepted = await generation.send(text, attachments.map((attachment) => attachment.id), swap);
    if (accepted) setModelRequired(false);
    return accepted;
  };

  const copy = async (message: TimelineMessage) => {
    setMenuId(null);
    try {
      await navigator.clipboard.writeText(textOf(message));
      toast.success(t("toast.copied"));
    } catch {
      toast.error(t("toast.copyFailed"));
    }
  };

  const togglePin = (message: TimelineMessage) => {
    setMenuId(null);
    void run(async () => {
      await pinMessage(services, message, !message.pinned);
      toast.success(message.pinned ? t("toast.unpinned") : t("toast.pinned"));
    });
  };

  const rewind = async (message: TimelineMessage, after: number) => {
    setMenuId(null);
    const sure = await confirm({
      title: t("confirm.rewindTitle"),
      message: t("confirm.rewindBody", { count: after }),
      confirmLabel: t("confirm.rewind"),
      cancelLabel: t("confirm.cancel"),
      destructive: true,
    });
    if (!sure) return;
    await run(async () => {
      await rewindTo(services, message);
      toast.success(t("toast.rewound", { count: after }));
    });
  };

  const remove = async (message: TimelineMessage) => {
    setMenuId(null);
    const sure = await confirm({
      title: t("confirm.deleteTitle"),
      message: message.candidate_count > 1 ? t("confirm.deleteBodyVersions", { count: message.candidate_count }) : t("confirm.deleteBody"),
      confirmLabel: t("confirm.delete"),
      cancelLabel: t("confirm.cancel"),
      destructive: true,
    });
    if (!sure) return;
    await run(async () => {
      await deleteMessage(services, message);
      toast.success(t("toast.deleted"));
    });
  };

  const retryMemory = () => {
    void run(async () => {
      const outcome = await api.call("memoryRewindRetry", { conversation_id: conversationId });
      await queryClient.invalidateQueries({ queryKey: queryKeys.conversations.detail(conversationId) });
      if (outcome.type === "still_failing") toast.warning(t("banner.memoryRetryStill"));
      else toast.success(t("banner.memoryRetried"));
    });
  };

  const strings = useMemo(
    () => ({ ...Object.fromEntries(crispKeys.map((key) => [key, t(`crisp.${key}`)])), composerRecording: (elapsed: string) => t("crisp.composerRecording", { elapsed }),
      composerAttachmentRemove: (name: string | undefined) => t("crisp.composerAttachmentRemove", { name: name ?? "" }),
    }),
    [t],
  );
  const menuAfter = shownMenu && !timeline.windowed ? messages.length - 1 - messages.findIndex((message) => message.id === shownMenu.id) : 0;
  const memoryBusy = memory.data?.status.latest_cycle_status === "queued" || memory.data?.status.latest_cycle_status === "processing";

  let body;
  if (timeline.pending) {
    body = <LoadingState />;
  } else if (!view) {
    body = <FailureState error={timeline.error} onRetry={timeline.refetch} />;
  } else {
    body = (
      <>
        <Timeline
          timeline={timeline}
          store={store}
          onStick={onStick}
          participants={participants}
          user={user ? { name: user.name, avatar: user.avatar?.url ?? null } : undefined}
          character={character ? { name: character.name, avatar: character.avatar?.url ?? null } : undefined}
          pendingTurn={pendingTurn}
          pendingMedia={pendingMedia}
          locale={i18n.language}
          query={search.query}
          focusId={hit}
          menuFor={menuId}
          onMenu={onMenu}
          onVariant={onVariant}
          onRetry={() => void generation.retry()}
        />
        <div className="relative shrink-0 px-3 pb-[calc(var(--safe-bottom)+10px)] pt-2 @xl:px-6">
          <ChatBanners
            blockers={{ missingModels: view.missing_models, modelRequired, memoryBlocked: view.memory_blocked }}
            swap={swap}
            onEndSwap={() => setSwap(false)}
            onRetryMemory={retryMemory}
          />
          <div>
            <ChatComposer
              api={api}
              conversationId={conversationId}
              handle={composer}
              onAttach={() => setPanel("menu")}
              name={name}
              sending={sending}
              disabled={!view.can_send && !sending}
              canContinue={messages.length > 0}
              onSend={send}
              onContinue={() => void generation.continueReply(swap)}
              onStop={() => generation.stop(view.pending_turn_id)}
              topSlot={inlineAuthorNote && settings.data ? <InlineAuthorNoteBar saved={settings.data.author_note} enterSaves={!phone} onSave={(author_note) => run(async () => { await updateSettings(services, { author_note }); })} /> : undefined}
            />
          </div>
        </div>
      </>
    );
  }

  return (
    <CrispStringsProvider strings={strings} locale={i18n.language}>
      <Surface image={background} scrim="soft" className={cn("h-full overflow-clip", !background && "bg-surface")}>
        <div className="relative flex h-full min-h-0 flex-col">
          <ChatHeader
            name={name}
            avatar={avatar}
            modelKnown={settings.data !== undefined}
            model={currentModel ? { label: `${currentModel.name} · ${currentModel.local ? t("header.onDevice") : currentModel.provider}`, local: currentModel.local } : null}
            phone={phone}
            memory={memoryOn ? { count: memory.data?.items.length ?? 0, busy: memoryBusy } : null}
            search={search}
            onBack={goBack}
            onOptions={openSettings}
            onMemory={() => void navigate({ to: "/chat/$conversationId/memory", params: { conversationId } })}
          />
          {body}
        </div>
      </Surface>

      <Sheet
        open={menuMessage !== null}
        onClose={() => setMenuId(null)}
        size="sm"
        title={shownMenu && t(shownMenu.role === "assistant" ? "message.titleAssistant" : shownMenu.role === "user" ? "message.titleUser" : shownMenu.role === "scene" ? "message.titleScene" : "message.titleSystem", { name })}
        description={shownMenu && formatTime(shownMenu.created_at, i18n.language)}
      >
        {shownMenu && (
          <MessageMenu
            api={api}
            conversationId={conversationId}
            message={shownMenu}
            after={menuAfter}
            busy={meta.active || pendingTurn}
            canEdit={shownMenu.role !== "user" || (!timeline.windowed && shownMenu.id === messages.findLast((message) => message.role === "user")?.id)}
            canRegenerate={shownMenu.role === "assistant" && !timeline.windowed && shownMenu.id === messages[messages.length - 1]?.id}
            onCopy={() => void copy(shownMenu)}
            onEdit={() => {
              setMenuId(null);
              setEditing(shownMenu);
            }}
            onRegenerate={() => {
              setMenuId(null);
              timeline.jumpToLatest();
              void generation.regenerate(shownMenu.id, swap);
            }}
            onPin={() => togglePin(shownMenu)}
            onRewind={() => void rewind(shownMenu, menuAfter)}
            onDelete={() => void remove(shownMenu)}
          />
        )}
      </Sheet>

      {view && (
        <ChatSettingsPanel
          open={panelOpen}
          api={api}
          services={services}
          conversationId={conversationId}
          view={view}
          settings={settings}
          name={name}
          avatar={avatar}
          characterId={view.source.type === "direct" ? view.source.character_id : null}
          onClose={closeSettings}
          onOpenChat={(id, replace) => void navigate({ to: "/chat/$conversationId", params: { conversationId: id }, search: {}, replace })}
          onHome={goHome}
          onMemories={() => void navigate({ to: "/chat/$conversationId/memory", params: { conversationId } })}
          onBranches={() => void navigate({ to: "/chat/$conversationId/branches", params: { conversationId } })}
          onModels={() => void navigate({ to: "/settings/$section", params: { section: "models" } })}
        />
      )}

      <ChatTools
        api={api}
        services={services}
        conversationId={conversationId}
        settings={settings.data}
        panel={panel}
        setPanel={setPanel}
        composer={composer}
        swap={swap}
        onSwap={setSwap}
        onFailure={fail}
      />

      <EditSheet
        message={editing}
        characterName={name}
        busy={busy}
        onClose={() => setEditing(null)}
        onSave={(message, text) => {
          void run(async () => {
            await editMessage(services, message, text);
            setEditing(null);
            toast.success(t("toast.edited"));
          });
        }}
      />
    </CrispStringsProvider>
  );
}
