import { useCallback, useEffect, useMemo, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { X, RefreshCw, PanelLeftOpen, PanelRightOpen } from "lucide-react";
import {
  createDefaultChatAppearanceSettings,
  mergeChatAppearance,
  type ChatAppearanceOverride,
  type ChatAppearanceSettings,
  type Group,
} from "../../../../../core/storage/schemas";
import { readSettings, updateGroupChatAppearance } from "../../../../../core/storage/repo";
import { useI18n } from "../../../../../core/i18n/context";
import { cn } from "../../../../design-tokens";
import { toast } from "../../../../components/toast";
import {
  AppearanceTabBar,
  ChatAppearanceForm,
  type AppearanceKey,
  type AppearanceTab,
} from "../../../chats/components/appearance/ChatAppearanceForm";
import {
  areOverridesEqual,
  deriveOverrideFromSettings,
  normalizeOverride,
} from "../../../chats/components/appearance/overrideHelpers";
import type { AppearanceFieldUpdater } from "../../GroupChatLayout";

interface GroupChatAppearanceDrawerProps {
  open: boolean;
  onClose: () => void;
  group: Group;
  onGroupUpdate: (next: Group) => void;
  setDraftOverride: (next: ChatAppearanceOverride | null) => void;
  registerFieldUpdater?: (fn: AppearanceFieldUpdater | null) => void;
}

export function GroupChatAppearanceDrawer({
  open,
  onClose,
  group,
  onGroupUpdate,
  setDraftOverride,
  registerFieldUpdater,
}: GroupChatAppearanceDrawerProps) {
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
  const [globalSettings, setGlobalSettings] = useState<ChatAppearanceSettings>(
    createDefaultChatAppearanceSettings(),
  );
  const [activeTab, setActiveTab] = useState<AppearanceTab>("typography");
  const [override, setOverride] = useState<ChatAppearanceOverride>(() =>
    normalizeOverride(group.chatAppearance ?? {}),
  );
  const [initialOverride, setInitialOverride] = useState<ChatAppearanceOverride>(() =>
    normalizeOverride(group.chatAppearance ?? {}),
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
        console.error("GroupChatAppearanceDrawer: failed to load global settings", err);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const next = normalizeOverride(group.chatAppearance ?? {});
    setOverride(next);
    setInitialOverride(next);
  }, [open, group.chatAppearance]);

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
      const saved = await updateGroupChatAppearance(
        group.id,
        Object.keys(derived).length > 0 ? derived : null,
      );
      const normalized = normalizeOverride(derived);
      onGroupUpdate({ ...saved, chatAppearance: normalized });
      setOverride(normalized);
      setInitialOverride(normalized);
      toast.success(
        t("groupChats.appearance.saveSuccessTitle"),
        t("groupChats.appearance.saveSuccessDesc"),
      );
    } catch (err) {
      console.error("GroupChatAppearanceDrawer: save failed", err);
      toast.error(
        t("groupChats.appearance.saveFailed"),
        err instanceof Error ? err.message : String(err),
      );
    } finally {
      setIsSaving(false);
    }
  }, [isDirty, isSaving, globalSettings, effectiveSettings, group.id, onGroupUpdate, t]);

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
        t("groupChats.appearance.unsavedTitle"),
        t("groupChats.appearance.unsavedDesc"),
        t("common.buttons.discard"),
        () => {
          handleDiscard();
          onClose();
        },
        "group-appearance-drawer-unsaved",
        {
          label: t("common.buttons.save"),
          onAction: () => {
            void handleSave().then(() => {
              toast.dismiss("group-appearance-drawer-unsaved");
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

  return (
    <AnimatePresence>
      {open && (
        <motion.aside
          className={cn(
            "fixed bottom-0 top-[var(--titlebar-h,0px)] z-50 flex w-[460px] flex-col",
            "bg-surface/96 backdrop-blur-2xl shadow-2xl",
            isRight ? "right-0 border-l border-fg/10" : "left-0 border-r border-fg/10",
          )}
          initial={{ x: exitX }}
          animate={{ x: 0 }}
          exit={{ x: exitX }}
          transition={{ type: "spring", stiffness: 320, damping: 32 }}
        >
          <header className="flex items-center justify-between border-b border-fg/10 px-4 py-3">
            <div>
              <div className="text-sm font-semibold text-fg">{t("groupChats.appearance.title")}</div>
              <div className="text-[11px] text-fg/45">{group.name}</div>
            </div>
            <div className="flex items-center gap-1">
              <button
                type="button"
                onClick={toggleSide}
                className="rounded-lg p-1.5 text-fg/50 hover:bg-fg/10 hover:text-fg"
                aria-label={
                  isRight
                    ? t("groupChats.appearance.moveDrawerLeft")
                    : t("groupChats.appearance.moveDrawerRight")
                }
              >
                {isRight ? <PanelLeftOpen size={16} /> : <PanelRightOpen size={16} />}
              </button>
              <button
                type="button"
                onClick={handleCloseAttempt}
                className="rounded-lg p-1.5 text-fg/50 hover:bg-fg/10 hover:text-fg"
                aria-label={t("topNav.extra.close")}
              >
                <X size={16} />
              </button>
            </div>
          </header>

          <div className="space-y-3 border-b border-fg/10 px-4 py-3">
            <AppearanceTabBar activeTab={activeTab} onChange={setActiveTab} />
            <button
              type="button"
              onClick={() => setOverride({})}
              className={cn(
                "flex w-full items-center justify-center gap-2 rounded-lg border py-2 text-[11px] font-medium transition-all",
                "border-fg/10 bg-fg/5 text-fg/50 hover:border-fg/20 hover:bg-fg/10 hover:text-fg/70",
              )}
            >
              <RefreshCw size={11} />
              {t("groupChats.appearance.clearAllOverrides")}
            </button>
          </div>

          <div className="flex-1 overflow-y-auto px-4 py-4">
            <ChatAppearanceForm
              settings={effectiveSettings}
              mode="character"
              activeTab={activeTab}
              showParticipantsBar
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
              {isSaving ? t("common.buttons.saving") : t("topNav.save")}
            </button>
          </footer>
        </motion.aside>
      )}
    </AnimatePresence>
  );
}
