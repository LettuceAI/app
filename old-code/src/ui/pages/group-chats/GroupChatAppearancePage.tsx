import { useCallback, useEffect, useMemo, useState } from "react";
import { ArrowLeft, RefreshCw, Loader2 } from "lucide-react";
import {
  createDefaultChatAppearanceSettings,
  mergeChatAppearance,
  type ChatAppearanceOverride,
  type ChatAppearanceSettings,
} from "../../../core/storage/schemas";
import { readSettings, updateGroupChatAppearance } from "../../../core/storage/repo";
import { useI18n } from "../../../core/i18n/context";
import { colors, cn, radius, interactive } from "../../design-tokens";
import { useNavigationManager } from "../../navigation";
import { toast } from "../../components/toast";
import {
  AppearanceTabBar,
  ChatAppearanceForm,
  type AppearanceKey,
  type AppearanceTab,
} from "../chats/components/appearance/ChatAppearanceForm";
import {
  areOverridesEqual,
  deriveOverrideFromSettings,
  normalizeOverride,
} from "../chats/components/appearance/overrideHelpers";
import { useGroupChatLayoutContext } from "./GroupChatLayout";

export function GroupChatAppearancePage() {
  const { t } = useI18n();
  const { backOrReplace } = useNavigationManager();
  const { group, updateGroup } = useGroupChatLayoutContext();

  const [globalSettings, setGlobalSettings] = useState<ChatAppearanceSettings>(
    createDefaultChatAppearanceSettings(),
  );
  const [activeTab, setActiveTab] = useState<AppearanceTab>("typography");
  const [override, setOverride] = useState<ChatAppearanceOverride>({});
  const [initialOverride, setInitialOverride] = useState<ChatAppearanceOverride>({});
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const settings = await readSettings();
        const global =
          settings.advancedSettings?.chatAppearance ?? createDefaultChatAppearanceSettings();
        if (!cancelled) setGlobalSettings(global);
      } catch (err) {
        console.error("GroupChatAppearancePage: failed to load global settings", err);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const next = normalizeOverride(group?.chatAppearance ?? {});
    setOverride(next);
    setInitialOverride(next);
  }, [group?.chatAppearance]);

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

  const handleSave = useCallback(async () => {
    if (!group || !isDirty || isSaving) return;
    setIsSaving(true);
    try {
      const derived = deriveOverrideFromSettings(globalSettings, effectiveSettings);
      const saved = await updateGroupChatAppearance(
        group.id,
        Object.keys(derived).length > 0 ? derived : null,
      );
      const normalized = normalizeOverride(derived);
      updateGroup({ ...saved, chatAppearance: normalized });
      setOverride(normalized);
      setInitialOverride(normalized);
      toast.success(
        t("groupChats.appearance.saveSuccessTitle"),
        t("groupChats.appearance.saveSuccessDesc"),
      );
    } catch (err) {
      console.error("GroupChatAppearancePage: save failed", err);
      toast.error(
        t("groupChats.appearance.saveFailed"),
        err instanceof Error ? err.message : String(err),
      );
    } finally {
      setIsSaving(false);
    }
  }, [group, isDirty, isSaving, globalSettings, effectiveSettings, updateGroup, t]);

  return (
    <div className={cn("flex h-screen flex-col", colors.surface.base, colors.text.primary)}>
      <div
        className={cn(
          "flex items-center gap-3 border-b pl-3 pb-3 pt-[calc(env(safe-area-inset-top)+12px)] shrink-0 z-20",
          "pr-3",
          colors.glass.strong,
        )}
      >
        <button
          onClick={() => backOrReplace("/group-chats")}
          className={cn(
            "flex shrink-0 items-center justify-center h-8 w-8",
            radius.full,
            "border bg-fg/5",
            colors.border.subtle,
            colors.text.primary,
            interactive.hover.brightness,
            interactive.active.scale,
          )}
          aria-label={t("chats.header.back")}
        >
          <ArrowLeft size={14} />
        </button>
        <div className="flex-1 min-w-0">
          <div className="text-sm font-semibold text-fg">{t("groupChats.appearance.title")}</div>
          {group && <div className="truncate text-[11px] text-fg/45">{group.name}</div>}
        </div>
        <button
          type="button"
          onClick={() => setOverride({})}
          className={cn(
            "flex shrink-0 items-center gap-1.5 rounded-lg border border-fg/10 bg-fg/5 px-2.5 py-1.5 text-[11px] font-medium text-fg/50",
            "hover:border-fg/20 hover:bg-fg/10 hover:text-fg/70",
          )}
        >
          <RefreshCw size={11} />
          {t("common.buttons.reset")}
        </button>
      </div>

      {!group ? (
        <div className="flex flex-1 items-center justify-center">
          <Loader2 className="animate-spin text-fg/30" size={24} />
        </div>
      ) : (
        <>
          <div className="border-b border-fg/10 px-4 py-3">
            <AppearanceTabBar activeTab={activeTab} onChange={setActiveTab} />
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
          <div className="flex gap-2 border-t border-fg/10 px-4 py-3 pb-[calc(env(safe-area-inset-bottom)+12px)]">
            <button
              type="button"
              onClick={() => setOverride(initialOverride)}
              disabled={!isDirty || isSaving}
              className={cn(
                "flex-1 rounded-lg border px-3 py-2.5 text-sm font-medium transition-all",
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
                "flex-1 rounded-lg border px-3 py-2.5 text-sm font-semibold transition-all",
                "border-accent/40 bg-accent/15 text-accent hover:border-accent/60 hover:bg-accent/25",
                "disabled:opacity-40 disabled:pointer-events-none",
              )}
            >
              {isSaving ? t("common.buttons.saving") : t("topNav.save")}
            </button>
          </div>
        </>
      )}
    </div>
  );
}
