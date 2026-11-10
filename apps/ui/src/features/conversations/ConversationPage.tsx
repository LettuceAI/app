import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  useCanGoBack,
  useNavigate,
  useParams,
  useRouter,
  useRouteContext,
  useSearch as useRouteSearch,
} from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import {
  cn,
  type ComposerAttachment,
  confirm,
  CrispStringsProvider,
  LoadingState,
  Sheet,
  Surface,
  toast,
  useCompactLayout,
} from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import { pickImage } from "@/entities/image-input";
import { queryKeys } from "@/api/query-keys";
import type { ConversationSettingsPatch_Deserialize, MemoryItemView, RuntimeNoticeCode, TimelineMessage } from "@/api/generated/bindings";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { FailureState } from "@/shared/ui/FailureState";
import {
  ChatAppearanceProvider,
  appAppearancePreferences,
  object,
  readChatLook,
  usePreferences,
  type ChatLook,
} from "@/entities/preferences";
import { ChatBanners, ChatComposer, type ComposerHandle } from "./components/ChatComposer";
import { ChatTools, type ToolPanel } from "./components/ChatTools";
import { ChatHeader } from "./components/ChatHeader";
import { InlineAuthorNoteBar } from "./components/InlineAuthorNoteBar";
import { GenerationProgressToast } from "./components/GenerationProgressToast";
import { BeetrootRain } from "./components/BeetrootRain";
import { ChatSettingsPanel } from "./components/settings/ChatSettingsPanel";
import { ChatAppearanceDrawer } from "./components/appearance/ChatAppearanceDrawer";
import { EditSheet } from "./components/EditSheet";
import { MessageMenu } from "./components/MessageMenu";
import { RegenerationSheet } from "./components/RegenerationSheet";
import { ParticipantsBar } from "./components/ParticipantsBar";
import { type Speaker } from "./components/SpeakerSheet";
import { BranchSheet, type BranchTarget } from "./components/BranchSheet";
import { ModelSheet } from "./components/settings/ModelSheet";
import { EffectSheet } from "./companion/EffectSheet";
import { GroupEffectSheet } from "./companion/GroupEffectSheet";
import { AudienceSheet } from "./companion/AudienceSheet";
import { SceneImageSheet } from "./scene-images/SceneImageSheet";
import { Timeline } from "./components/Timeline";
import {
  addUserMessage,
  addSystemMessage,
  branchToNewChat,
  deleteMessage,
  editMessage,
  pinMessage,
  rewindTo,
  selectCandidate,
  selectScene,
  setMemberMuted,
  startDirectChat,
  startGroupChat,
  updateSettings,
  type ConversationServices,
} from "./data/actions";
import {
  authorOf,
  characterParticipant,
  formatTime,
  textOf,
  userParticipant,
  type MediaItem,
} from "./data/messages";
import { candidatesQuery, memoryQuery, personasQuery, settingsQuery } from "./data/queries";
import { characterQuery, libraryCastQuery, messageCountQuery } from "./data/settings-queries";
import { useGeneration } from "./data/use-generation";
import { useSearch } from "./data/use-search";
import { useTimeline } from "./data/use-timeline";
import { useInlineAuthorNote } from "./data/use-inline-author-note";
import { WidgetContext, useStableActions, type WidgetActions, type WidgetPerson } from "./widgets/context";
import { useAreaWidth, widgetLayout } from "./widgets/layout";
import { updateNode as updateWidgetNode } from "./widgets/nodes";
import { useWidgetSlots } from "./widgets/use-widget-slots";
import { WidgetArea } from "./widgets/WidgetArea";

const noMemories: readonly MemoryItemView[] = [];

export function ConversationPage() {
  const { conversationId } = useParams({ strict: false });
  return conversationId ? (
    <ConversationScreen key={conversationId} conversationId={conversationId} />
  ) : null;
}

