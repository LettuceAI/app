import { useEffect, useState } from "react";
import { ArrowLeft, Brain, Loader2, AlertTriangle, BookOpen } from "lucide-react";
import { listen } from "@tauri-apps/api/event";

import { useI18n } from "../../../../core/i18n/context";
import type { GroupSession, Character } from "../../../../core/storage/schemas";
import { AvatarImage } from "../../../components/AvatarImage";
import { cn } from "../../../design-tokens";
import { useDragRegionProps } from "../../../components/App/TopNav";
import { useAvatar } from "../../../hooks/useAvatar";
import { isRenderableImageUrl } from "../../../../core/utils/image";

export function GroupChatHeader({
  session,
  characters,
  onBack,
  onSettings,
  onMemories,
  onLorebooks,
  hasBackgroundImage,
  headerOverlayClassName,
}: {
  session: GroupSession;
  characters: Character[];
  onBack: () => void;
  onSettings: () => void;
  onMemories: () => void;
  onLorebooks: () => void;
  hasBackgroundImage?: boolean;
  headerOverlayClassName?: string;
}) {
  const { t } = useI18n();
  const dragRegionProps = useDragRegionProps();
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
        hasBackgroundImage ? headerOverlayClassName || "bg-surface/40" : "bg-surface",
      )}
      style={{
        paddingTop: "calc(env(safe-area-inset-top) + 12px)",
        paddingBottom: "12px",
      }}
      {...dragRegionProps}
    >
      <div className="flex items-center h-10" {...dragRegionProps}>
        <button
          onClick={onBack}
          className="flex px-[0.6em] py-[0.3em] shrink-0 items-center justify-center -ml-2 text-fg transition hover:text-fg/80"
          aria-label={t("groupChats.header.back")}
        >
          <ArrowLeft size={18} strokeWidth={2.5} />
        </button>

        <button
          onClick={onSettings}
          className="min-w-0 flex-1 truncate text-left text-xl font-bold text-fg/90 transition-opacity hover:opacity-80"
          aria-label={t("groupChats.header.settings")}
        >
          <span className="block truncate">{session.name}</span>
        </button>

        <div className="flex shrink-0 items-center gap-1.5">
          {/* Memory Button */}
          <button
            onClick={onMemories}
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

          <button
            onClick={onLorebooks}
            className="flex items-center justify-center px-[0.6em] py-[0.3em] text-fg/75 transition hover:text-fg"
            aria-label={t("chats.header.manageLorebooks")}
          >
            <BookOpen size={18} strokeWidth={2.5} />
          </button>

          {/* Stacked character avatars */}
          <button
            onClick={onSettings}
            className="relative shrink-0 overflow-hidden rounded-full ring-1 ring-fg/15 transition hover:ring-fg/25"
            style={{
              minWidth: "36px",
              minHeight: "36px",
              height: "36px",
              paddingInline: characters.length > 1 ? "6px" : "0px",
            }}
            aria-label={t("groupChats.header.settings")}
          >
            <div className="flex h-full items-center justify-center -space-x-2 px-1">
              {characters.slice(0, 3).map((char, index) => (
                <CharacterMiniAvatar
                  key={char.id}
                  character={char}
                  index={index}
                  total={Math.min(characters.length, 3)}
                />
              ))}
              {characters.length > 3 && (
                <div
                  className={cn(
                    "flex h-8 w-8 items-center justify-center rounded-full",
                    "bg-linear-to-br from-secondary/30 to-info/80/30",
                    "text-[10px] font-semibold text-fg shadow-lg",
                    "ring-1 ring-fg/15",
                  )}
                  style={{ marginLeft: "-8px", zIndex: 0 }}
                >
                  +{characters.length - 3}
                </div>
              )}
            </div>
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
        "h-8 w-8 rounded-full overflow-hidden",
        "bg-linear-to-br from-fg/8 to-fg/4",
        "shadow-lg ring-1 ring-fg/15",
        "transition-transform",
      )}
      style={{
        marginLeft: index > 0 ? "-10px" : "0",
        zIndex: total - index,
      }}
    >
      {avatarUrl && isImageLike(avatarUrl) ? (
        <AvatarImage src={avatarUrl} alt={character.name} crop={character.avatarCrop} applyCrop />
      ) : (
        <div className="flex h-full w-full items-center justify-center bg-linear-to-br from-secondary/40 to-info/80/40 text-[11px] font-bold text-fg">
          {character.name.slice(0, 1).toUpperCase()}
        </div>
      )}
    </div>
  );
}
