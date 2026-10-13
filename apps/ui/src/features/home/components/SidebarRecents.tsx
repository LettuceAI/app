import { useEffect, useRef, useState } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import { useTranslation } from "react-i18next";
import { Avatar, cn, motion as m, NavLabel, useLongPress, useNavExpanded } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { CharacterSummary, ConversationSummary } from "@/api/generated/bindings";
import { homeCharactersQuery } from "../data/home-characters";
import { byRecency, recentChatsQuery } from "../data/recent";
import type { ChatMenuTarget } from "./ChatMenu";

function SidebarChat({ chat, character, held, onMenu }: { chat: ConversationSummary; character: CharacterSummary | null; held: boolean; onMenu: () => void }) {
  const press = useLongPress(onMenu);
  const name = character?.name ?? chat.title;
  return (
    <button
      type="button"
      aria-haspopup="dialog"
      title={`${name} · ${chat.title}`}
      {...press.handlers}
      className={cn(
        "flex h-9 w-full items-center overflow-hidden rounded-lg text-left text-sm transition-colors motion-instant",
        held ? "bg-fill-2 text-fg" : "text-fg-2 hover:bg-fill hover:text-fg",
      )}
    >
      <span className="flex w-11 shrink-0 items-center justify-center">
        <Avatar src={character?.avatar?.url ?? chat.avatars[0]?.url ?? null} name={name} size="xs" />
      </span>
      <NavLabel>{chat.title}</NavLabel>
    </button>
  );
}

/** Active chats in the desktop sidebar, newest first and grouped by when; more load as the end scrolls into view. */
export function SidebarRecents({ api, onMenu, heldId }: { api: ApiClient; onMenu: (target: ChatMenuTarget) => void; heldId: string | null }) {
  const { t } = useTranslation("home");
  const open = useNavExpanded();
  const reduced = useReducedMotion();
  const chats = useInfiniteQuery(recentChatsQuery(api));
  const characters = useQuery(homeCharactersQuery(api));
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
  }, [hasNextPage, isFetchingNextPage, fetchNextPage, chats.data]);

  if (!chats.data) return null;
  const all = chats.data.pages.flatMap((page) => page.items);
  if (all.length === 0) return open ? <p className="whitespace-nowrap px-3 py-2 text-sm text-fg-3">{t("sidebar.empty")}</p> : null;

  const byId = new Map((characters.data ?? []).map((home) => [home.character.id, home.character]));
  const characterOf = (chat: ConversationSummary) => (chat.source.type === "direct" ? (byId.get(chat.source.character_id) ?? null) : null);
  const spring = reduced ? { duration: 0 } : m.sidebar;
  const fade = reduced ? { duration: 0 } : open ? { duration: 0.2, delay: 0.1 } : { duration: 0.1 };

  return (
    <div className="pb-2" aria-label={t("sidebar.label")}>
      {byRecency(all, openedAt).map((entry, index) => (
        <motion.section key={entry.group} aria-label={t(`recent.${entry.group}`)} initial={false} animate={{ marginTop: index === 0 ? 0 : open ? 12 : 2 }} transition={spring}>
          <motion.h2
            initial={false}
            animate={{ height: open ? 24 : 0, opacity: open ? 1 : 0 }}
            transition={{ height: spring, opacity: fade }}
            aria-hidden={!open}
            className="overflow-hidden whitespace-nowrap px-3 text-2xs font-medium leading-6 text-fg-3"
          >
            {t(`recent.${entry.group}`)}
          </motion.h2>
          <div className="relative space-y-0.5">
            <AnimatePresence mode="popLayout" initial={false}>
              {entry.chats.map((chat) => (
                <motion.div key={chat.id} layout exit={{ opacity: 0 }} transition={m.settle}>
                  <SidebarChat chat={chat} character={characterOf(chat)} held={chat.id === heldId} onMenu={() => onMenu({ chat, character: characterOf(chat) })} />
                </motion.div>
              ))}
            </AnimatePresence>
          </div>
        </motion.section>
      ))}
      <div ref={end} aria-hidden className="h-px" />
    </div>
  );
}
