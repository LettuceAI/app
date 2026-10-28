import { useEffect, useRef, useState } from "react";
import { useInfiniteQuery } from "@tanstack/react-query";
import { MessageSquare } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, AvatarGroup, cn, EmptyState, icon, InlineMarkdown, relativeTime, SkeletonRow, useLongPress } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { CharacterSummary, ConversationSummary } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { byRecency, recentChatsQuery } from "../data/recent";

function RecentRow({ chat, character, onOpen, onMenu, held }: { chat: ConversationSummary; character: CharacterSummary | null; onOpen: () => void; onMenu: () => void; held: boolean }) {
  const { i18n } = useTranslation("home");
  const name = character?.name ?? chat.title;
  const press = useLongPress(onMenu);
  return (
    <button
      type="button"
      aria-haspopup="dialog"
      {...press.handlers}
      onClick={press.guardClick(onOpen)}
      className={cn("touch-target flex w-full items-start gap-3 rounded-2xl px-3 py-2.5 text-left transition-colors motion-instant", held ? "bg-fill-2" : "hover:bg-fill")}
    >
      {character ? (
        <Avatar src={character.avatar?.url ?? null} name={name} size="md" className="mt-0.5" />
      ) : (
        <AvatarGroup people={chat.avatars.map((avatar, index) => ({ name: `${chat.title} ${index + 1}`, src: avatar.url }))} size="sm" max={3} />
      )}
      <span className="min-w-0 flex-1">
        <span className="flex min-w-0 items-baseline gap-2">
          <span className="min-w-0 truncate text-base font-semibold text-fg">{name}</span>
          {character && <span className="min-w-0 flex-1 truncate text-sm text-fg-3">{chat.title}</span>}
          <span className="ml-auto shrink-0 text-2xs tabular-nums text-fg-3">{relativeTime(chat.updated_at, undefined, i18n.language)}</span>
        </span>
        {chat.last_message_preview && <span className="mt-0.5 line-clamp-2 wrap-anywhere text-sm leading-snug text-fg-3"><InlineMarkdown>{chat.last_message_preview}</InlineMarkdown></span>}
      </span>
    </button>
  );
}

/** Every active chat, newest first, grouped by when; more load as the end comes into view. */
export function RecentList({
  api,
  characters,
  search,
  onOpen,
  onMenu,
  heldId,
}: {
  api: ApiClient;
  characters: ReadonlyMap<string, CharacterSummary>;
  search: string;
  onOpen: (chat: ConversationSummary) => void;
  onMenu: (chat: ConversationSummary, character: CharacterSummary | null) => void;
  heldId: string | null;
}) {
  const { t } = useTranslation("home");
  const chats = useInfiniteQuery(recentChatsQuery(api));
  const [openedAt] = useState(() => Date.now());
  const end = useRef<HTMLDivElement>(null);
  const { hasNextPage, isFetchingNextPage, fetchNextPage } = chats;

  useEffect(() => {
    const node = end.current;
    if (!node || !hasNextPage) return;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting) && !isFetchingNextPage) void fetchNextPage();
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, [hasNextPage, isFetchingNextPage, fetchNextPage]);

  if (chats.isPending) {
    return (
      <div className="space-y-1 px-3">
        {Array.from({ length: 8 }, (_, index) => <SkeletonRow key={index} />)}
      </div>
    );
  }
  if (chats.isError) return <FailureState error={chats.error} onRetry={() => void chats.refetch()} />;
  const needle = search.trim().toLowerCase();
  const characterOf = (chat: ConversationSummary) => (chat.source.type === "direct" ? (characters.get(chat.source.character_id) ?? null) : null);
  const all = chats.data.pages.flatMap((page) => page.items);
  const shown = needle
    ? all.filter((chat) => `${characterOf(chat)?.name ?? ""} ${chat.title} ${chat.last_message_preview ?? ""}`.toLowerCase().includes(needle))
    : all;
  if (shown.length === 0) return <EmptyState icon={<MessageSquare size={icon.xl} />} title={t("recent.empty")} />;

  return (
    <div className="space-y-5">
      {byRecency(shown, openedAt).map((entry) => (
        <section key={entry.group} aria-label={t(`recent.${entry.group}`)}>
          <h2 className="px-3 pb-1 text-sm font-medium text-fg-3">{t(`recent.${entry.group}`)}</h2>
          {entry.chats.map((chat) => (
            <RecentRow key={chat.id} chat={chat} character={characterOf(chat)} held={chat.id === heldId} onOpen={() => onOpen(chat)} onMenu={() => onMenu(chat, characterOf(chat))} />
          ))}
        </section>
      ))}
      <div ref={end} aria-hidden className="h-px" />
    </div>
  );
}
