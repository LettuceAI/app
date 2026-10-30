import { useEffect, useMemo } from "react";
import { AnimatePresence, motion, useAnimationControls } from "framer-motion";
import { Clapperboard, VolumeX } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, cn, icon, motion as m, useLongPress } from "@lettuceai/crisp";
import { useChatLook } from "@/entities/preferences";
import { findMention, toggleMention } from "../data/mentions";
import type { Speaker } from "./SpeakerSheet";

export type BarMember = Speaker;

export interface DirectorState {
  behavior: "cue" | "action";
  selectedId: string | null;
  wiggle: number;
}

export interface ParticipantsBarProps {
  members: readonly BarMember[];
  draft: string;
  setDraft: (draft: string) => void;
  disabled: boolean;
  director: DirectorState | null;
  onSelect: (member: BarMember) => void;
  onToggleMute: (member: BarMember, muted: boolean) => void;
}

const sizes = { small: "h-10 w-10", medium: "h-12 w-12", large: "h-14 w-14" } as const;
const shapes = { round: "rounded-full", boxed: "rounded-none", rounded_box: "rounded-xl" } as const;
const gaps = { tight: "gap-1", normal: "gap-2", relaxed: "gap-3" } as const;
const aligns = { left: "justify-start", center: "justify-center", right: "justify-end" } as const;
const backgrounds = {
  solid: "bg-surface",
  fading: "bg-linear-to-t from-surface to-transparent",
  transparent: "",
} as const;

interface FaceProps {
  member: BarMember;
  mentioned: boolean;
  selected: boolean;
  dimmed: boolean;
  disabled: boolean;
  director: boolean;
  onTap: () => void;
  onMute: () => void;
}

function Face({ member, mentioned, selected, dimmed, disabled, director, onTap, onMute }: FaceProps) {
  const { t } = useTranslation("conversations");
  const look = useChatLook();
  const press = useLongPress(onMute, { enabled: !disabled });
  const label = director
    ? t("participants.speak", { name: member.name })
    : member.muted
      ? t("participants.muted", { name: member.name })
      : mentioned
        ? t("participants.unmention", { name: member.name })
        : t("participants.mention", { name: member.name });
  return (
    <motion.button
      type="button"
      {...press.handlers}
      onClick={press.guardClick(onTap)}
      disabled={disabled}
      title={label}
      aria-label={label}
      aria-pressed={director ? selected : mentioned}
      animate={{ scale: selected ? 1.08 : 1 }}
      {...(disabled ? {} : { whileTap: { scale: 0.95 } })}
      transition={m.snap}
      className={cn("pointer-events-auto relative shrink-0 select-none disabled:cursor-not-allowed", sizes[look.participantsBarAvatarSize])}
    >
      <Avatar
        src={member.avatar}
        name={member.name}
        ring={selected || mentioned}
        className={cn(
          "h-full w-full transition-opacity",
          shapes[look.participantsBarAvatarShape],
          dimmed && "opacity-40 grayscale",
          director && !selected && !dimmed && "ring-1 ring-accent/30",
        )}
      />
      {member.muted && (
        <span className="absolute -bottom-0.5 -right-0.5 z-10 flex h-5 w-5 items-center justify-center rounded-full bg-surface text-fg-2 ring-1 ring-line">
          <VolumeX size={icon.xs} />
        </span>
      )}
    </motion.button>
  );
}

export function ParticipantsBar({ members, draft, setDraft, disabled, director, onSelect, onToggleMute }: ParticipantsBarProps) {
  const { t } = useTranslation("conversations");
  const look = useChatLook();
  const wiggle = useAnimationControls();
  const nonce = director?.wiggle ?? 0;
  useEffect(() => {
    if (nonce > 0) void wiggle.start({ x: [0, -8, 8, -7, 7, -4, 4, 0], transition: { duration: 0.45, ease: "easeInOut" } });
  }, [nonce, wiggle]);
  const mentioned = useMemo(
    () => new Set(director ? [] : members.filter((member) => findMention(draft, member.name)).map((member) => member.id)),
    [members, draft, director],
  );
  const selected = director?.selectedId ? members.find((member) => member.id === director.selectedId) : undefined;
  const placement = look.participantsBarHintPosition;
  const hint =
    director && placement !== "hidden" ? (
      <div className={cn("flex items-center gap-1.5 px-1.5 text-xs text-fg-3", aligns[look.participantsBarAlign], placement === "top" ? "mb-1" : "mt-1")}>
        <Clapperboard size={icon.xs} className="text-accent" />
        <AnimatePresence mode="wait" initial={false}>
          <motion.span
            key={selected?.id ?? "none"}
            initial={{ opacity: 0, y: 3 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -3 }}
            transition={m.instant}
          >
            {selected
              ? t(director.behavior === "action" ? "director.respondingHint" : "director.selectedHint", { name: selected.name })
              : t(director.behavior === "action" ? "director.actionHint" : "director.hint")}
          </motion.span>
        </AnimatePresence>
      </div>
    ) : null;
  return (
    <div className={cn("mb-1 px-1", backgrounds[look.participantsBarBackground])}>
      {placement === "top" && hint}
      <motion.div
        animate={wiggle}
        className={cn("flex items-center overflow-x-auto px-2 py-2 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden", gaps[look.participantsBarGap], aligns[look.participantsBarAlign])}
      >
        {members.map((member) => (
          <Face
            key={member.id}
            member={member}
            mentioned={mentioned.has(member.id)}
            selected={director?.selectedId === member.id}
            dimmed={member.muted || (mentioned.size > 0 && !mentioned.has(member.id))}
            disabled={disabled}
            director={director !== null}
            onTap={() => (director ? onSelect(member) : setDraft(toggleMention(draft, member.name)))}
            onMute={() => onToggleMute(member, !member.muted)}
          />
        ))}
      </motion.div>
      {placement === "bottom" && hint}
    </div>
  );
}
