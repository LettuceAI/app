import { useMemo, useState, useEffect } from "react";
import { ArrowLeft, Brain, Loader2, AlertTriangle, Search, BookOpen } from "lucide-react";
import { useNavigate, useParams } from "react-router-dom";
import type { Character, Persona, Session } from "../../../../core/storage/schemas";
import { AvatarImage } from "../../../components/AvatarImage";
import { useAvatar } from "../../../hooks/useAvatar";
import { listen } from "@tauri-apps/api/event";
import { Routes } from "../../../navigation";
import { cn } from "../../../design-tokens";
import {
  WindowControlButtons,
  useDragRegionProps,
  hasCustomWindowControls,
} from "../../../components/App/TopNav";
import { useI18n } from "../../../../core/i18n/context";
import { isRenderableImageUrl } from "../../../../core/utils/image";

interface ChatHeaderProps {
  character: Character;
  persona?: Persona | null;
  swapPlaces?: boolean;
  sessionId?: string;
  session?: Session | null;
  hasBackgroundImage?: boolean;
  headerOverlayClassName?: string;
  onSessionUpdate?: () => void;
  onBeforeSettingsOpen?: () => void;
  onSettingsOpen?: () => void;
}

function isImageLike(value?: string) {
  return isRenderableImageUrl(value);
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
  onBeforeSettingsOpen,
  onSettingsOpen,
}: ChatHeaderProps) {
  const navigate = useNavigate();
  const { characterId } = useParams<{ characterId: string }>();
  const { t } = useI18n();
  const avatarUrl = useAvatar(
    swapPlaces ? "persona" : "character",
    swapPlaces ? persona?.id : character?.id,
    swapPlaces ? persona?.avatarPath : character?.avatarPath,
    "round",
  );
  const dragRegionProps = useDragRegionProps();
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
    let unlistenCancelled: (() => void) | undefined;
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

      unlistenCancelled = await listen("dynamic-memory:cancelled", (event: any) => {
        if (event.payload?.sessionId && sessionId && event.payload.sessionId !== sessionId) return;
        setMemoryBusy(false);
        setMemoryError(null);
        onSessionUpdate?.();
      });
      if (disposed) {
        unlistenCancelled();
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
      unlistenCancelled?.();
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
    <div className="flex h-full w-full items-center justify-center rounded-full bg-fg/10 text-xs font-semibold text-fg">
      {initials}
    </div>
  );

  const headerTitle = useMemo(() => {
    if (swapPlaces) {
      if (!persona) return "Unknown";
      return persona.nickname ? `${persona.title} (${persona.nickname})` : persona.title;
    }
    return character?.name ?? "Unknown";
  }, [character?.name, persona, swapPlaces]);

  return (
    <>
      <header
        className={cn(
          "z-20 shrink-0 border-b border-fg/10 pl-3 lg:pl-8",
          hasCustomWindowControls ? "pr-0" : "pr-3 lg:pr-8",
          hasBackgroundImage ? headerOverlayClassName || "bg-surface/40" : "bg-surface",
        )}
        style={{
          paddingTop: "calc(env(safe-area-inset-top) + 12px)",
          paddingBottom: "12px",
        }}
        {...dragRegionProps}
      >
        <div className="flex items-center justify-between h-10" {...dragRegionProps}>
          <div className="flex items-center min-w-0">
            <button
              onClick={() => navigate("/chat")}
              className="flex shrink-0 items-center justify-center -ml-2 px-[0.6em] py-[0.3em] text-fg transition hover:text-fg/80"
              aria-label={t("chats.header.back")}
            >
              <ArrowLeft size={18} strokeWidth={2.5} />
            </button>

            <button
              data-tour-id="chat-title"
              onPointerDown={() => onBeforeSettingsOpen?.()}
              onClick={() => {
                if (onSettingsOpen) {
                  onSettingsOpen();
                  return;
                }
                if (!characterId) return;
                navigate(Routes.chatSettingsSession(characterId, sessionId));
              }}
              className="min-w-0 shrink truncate p-0 text-left text-xl font-bold text-fg/90 transition-opacity hover:opacity-80"
              aria-label={t("chats.header.openSettings")}
            >
              {headerTitle}
            </button>
          </div>

          <div className="flex shrink-0 items-center gap-1.5">
            {/* Memory Button */}
            {session &&
              (() => {
                const isBusy = isDynamic && (memoryBusy || session.memoryStatus === "processing");
                const isError = isDynamic && (!!memoryError || session.memoryStatus === "failed");
                const effectiveError = isDynamic ? memoryError || session.memoryError : null;

                return (
                  <button
                    data-tour-id="chat-memory"
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
                    className="relative flex h-10 w-10 items-center justify-center px-[0.6em] py-[0.3em] text-fg/80 transition hover:text-fg"
                    aria-label={t("chats.header.manageMemories")}
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
                      <span className="absolute right-0.5 top-0.5 inline-flex h-4 min-w-4 items-center justify-center rounded-full bg-emerald-500 px-1 text-[10px] font-bold leading-none text-white shadow-md ring-1 ring-emerald-200/40">
                        {session.memories.length > 99 ? "99+" : session.memories.length}
                      </span>
                    )}
                  </button>
                );
              })()}

            {/* Search Button */}
            {session && (
              <button
                data-tour-id="chat-search"
                onClick={() => {
                  if (!characterId || !sessionId) return;
                  navigate(Routes.chatSearch(characterId, sessionId));
                }}
                className="flex items-center justify-center px-[0.6em] py-[0.3em] text-fg/80 transition hover:text-fg"
                aria-label={t("chats.header.searchMessages")}
              >
                <Search size={18} strokeWidth={2.5} />
              </button>
            )}

            {/* Lorebooks Button */}
            <button
              data-tour-id="chat-lorebook"
              onClick={() => {
                if (!characterId) return;
                navigate(Routes.characterLorebook(characterId));
              }}
              className="flex items-center justify-center px-[0.6em] py-[0.3em] text-fg/80 transition hover:text-fg"
              aria-label={t("chats.header.manageLorebooks")}
            >
              <BookOpen size={18} strokeWidth={2.5} />
            </button>

            {/* Avatar (Settings) Button */}
            <button
              onPointerDown={() => onBeforeSettingsOpen?.()}
              onClick={() => {
                if (onSettingsOpen) {
                  onSettingsOpen();
                  return;
                }
                if (!characterId) return;
                navigate(Routes.chatSettingsSession(characterId, sessionId));
              }}
              className="relative shrink-0 overflow-hidden rounded-full ring-1 ring-fg/20 transition hover:ring-fg/40"
              style={{
                width: "36px",
                height: "36px",
                minWidth: "36px",
                minHeight: "36px",
                flexShrink: 0,
              }}
              aria-label={t("chats.header.conversationSettings")}
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
            <WindowControlButtons />
          </div>
        </div>
      </header>
    </>
  );
}
