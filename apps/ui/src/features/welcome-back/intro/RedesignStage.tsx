import { useMemo, useState, type ReactNode } from "react";
import { motion, type Variants } from "framer-motion";
import { useTranslation } from "react-i18next";
import { Composer, ListGroup, cn } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import { MemoryRow, MessageRow, WidgetContext, DiceWidget, MemoryWidget, TimeWidget, BondTile, type WidgetActions } from "@/features/conversations";
import { GroupCard } from "@/features/groups";
import { HeroCard, HomeCharacterCard, RecentRow } from "@/features/home";
import type { Platform } from "./platform";
import type { Showcase } from "./showcase";

const noop = () => {};

type PieceKey = "reply" | "cardA" | "bond" | "time" | "hero" | "memory" | "dice" | "session" | "memoryWidget" | "composer" | "group";

/** Where each piece rests (percent of the stage) and its tilt. The middle stays clear for the title. */
const DESKTOP: { key: PieceKey; x: number; y: number; rotate: number }[] = [
  { key: "reply", x: 2, y: 3, rotate: -2 },
  { key: "cardA", x: 35, y: 3, rotate: 2 },
  { key: "bond", x: 63, y: 5, rotate: -3 },
  { key: "dice", x: 81, y: 9, rotate: 2 },
  { key: "hero", x: -4, y: 25, rotate: 2 },
  { key: "memory", x: 74, y: 27, rotate: -2 },
  { key: "time", x: 80, y: 50, rotate: 3 },
  { key: "session", x: 1, y: 60, rotate: -2 },
  { key: "memoryWidget", x: 3, y: 74, rotate: 2 },
  { key: "composer", x: 30, y: 78, rotate: 1 },
  { key: "group", x: 61, y: 66, rotate: -2 },
];

const PHONE: { key: PieceKey; x: number; y: number; rotate: number }[] = [
  { key: "reply", x: -50, y: 2, rotate: -2 },
  { key: "bond", x: 53, y: 8, rotate: 3 },
  { key: "cardA", x: 10, y: 17, rotate: 2 },
  { key: "memory", x: -36, y: 56, rotate: -2 },
  { key: "time", x: 44, y: 70, rotate: 3 },
];

const stage: Variants = {
  hidden: {},
  show: { transition: { staggerChildren: 0.07, delayChildren: 0.55 } },
  exit: { transition: { staggerChildren: 0.025, staggerDirection: -1 } },
};

/** Each piece comes in from the edge it is nearest to and leaves the same way. */
const piece: Variants = {
  hidden: ({ dx, dy, rotate }: { dx: number; dy: number; rotate: number }) => ({ x: dx, y: dy, rotate: rotate * 4, opacity: 0 }),
  show: ({ rotate }: { rotate: number }) => ({ x: 0, y: 0, rotate, opacity: 1, transition: { type: "spring", visualDuration: 1.1, bounce: 0.16 } }),
  exit: ({ dx, dy, rotate }: { dx: number; dy: number; rotate: number }) => ({ x: dx, y: dy, rotate: rotate * 4, opacity: 0, transition: { duration: 0.5, ease: [0.55, 0, 1, 0.45] } }),
};

const title: Variants = {
  hidden: { opacity: 0, scale: 0.94, filter: "blur(10px)" },
  show: { opacity: 1, scale: 1, filter: "blur(0px)", transition: { duration: 1.1, delay: 0.9, ease: [0.22, 1, 0.36, 1] } },
  exit: { opacity: 0, scale: 1.04, filter: "blur(8px)", transition: { duration: 0.4 } },
};

function widgetActions(api: ApiClient, showcase: Showcase): WidgetActions {
  const person = { id: showcase.cassian.id, name: showcase.cassian.name, avatar: showcase.cassian.avatar?.url ?? null, description: showcase.cassian.description };
  return {
    api,
    conversationId: showcase.session.id,
    isGroup: false,
    companion: false,
    character: person,
    members: [],
    persona: null,
    personaLoading: false,
    settings: showcase.chatSettings,
    messageCount: showcase.session.message_count,
    sceneName: null,
    memoryOn: true,
    memories: showcase.facts,
    memoryBusy: false,
    sending: false,
    canRegenerate: false,
    canContinue: false,
    swap: false,
    assets: new Map(),
    regenerate: noop,
    continueReply: noop,
    stop: noop,
    toggleSwap: noop,
    newChat: noop,
    openHistory: noop,
    openMemories: noop,
    openSearch: noop,
    openPersona: noop,
    openNote: noop,
    openModel: noop,
    saveAuthorNote: async () => true,
    insertText: noop,
    updateNode: async () => true,
    pickImage: async () => null,
    setTime: async () => true,
  };
}

