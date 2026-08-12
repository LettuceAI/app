import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { X, RefreshCw, PanelLeftOpen, PanelRightOpen } from "lucide-react";
import {
  createDefaultChatAppearanceSettings,
  mergeChatAppearance,
  type ChatAppearanceOverride,
  type ChatAppearanceSettings,
  type Character,
} from "../../../../../core/storage/schemas";
import {
  readSettings,
  updateCharacterChatAppearance,
} from "../../../../../core/storage/repo";
import { useI18n } from "../../../../../core/i18n/context";
import { cn } from "../../../../design-tokens";
import { toast } from "../../../../components/toast";
import {
  AppearanceTabBar,
  ChatAppearanceForm,
  type AppearanceKey,
  type AppearanceTab,
} from "./ChatAppearanceForm";
import {
  areOverridesEqual,
  deriveOverrideFromSettings,
  normalizeOverride,
} from "./overrideHelpers";
import type { AppearanceFieldUpdater } from "../../ChatLayout";

const MIN_DRAWER_WIDTH = 360;
const DEFAULT_DRAWER_WIDTH = 460;

// Max width is always half the current viewport, read live so it tracks window resizes.
function maxDrawerWidth(): number {
  return typeof window === "undefined" ? Infinity : window.innerWidth / 2;
}

function clampDrawerWidth(width: number): number {
  return Math.max(MIN_DRAWER_WIDTH, Math.min(width, maxDrawerWidth()));
}

interface ChatAppearanceDrawerProps {
  open: boolean;
  onClose: () => void;
  character: Character;
  onCharacterUpdate: (next: Character) => void;
  setDraftOverride: (next: ChatAppearanceOverride | null) => void;
  registerFieldUpdater?: (fn: AppearanceFieldUpdater | null) => void;
}