const crispKeys = [
  "composerPlaceholder",
  "composerMessage",
  "composerStop",
  "composerSend",
  "composerSendAsSystem",
  "composerSending",
  "composerContinue",
  "messageScene",
  "composerAddAttachment",
  "composerAttachment",
  "composerFile",
  "composerCancelRecording",
  "composerRecordVoice",
  "composerStopAndTranscribe",
  "composerTranscribing",
  "messageSystem",
  "messagePinned",
  "messageMenuTitle",
  "messageReasoning",
  "messageTyping",
  "sheetClose",
  "bannerDismiss",
  "alertDismiss",
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
  const panelOpen = useRouteSearch({
    strict: false,
    select: (search) => (search as { panel?: string }).panel === "settings",
  });
  const requestedMessage = useRouteSearch({
    strict: false,
    select: (search) => (search as { message?: string }).message ?? null,
  });
  const sceneImageId = useRouteSearch({ strict: false, select: (search) => (search as { sceneImage?: string }).sceneImage ?? null });
  const [landing, setLanding] = useState<string | null>(requestedMessage ?? sceneImageId);
  const homing = useRef(false);
  const stuck = useRef(true);
  const [followRequest, setFollowRequest] = useState(0);
  const onStick = useCallback((value: boolean) => {
    stuck.current = value;
  }, []);
  const services = useMemo<ConversationServices>(
    () => ({ api, queryClient, conversationId, isStuck: () => stuck.current }),
    [api, queryClient, conversationId],
  );

  const timeline = useTimeline(api, conversationId);
  const latest = timeline.windowed ? undefined : timeline.messages[timeline.messages.length - 1];
  useQuery({
    ...candidatesQuery(api, conversationId, latest?.id ?? "", latest?.candidate_count ?? 0),
    enabled: latest?.role === "assistant" && latest.candidate_count > 1,
  });
  const { view, messages } = timeline;
  const settings = useQuery(settingsQuery(api, conversationId));
  const preferences = usePreferences();
  const [appearanceOpen, setAppearanceOpen] = useState(false);
  const [draftLook, setDraftLook] = useState<ChatLook | null>(null);
  const savedLook = useMemo(
    () =>
      readChatLook({
        ...appAppearancePreferences(preferences.value, view?.kind === "group"),
        ...object(
          settings.data && "appearance" in settings.data ? settings.data.appearance : undefined,
        ),
      }),
    [preferences.value, view?.kind, settings.data],
  );
  const look = draftLook ?? savedLook;
  const [areaRef, areaWidth] = useAreaWidth<HTMLDivElement>();
  const layout = useMemo(() => widgetLayout(look, areaWidth, phone), [look, areaWidth, phone]);
  const columnStyle = useMemo(() => {
    if (layout.enabled || phone || look.chatColumnWidth === "full") return {};
    const widths = {
      narrow: 640,
      normal: 800,
      wide: 1024,
      xl: 1280,
      custom: look.chatColumnWidthPx,
    };
    return {
      maxWidth: `min(100%, ${widths[look.chatColumnWidth]}px)`,
      marginLeft: look.chatColumnAlign === "left" ? 0 : "auto",
      marginRight: look.chatColumnAlign === "right" ? 0 : "auto",
      width: "100%",
    };
  }, [layout.enabled, look.chatColumnAlign, look.chatColumnWidth, look.chatColumnWidthPx, phone]);
  const mode = settings.data?.memory.mode;
  const memoryOn = mode !== undefined && mode !== "disabled";
  const memory = useQuery({ ...memoryQuery(api, conversationId), enabled: memoryOn });
  const search = useSearch(api, conversationId);

  const [menuId, setMenuId] = useState<string | null>(null);
  const [branching, setBranching] = useState<BranchTarget | null>(null);
  const { shown: shownBranch, session: branchSession } = useClosingValue(branching);
  const [selection, setSelection] = useState<{ behavior: "cue" | "action"; id: string } | null>(null);
  const [directorWiggle, setDirectorWiggle] = useState(0);
  const [effectMessage, setEffectMessage] = useState<TimelineMessage | null>(null);
  const { shown: shownEffect } = useClosingValue(effectMessage);
  const [audienceMessage, setAudienceMessage] = useState<TimelineMessage | null>(null);
  const { shown: shownAudience } = useClosingValue(audienceMessage);
  const [editing, setEditing] = useState<TimelineMessage | null>(null);
  const [regenerating, setRegenerating] = useState<{ message: TimelineMessage } | null>(null);
  const { shown: shownRegeneration, session: regenerationSession } = useClosingValue(regenerating);
  const [busy, setBusy] = useState(false);
  const [modelRequired, setModelRequired] = useState(false);
  const [panel, setPanel] = useState<ToolPanel | null>(null);
  const [swap, setSwap] = useState(false);
  const [modelOpen, setModelOpen] = useState(false);
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
      else if (
        failure.code === "unsupported" &&
        failure.details?.type === "invalid_field" &&
        failure.details.field === "media.image"
      )
        toast.error(t("toast.mediaImageUnsupported"));
      else if (
        failure.code === "unsupported" &&
        failure.details?.type === "invalid_field" &&
        failure.details.field === "media.audio"
      )
        toast.error(t("toast.mediaAudioUnsupported"));
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
  const sceneImageMessage = messages.find((message) => message.id === sceneImageId && message.role === "assistant");
  const sceneImageClosing = useRef(false);
  useEffect(() => { sceneImageClosing.current = false; }, [sceneImageId]);
  const openSceneImage = useCallback((message: TimelineMessage) => {
    setMenuId(null);
    void navigate({ to: "/chat/$conversationId", params: { conversationId }, search: { sceneImage: message.id }, state: { sceneImagePushed: true }, resetScroll: false });
  }, [navigate, conversationId]);
  const closeSceneImage = useCallback(() => {
    if (sceneImageClosing.current || !router.state.location.search.sceneImage) return;
    sceneImageClosing.current = true;
    if (router.state.location.state.sceneImagePushed) router.history.back();
    else void navigate({ to: "/chat/$conversationId", params: { conversationId }, search: {}, replace: true, resetScroll: false });
  }, [router, navigate, conversationId]);
  const seenSceneImage = useRef<string | null>(null);
  useEffect(() => {
    if (sceneImageMessage) seenSceneImage.current = sceneImageMessage.id;
    else if (sceneImageId && seenSceneImage.current === sceneImageId) closeSceneImage();
    if (!sceneImageId) seenSceneImage.current = null;
  }, [sceneImageId, sceneImageMessage, closeSceneImage]);
  const { shown: shownMenu } = useClosingValue(menuMessage);

  const isGroup = view?.kind === "group";
  const character = isGroup ? null : characterParticipant(view);
  const personaChoice = settings.data?.persona;
  const personas = useQuery({
    ...personasQuery(api, conversationId),
    enabled: personaChoice !== undefined,
  });
  const personaId = personaChoice
    ? (personaChoice.id ??
      (personaChoice.source === "app_default" ? (personas.data?.defaultId ?? null) : null))
    : null;
  const persona = personaId
    ? personas.data?.items.find((item) => item.id === personaId)
    : undefined;
  const participant = userParticipant(view);
  const user = useMemo(
    () =>
      participant && persona
        ? { ...participant, name: persona.title, avatar: persona.avatar }
        : participant,
    [participant, persona],
  );
  const participants = useMemo(
    () =>
      view ? view.participants.map((entry) => (user && entry.id === user.id ? user : entry)) : [],
    [view, user],
  );
  const name = character?.name ?? view?.title ?? t("header.untitled");
  const avatar = character?.avatar?.url ?? null;
  const cast = useQuery({ ...libraryCastQuery(api), enabled: isGroup });
  const descriptions = useMemo(
    () => new Map((cast.data ?? []).map((character) => [character.id, character.description ?? ""])),
    [cast.data],
  );
  const members = useMemo(() => participants.filter((entry) => entry.role === "character"), [participants]);
  const faces = useMemo(
    () => members.map((member) => ({ name: member.name, src: member.avatar?.url ?? null })),
    [members],
  );
  const speakers = useMemo<Speaker[]>(
    () =>
      members.map((member) => ({
        id: member.id,
        name: member.name,
        avatar: member.avatar?.url ?? null,
        characterId: member.character_id,
        description: member.character_id ? (descriptions.get(member.character_id) ?? null) : null,
        muted:
          settings.data?.members?.participants.find((entry) => entry.character_id === member.character_id)
            ?.muted ?? false,
      })),
    [members, settings.data, descriptions],
  );
  const method = settings.data?.speaker_selection?.method;
  const directorBehavior = !isGroup ? null : method === "director" ? "cue" : method === "director_action" ? "action" : null;
  const directorSelected = directorBehavior && selection?.behavior === directorBehavior ? selection.id : null;
  const barVisible = isGroup && speakers.length >= 2 && (look.participantsBarEnabled || directorBehavior !== null);
  const pendingTurn = Boolean(view?.pending_turn_id) && !meta.active;
  const sending = meta.active || pendingTurn;
  const background = settings.data?.background.asset?.url ?? null;

  const lastJump = useRef<string | null>(null);
  const hit = search.current?.message_id ?? landing;
  const { jumpTo, jumpToLatest, windowed, pending } = timeline;
  useEffect(() => {
    if (!hit) {
      lastJump.current = null;
      return;
    }
    if (pending || messages.some((message) => message.id === hit) || lastJump.current === hit)
      return;
    lastJump.current = hit;
    jumpTo(hit);
  }, [hit, messages, pending, jumpTo]);
  useEffect(() => {
    if (requestedMessage)
      void navigate({
        to: "/chat/$conversationId",
        params: { conversationId },
        search: {},
        replace: true,
      });
  }, [requestedMessage, navigate, conversationId]);
  const { windowError } = timeline;
  if (landing !== null && (search.open || windowError)) setLanding(null);
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
        if (router.state.location.pathname.startsWith("/chat/"))
          void navigate({ to: "/", replace: true });
      });
      router.history.back();
    } else {
      void navigate({ to: "/", replace: true });
    }
  }, [router, navigate]);
  useEffect(() => {
    let active = true;
    let stop: (() => void) | undefined;
    void api
      .subscribe((event) => {
        if (event.type === "conversation_removed" && event.conversation_id === conversationId)
          goHome();
      })
      .then((unsubscribe) => {
        if (active) stop = unsubscribe;
        else unsubscribe();
      });
    return () => {
      active = false;
      stop?.();
    };
  }, [api, conversationId, goHome]);
  useEffect(() => {
    if (!search.open && windowed && landing === null) jumpToLatest();
  }, [search.open, windowed, landing, jumpToLatest]);

  const openSettings = () => {
    void navigate({
      to: "/chat/$conversationId",
      params: { conversationId },
      search: { panel: "settings" },
      state: { panelPushed: true },
    });
  };
  const closeSettings = () => {
    if (router.state.location.state.panelPushed === true) router.history.back();
    else
      void navigate({
        to: "/chat/$conversationId",
        params: { conversationId },
        search: {},
        replace: true,
      });
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

  const sceneCharacter = useQuery(characterQuery(api, view?.source.type === "direct" ? view.source.character_id : null));
  const companion = view?.kind === "direct" && (settings.data?.companion === true || sceneCharacter.data?.draft.defaults?.interaction_mode === "companion");
  const groupCompanion = view?.kind === "group" && settings.data?.companion_mode?.value === true;
  const companionPage = companion || groupCompanion;
  const openMemories = () => void navigate({ to: companionPage ? "/chat/$conversationId/companion" : "/chat/$conversationId/memory", params: { conversationId } });
  const openMemoryList = () => void navigate({ to: companion ? "/chat/$conversationId/companion" : "/chat/$conversationId/memory", params: { conversationId } });
  const sceneMessage = timeline.windowed ? undefined : messages.find((message) => message.role === "scene");
  const scenes = useMemo(
    () => [...(sceneCharacter.data?.draft.scenes ?? [])].sort((a, b) => a.ordinal - b.ordinal),
    [sceneCharacter.data],
  );
  const sceneIndex = scenes.findIndex((scene) => scene.id === settings.data?.scene.id);
  const sceneSwitch = useMemo(
    () => (sceneMessage && scenes.length > 1 && sceneIndex >= 0 ? { messageId: sceneMessage.id, index: sceneIndex, total: scenes.length } : null),
    [sceneMessage, scenes.length, sceneIndex],
  );
  const onScene = useCallback(
    (message: TimelineMessage, delta: -1 | 1) => {
      const target = scenes[sceneIndex + delta];
      if (!target || sceneIndex < 0 || busyRef.current) return;
      busyRef.current = true;
      void run(async () => {
        await selectScene(services, message, target.id);
      }).finally(() => {
        busyRef.current = false;
      });
    },
    [scenes, sceneIndex, services, run],
  );

  const followLatest = () => {
    stuck.current = true;
    timeline.jumpToLatest();
    setFollowRequest((current) => current + 1);
  };

  const send = async (text: string, attachments: ComposerAttachment[]) => {
    followLatest();
    setPendingMedia(
      attachments.map((attachment) => ({
        id: attachment.id,
        url: attachment.src ?? null,
        kind: attachment.src ? "image" : "audio",
        ...(attachment.name ? { name: attachment.name } : {}),
      })),
    );
    const accepted = await generation.send(
      text,
      attachments.map((attachment) => attachment.id),
      swap,
    );
    if (accepted) setModelRequired(false);
    return accepted;
  };

  const sendDirector = async (text: string, attachments: ComposerAttachment[]) => {
    if (attachments.length > 0) {
      toast.error(t("director.noAttachments"));
      return false;
    }
    if (directorBehavior === "cue" && !directorSelected) {
      setDirectorWiggle((count) => count + 1);
      return false;
    }
    followLatest();
    if (text && !(await run(() => addUserMessage(services, text)))) return false;
    if (directorBehavior === "cue" && directorSelected) await generation.continueReply(swap, directorSelected);
    return true;
  };

  const sendSystem = async (text: string, attachments: ComposerAttachment[]) => {
    followLatest();
    return run(() => addSystemMessage(services, text, attachments.map((attachment) => attachment.id)));
  };

  const directorContinue = () => {
    if (directorSelected) void generation.continueReply(swap, directorSelected);
    else setDirectorWiggle((count) => count + 1);
  };

  const directorTap = async (member: Speaker) => {
    if (directorBehavior === "cue") {
      setSelection({ behavior: "cue", id: member.id });
      return;
    }
    const text = composer.current?.read().trim() ?? "";
    followLatest();
    if (text) {
      composer.current?.replace("");
      if (!(await run(() => addUserMessage(services, text)))) {
        composer.current?.replace(text);
        return;
      }
    }
    await generation.continueReply(swap, member.id);
  };

  const toggleMute = (member: Speaker, muted: boolean) => {
    if (muted && speakers.filter((entry) => !entry.muted).length <= 1) {
      toast.error(t("settings.members.last"));
      return;
    }
    void run(() => setMemberMuted(services, member.id, muted));
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

  const branchHere = (message: TimelineMessage, characterId: string) => {
    setMenuId(null);
    void run(async () => {
      const id = await branchToNewChat(services, message, characterId);
      toast.success(t("toast.branched"));
      void navigate({ to: "/chat/$conversationId", params: { conversationId: id }, search: {} });
    });
  };

  const chooseBranch = (messageId: string, kind: BranchTarget["kind"]) => {
    if (!view) return;
    setMenuId(null);
    setBranching({ messageId, revision: view.revision, kind });
  };

  const remove = async (message: TimelineMessage) => {
    setMenuId(null);
    const sure = await confirm({
      title: t("confirm.deleteTitle"),
      message:
        message.candidate_count > 1
          ? t("confirm.deleteBodyVersions", { count: message.candidate_count })
          : t("confirm.deleteBody"),
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
      await queryClient.invalidateQueries({
        queryKey: queryKeys.conversations.detail(conversationId),
      });
      if (outcome.type === "still_failing") toast.warning(t("banner.memoryRetryStill"));
      else toast.success(t("banner.memoryRetried"));
    });
  };

  const strings = useMemo(
    () => ({
      ...Object.fromEntries(crispKeys.map((key) => [key, t(`crisp.${key}`)])),
      ...(directorBehavior === "action" ? { composerSend: t("director.addMessage") } : {}),
      composerRecording: (elapsed: string) => t("crisp.composerRecording", { elapsed }),
      composerAttachmentRemove: (name: string | undefined) =>
        t("crisp.composerAttachmentRemove", { name: name ?? "" }),
    }),
    [t, directorBehavior],
  );
  const directCharacterId = view?.source.type === "direct" ? view.source.character_id : null;
  const menuAfter =
    shownMenu && !timeline.windowed
      ? messages.length - 1 - messages.findIndex((message) => message.id === shownMenu.id)
      : 0;
  const memoryBusy =
    memory.data?.status.latest_cycle_status === "queued" ||
    memory.data?.status.latest_cycle_status === "processing";

  const canContinue = directorBehavior !== "action" && messages.length > 0;
  const continueReply = () => (directorBehavior ? directorContinue() : void generation.continueReply(swap));
  const groupId = view?.source.type === "group" ? view.source.group_id : null;
  const saveSettings = useCallback(
    (change: Partial<ConversationSettingsPatch_Deserialize>) =>
      run(async () => {
        await updateSettings(services, change);
      }),
    [run, services],
  );
  const widgetSlots = useWidgetSlots(api, { characterId: directCharacterId, groupId }, layout.enabled);
  const messageCount = useQuery({ ...messageCountQuery(api, conversationId), enabled: layout.enabled });
  const lastMessage = timeline.windowed ? undefined : messages[messages.length - 1];
  const widgetCharacter = useMemo<WidgetPerson | null>(
    () =>
      character && directCharacterId
        ? { id: directCharacterId, name: character.name, avatar: character.avatar?.url ?? null, description: sceneCharacter.data?.draft.profile?.description ?? null }
        : null,
    [character, directCharacterId, sceneCharacter.data],
  );
  const widgetMembers = useMemo<WidgetPerson[]>(
    () =>
      isGroup
        ? members.map((member) => ({
            id: member.character_id,
            name: member.name,
            avatar: member.avatar?.url ?? null,
            description: member.character_id ? (descriptions.get(member.character_id) ?? null) : null,
          }))
        : [],
    [isGroup, members, descriptions],
  );
  const widgetPersona = useMemo<WidgetPerson | null>(
    () => (persona ? { id: persona.id, name: persona.title, avatar: persona.avatar?.url ?? null, description: persona.description || null } : null),
    [persona],
  );
  const sceneName = sceneIndex < 0 ? null : scenes[sceneIndex]?.direction?.trim() || t("message.titleScene");
  const widgetActions = useStableActions({
    regenerate: () => {
      if (lastMessage?.role !== "assistant") return;
      timeline.jumpToLatest();
      void generation.regenerate(lastMessage.id, swap);
    },
    continueReply,
    stop: () => generation.stop(view?.pending_turn_id ?? null),
    toggleSwap: () => setSwap((current) => !current),
    newChat: () =>
      void run(async () => {
        let id: string;
        if (groupId) id = await startGroupChat(api, queryClient, groupId);
        else if (directCharacterId) {
          if ((sceneCharacter.data?.draft.starters.length ?? 0) > 0) {
            openSettings();
            return;
          }
          id = await startDirectChat(api, queryClient, directCharacterId);
        } else return;
        await navigate({ to: "/chat/$conversationId", params: { conversationId: id }, search: {}, replace: true });
      }),
    openHistory: () =>
      directCharacterId
        ? void navigate({ to: "/characters/$characterId/history", params: { characterId: directCharacterId } })
        : openSettings(),
    openMemories,
    openSearch: () => search.start(),
    openPersona: () => setPanel("persona"),
    openNote: () => setPanel("note"),
    openModel: () => setModelOpen(true),
    saveAuthorNote: (author_note: string) => saveSettings({ author_note }),
    insertText: (text: string) => composer.current?.append(text),
    updateNode: (id: string, change: Parameters<WidgetActions["updateNode"]>[1]) =>
      widgetSlots.save((slots) => ({ left: updateWidgetNode(slots.left, id, change), right: updateWidgetNode(slots.right, id, change) })),
    pickImage: async () => {
      try {
        const asset = await pickImage(api, "reference_image");
        if (asset) widgetSlots.remember(asset.asset_id, asset.url);
        return asset;
      } catch (error) {
        fail(error);
        return null;
      }
    },
    setTime: (time: Parameters<WidgetActions["setTime"]>[0]) => saveSettings({ time }),
  });
  const memories = memoryOn ? (memory.data?.items ?? noMemories) : noMemories;
  const widgets = useMemo<WidgetActions>(
    () => ({
      ...widgetActions,
      api,
      conversationId,
      isGroup,
      companion,
      character: widgetCharacter,
      members: widgetMembers,
      persona: widgetPersona,
      personaLoading: personas.isPending,
      settings: settings.data,
      messageCount: messageCount.data,
      sceneName,
      memoryOn,
      memories,
      memoryBusy,
      sending,
      canRegenerate: !sending && lastMessage?.role === "assistant",
      canContinue: canContinue && !sending,
      swap,
      assets: widgetSlots.assets,
    }),
    [widgetActions, api, conversationId, isGroup, companion, widgetCharacter, widgetMembers, widgetPersona, personas.isPending, settings.data, messageCount.data, sceneName, memoryOn, memories, memoryBusy, sending, lastMessage?.role, canContinue, swap, widgetSlots.assets],
  );
  const where = useMemo(() => ({ companion, group: isGroup }), [companion, isGroup]);
  const headerInside = layout.enabled && look.chatHeaderMoves;
  const header = (
    <ChatHeader
      columnStyle={look.chatHeaderMoves ? columnStyle : undefined}
      name={name}
      avatar={avatar}
      {...(isGroup ? { faces } : {})}
      phone={phone}
      memory={memoryOn || companionPage ? { count: memory.data?.items.length ?? 0, busy: memoryBusy, ...(companionPage ? { label: t("companion.title") } : {}) } : null}
      search={search}
      onBack={goBack}
      onOptions={openSettings}
      onMemory={openMemories}
    />
  );

  const footerStyle = !look.chatFooterMoves
    ? undefined
    : layout.enabled && layout.columnPx !== null
      ? { width: layout.columnPx, maxWidth: "100%", marginLeft: layout.showLeft ? "auto" : 0, marginRight: layout.showRight ? "auto" : 0 }
      : columnStyle;
  let body;
  let footer = null;
  if (timeline.pending) {
    body = <LoadingState />;
  } else if (!view) {
    body = <FailureState error={timeline.error} onRetry={timeline.refetch} />;
  } else {
    body = (
        <Timeline
          conversationId={conversationId}
          onSceneImage={openSceneImage}
          preserveScroll={sceneImageId !== null}
          columnStyle={columnStyle}
          timeline={timeline}
          store={store}
          onStick={onStick}
          followRequest={followRequest}
          participants={participants}
          user={user ? { name: user.name, avatar: user.avatar?.url ?? null } : undefined}
          character={
            character ? { name: character.name, avatar: character.avatar?.url ?? null } : undefined
          }
          pendingTurn={pendingTurn}
          pendingMedia={pendingMedia}
          locale={i18n.language}
          query={search.query}
          focusId={hit}
          menuFor={menuId}
          onMenu={onMenu}
          onVariant={onVariant}
          scene={sceneSwitch}
          onScene={onScene}
          onRetry={() => void generation.retry()}
          bottomInset={barVisible}
        />
    );
    footer = (
        <div
          className="relative shrink-0 px-3 pb-[calc(var(--safe-bottom)+10px)] pt-2 @xl:px-6"
          style={footerStyle}
        >
          <ChatBanners
            blockers={{
              missingModels: view.missing_models,
              modelRequired,
              memoryBlocked: view.memory_blocked,
            }}
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
              canContinue={canContinue}
              onSend={directorBehavior ? sendDirector : send}
              onSendSystem={sendSystem}
              onContinue={continueReply}
              mentions={isGroup ? speakers : undefined}
              above={
                barVisible
                  ? (draft, setDraft) => (
                      <ParticipantsBar
                        members={speakers}
                        draft={draft}
                        setDraft={setDraft}
                        disabled={sending}
                        director={directorBehavior ? { behavior: directorBehavior, selectedId: directorSelected, wiggle: directorWiggle } : null}
                        onSelect={(member) => void directorTap(member)}
                        onToggleMute={toggleMute}
                      />
                    )
                  : undefined
              }
              onStop={() => generation.stop(view.pending_turn_id)}
              topSlot={
                inlineAuthorNote && settings.data ? (
                  <InlineAuthorNoteBar
                    saved={settings.data.author_note}
                    enterSaves={!phone}
                    onSave={(author_note) =>
                      run(async () => {
                        await updateSettings(services, { author_note });
                      })
                    }
                  />
                ) : undefined
              }
            />
          </div>
        </div>
    );
  }

  return (
    <ChatAppearanceProvider value={look}>
      <CrispStringsProvider strings={strings} locale={i18n.language}>
        <GenerationProgressToast store={generation.store} />
        <Surface
          image={background}
          dim={look.backgroundDim}
          blur={look.backgroundBlur}
          {...(background && look.textMode !== "auto"
            ? { tone: look.textMode === "light" ? ("dark" as const) : ("light" as const) }
            : {})}
          className={cn("h-full overflow-clip", !background && "bg-surface")}
        >
          <div ref={areaRef} className="relative flex h-full min-h-0 flex-col">
            {!headerInside && header}
            <WidgetContext.Provider value={widgets}>
              <WidgetArea layout={layout} slots={widgetSlots.slots} failure={widgetSlots.failure} where={where} save={widgetSlots.save}>
                {headerInside && header}
                {body}
              </WidgetArea>
            </WidgetContext.Provider>
            {footer}
            <BeetrootRain store={store} />
          </div>
        </Surface>

        <Sheet
          open={menuMessage !== null}
          onClose={() => setMenuId(null)}
          size="sm"
          title={
            shownMenu &&
            t(
              shownMenu.role === "assistant"
                ? "message.titleAssistant"
                : shownMenu.role === "user"
                  ? "message.titleUser"
                  : shownMenu.role === "scene"
                    ? "message.titleScene"
                    : "message.titleSystem",
              { name: (isGroup ? authorOf(participants, shownMenu)?.name : undefined) ?? name },
            )
          }
          description={shownMenu && formatTime(shownMenu.created_at, i18n.language)}
        >
          {shownMenu && (
            <MessageMenu
              api={api}
              conversationId={conversationId}
              message={shownMenu}
              companion={companion}
              groupCompanion={groupCompanion}
              onCompanionEffect={() => { setMenuId(null); setEffectMessage(shownMenu); }}
              onAudience={() => { setMenuId(null); setAudienceMessage(shownMenu); }}
              after={menuAfter}
              busy={meta.active || pendingTurn}
              canEdit={
                shownMenu.role !== "user" ||
                (!timeline.windowed &&
                  shownMenu.id === messages.findLast((message) => message.role === "user")?.id)
              }
              canRegenerate={
                shownMenu.role === "assistant" &&
                !timeline.windowed &&
                shownMenu.id === messages[messages.length - 1]?.id
              }
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
              onRegenerateOptions={() => {
                setMenuId(null);
                setRegenerating({ message: shownMenu });
              }}
              onPin={() => togglePin(shownMenu)}
              onRewind={() => void rewind(shownMenu, menuAfter)}
              {...(directCharacterId
                ? { onBranch: () => branchHere(shownMenu, directCharacterId) }
                : isGroup
                  ? { groupBranch: true }
                  : {})}
              onBranchCharacter={() => chooseBranch(shownMenu.id, "character")}
              {...(directCharacterId ? { onBranchGroup: () => chooseBranch(shownMenu.id, "group") } : {})}
              onDelete={() => void remove(shownMenu)}
              {...(shownMenu.role === "assistant" ? { onSceneImage: () => openSceneImage(shownMenu) } : {})}
            />
          )}
        </Sheet>

        {sceneImageMessage && <SceneImageSheet key={sceneImageMessage.id} api={api} conversationId={conversationId} message={sceneImageMessage} busy={sending} onClose={closeSceneImage} />}

        {shownRegeneration && <RegenerationSheet
          key={regenerationSession}
          api={api}
          conversationId={conversationId}
          open={regenerating !== null}
          group={isGroup}
          speakers={speakers}
          originalName={authorOf(participants, shownRegeneration.message)?.name ?? name}
          originalAvatar={authorOf(participants, shownRegeneration.message)?.avatar ?? null}
          busy={meta.active || pendingTurn}
          onClose={() => setRegenerating(null)}
          onRegenerate={async (options) => {
            const accepted = await generation.regenerate(shownRegeneration.message.id, swap, options);
            if (accepted) timeline.jumpToLatest();
            return accepted;
          }}
        />}

        {shownBranch && view && <BranchSheet
          key={branchSession} open={branching !== null}
          services={services} view={view} target={shownBranch}
          onClose={() => setBranching(null)}
          onCreated={(id) => {
            setBranching(null);
            toast.success(t("toast.branched"));
            void navigate({ to: "/chat/$conversationId", params: { conversationId: id }, search: {} });
          }}
        />}

        {shownAudience && groupCompanion && view && <AudienceSheet key={shownAudience.id} api={api} conversationId={conversationId} message={shownAudience} participants={view.participants} open={audienceMessage !== null} onClose={() => setAudienceMessage(null)} />}
        {shownEffect && (groupCompanion && view
          ? <GroupEffectSheet key={shownEffect.id} api={api} conversationId={conversationId} message={shownEffect} participants={view.participants} open={effectMessage !== null} onClose={() => setEffectMessage(null)} />
          : <EffectSheet key={shownEffect.id} api={api} conversationId={conversationId} message={shownEffect} open={effectMessage !== null} onClose={() => setEffectMessage(null)} />)}

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
            onOpenChat={(id, replace) =>
              void navigate({
                to: "/chat/$conversationId",
                params: { conversationId: id },
                search: {},
                replace,
              })
            }
            onHome={goHome}
            onMemories={openMemoryList}
            onBranches={() =>
              void navigate({ to: "/chat/$conversationId/branches", params: { conversationId } })
            }
            onBranchTarget={(kind) => void run(async () => {
              const page = await api.call("conversationMessages", { conversation_id: conversationId, before_cursor: null, after_cursor: null, limit: 1 });
              const message = page.items.at(-1);
              if (!message) return;
              await navigate({ to: "/chat/$conversationId", params: { conversationId }, search: {}, replace: true });
              chooseBranch(message.id, kind);
            })}
            onModels={() =>
              void navigate({ to: "/settings/$section", params: { section: "models" } })
            }
            onAppearance={() => setAppearanceOpen(true)}
          />
        )}

        {view && (
          <ChatAppearanceDrawer
            open={appearanceOpen && !panelOpen}
            api={api}
            characterId={view.source.type === "direct" ? view.source.character_id : null}
            groupId={view.source.type === "group" ? view.source.group_id : null}
            name={name}
            onClose={() => setAppearanceOpen(false)}
            onDraft={setDraftLook}
          />
        )}

        <ModelSheet ctx={{ api, settings: settings.data, busy, save: saveSettings }} open={modelOpen} onClose={() => setModelOpen(false)} />

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
    </ChatAppearanceProvider>
  );
}
