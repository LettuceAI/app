import { useEffect, useState } from "react";
import {
  ArrowLeft,
  Brain,
  Loader2,
  AlertTriangle,
  BookOpen,
  Palette,
  Search,
  LayoutGrid,
} from "lucide-react";
import { listen } from "@tauri-apps/api/event";

import { useI18n } from "../../../../core/i18n/context";
import type { GroupSession, Character } from "../../../../core/storage/schemas";
import { AvatarImage } from "../../../components/AvatarImage";
import { cn } from "../../../design-tokens";
import { useAvatar } from "../../../hooks/useAvatar";
import { isRenderableImageUrl } from "../../../../core/utils/image";

export function GroupChatHeader({
  session,
  characters,
  onBack,
  onSettings,
  onMemories,
  onLorebooks,
  onAppearance,
  onSearch,
  onEditWidgets,
  hasBackgroundImage,
  headerOverlayClassName,
  transparentHeader = false,
}: {
  session: GroupSession;
  characters: Character[];
  onBack: () => void;
  onSettings: () => void;
  onMemories: () => void;
  onLorebooks: () => void;
  onAppearance?: () => void;
  onSearch?: () => void;
  onEditWidgets?: () => void;
  hasBackgroundImage?: boolean;
  headerOverlayClassName?: string;
  transparentHeader?: boolean;
}) {
  const { t } = useI18n();
  const [memoryBusy, setMemoryBusy] = useState(false);
  const [memoryError, setMemoryError] = useState<string | null>(null);

  useEffect(() => {
    let unlistenProcessing: (() => void) | undefined;
    let unlistenSuccess: (() => void) | undefined;
    let unlistenCancelled: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;
    let disposed = false;

    const setupListeners = async () => {
      unlistenProcessing = await listen("group-dynamic-memory:processing", (event: any) => {
        if (event.payload?.sessionId && event.payload.sessionId !== session.id) return;
        setMemoryBusy(true);
      });
      if (disposed) {
        unlistenProcessing();
        return;
      }

      unlistenSuccess = await listen("group-dynamic-memory:success", (event: any) => {
        if (event.payload?.sessionId && event.payload.sessionId !== session.id) return;
        setMemoryBusy(false);
        setMemoryError(null);
      });
      if (disposed) {
        unlistenSuccess();
        return;
      }

      unlistenCancelled = await listen("group-dynamic-memory:cancelled", (event: any) => {
        if (event.payload?.sessionId && event.payload.sessionId !== session.id) return;
        setMemoryBusy(false);
        setMemoryError(null);
      });
      if (disposed) {
        unlistenCancelled();
        return;
      }

      unlistenError = await listen("group-dynamic-memory:error", (event: any) => {
        if (event.payload?.sessionId && event.payload.sessionId !== session.id) return;
        setMemoryBusy(false);
        setMemoryError(
          typeof event.payload === "string"
            ? event.payload
            : event.payload?.error || "Unknown error",
        );
      });
      if (disposed) {
        unlistenError();
      }
    };

    void setupListeners();

    return () => {
      disposed = true;
      unlistenProcessing?.();
      unlistenSuccess?.();
      unlistenCancelled?.();
      unlistenError?.();
    };
  }, [session.id]);

  const memoryCount = session.memories?.length ?? 0;
  const effectiveMemoryBusy = memoryBusy || session.memoryStatus === "processing";
  const effectiveMemoryError = memoryError || session.memoryError || null;

  return (
    <header
      className={cn(
        "z-20 shrink-0 border-b border-fg/10 px-3 lg:px-8",
        hasBackgroundImage
          ? transparentHeader
            ? "bg-transparent"
            : headerOverlayClassName || "bg-surface/40"
          : "bg-surface",
      )}
      style={{
        paddingTop: "calc(env(safe-area-inset-top) + 12px)",
        paddingBottom: "12px",
      }}
    >
      <div className="flex items-center h-10">
        <button
          onClick={onBack}
          className="flex px-[0.6em] py-[0.3em] shrink-0 items-center justify-center -ml-2 text-fg transition hover:text-fg/80"
          aria-label={t("groupChats.header.back")}
        >
          <ArrowLeft size={18} strokeWidth={2.5} />
        </button>

        <button
          onClick={onSettings}
          data-tour-id="group-chat-title"
          className="min-w-0 flex-1 truncate text-left text-xl font-bold text-fg/90 transition-opacity hover:opacity-80"
          aria-label={t("groupChats.header.settings")}
        >
          <span className="block truncate">{session.name}</span>
        </button>

        <div className="flex shrink-0 items-center gap-1.5">
          {/* Memory Button */}
          <button
            onClick={onMemories}
            data-tour-id="group-chat-memory"
            className="relative flex h-10 w-10 items-center justify-center px-[0.6em] py-[0.3em] text-fg/75 transition hover:text-fg"
            aria-label={t("groupChats.header.memories")}
          >
            {effectiveMemoryBusy ? (
              <Loader2 size={18} strokeWidth={2.5} className="animate-spin text-emerald-400" />
            ) : effectiveMemoryError ? (
              <AlertTriangle size={18} strokeWidth={2.5} className="text-red-400" />
            ) : (
              <Brain size={18} strokeWidth={2.5} />
            )}
            {!effectiveMemoryBusy && !effectiveMemoryError && memoryCount > 0 && (
              <span className="absolute right-0.5 top-0.5 inline-flex min-w-4 h-4 items-center justify-center rounded-full bg-emerald-500 px-1 text-[10px] font-bold leading-none text-white shadow-md ring-1 ring-emerald-200/40">
                {memoryCount > 99 ? "99+" : memoryCount}
              </span>
            )}
          </button>

          {onSearch && (
            <button
              onClick={onSearch}
              className="flex items-center justify-center px-[0.6em] py-[0.3em] text-fg/75 transition hover:text-fg"
              aria-label={t("chats.search.placeholder")}
            >
              <Search size={18} strokeWidth={2.5} />
            </button>
          )}

          <button
            onClick={onLorebooks}
            className="flex items-center justify-center px-[0.6em] py-[0.3em] text-fg/75 transition hover:text-fg"
            aria-label={t("chats.header.manageLorebooks")}
          >
            <BookOpen size={18} strokeWidth={2.5} />
          </button>

          {onEditWidgets && (
            <button
              onClick={onEditWidgets}
              className="hidden items-center justify-center px-[0.6em] py-[0.3em] text-fg/75 transition hover:text-fg lg:flex"
              aria-label={t("groupChats.header.editWidgets")}
            >
              <LayoutGrid size={18} strokeWidth={2.5} />
            </button>
          )}

          {onAppearance && (
            <button
              onClick={onAppearance}
              className="flex items-center justify-center px-[0.6em] py-[0.3em] text-fg/75 transition hover:text-fg"
              aria-label={t("groupChats.header.customizeAppearance")}
            >
              <Palette size={18} strokeWidth={2.5} />
            </button>
          )}

          {/* Stacked character avatars */}
          <button
            onClick={onSettings}
            data-tour-id="group-chat-participants"
            className="ml-1 flex shrink-0 items-center -space-x-2.5 transition hover:opacity-80 active:scale-95"
            aria-label={t("groupChats.header.settings")}
          >
            {characters.slice(0, 3).map((char, index) => (
              <CharacterMiniAvatar
                key={char.id}
                character={char}
                index={index}
                total={Math.min(characters.length, 3)}
              />
            ))}
            {characters.length > 3 && (
              <div className="relative z-0 flex h-8 w-8 items-center justify-center rounded-full bg-surface-el text-[10px] font-bold text-fg/70 ring-2 ring-surface">
                +{characters.length - 3}
              </div>
            )}
          </button>

        </div>
      </div>
    </header>
  );
}

function isImageLike(s?: string) {
  return isRenderableImageUrl(s);
}

function CharacterMiniAvatar({
  character,
  index,
  total,
}: {
  character: Character;
  index: number;
  total: number;
}) {
  const avatarUrl = useAvatar("character", character.id, character.avatarPath, "round");

  return (
    <div
      className={cn(
        "relative h-8 w-8 overflow-hidden rounded-full",
        "bg-linear-to-br from-fg/8 to-fg/4",
        "ring-2 ring-surface",
      )}
      style={{ zIndex: total - index }}
    >
      {avatarUrl && isImageLike(avatarUrl) ? (
        <AvatarImage src={avatarUrl} alt={character.name} crop={character.avatarCrop} applyCrop />
      ) : (
        <div className="flex h-full w-full items-center justify-center text-[10px] font-bold text-fg/60">
          {character.name.slice(0, 2).toUpperCase()}
        </div>
      )}
    </div>
  );
}
