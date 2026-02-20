import { useMemo, useState, useEffect } from "react";
import { ArrowLeft, Brain, Loader2, AlertTriangle, Search, BookOpen } from "lucide-react";
import { useNavigate, useParams } from "react-router-dom";
import type { Character, Persona, Session } from "../../../../core/storage/schemas";
import { AvatarImage } from "../../../components/AvatarImage";
import { useAvatar } from "../../../hooks/useAvatar";
import { listen } from "@tauri-apps/api/event";
import { Routes } from "../../../navigation";
import { cn } from "../../../design-tokens";

interface ChatHeaderProps {
  character: Character;
  persona?: Persona | null;
  swapPlaces?: boolean;
  sessionId?: string;
  session?: Session | null;
  hasBackgroundImage?: boolean;
  headerOverlayClassName?: string;
  onSessionUpdate?: () => void;
}

function isImageLike(value?: string) {
  if (!value) return false;
  const lower = value.toLowerCase();
  return (
    lower.startsWith("http://") || lower.startsWith("https://") || lower.startsWith("data:image")
  );
}

export function ChatHeader({
  character,
  persona = null,
  swapPlaces = false,
  sessionId,
  session,
  hasBackgroundImage,
  headerOverlayClassName,
  onSessionUpdate,
}: ChatHeaderProps) {
  const navigate = useNavigate();
  const { characterId } = useParams<{ characterId: string }>();
  const avatarUrl = useAvatar(
    swapPlaces ? "persona" : "character",
    swapPlaces ? persona?.id : character?.id,
    swapPlaces ? persona?.avatarPath : character?.avatarPath,
    "round",
  );
  const [memoryBusy, setMemoryBusy] = useState(false);
  const [memoryError, setMemoryError] = useState<string | null>(null);
  const isDynamic = useMemo(() => character?.memoryType === "dynamic", [character?.memoryType]);

  useEffect(() => {
    if (!isDynamic) {
      setMemoryBusy(false);
      setMemoryError(null);
      return;
    }

    let unlistenProcessing: (() => void) | undefined;
    let unlistenSuccess: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;
    let disposed = false;

    const setupListeners = async () => {
      unlistenProcessing = await listen("dynamic-memory:processing", (event: any) => {
        // Check if event belongs to current session?
        // Payload might have sessionId.
        // For now, assuming global or checking payload if available.
        // User didn't specify sessionId filter strictly but it's good practice.
        // The event payload is { sessionId }.
        if (event.payload?.sessionId && sessionId && event.payload.sessionId !== sessionId) return;
        setMemoryBusy(true);
      });
      if (disposed) {
        unlistenProcessing();
        return;
      }

      unlistenSuccess = await listen("dynamic-memory:success", (event: any) => {
        if (event.payload?.sessionId && sessionId && event.payload.sessionId !== sessionId) return;
        setMemoryBusy(false);
        setMemoryError(null);
        onSessionUpdate?.();
      });
      if (disposed) {
        unlistenSuccess();
        return;
      }

      unlistenError = await listen("dynamic-memory:error", (event: any) => {
        if (event.payload?.sessionId && sessionId && event.payload.sessionId !== sessionId) return;
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
  }, [sessionId, onSessionUpdate, isDynamic]);

  const avatarImageUrl = useMemo(() => {
    if (avatarUrl && isImageLike(avatarUrl)) return avatarUrl;
    return null;
  }, [avatarUrl]);

  const initials = useMemo(() => {
    if (swapPlaces) {
      return persona?.title ? persona.title.slice(0, 2).toUpperCase() : "?";
    }
    return character?.name ? character.name.slice(0, 2).toUpperCase() : "?";
  }, [character, persona, swapPlaces]);

  const avatarFallback = (
    <div className="flex h-full w-full items-center justify-center rounded-full bg-white/10 text-xs font-semibold text-white">
      {initials}
    </div>
  );

  const headerTitle = useMemo(() => {
    if (swapPlaces) return persona?.title ?? "Unknown";
    return character?.name ?? "Unknown";
  }, [character?.name, persona?.title, swapPlaces]);

  return (
    <>
      <header
        className={cn(
          "z-20 shrink-0 border-b border-white/10 px-4 pb-3 pt-10",
          hasBackgroundImage ? headerOverlayClassName || "bg-surface/40" : "bg-surface",
        )}
      >
        <div className="flex items-center">
          <button
            onClick={() => navigate("/chat")}
            className="flex px-[0.6em] py-[0.3em] shrink-0 items-center justify-center -ml-2 text-white transition hover:text-white/80"
            aria-label="Back"
          >
            <ArrowLeft size={18} strokeWidth={2.5} />
          </button>

          <button
            onClick={() => {
              if (!characterId) return;
              navigate(Routes.chatSettingsSession(characterId, sessionId));
            }}
            className="min-w-0 flex-1 text-left truncate text-xl font-bold text-white/90 p-0 hover:opacity-80 transition-opacity"
            aria-label="Open chat settings"
          >
            {headerTitle}
          </button>

          <div className="flex shrink-0 items-center gap-1">
            {/* Memory Button */}
            {session &&
              (() => {
                const isBusy = isDynamic && (memoryBusy || session.memoryStatus === "processing");
                const isError = isDynamic && (!!memoryError || session.memoryStatus === "failed");
                const effectiveError = isDynamic ? memoryError || session.memoryError : null;

                return (
                  <button
                    onClick={() => {
                      if (!characterId || !sessionId) return;
                      navigate(
                        Routes.chatMemories(
                          characterId,
                          sessionId,
                          effectiveError ? { error: effectiveError } : undefined,
                        ),
                      );
                    }}
                    className="relative flex px-[0.6em] py-[0.3em] h-10 w-10 items-center justify-center text-white/80 transition hover:text-white"
                    aria-label="Manage memories"
                  >
                    {isBusy ? (
                      <Loader2
                        size={18}
                        strokeWidth={2.5}
                        className="animate-spin text-emerald-400"
                      />
                    ) : isError ? (
                      <AlertTriangle size={18} strokeWidth={2.5} className="text-red-400" />
                    ) : (
                      <Brain size={18} strokeWidth={2.5} />
                    )}
                    {!isBusy && !isError && session.memories && session.memories.length > 0 && (
                      <span className="absolute right-1 top-1 flex h-3.5 w-3.5 items-center justify-center rounded-full bg-emerald-500 text-[3px] font-semibold leading-none text-[#050505]">
                        {session.memories.length}
                      </span>
                    )}
                  </button>
                );
              })()}

            {/* Search Button */}
            {session && (
              <button
                onClick={() => {
                  if (!characterId || !sessionId) return;
                  navigate(Routes.chatSearch(characterId, sessionId));
                }}
                className="flex items-center px-[0.6em] py-[0.3em] justify-center text-white/80 transition hover:text-white"
                aria-label="Search messages"
              >
                <Search size={18} strokeWidth={2.5} />
              </button>
            )}

            {/* Lorebooks Button */}
            <button
              onClick={() => {
                if (!characterId) return;
                navigate(Routes.characterLorebook(characterId));
              }}
              className="flex items-center px-[0.6em] py-[0.3em] justify-center text-white/80 transition hover:text-white"
              aria-label="Manage lorebooks"
            >
              <BookOpen size={18} strokeWidth={2.5} />
            </button>

            {/* Avatar (Settings) Button */}
            <button
              onClick={() => {
                if (!characterId) return;
                navigate(Routes.chatSettingsSession(characterId, sessionId));
              }}
              className="relative shrink-0 rounded-full overflow-hidden ring-1 ring-white/20 transition hover:ring-white/40"
              style={{
                width: "36px",
                height: "36px",
                minWidth: "36px",
                minHeight: "36px",
                flexShrink: 0,
              }}
              aria-label="Conversation settings"
            >
              {avatarImageUrl ? (
                <AvatarImage
                  src={avatarImageUrl}
                  alt={swapPlaces ? persona?.title || "Avatar" : character?.name || "Avatar"}
                  crop={swapPlaces ? persona?.avatarCrop : character?.avatarCrop}
                  applyCrop
                  className="absolute inset-0 z-10"
                />
              ) : (
                avatarFallback
              )}
            </button>
          </div>
        </div>
      </header>
    </>
  );
}