/** The pieces lifted from the real screens, each the app's own component rendered with the bundled cast. */
function usePieces(api: ApiClient, showcase: Showcase) {
  const { t, i18n } = useTranslation("conversations");
  const [draft, setDraft] = useState("");
  const { cassian, mireille, session, reply, participants, companion, chatMemories, groups } = showcase;
  return {
    cardA: (
      <div className="w-[320px]">
        <HomeCharacterCard api={api} character={mireille} onOpen={noop} onMenu={noop} held={false} />
      </div>
    ),
    hero: (
      <div className="w-[360px]">
        <HeroCard api={api} home={{ character: cassian, latest: session, activeAt: session.updated_at }} compact onOpen={noop} onMenu={noop} held={false} />
      </div>
    ),
    group: (
      <div className="w-[240px]">
        <GroupCard entry={groups[0]!} layout="gallery" compact={false} held={false} onOpen={noop} onMenu={noop} />
      </div>
    ),
    session: (
      <div className="w-[340px] overflow-hidden rounded-xl border border-line bg-surface">
        <RecentRow chat={session} character={cassian} held={false} onOpen={noop} onMenu={noop} />
      </div>
    ),
    reply: (
      <div className="w-[380px]">
        <MessageRow
          message={reply}
          group="only"
          participants={participants}
          locale={i18n.language}
          query=""
          dimmed={false}
          held={false}
          latest={false}
          onMenu={noop}
          onVariant={noop}
          conversationId={session.id}
          onSceneImage={noop}
        />
      </div>
    ),
    composer: (
      <div className="w-[380px]">
        <Composer value={draft} onChange={setDraft} onSend={() => setDraft("")} placeholder={t("composer.placeholder", { name: cassian.name })} />
      </div>
    ),
    memory: (
      <div className="w-[360px]">
        <ListGroup>
          <MemoryRow item={chatMemories[0]!} onOpen={noop} />
        </ListGroup>
      </div>
    ),
    bond: (
      <div className="w-[200px]">
        <BondTile axis="trust" value={companion.relationship.trust ?? 0} reference={companion.relationshipDefaults.trust} />
      </div>
    ),
    memoryWidget: (
      <div className="w-[260px]">
        <MemoryWidget node={{ type: "memory", id: "showcase-memory", design: null, title: null, limit: 3 }} />
      </div>
    ),
    dice: (
      <div className="w-[240px]">
        <DiceWidget node={{ type: "dice", id: "showcase-dice", design: null, title: null, description: null, notation: "1d20" }} />
      </div>
    ),
    time: (
      <div className="w-[240px]">
        <TimeWidget node={{ type: "time", id: "showcase-time", design: null, title: null, hour_format: "h24", show_seconds: false, show_date: true }} />
      </div>
    ),
  } satisfies Record<PieceKey, ReactNode>;
}

/** The redesign page: the app's own pieces fly in around the title and stay where the screens would put them. */
export function RedesignStage({ platform, api, showcase }: { platform: Platform; api: ApiClient; showcase: Showcase }) {
  const { t } = useTranslation("welcome-back");
  const phone = platform === "phone";
  const pieces = usePieces(api, showcase);
  const actions = useMemo(() => widgetActions(api, showcase), [api, showcase]);
  const layout = phone ? PHONE : DESKTOP;
  return (
    <WidgetContext.Provider value={actions}>
      <motion.div variants={stage} initial="hidden" animate="show" exit="exit" className="absolute inset-0 overflow-hidden">
        {layout.map(({ key, x, y, rotate }) => {
          const dx = (x < 50 ? -1 : 1) * (phone ? 420 : 900);
          const dy = (y < 50 ? -1 : 1) * (phone ? 200 : 320);
          return (
            <motion.div
              key={key}
              custom={{ dx, dy, rotate }}
              variants={piece}
              className="pointer-events-auto absolute"
              style={{ left: `${x}%`, top: `${y}%` }}
            >
              {pieces[key]}
            </motion.div>
          );
        })}
        <motion.div variants={title} className={cn("pointer-events-none absolute inset-0 flex flex-col items-center justify-center px-6 text-center", phone ? "pb-28" : "pb-20")}>
          <h1 className={cn("max-w-3xl text-balance font-bold tracking-[-0.04em] text-fg [text-shadow:0_4px_40px_rgb(0_0_0/0.6)]", phone ? "text-[2.6rem] leading-[1.05]" : "text-7xl leading-[1.02]")}>
            {t("intro.redesign.title")}
          </h1>
          <p className={cn("mt-5 max-w-md text-balance text-fg-2 [text-shadow:0_2px_24px_rgb(0_0_0/0.7)]", phone ? "text-base" : "text-xl")}>
            {t("intro.redesign.body")}
          </p>
        </motion.div>
      </motion.div>
    </WidgetContext.Provider>
  );
}
