import { Fragment, useCallback, useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore, type PointerEvent } from "react";
import { ArrowDown } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Alert, Button, cn, EmptyState, icon, IconButton, MessageDivider, Message, useCompactLayout } from "@lettuceai/crisp";
import { MessageSquare } from "lucide-react";
import type { ParticipantView, TimelineMessage } from "@/api/generated/bindings";
import type { GenerationStore } from "../data/generation-store";
import { authorOf, dayLabel, groupOf, sameDay, spacing, textOf, type Author, type MediaItem } from "../data/messages";
import type { Timeline as TimelineData } from "../data/use-timeline";
import { Attached, MessageBody, MessageRow } from "./MessageRow";
import { PendingReply, StreamingReply } from "./StreamingReply";

export interface TimelineProps {
  timeline: TimelineData;
  store: GenerationStore;
  onStick: (stuck: boolean) => void;
  participants: readonly ParticipantView[];
  user: Author | undefined;
  character: Author | undefined;
  pendingTurn: boolean;
  pendingMedia: readonly MediaItem[];
  locale: string;
  query: string;
  focusId: string | null;
  menuFor: string | null;
  onMenu: (message: TimelineMessage) => void;
  onVariant: (message: TimelineMessage, delta: -1 | 1) => void;
  onRetry: () => void;
}

const nearBottom = 80;

