import { useCallback, useEffect, useMemo, useRef } from "react";
import { motion, AnimatePresence, useAnimationControls } from "framer-motion";
import { VolumeX, Clapperboard } from "lucide-react";
import { useI18n } from "../../../../core/i18n/context";
import type { Character } from "../../../../core/storage/schemas";
import { cn } from "../../../design-tokens";
import { useAvatar } from "../../../hooks/useAvatar";
import { AvatarImage } from "../../../components/AvatarImage";

const LONG_PRESS_MS = 450;

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

interface MentionMatch {
  index: number;
  length: number;
}

function findMention(draft: string, name: string): MentionMatch | null {
  const escaped = escapeRegExp(name);

  const quoted = new RegExp(`@"${escaped}"`, "i").exec(draft);
  if (quoted) return { index: quoted.index, length: quoted[0].length };

  const unquoted = new RegExp(`(^|\\s)@${escaped}(?=$|\\s|[.,!?;:])`, "i").exec(draft);
  if (unquoted) {
    const at = unquoted.index + unquoted[1].length;
    return { index: at, length: name.length + 1 };
  }

  return null;
}

function mentionToken(name: string): string {
  return name.includes(" ") ? `@"${name}"` : `@${name}`;
}

function toggleMention(draft: string, name: string): string {
  const match = findMention(draft, name);
  if (match) {
    const before = draft.slice(0, match.index);
    const after = draft.slice(match.index + match.length);
    return `${before}${after}`.replace(/\s{2,}/g, " ").replace(/^\s+/, "");
  }
  const trimmed = draft.replace(/\s+$/, "");
  return trimmed.length ? `${trimmed} ${mentionToken(name)} ` : `${mentionToken(name)} `;
}

export type ParticipantsBarSize = "small" | "medium" | "large";
export type ParticipantsBarGap = "tight" | "normal" | "relaxed";
export type ParticipantsBarAlign = "left" | "center" | "right";
export type ParticipantsBarShape = "round" | "boxed" | "rounded_box";
export type ParticipantsBarBackground = "solid" | "fading" | "transparent";

const SIZE_CLASS: Record<ParticipantsBarSize, string> = {
  small: "h-10 w-10",
  medium: "h-12 w-12",
  large: "h-14 w-14",
};

const SHAPE_CLASS: Record<ParticipantsBarShape, string> = {
  round: "rounded-full",
  boxed: "rounded-none",
  rounded_box: "rounded-xl",
};

const GAP_CLASS: Record<ParticipantsBarGap, string> = {
  tight: "gap-1",
  normal: "gap-2",
  relaxed: "gap-3",
};

const ALIGN_CLASS: Record<ParticipantsBarAlign, string> = {
  left: "justify-start",
  center: "justify-center",
  right: "justify-end",
};

interface GroupChatParticipantsBarProps {
  characters: Character[];
  draft: string;
  setDraft: (value: string) => void;
  mutedCharacterIds: Set<string>;
  onToggleMute: (characterId: string, muted: boolean) => void;
  disabled?: boolean;
  size?: ParticipantsBarSize;
  shape?: ParticipantsBarShape;
  gap?: ParticipantsBarGap;
  align?: ParticipantsBarAlign;
  directorMode?: boolean;
  directorBehavior?: "cue" | "action";
  selectedId?: string | null;
  wiggleNonce?: number;
  hintPosition?: "top" | "bottom" | "hidden";
  onSelectSpeaker?: (characterId: string) => void;
}

export function GroupChatParticipantsBar({
  characters,
  draft,
  setDraft,
  mutedCharacterIds,
  onToggleMute,
  disabled = false,
  size = "medium",
  shape = "round",
  gap = "normal",
  align = "left",
  directorMode = false,
  directorBehavior = "cue",
  selectedId = null,
  wiggleNonce = 0,
  hintPosition = "bottom",
  onSelectSpeaker,
}: GroupChatParticipantsBarProps) {
  const { t } = useI18n();
  const wiggleControls = useAnimationControls();

  useEffect(() => {
    if (wiggleNonce > 0) {
      void wiggleControls.start({
        x: [0, -8, 8, -7, 7, -4, 4, 0],
        transition: { duration: 0.45, ease: "easeInOut" },
      });
    }
  }, [wiggleNonce, wiggleControls]);
  const mentionedIds = useMemo(() => {
    if (directorMode) return new Set<string>();
    const set = new Set<string>();
    for (const character of characters) {
      if (findMention(draft, character.name)) set.add(character.id);
    }
    return set;
  }, [characters, draft, directorMode]);

  const hasActiveMention = mentionedIds.size > 0;
  const selectedName = selectedId
    ? characters.find((c) => c.id === selectedId)?.name
    : undefined;

  if (characters.length < 2) return null;

  const hint =
    directorMode && hintPosition !== "hidden" ? (
      <div
        className={cn(
          "flex items-center gap-1.5 px-1.5 text-[11px] text-fg/55 drop-shadow-[0_1px_2px_rgba(0,0,0,0.6)]",
          ALIGN_CLASS[align],
          hintPosition === "top" ? "mb-1" : "mt-1",
        )}
      >
        <Clapperboard size={12} className="text-accent/80" />
        <AnimatePresence mode="wait" initial={false}>
          <motion.span
            key={selectedName ?? "none"}
            initial={{ opacity: 0, y: 3 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -3 }}
            transition={{ duration: 0.16 }}
          >
            {selectedName
              ? directorBehavior === "action"
                ? t("groupChats.footer.directorActionRespondingHint", { name: selectedName })
                : t("groupChats.footer.directorSelectedHint", { name: selectedName })
              : directorBehavior === "action"
                ? t("groupChats.footer.directorActionHint")
                : t("groupChats.footer.directorHint")}
          </motion.span>
        </AnimatePresence>
      </div>
    ) : null;

  return (
    <div className="mb-1">
      {hintPosition === "top" && hint}
      <motion.div
        animate={wiggleControls}
        className={cn(
          "flex items-center overflow-x-auto px-1 py-1.5 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
          GAP_CLASS[gap],
          ALIGN_CLASS[align],
        )}
      >
        {characters.map((character) => {
            const isSelected = directorMode && selectedId === character.id;
            return (
              <ParticipantAvatar
                key={character.id}
                character={character}
                muted={mutedCharacterIds.has(character.id)}
                mentioned={mentionedIds.has(character.id)}
                selected={isSelected}
                dimmed={
                  mutedCharacterIds.has(character.id) ||
                  (hasActiveMention && !mentionedIds.has(character.id))
                }
                disabled={disabled}
                sizeClass={SIZE_CLASS[size]}
                shapeClass={SHAPE_CLASS[shape]}
                directorMode={directorMode}
                onMention={() =>
                  directorMode
                    ? onSelectSpeaker?.(character.id)
                    : setDraft(toggleMention(draft, character.name))
                }
                onToggleMute={(muted) => onToggleMute(character.id, muted)}
              />
            );
          })}
      </motion.div>
      {hintPosition === "bottom" && hint}
    </div>
  );
}

