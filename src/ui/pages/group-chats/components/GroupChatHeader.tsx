import { useEffect, useState } from "react";
import { ArrowLeft, Brain, Loader2, AlertTriangle, Settings } from "lucide-react";
import { listen } from "@tauri-apps/api/event";

import { useI18n } from "../../../../core/i18n/context";
import type { GroupSession, Character } from "../../../../core/storage/schemas";
import { AvatarImage } from "../../../components/AvatarImage";
import { cn } from "../../../design-tokens";
import { useAvatar } from "../../../hooks/useAvatar";

export function GroupChatHeader({
  session,
  characters,
  onBack,
  onSettings,
  onMemories,
  hasBackgroundImage,
  headerOverlayClassName,
}: {
  session: GroupSession;
  characters: Character[];
  onBack: () => void;
  onSettings: () => void;
  onMemories: () => void;
  hasBackgroundImage?: boolean;
  headerOverlayClassName?: string;
}) {
  const { t } = useI18n();
  const [memoryBusy, setMemoryBusy] = useState(false);
  const [memoryError, setMemoryError] = useState<string | null>(null);

  useEffect(() => {
    let unlistenProcessing: (() => void) | undefined;
    let unlistenSuccess: (() => void) | undefined;
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
      unlistenError?.();
    };
  }, [session.id]);

  const memoryCount = session.memories?.length ?? 0;

  return (
    <header
      className={cn(
        "z-20 shrink-0 border-b border-white/10 px-3 lg:px-8",
        hasBackgroundImage ? headerOverlayClassName || "bg-surface/40" : "bg-surface",
      )}
      style={{
        paddingTop: "calc(env(safe-area-inset-top))",
        paddingBottom: "12px",
      }}
    >
      <div className="flex items-center h-10">
        <button
          onClick={onBack}
          className="flex px-[0.6em] py-[0.3em] shrink-0 items-center justify-center -ml-2 text-white transition hover:text-white/80"
          aria-label={t("groupChats.header.back")}
        >
          <ArrowLeft size={18} strokeWidth={2.5} />
        </button>

        <button
          onClick={onSettings}
          className="min-w-0 flex-1 text-left truncate text-xl font-bold text-white/90 p-0 hover:opacity-80 transition-opacity"
          aria-label={t("groupChats.header.settings")}
        >
          {session.name}
        </button>

        <div className="flex shrink-0 items-center gap-1.5">
          {/* Memory Button */}
          <button
            onClick={onMemories}
            className="relative flex px-[0.6em] py-[0.3em] h-10 w-10 items-center justify-center text-white/80 transition hover:text-white"
            aria-label={t("groupChats.header.memories")}
          >
            {memoryBusy ? (
              <Loader2
                size={18}
                strokeWidth={2.5}
                className="animate-spin text-emerald-400"
              />
            ) : memoryError ? (
              <AlertTriangle size={18} strokeWidth={2.5} className="text-red-400" />
            ) : (
              <Brain size={18} strokeWidth={2.5} />
            )}
            {!memoryBusy && !memoryError && memoryCount > 0 && (
              <span className="absolute right-0.5 top-0.5 inline-flex min-w-[1rem] h-4 items-center justify-center rounded-full bg-emerald-500 px-1 text-[10px] font-bold leading-none text-white shadow-md ring-1 ring-emerald-200/40">
                {memoryCount > 99 ? "99+" : memoryCount}
              </span>
            )}
          </button>

          {/* Settings Button */}
          <button
            onClick={onSettings}
            className="flex items-center px-[0.6em] py-[0.3em] justify-center text-white/80 transition hover:text-white"
            aria-label={t("groupChats.header.settings")}
          >
            <Settings size={18} strokeWidth={2.5} />
          </button>

          {/* Stacked character avatars */}
          <button
            onClick={onSettings}
            className="relative shrink-0 flex items-center"
            aria-label={t("groupChats.header.settings")}
          >
            <div className="flex -space-x-2">
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
                    "h-8 w-8 rounded-full",
                    "bg-linear-to-br from-secondary/30 to-info/80/30",
                    "flex items-center justify-center",
                    "text-[10px] font-semibold text-fg shadow-lg",
                    "ring-1 ring-white/20",
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
  if (!s) return false;
  const lower = s.toLowerCase();
  return (
    lower.startsWith("http://") || lower.startsWith("https://") || lower.startsWith("data:image")
  );
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
        "bg-linear-to-br from-white/10 to-white/5",
        "shadow-lg ring-1 ring-white/20",
        "transition-transform hover:scale-110 hover:z-50",
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