export function Timeline({
  timeline, store, onStick, participants, user, character, pendingTurn, pendingMedia, locale, query, focusId, menuFor, onMenu, onVariant, onRetry,
}: TimelineProps) {
  const { t } = useTranslation("conversations");
  const phone = useCompactLayout();
  const meta = useSyncExternalStore(store.subscribeMeta, store.getMeta);
  const scroller = useRef<HTMLDivElement>(null);
  const content = useRef<HTMLDivElement>(null);
  const top = useRef<HTMLDivElement>(null);
  const bottom = useRef<HTMLDivElement>(null);
  const stick = useRef(true);
  const setStick = useCallback(
    (value: boolean) => {
      stick.current = value;
      onStick(value);
    },
    [onStick],
  );
  const prepend = useRef<{ height: number; top: number } | null>(null);
  const lastTop = useRef(0);
  const dragging = useRef(false);
  const touchY = useRef<number | null>(null);
  const focused = useRef<string | null>(null);
  const [atBottom, setAtBottom] = useState(true);
  const { messages, hasOlder, loadingOlder, loadOlder, hasNewer, loadNewer, windowed, jumpToLatest } = timeline;
  const needle = query.trim().toLowerCase();
  const lastMessage = messages[messages.length - 1];
  const latestAssistantId = !windowed && lastMessage?.role === "assistant" ? lastMessage.id : null;
  const firstId = messages[0]?.id;

  const [now] = useState(() => Date.now());

  const requestOlder = useCallback(() => {
    const el = scroller.current;
    if (el) prepend.current = { height: el.scrollHeight, top: el.scrollTop };
    loadOlder();
  }, [loadOlder]);

  useLayoutEffect(() => {
    const el = scroller.current;
    const saved = prepend.current;
    if (el && saved) {
      el.scrollTop = saved.top + (el.scrollHeight - saved.height);
      prepend.current = null;
    }
  }, [firstId]);

  useEffect(() => {
    if (!loadingOlder) prepend.current = null;
  }, [loadingOlder]);

  const changeVariant = useCallback(
    (message: TimelineMessage, delta: -1 | 1) => {
      setStick(true);
      const el = scroller.current;
      if (el) el.scrollTop = el.scrollHeight;
      onVariant(message, delta);
    },
    [onVariant, setStick],
  );

  useLayoutEffect(() => {
    const el = scroller.current;
    if (el && stick.current) el.scrollTop = el.scrollHeight;
  }, [messages, meta.active, meta.userText, meta.failure, pendingTurn]);

  useEffect(() => {
    const body = content.current;
    if (!body || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => {
      const el = scroller.current;
      if (!el) return;
      if (stick.current && !prepend.current) el.scrollTop = el.scrollHeight;
      setAtBottom(el.scrollHeight - el.scrollTop - el.clientHeight < nearBottom);
    });
    observer.observe(body);
    if (scroller.current) observer.observe(scroller.current);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const root = scroller.current;
    const target = top.current;
    if (!root || !target || !hasOlder || loadingOlder || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) requestOlder();
    }, { root });
    observer.observe(target);
    return () => observer.disconnect();
  }, [hasOlder, loadingOlder, requestOlder, firstId]);

  useEffect(() => {
    const root = scroller.current;
    const target = bottom.current;
    if (!root || !target || !hasNewer || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) loadNewer();
    }, { root });
    observer.observe(target);
    return () => observer.disconnect();
  }, [hasNewer, loadNewer, messages.length]);

  useEffect(() => {
    if (!focusId) {
      focused.current = null;
      return;
    }
    if (focused.current === focusId) return;
    const element = scroller.current?.querySelector(`[id="m-${focusId}"]`);
    if (!element) return;
    focused.current = focusId;
    setStick(false);
    const root = scroller.current;
    if (!root || !(element instanceof HTMLElement)) return;
    const offset = element.getBoundingClientRect().top - root.getBoundingClientRect().top;
    root.scrollTop += offset - (root.clientHeight - element.offsetHeight) / 2;
  }, [focusId, messages, setStick]);

  const onScroll = () => {
    const el = scroller.current;
    if (!el) return;
    if (stick.current && !dragging.current && el.scrollHeight - el.scrollTop - el.clientHeight > 2) {
      el.scrollTop = el.scrollHeight;
      lastTop.current = el.scrollTop;
      setAtBottom(true);
      return;
    }
    const distance = el.scrollHeight - el.scrollTop - el.clientHeight;
    const movedUp = el.scrollTop < lastTop.current - 1;
    lastTop.current = el.scrollTop;
    const near = distance < nearBottom;
    if (dragging.current && movedUp && distance > 2) setStick(false);
    else if (near && !stick.current && !movedUp && !windowed) setStick(true);
    setAtBottom(near);
  };

  const leave = () => {
    const el = scroller.current;
    if (el && stick.current && el.scrollHeight > el.clientHeight) setStick(false);
  };

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget) return;
    dragging.current = true;
    window.addEventListener("pointerup", () => (dragging.current = false), { once: true });
  };

  const jump = () => {
    setStick(true);
    if (windowed) jumpToLatest();
    const el = scroller.current;
    if (el) el.scrollTo({ top: el.scrollHeight, behavior: "smooth" });
  };

  const showUser = meta.userText !== null && !(meta.userMessageId && messages.some((message) => message.id === meta.userMessageId));
  const trailing = meta.active && meta.targetId === null;
  const regenerating = meta.active ? meta.targetId : null;

  return (
    <div className="relative flex min-h-0 flex-1 flex-col">
      <div
        ref={scroller}
        onScroll={onScroll}
        onWheel={(event) => event.deltaY < 0 && leave()}
        onTouchStart={(event) => (touchY.current = event.touches[0]?.clientY ?? null)}
        onTouchMove={(event) => {
          const y = event.touches[0]?.clientY;
          if (y !== undefined && touchY.current !== null && y > touchY.current + 4) leave();
        }}
        onKeyDown={(event) => (event.key === "ArrowUp" || event.key === "PageUp" || event.key === "Home") && leave()}
        onPointerDown={onPointerDown}
        className="scrollbar-thin min-h-0 flex-1 overflow-y-auto [overflow-anchor:none]">
        <div ref={content} className={cn("pb-6 pt-3", phone ? "px-3" : "px-6")}>
          <div ref={top} aria-hidden className="h-px" />
          {loadingOlder && <p role="status" className="pb-2 text-center text-2xs text-fg-3">{t("timeline.loadingOlder")}</p>}
          {messages.length === 0 && !showUser && !trailing && (
            <EmptyState size="sm" icon={<MessageSquare size={icon.lg} />} title={t("timeline.empty")} />
          )}
          {messages.map((message, index) => {
            const previous = messages[index - 1];
            const label = !previous || !sameDay(previous.created_at, message.created_at) ? dayLabel(message.created_at, now, locale) : null;
            const group = groupOf(messages, index);
            const dimmed = needle.length > 0 && !textOf(message).toLowerCase().includes(needle);
            return (
              <Fragment key={message.id}>
                {label && <MessageDivider>{label.kind === "date" ? label.text : t(`timeline.${label.kind}`)}</MessageDivider>}
                {regenerating === message.id ? (
                  <StreamingReply store={store} author={authorOf(participants, message)} group={group} pinned={message.pinned} />
                ) : (
                  <MessageRow
                    message={message}
                    group={group}
                    participants={participants}
                    locale={locale}
                    query={needle ? query.trim() : ""}
                    dimmed={dimmed}
                    held={menuFor === message.id}
                    latest={latestAssistantId === message.id}
                    onMenu={onMenu}
                    onVariant={changeVariant}
                  />
                )}
              </Fragment>
            );
          })}
          {showUser && (
            <div className={spacing("only")}>
              <Message role="user" {...(user ? { author: user } : {})} group="only" className="mt-0!" {...(pendingMedia.length > 0 ? { attachments: <Attached media={pendingMedia} /> } : {})}>
                <MessageBody text={meta.userText ?? ""} query="" />
              </Message>
            </div>
          )}
          {trailing && <StreamingReply store={store} author={character} group="only" />}
          {!meta.active && pendingTurn && <PendingReply author={character} group="only" />}
          {meta.failure && (
            <div className="mt-5">
              <Alert
                tone="danger"
                title={t("failure.title")}
                description={t(`failure.code.${meta.failure.code}`)}
                action={<Button variant="secondary" size="sm" onClick={onRetry}>{t("failure.retry")}</Button>}
                onDismiss={() => store.dismissFailure()}
              />
            </div>
          )}
          <div ref={bottom} aria-hidden className="h-px" />
        </div>
      </div>
      {(!atBottom || windowed) && (
        <div className="pointer-events-none absolute inset-x-0 bottom-3 flex justify-center">
          <IconButton label={t("timeline.jumpToLatest")} shape="round" variant="secondary" onClick={jump} className="pointer-events-auto shadow-raised">
            <ArrowDown size={icon.md} />
          </IconButton>
        </div>
      )}
    </div>
  );
}