function ParticipantAvatar({
  character,
  muted,
  mentioned,
  selected,
  dimmed,
  disabled,
  sizeClass,
  shapeClass,
  directorMode,
  onMention,
  onToggleMute,
}: {
  character: Character;
  muted: boolean;
  mentioned: boolean;
  selected: boolean;
  dimmed: boolean;
  disabled: boolean;
  sizeClass: string;
  shapeClass: string;
  directorMode: boolean;
  onMention: () => void;
  onToggleMute: (muted: boolean) => void;
}) {
  const { t } = useI18n();
  const avatarUrl = useAvatar("character", character.id, character.avatarPath, "round");
  const longPressTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const longPressFired = useRef(false);

  const clearTimer = useCallback(() => {
    if (longPressTimer.current) {
      clearTimeout(longPressTimer.current);
      longPressTimer.current = null;
    }
  }, []);

  const handlePointerDown = useCallback(() => {
    if (disabled) return;
    longPressFired.current = false;
    longPressTimer.current = setTimeout(() => {
      longPressFired.current = true;
      onToggleMute(!muted);
    }, LONG_PRESS_MS);
  }, [disabled, muted, onToggleMute]);

  const handleClick = useCallback(() => {
    clearTimer();
    if (longPressFired.current) {
      longPressFired.current = false;
      return;
    }
    if (!disabled) onMention();
  }, [clearTimer, disabled, onMention]);

  const label = directorMode
    ? t("groupChats.footer.directorSpeak", { name: character.name })
    : muted
      ? t("groupChats.footer.participantMuted", { name: character.name })
      : mentioned
        ? t("groupChats.footer.removeMention", { name: character.name })
        : t("groupChats.footer.mentionParticipant", { name: character.name });

  return (
    <motion.button
      type="button"
      onPointerDown={handlePointerDown}
      onPointerUp={clearTimer}
      onPointerLeave={clearTimer}
      onPointerCancel={clearTimer}
      onContextMenu={(event) => {
        event.preventDefault();
        clearTimer();
        if (!disabled) onToggleMute(!muted);
      }}
      onClick={handleClick}
      disabled={disabled}
      title={label}
      aria-label={label}
      animate={{ scale: selected ? 1.08 : 1 }}
      whileTap={disabled ? undefined : { scale: 0.95 }}
      transition={{ type: "spring", stiffness: 420, damping: 26 }}
      className={cn("relative shrink-0 select-none", sizeClass, "disabled:cursor-not-allowed")}
    >
      <div
        className={cn(
          "h-full w-full overflow-hidden bg-transparent",
          shapeClass,
          "ring-2 transition-all",
          selected
            ? "ring-accent shadow-[0_0_10px_-1px_var(--color-accent)]"
            : mentioned
              ? "ring-accent"
              : directorMode
                ? "ring-accent/25"
                : "ring-fg/10",
          dimmed ? "opacity-40 grayscale" : "opacity-100",
        )}
      >
        {avatarUrl ? (
          <AvatarImage src={avatarUrl} alt={character.name} crop={character.avatarCrop} applyCrop />
        ) : (
          <div className="flex h-full w-full items-center justify-center text-sm font-bold text-fg/60">
            {character.name.slice(0, 1).toUpperCase()}
          </div>
        )}
      </div>
      {muted && (
        <span className="absolute -bottom-0.5 -right-0.5 z-10 flex h-5 w-5 items-center justify-center rounded-full bg-surface text-fg/70 ring-1 ring-fg/15">
          <VolumeX size={11} />
        </span>
      )}
    </motion.button>
  );
}