export function ChatAppearanceDrawer({
  open,
  onClose,
  character,
  onCharacterUpdate,
  setDraftOverride,
  registerFieldUpdater,
}: ChatAppearanceDrawerProps) {
  const { t } = useI18n();
  const [side, setSide] = useState<"left" | "right">(() => {
    if (typeof window === "undefined") return "right";
    return window.localStorage.getItem("chatAppearanceDrawer.side") === "left" ? "left" : "right";
  });
  const toggleSide = useCallback(() => {
    setSide((prev) => {
      const next = prev === "right" ? "left" : "right";
      try {
        window.localStorage.setItem("chatAppearanceDrawer.side", next);
      } catch {
        // ignore storage errors
      }
      return next;
    });
  }, []);
  const [width, setWidth] = useState<number>(() => {
    if (typeof window === "undefined") return DEFAULT_DRAWER_WIDTH;
    const stored = Number(window.localStorage.getItem("chatAppearanceDrawer.width"));
    return clampDrawerWidth(Number.isFinite(stored) && stored > 0 ? stored : DEFAULT_DRAWER_WIDTH);
  });
  const asideRef = useRef<HTMLElement>(null);
  const resizingRef = useRef(false);

  const handleResizeMove = useCallback(
    (e: PointerEvent) => {
      if (!resizingRef.current || !asideRef.current) return;
      const raw = side === "right" ? window.innerWidth - e.clientX : e.clientX;
      asideRef.current.style.width = `${clampDrawerWidth(raw)}px`;
    },
    [side],
  );

  const handleResizeEnd = useCallback(() => {
    if (!resizingRef.current) return;
    resizingRef.current = false;
    window.removeEventListener("pointermove", handleResizeMove);
    window.removeEventListener("pointerup", handleResizeEnd);
    document.body.style.userSelect = "";
    document.body.style.cursor = "";
    const measured = asideRef.current?.getBoundingClientRect().width ?? width;
    const clamped = clampDrawerWidth(measured);
    setWidth(clamped);
    try {
      window.localStorage.setItem("chatAppearanceDrawer.width", String(Math.round(clamped)));
    } catch {
      // ignore storage errors
    }
  }, [handleResizeMove, width]);

  const handleResizeStart = useCallback(
    (e: React.PointerEvent) => {
      e.preventDefault();
      resizingRef.current = true;
      document.body.style.userSelect = "none";
      document.body.style.cursor = "col-resize";
      window.addEventListener("pointermove", handleResizeMove);
      window.addEventListener("pointerup", handleResizeEnd);
    },
    [handleResizeMove, handleResizeEnd],
  );

  useEffect(() => {
    if (!open) return;
    const onWindowResize = () => setWidth((w) => clampDrawerWidth(w));
    window.addEventListener("resize", onWindowResize);
    return () => window.removeEventListener("resize", onWindowResize);
  }, [open]);

  const [globalSettings, setGlobalSettings] = useState<ChatAppearanceSettings>(
    createDefaultChatAppearanceSettings(),
  );
  const [activeTab, setActiveTab] = useState<AppearanceTab>("typography");
  const [override, setOverride] = useState<ChatAppearanceOverride>(() =>
    normalizeOverride(character.chatAppearance ?? {}),
  );
  const [initialOverride, setInitialOverride] = useState<ChatAppearanceOverride>(() =>
    normalizeOverride(character.chatAppearance ?? {}),
  );
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    void (async () => {
      try {
        const settings = await readSettings();
        const global =
          settings.advancedSettings?.chatAppearance ?? createDefaultChatAppearanceSettings();
        if (!cancelled) setGlobalSettings(global);
      } catch (err) {
        console.error("ChatAppearanceDrawer: failed to load global settings", err);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const next = normalizeOverride(character.chatAppearance ?? {});
    setOverride(next);
    setInitialOverride(next);
  }, [open, character.chatAppearance]);

  useEffect(() => {
    if (!open) {
      setDraftOverride(null);
      return;
    }
    setDraftOverride(override);
    return () => {
      setDraftOverride(null);
    };
  }, [open, override, setDraftOverride]);

  const effectiveSettings = useMemo(
    () => mergeChatAppearance(globalSettings, override),
    [globalSettings, override],
  );

  const updateField = useCallback(
    <K extends AppearanceKey>(key: K, value: ChatAppearanceSettings[K]) => {
      setOverride((prev) => ({ ...prev, [key]: value }));
    },
    [],
  );

  useEffect(() => {
    if (!registerFieldUpdater) return;
    registerFieldUpdater(open ? updateField : null);
    return () => registerFieldUpdater(null);
  }, [open, updateField, registerFieldUpdater]);

  const resetField = useCallback((key: AppearanceKey) => {
    setOverride((prev) => {
      const next = { ...prev };
      delete next[key];
      return next;
    });
  }, []);

  const isOverridden = useCallback(
    (key: AppearanceKey): boolean => key in override && override[key] !== undefined,
    [override],
  );

  const isDirty = useMemo(
    () => !areOverridesEqual(override, initialOverride),
    [override, initialOverride],
  );

  const handleDiscard = useCallback(() => {
    setOverride(initialOverride);
  }, [initialOverride]);

  const handleSave = useCallback(async () => {
    if (!isDirty || isSaving) return;
    setIsSaving(true);
    try {
      const derived = deriveOverrideFromSettings(globalSettings, effectiveSettings);
      const saved = await updateCharacterChatAppearance(
        character.id,
        Object.keys(derived).length > 0 ? derived : null,
      );
      const normalized = normalizeOverride(derived);
      onCharacterUpdate({ ...saved, chatAppearance: normalized });
      setOverride(normalized);
      setInitialOverride(normalized);
      toast.success(
        t("chatAppearance.drawer.savedTitle"),
        t("chatAppearance.drawer.savedDesc"),
      );
    } catch (err) {
      console.error("ChatAppearanceDrawer: save failed", err);
      toast.error(
        t("chatAppearance.drawer.saveFailed"),
        err instanceof Error ? err.message : String(err),
      );
    } finally {
      setIsSaving(false);
    }
  }, [isDirty, isSaving, globalSettings, effectiveSettings, character.id, onCharacterUpdate, t]);

  useEffect(() => {
    const globalWindow = window as any;
    if (!open) {
      globalWindow.__unsavedChanges = false;
      return;
    }
    globalWindow.__unsavedChanges = isDirty && !isSaving;
    return () => {
      globalWindow.__unsavedChanges = false;
    };
  }, [open, isDirty, isSaving]);

  useEffect(() => {
    if (!open) return;
    const onDiscard = () => handleDiscard();
    const onSave = () => void handleSave();
    window.addEventListener("unsaved:discard", onDiscard);
    window.addEventListener("unsaved:save", onSave);
    return () => {
      window.removeEventListener("unsaved:discard", onDiscard);
      window.removeEventListener("unsaved:save", onSave);
    };
  }, [open, handleDiscard, handleSave]);

  const handleCloseAttempt = useCallback(() => {
    if (isDirty) {
      toast.warningSticky(
        t("chatAppearance.drawer.unsavedTitle"),
        t("chatAppearance.drawer.unsavedDesc"),
        t("common.buttons.discard"),
        () => {
          handleDiscard();
          onClose();
        },
        "appearance-drawer-unsaved",
        {
          label: t("common.buttons.save"),
          onAction: () => {
            void handleSave().then(() => {
              toast.dismiss("appearance-drawer-unsaved");
              onClose();
            });
          },
        },
      );
      return;
    }
    onClose();
  }, [isDirty, handleDiscard, handleSave, onClose, t]);

  const isRight = side === "right";
  const exitX = isRight ? "100%" : "-100%";
  const overrideCount = Object.values(override).filter((v) => v !== undefined).length;

  return (
    <AnimatePresence>
      {open && (
        <motion.aside
          ref={asideRef}
          style={{ width }}
          className={cn(
            "fixed bottom-0 top-[var(--titlebar-h,0px)] z-50 flex flex-col",
            "bg-surface/96 backdrop-blur-2xl shadow-2xl",
            isRight ? "right-0 border-l border-fg/10" : "left-0 border-r border-fg/10",
          )}
          initial={{ x: exitX }}
          animate={{ x: 0 }}
          exit={{ x: exitX }}
          transition={{ type: "spring", stiffness: 320, damping: 32 }}
        >
          <div
            onPointerDown={handleResizeStart}
            role="separator"
            aria-orientation="vertical"
            aria-label={t("chatAppearance.drawer.resize")}
            className={cn(
              "group absolute inset-y-0 z-30 flex w-2 cursor-col-resize touch-none",
              isRight ? "left-0 justify-start" : "right-0 justify-end",
            )}
          >
            <span className="h-full w-px bg-transparent transition-colors group-hover:bg-accent/40" />
          </div>
          <header className="flex items-center justify-between border-b border-fg/10 px-4 py-3">
            <div>
              <div className="text-sm font-semibold text-fg">{t("chats.chatAppearance")}</div>
              <div className="text-[11px] text-fg/45">
                {t("chatAppearance.drawer.characterOnly", { name: character.name })}
              </div>
            </div>
            <div className="flex items-center gap-1">
              {overrideCount > 0 && (
                <button
                  type="button"
                  onClick={() => setOverride({})}
                  title={t("chatAppearance.drawer.clearAllOverrides")}
                  aria-label={t("chatAppearance.drawer.clearAllOverrides")}
                  className="mr-0.5 flex items-center gap-1.5 rounded-lg border border-accent/20 bg-accent/10 px-2 py-1 text-[11px] font-medium text-accent/80 transition-colors hover:bg-accent/15 hover:text-accent"
                >
                  <RefreshCw size={11} />
                  {overrideCount}
                </button>
              )}
              <button
                type="button"
                onClick={toggleSide}
                className="rounded-lg p-1.5 text-fg/50 hover:bg-fg/10 hover:text-fg"
                aria-label={
                  isRight
                    ? t("chatAppearance.drawer.moveLeft")
                    : t("chatAppearance.drawer.moveRight")
                }
              >
                {isRight ? <PanelLeftOpen size={16} /> : <PanelRightOpen size={16} />}
              </button>
              <button
                type="button"
                onClick={handleCloseAttempt}
                className="rounded-lg p-1.5 text-fg/50 hover:bg-fg/10 hover:text-fg"
                aria-label={t("chatAppearance.drawer.close")}
              >
                <X size={16} />
              </button>
            </div>
          </header>

            <div className="border-b border-fg/10 px-4 py-3">
              <AppearanceTabBar activeTab={activeTab} onChange={setActiveTab} />
            </div>

            <div className="flex-1 overflow-y-auto px-4 py-4">
              <ChatAppearanceForm
                settings={effectiveSettings}
                mode="character"
                activeTab={activeTab}
                onUpdate={updateField}
                onResetField={resetField}
                isOverridden={isOverridden}
              />
            </div>

            <footer className="flex gap-2 border-t border-fg/10 px-4 py-3">
              <button
                type="button"
                onClick={handleDiscard}
                disabled={!isDirty || isSaving}
                className={cn(
                  "flex-1 rounded-lg border px-3 py-2 text-xs font-medium transition-all",
                  "border-fg/10 bg-fg/5 text-fg/60 hover:border-fg/20 hover:bg-fg/10 hover:text-fg",
                  "disabled:opacity-40 disabled:pointer-events-none",
                )}
              >
                {t("common.buttons.discard")}
              </button>
              <button
                type="button"
                onClick={() => void handleSave()}
                disabled={!isDirty || isSaving}
                className={cn(
                  "flex-1 rounded-lg border px-3 py-2 text-xs font-semibold transition-all",
                  "border-accent/40 bg-accent/15 text-accent hover:border-accent/60 hover:bg-accent/25",
                  "disabled:opacity-40 disabled:pointer-events-none",
                )}
              >
                {isSaving ? t("common.buttons.saving") : t("common.buttons.save")}
              </button>
          </footer>
        </motion.aside>
      )}
    </AnimatePresence>
  );
}
