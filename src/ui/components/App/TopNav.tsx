import { useMemo, useState, useEffect, useCallback, useRef } from "react";
import { useNavigate } from "react-router-dom";
import {
  ArrowLeft,
  Filter,
  Search,
  Settings,
  Plus,
  Check,
  Loader2,
  HelpCircle,
  HardDrive,
  LayoutList,
  LayoutGrid,
  Grid3X3,
  Upload,
  Eye,
  Minus,
  Square,
  X,
} from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { typography, interactive, cn } from "../../design-tokens";
import { toast } from "../toast";
import { openDocs } from "../../../core/utils/docs";
import { type TranslationKey, useI18n } from "../../../core/i18n/context";
import { getPlatform } from "../../../core/utils/platform";

interface TopNavProps {
  currentPath: string;
  onBackOverride?: () => void;
  titleOverride?: string;
  rightAction?: React.ReactNode;
}

const appPlatform = getPlatform();
const isDesktop = appPlatform.type === "desktop";
const isMacOS = appPlatform.os === "macos";
/** True when custom window control buttons will render (Linux/Windows desktop). */
export const hasCustomWindowControls = isDesktop && !isMacOS;

// Cache window chrome flags from CLI args (--osdecorations, --nobuttons).
let _chromeFlags: { osDecorations: boolean; noButtons: boolean } | null = null;
const chromeFlagsPromise = isDesktop
  ? invoke<[boolean, boolean]>("get_window_chrome_flags")
      .then(([osDecorations, noButtons]) => {
        _chromeFlags = { osDecorations, noButtons };
        return _chromeFlags;
      })
      .catch(() => ({ osDecorations: false, noButtons: false }))
  : Promise.resolve({ osDecorations: false, noButtons: false });

function useChromeFlags() {
  const [flags, setFlags] = useState(_chromeFlags);
  useEffect(() => {
    if (_chromeFlags) {
      setFlags(_chromeFlags);
    } else {
      chromeFlagsPromise.then((f) => setFlags(f));
    }
  }, []);
  return flags;
}

export function TopNav({ currentPath, onBackOverride, titleOverride, rightAction }: TopNavProps) {
  const navigate = useNavigate();
  const { t } = useI18n();
  const chromeFlags = useChromeFlags();
  const showWindowControls =
    isDesktop && !isMacOS && !chromeFlags?.osDecorations && !chromeFlags?.noButtons;
  const showDragRegion = isDesktop && !chromeFlags?.osDecorations;
  const basePath = useMemo(() => currentPath.split("?")[0], [currentPath]);
  const hasAdvancedView = useMemo(() => currentPath.includes("view=advanced"), [currentPath]);
  const wasUnsavedRef = useRef(false);

  const title = useMemo(() => {
    if (titleOverride) return titleOverride;

    const rules: Array<{
      match: (path: string) => boolean;
      titleKey: TranslationKey;
    }> = [
      { match: (p) => p === "/discover", titleKey: "common.bottomNav.discover" },
      { match: (p) => p === "/discover/search", titleKey: "common.bottomNav.discover" },
      { match: (p) => p.startsWith("/discover/browse"), titleKey: "common.bottomNav.discover" },
      { match: (p) => p.startsWith("/discover/card/"), titleKey: "common.bottomNav.discover" },
      { match: (p) => p === "/settings/providers", titleKey: "common.nav.providers" },
      { match: (p) => p.includes("view=advanced"), titleKey: "common.nav.responseStyle" },
      { match: (p) => p === "/settings/models/installed", titleKey: "hfBrowser.libraryTitle" },
      { match: (p) => p === "/settings/models/browse", titleKey: "hfBrowser.title" },
      {
        match: (p) => p === "/settings/models" || p.startsWith("/settings/models/"),
        titleKey: "common.nav.models",
      },
      { match: (p) => p === "/settings/security", titleKey: "common.nav.security" },
      { match: (p) => p === "/settings/accessibility", titleKey: "common.nav.accessibility" },
      { match: (p) => p === "/settings/reset", titleKey: "common.nav.reset" },
      { match: (p) => p === "/settings/backup", titleKey: "common.nav.backupRestore" },
      { match: (p) => p === "/settings/convert", titleKey: "common.nav.convertFiles" },
      {
        match: (p) => p.startsWith("/settings/usage/activity"),
        titleKey: "common.nav.usageAnalytics",
      },
      { match: (p) => p === "/settings/usage", titleKey: "common.nav.usageAnalytics" },
      { match: (p) => p === "/settings/changelog", titleKey: "common.nav.changelog" },
      { match: (p) => p === "/settings/about", titleKey: "common.nav.about" },
      { match: (p) => p.includes("/debug/"), titleKey: "common.nav.messageDebug" },
      { match: (p) => p === "/settings/prompts/new", titleKey: "common.nav.createSystemPrompt" },
      { match: (p) => p.startsWith("/settings/prompts/"), titleKey: "common.nav.editSystemPrompt" },
      { match: (p) => p === "/settings/prompts", titleKey: "common.nav.systemPrompts" },
      { match: (p) => p === "/settings/developer", titleKey: "common.nav.developer" },
      { match: (p) => p === "/settings/advanced", titleKey: "common.nav.advanced" },
      { match: (p) => p === "/settings/characters", titleKey: "common.nav.characters" },
      { match: (p) => p.includes("/lorebook"), titleKey: "common.nav.lorebooks" },
      { match: (p) => p === "/settings/personas", titleKey: "common.nav.personas" },
      { match: (p) => p === "/settings/advanced/memory", titleKey: "common.nav.dynamicMemory" },
      {
        match: (p) => p === "/settings/advanced/creation-helper",
        titleKey: "common.nav.creationHelper",
      },
      {
        match: (p) => p === "/settings/advanced/help-me-reply",
        titleKey: "common.nav.helpMeReply",
      },
      {
        match: (p) => p.startsWith("/personas/") && p.endsWith("/edit"),
        titleKey: "common.nav.editPersona",
      },
      {
        match: (p) => p.startsWith("/settings/personas/") && p.endsWith("/edit"),
        titleKey: "common.nav.editPersona",
      },
      {
        match: (p) => p.startsWith("/settings/characters/") && p.includes("/templates/new"),
        titleKey: "common.nav.newTemplate",
      },
      {
        match: (p) =>
          p.startsWith("/settings/characters/") &&
          p.includes("/templates/") &&
          !p.endsWith("/templates"),
        titleKey: "common.nav.editTemplate",
      },
      {
        match: (p) => p.startsWith("/settings/characters/") && p.endsWith("/templates"),
        titleKey: "common.nav.chatTemplates",
      },
      {
        match: (p) => p.startsWith("/settings/characters/") && p.endsWith("/edit"),
        titleKey: "common.nav.editCharacter",
      },
      { match: (p) => p === "/settings/sync", titleKey: "common.nav.sync" },
      {
        match: (p) => p.startsWith("/settings/engine/") && p.includes("/character/new"),
        titleKey: "common.nav.newCharacter",
      },
      {
        match: (p) => p.startsWith("/settings/engine/") && p.endsWith("/setup"),
        titleKey: "common.nav.engineSetup",
      },
      {
        match: (p) => p.startsWith("/settings/engine/") && p.endsWith("/providers"),
        titleKey: "common.nav.llmProviders",
      },
      {
        match: (p) => p.startsWith("/settings/engine/") && p.endsWith("/settings"),
        titleKey: "common.nav.engineSettings",
      },
      { match: (p) => p.startsWith("/settings/engine/"), titleKey: "common.nav.lettuceEngine" },
      { match: (p) => p.startsWith("/settings"), titleKey: "common.nav.settings" },
      { match: (p) => p.startsWith("/create"), titleKey: "common.nav.create" },
      { match: (p) => p.startsWith("/onboarding"), titleKey: "common.nav.setup" },
      { match: (p) => p.startsWith("/welcome"), titleKey: "common.nav.welcome" },
      { match: (p) => p.startsWith("/chat/"), titleKey: "common.nav.conversation" },
      { match: (p) => p === "/library/images/pick", titleKey: "common.nav.library" },
      { match: (p) => p === "/library", titleKey: "common.nav.library" },
      { match: (p) => p === "/group-chats", titleKey: "common.nav.groupChats" },
      { match: (p) => p.startsWith("/group-chats/"), titleKey: "common.nav.groupChat" },
    ];

    const rule = rules.find((r) => r.match(basePath));
    return rule ? t(rule.titleKey) : t("common.nav.chats");
  }, [basePath, t, titleOverride]);

  const showBackButton = useMemo(() => {
    if (onBackOverride) return true;
    if (basePath.startsWith("/settings/") || basePath === "/settings") return true;
    if (basePath.startsWith("/create/")) return true;
    if (basePath.startsWith("/personas/") && basePath.endsWith("/edit")) return true;
    if (basePath.startsWith("/library/")) return true;
    if (basePath.startsWith("/library/lorebooks")) return true;
    if (basePath === "/group-chats/new") return true;
    if (basePath.startsWith("/discover/")) return true;
    return false;
  }, [basePath, onBackOverride]);

  const showFilterButton = useMemo(() => {
    return (
      basePath === "/settings/usage" ||
      basePath === "/settings/changelog" ||
      basePath === "/settings/models"
    );
  }, [basePath]);

  const showSearchButton = useMemo(() => {
    return (
      basePath === "/chat" ||
      basePath === "/" ||
      basePath === "/library" ||
      basePath === "/group-chats"
    );
  }, [basePath]);

  const showSettingsButton = useMemo(() => {
    return (
      basePath === "/chat" ||
      basePath === "/" ||
      basePath === "/library" ||
      basePath === "/group-chats"
    );
  }, [basePath]);

  const showLayoutToggle = useMemo(() => {
    return basePath === "/chat" || basePath === "/" || basePath === "/settings/models";
  }, [basePath]);

  const [layoutViewMode, setLayoutViewMode] = useState<string>("hero");
  useEffect(() => {
    if (!showLayoutToggle) return;
    const sync = () => {
      if (basePath === "/settings/models") {
        const mode = (window as any).__modelsViewMode;
        if (mode) setLayoutViewMode(mode);
        return;
      }
      const mode = (window as any).__chatsViewMode;
      if (mode) setLayoutViewMode(mode);
    };
    sync();
    const eventName =
      basePath === "/settings/models" ? "models:viewModeChanged" : "chats:viewModeChanged";
    window.addEventListener(eventName, sync);
    return () => window.removeEventListener(eventName, sync);
  }, [basePath, showLayoutToggle]);

  const LayoutToggleIcon =
    basePath === "/settings/models"
      ? layoutViewMode === "grid"
        ? LayoutList
        : LayoutGrid
      : layoutViewMode === "hero"
        ? LayoutGrid
        : layoutViewMode === "gallery"
          ? Grid3X3
          : LayoutList;

  const showAddButton = useMemo(() => {
    if (basePath.startsWith("/settings/providers")) return true;
    // Only show + on models list page, not on edit pages (/settings/models/xxx)
    if (basePath === "/settings/models" && !hasAdvancedView) return true;
    if (basePath === "/settings/prompts") return true;
    if (/^\/settings\/characters\/[^/]+\/templates$/.test(basePath)) return true;
    if (basePath.includes("/lorebook")) return true;
    return false;
  }, [basePath, hasAdvancedView]);

  const showImportButton = useMemo(
    () =>
      basePath === "/settings/models" ||
      /^\/settings\/characters\/[^/]+\/templates$/.test(basePath),
    [basePath],
  );

  const showInstalledModelsButton = useMemo(
    () => basePath === "/settings/models/browse",
    [basePath],
  );

  // Map paths to docs keys for contextual help
  const docsKeyForPath = useMemo(() => {
    if (basePath === "/settings/providers") return "providers";
    if (basePath === "/settings/models" || basePath.startsWith("/settings/models/"))
      return "models";
    if (basePath === "/settings/prompts" || basePath.startsWith("/settings/prompts/"))
      return "systemPrompts";
    if (
      basePath === "/settings/characters" ||
      (basePath.startsWith("/settings/characters/") && basePath.endsWith("/edit"))
    )
      return "characters";
    if (
      (basePath.startsWith("/personas/") && basePath.endsWith("/edit")) ||
      basePath === "/settings/personas" ||
      (basePath.startsWith("/settings/personas/") && basePath.endsWith("/edit"))
    )
      return "personas";
    if (basePath === "/settings/accessibility") return "accessibility";
    if (basePath === "/settings/sync") return "sync";
    if (basePath === "/settings/advanced/memory") return "memorySystem";
    if (basePath.includes("/lorebook")) return "lorebooks";
    return null;
  }, [basePath]);

  const showHelpButton = useMemo(() => docsKeyForPath !== null, [docsKeyForPath]);

  const isCenteredTitle = useMemo(() => {
    return (
      basePath.startsWith("/settings") ||
      (basePath.startsWith("/personas/") && basePath.endsWith("/edit"))
    );
  }, [basePath]);

  const isCharacterEdit = useMemo(
    () => /^\/settings\/characters\/[^/]+\/edit$/.test(basePath),
    [basePath],
  );
  const isPersonaEdit = useMemo(
    () =>
      /^\/settings\/personas\/[^/]+\/edit$/.test(basePath) ||
      /^\/personas\/[^/]+\/edit$/.test(basePath),
    [basePath],
  );
  const isModelEdit = useMemo(
    () =>
      /^\/settings\/models\/[^/]+$/.test(basePath) &&
      basePath !== "/settings/models/new" &&
      basePath !== "/settings/models/browse",
    [basePath],
  );
  const isModelNew = useMemo(() => basePath === "/settings/models/new", [basePath]);
  const isPromptEdit = useMemo(
    () => /^\/settings\/prompts\/[^/]+$/.test(basePath) && basePath !== "/settings/prompts/new",
    [basePath],
  );
  const isPromptNew = useMemo(() => basePath === "/settings/prompts/new", [basePath]);
  const isChatAppearanceEdit = useMemo(
    () => basePath === "/settings/accessibility/chat",
    [basePath],
  );
  const isColorCustomizationEdit = useMemo(
    () => basePath === "/settings/accessibility/colors",
    [basePath],
  );
  const isTemplateEdit = useMemo(
    () => /^\/settings\/characters\/[^/]+\/templates\/[^/]+$/.test(basePath),
    [basePath],
  );
  const showSaveButton =
    isCharacterEdit ||
    isPersonaEdit ||
    isModelEdit ||
    isModelNew ||
    isPromptEdit ||
    isPromptNew ||
    isChatAppearanceEdit ||
    isColorCustomizationEdit ||
    isTemplateEdit;

  const [isMobileViewport, setIsMobileViewport] = useState(() =>
    typeof window !== "undefined" ? window.matchMedia("(max-width: 1023px)").matches : false,
  );

  useEffect(() => {
    if (typeof window === "undefined") return;
    const mediaQuery = window.matchMedia("(max-width: 1023px)");
    const syncViewport = () => setIsMobileViewport(mediaQuery.matches);
    syncViewport();
    mediaQuery.addEventListener("change", syncViewport);
    return () => mediaQuery.removeEventListener("change", syncViewport);
  }, []);

  const showChatAppearancePreviewButton = isChatAppearanceEdit && isMobileViewport;
  const chatAppearancePreviewLabel = t("chatAppearance.preview.label");

  // Track save button state from window globals
  const [canSave, setCanSave] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const isUnsaved = showSaveButton && canSave && !isSaving;

  useEffect(() => {
    if (!showSaveButton) return;

    const checkGlobals = () => {
      const globalWindow = window as any;

      if (isCharacterEdit) {
        const newCanSave = !!globalWindow.__saveCharacterCanSave;
        const newIsSaving = !!globalWindow.__saveCharacterSaving;
        setCanSave((prev) => (prev !== newCanSave ? newCanSave : prev));
        setIsSaving((prev) => (prev !== newIsSaving ? newIsSaving : prev));
      } else if (isPersonaEdit) {
        const newCanSave = !!globalWindow.__savePersonaCanSave;
        const newIsSaving = !!globalWindow.__savePersonaSaving;
        setCanSave((prev) => (prev !== newCanSave ? newCanSave : prev));
        setIsSaving((prev) => (prev !== newIsSaving ? newIsSaving : prev));
      } else if (isModelEdit || isModelNew) {
        const newCanSave = !!globalWindow.__saveModelCanSave;
        const newIsSaving = !!globalWindow.__saveModelSaving;
        setCanSave((prev) => (prev !== newCanSave ? newCanSave : prev));
        setIsSaving((prev) => (prev !== newIsSaving ? newIsSaving : prev));
      } else if (isPromptEdit || isPromptNew) {
        const newCanSave = !!globalWindow.__savePromptCanSave;
        const newIsSaving = !!globalWindow.__savePromptSaving;
        setCanSave((prev) => (prev !== newCanSave ? newCanSave : prev));
        setIsSaving((prev) => (prev !== newIsSaving ? newIsSaving : prev));
      } else if (isChatAppearanceEdit) {
        const newCanSave = !!globalWindow.__saveChatAppearanceCanSave;
        const newIsSaving = !!globalWindow.__saveChatAppearanceSaving;
        setCanSave((prev) => (prev !== newCanSave ? newCanSave : prev));
        setIsSaving((prev) => (prev !== newIsSaving ? newIsSaving : prev));
      } else if (isColorCustomizationEdit) {
        const newCanSave = !!globalWindow.__saveColorCustomizationCanSave;
        const newIsSaving = !!globalWindow.__saveColorCustomizationSaving;
        setCanSave((prev) => (prev !== newCanSave ? newCanSave : prev));
        setIsSaving((prev) => (prev !== newIsSaving ? newIsSaving : prev));
      } else if (isTemplateEdit) {
        const newCanSave = !!globalWindow.__saveCharacterCanSave;
        const newIsSaving = !!globalWindow.__saveCharacterSaving;
        setCanSave((prev) => (prev !== newCanSave ? newCanSave : prev));
        setIsSaving((prev) => (prev !== newIsSaving ? newIsSaving : prev));
      }
    };

    // Check immediately and on interval
    checkGlobals();
    const interval = setInterval(checkGlobals, 200);

    return () => clearInterval(interval);
  }, [
    showSaveButton,
    isCharacterEdit,
    isPersonaEdit,
    isModelEdit,
    isModelNew,
    isPromptEdit,
    isPromptNew,
    isChatAppearanceEdit,
    isColorCustomizationEdit,
    isTemplateEdit,
  ]);

  useEffect(() => {
    const globalWindow = window as any;
    globalWindow.__unsavedChanges = isUnsaved;
    return () => {
      if (globalWindow.__unsavedChanges === isUnsaved) {
        globalWindow.__unsavedChanges = false;
      }
    };
  }, [isUnsaved]);

  const ensureUnsavedToast = useCallback(() => {
    if (!toast.isVisible("unsaved-changes")) {
      toast.warningSticky(
        t("topNav.unsavedChangesTitle"),
        t("topNav.unsavedChangesMessage"),
        t("common.buttons.discard"),
        () => window.dispatchEvent(new CustomEvent("unsaved:discard")),
        "unsaved-changes",
      );
    }
  }, [t]);

  useEffect(() => {
    if (isUnsaved && !wasUnsavedRef.current) {
      ensureUnsavedToast();
    } else if (!isUnsaved) {
      toast.dismiss("unsaved-changes");
    }
    wasUnsavedRef.current = isUnsaved;
  }, [isUnsaved, ensureUnsavedToast]);

  const handleBack = () => {
    if (isUnsaved) {
      ensureUnsavedToast();
      return;
    }
    if (onBackOverride) {
      onBackOverride();
      return;
    }
    navigate(-1);
  };

  const handleAddClick = () => {
    if (basePath.startsWith("/settings/providers")) {
      window.dispatchEvent(new CustomEvent("providers:add"));
      return;
    }
    if (basePath.startsWith("/settings/models") && !hasAdvancedView) {
      window.dispatchEvent(new CustomEvent("models:add"));
      return;
    }
    if (basePath === "/settings/prompts") {
      window.dispatchEvent(new CustomEvent("prompts:add"));
      return;
    }
    if (/^\/settings\/characters\/[^/]+\/templates$/.test(basePath)) {
      window.dispatchEvent(new CustomEvent("templates:add"));
      return;
    }
    if (basePath.includes("/lorebook")) {
      window.dispatchEvent(new CustomEvent("lorebook:add"));
      return;
    }
  };

  const handleFilterClick = () => {
    if (basePath === "/settings/changelog") {
      window.dispatchEvent(new CustomEvent("changelog:openVersionSelector"));
      return;
    }
    if (basePath === "/settings/models") {
      const globalWindow = window as any;
      if (typeof globalWindow.__openModelsSort === "function") {
        globalWindow.__openModelsSort();
      } else {
        window.dispatchEvent(new CustomEvent("models:sort"));
      }
      return;
    }
    if (typeof window !== "undefined") {
      const globalWindow = window as any;
      if (typeof globalWindow.__openUsageFilters === "function") {
        globalWindow.__openUsageFilters();
      } else {
        window.dispatchEvent(new CustomEvent("usage:filters"));
      }
    }
  };

  const handleImportClick = () => {
    if (basePath === "/settings/models") {
      window.dispatchEvent(new CustomEvent("models:import"));
      return;
    }
    if (/^\/settings\/characters\/[^/]+\/templates$/.test(basePath)) {
      window.dispatchEvent(new CustomEvent("templates:import"));
    }
  };

  return (
    <header
      className="fixed top-0 left-0 right-0 z-40 border-b border-fg/10 backdrop-blur-md bg-nav/80"
      style={{
        paddingTop: isDesktop ? "8px" : "calc(env(safe-area-inset-top) + 12px)",
        paddingBottom: isDesktop ? "8px" : "12px",
      }}
      {...(showDragRegion ? { "data-tauri-drag-region": "" } : {})}
    >
      <div
        className={cn(
          "relative mx-auto flex w-full max-w-md lg:max-w-none items-center justify-between px-3 h-10",
          showWindowControls ? "lg:pl-8 lg:pr-0" : "lg:px-8",
        )}
        style={isMacOS ? { paddingLeft: "72px" } : undefined}
        {...(showDragRegion ? { "data-tauri-drag-region": "" } : {})}
      >
        {/* Left side: */}
        <div className="flex items-center gap-1 overflow-hidden h-full" {...(showDragRegion ? { "data-tauri-drag-region": "" } : {})}>
          <div
            className={cn(
              "flex items-center justify-center shrink-0",
              showBackButton ? "w-10" : "w-0",
            )}
          >
            <AnimatePresence mode="wait" initial={false}>
              {showBackButton && (
                <motion.button
                  key="back"
                  initial={{ opacity: 0, scale: 0.8 }}
                  animate={{ opacity: 1, scale: 1 }}
                  exit={{ opacity: 0, scale: 0.8 }}
                  transition={{ duration: 0.2 }}
                  onClick={handleBack}
                  className={cn(
                    "flex items-center px-[0.6em] py-[0.3em] justify-center rounded-full p-2",
                    "text-fg/70 hover:text-fg hover:bg-fg/10",
                    interactive.transition.fast,
                    interactive.active.scale,
                  )}
                  aria-label={t("topNav.goBack")}
                >
                  <ArrowLeft size={20} strokeWidth={2.5} />
                </motion.button>
              )}
            </AnimatePresence>
          </div>

          <motion.h1
            key={title}
            initial={{ opacity: 0, y: 5 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.3, ease: "easeOut" }}
            {...(showDragRegion ? { "data-tauri-drag-region": "" } : {})}
            className={cn(
              typography.h1.size,
              "font-bold text-fg tracking-tight truncate leading-none",
              isCenteredTitle && "absolute left-1/2 -translate-x-1/2 w-auto",
            )}
          >
            {title}
          </motion.h1>
        </div>

        <div className="flex items-center justify-end gap-1 shrink-0 min-w-10 h-full" {...(showDragRegion ? { "data-tauri-drag-region": "" } : {})}>
          {showLayoutToggle && (
            <button
              onClick={() =>
                window.dispatchEvent(
                  new CustomEvent(
                    basePath === "/settings/models"
                      ? "models:cycleViewMode"
                      : "chats:cycleViewMode",
                  ),
                )
              }
              className={cn(
                "hidden lg:flex items-center px-[0.6em] py-[0.3em] justify-center rounded-full",
                "text-fg/70 hover:text-fg hover:bg-fg/10",
                interactive.transition.fast,
                interactive.active.scale,
              )}
              aria-label={t("topNav.changeLayout")}
            >
              <LayoutToggleIcon size={20} strokeWidth={2.5} className="text-fg" />
            </button>
          )}
          {showSearchButton && (
            <button
              onClick={() => navigate("/search")}
              className={cn(
                "flex items-center px-[0.6em] py-[0.3em] justify-center rounded-full",
                "text-fg/70 hover:text-fg hover:bg-fg/10",
                interactive.transition.fast,
                interactive.active.scale,
              )}
              aria-label={t("topNav.search")}
            >
              <Search size={20} strokeWidth={2.5} className="text-fg" />
            </button>
          )}
          {showSettingsButton && (
            <button
              onClick={() => navigate("/settings")}
              className={cn(
                "flex items-center px-[0.6em] py-[0.3em] justify-center rounded-full",
                "text-fg/70 hover:text-fg hover:bg-fg/10",
                interactive.transition.fast,
                interactive.active.scale,
              )}
              aria-label={t("topNav.settings")}
            >
              <Settings size={20} strokeWidth={2.5} className="text-fg" />
            </button>
          )}
          {showHelpButton && (
            <button
              onClick={() => docsKeyForPath && openDocs(docsKeyForPath as any)}
              className={cn(
                "flex items-center px-[0.6em] py-[0.3em] justify-center rounded-full",
                "text-fg/80 hover:text-fg hover:bg-fg/10",
                interactive.transition.fast,
                interactive.active.scale,
              )}
              aria-label={t("topNav.help")}
            >
              <HelpCircle size={20} strokeWidth={2.5} className="text-fg/50" />
            </button>
          )}
          {showInstalledModelsButton && (
            <button
              onClick={() => navigate("/settings/models/installed")}
              className={cn(
                "flex items-center px-[0.6em] py-[0.3em] justify-center rounded-full",
                "text-fg/70 hover:text-fg hover:bg-fg/10",
                interactive.transition.fast,
                interactive.active.scale,
              )}
              aria-label="Installed Models"
              title="Installed Models"
            >
              <HardDrive size={18} strokeWidth={2.2} className="text-fg/75" />
            </button>
          )}
          {showImportButton && (
            <button
              onClick={handleImportClick}
              className={cn(
                "flex items-center px-[0.6em] py-[0.3em] justify-center rounded-full",
                "text-fg/70 hover:text-fg hover:bg-fg/10",
                interactive.transition.fast,
                interactive.active.scale,
              )}
              aria-label={t("common.buttons.import")}
            >
              <Upload size={20} strokeWidth={2.5} className="text-fg" />
            </button>
          )}
          {showChatAppearancePreviewButton && (
            <button
              onClick={() => {
                const globalWindow = window as any;
                if (typeof globalWindow.__openChatAppearancePreview === "function") {
                  globalWindow.__openChatAppearancePreview();
                }
              }}
              className={cn(
                "flex items-center px-[0.6em] py-[0.3em] justify-center rounded-full",
                "text-fg/70 hover:text-fg hover:bg-fg/10",
                interactive.transition.fast,
                interactive.active.scale,
              )}
              aria-label={chatAppearancePreviewLabel}
              title={chatAppearancePreviewLabel}
            >
              <Eye size={20} strokeWidth={2.5} className="text-fg" />
            </button>
          )}
          {showAddButton && (
            <button
              onClick={handleAddClick}
              className={cn(
                "flex items-center px-[0.6em] py-[0.3em] justify-center rounded-full",
                "text-fg/70 hover:text-fg hover:bg-fg/10",
                interactive.transition.fast,
                interactive.active.scale,
              )}
              aria-label={t("topNav.add")}
            >
              <Plus size={20} strokeWidth={2.5} className="text-fg" />
            </button>
          )}
          {showFilterButton && (
            <button
              onClick={handleFilterClick}
              className={cn(
                "flex items-center px-[0.6em] py-[0.3em] justify-center rounded-full",
                "text-fg/70 hover:text-fg hover:bg-fg/10",
                interactive.transition.fast,
                interactive.active.scale,
              )}
              aria-label={t("topNav.openFilters")}
            >
              <Filter size={20} strokeWidth={2.5} className="text-fg" />
            </button>
          )}
          {showSaveButton && (
            <button
              onClick={() => {
                const globalWindow = window as any;
                if (isCharacterEdit && typeof globalWindow.__saveCharacter === "function") {
                  globalWindow.__saveCharacter();
                } else if (isPersonaEdit && typeof globalWindow.__savePersona === "function") {
                  globalWindow.__savePersona();
                } else if (
                  (isModelEdit || isModelNew) &&
                  typeof globalWindow.__saveModel === "function"
                ) {
                  globalWindow.__saveModel();
                } else if (isPromptEdit || isPromptNew) {
                  window.dispatchEvent(new CustomEvent("prompt:save"));
                } else if (
                  isChatAppearanceEdit &&
                  typeof globalWindow.__saveChatAppearance === "function"
                ) {
                  globalWindow.__saveChatAppearance();
                } else if (
                  isColorCustomizationEdit &&
                  typeof globalWindow.__saveColorCustomization === "function"
                ) {
                  globalWindow.__saveColorCustomization();
                } else if (isTemplateEdit && typeof globalWindow.__saveCharacter === "function") {
                  globalWindow.__saveCharacter();
                }
              }}
              disabled={!canSave || isSaving}
              className={cn(
                "flex items-center justify-center gap-1.5 rounded-lg px-2.5 py-1.5",
                interactive.transition.fast,
                canSave && !isSaving
                  ? "bg-accent/20 border border-accent/40 text-accent hover:bg-accent/30"
                  : "bg-fg/5 border border-fg/10 text-fg/40 cursor-not-allowed",
              )}
              aria-label={t("topNav.save")}
            >
              {isSaving ? <Loader2 size={14} className="animate-spin" /> : <Check size={14} />}
              <span className="text-xs font-medium">{t("topNav.save")}</span>
            </button>
          )}
          {rightAction}

          {showWindowControls && (
            <div className="ml-1 flex items-center">
              <button
                type="button"
                onPointerDown={(e) => e.stopPropagation()}
                onClick={async (e) => {
                  e.stopPropagation();
                  await getCurrentWindow().minimize();
                }}
                className="flex h-8 w-10 items-center justify-center text-fg/45 transition hover:bg-fg/10 hover:text-fg"
                aria-label="Minimize"
              >
                <Minus size={15} strokeWidth={1.5} />
              </button>
              <button
                type="button"
                onPointerDown={(e) => e.stopPropagation()}
                onClick={async (e) => {
                  e.stopPropagation();
                  await getCurrentWindow().toggleMaximize();
                }}
                className="flex h-8 w-10 items-center justify-center text-fg/45 transition hover:bg-fg/10 hover:text-fg"
                aria-label="Maximize"
              >
                <Square size={12} strokeWidth={1.5} />
              </button>
              <button
                type="button"
                onPointerDown={(e) => e.stopPropagation()}
                onClick={async (e) => {
                  e.stopPropagation();
                  await getCurrentWindow().close();
                }}
                className="flex h-8 w-10 items-center justify-center text-fg/45 transition hover:bg-red-500/80 hover:text-white"
                aria-label="Close"
              >
                <X size={15} strokeWidth={1.5} />
              </button>
            </div>
          )}
        </div>
      </div>
    </header>
  );
}

/**
 * Just the minimize/maximize/close buttons. Import this into custom headers
 * (e.g. discovery pages) that don't use TopNav.
 */
export function WindowControlButtons() {
  const chromeFlags = useChromeFlags();
  const show =
    isDesktop && !isMacOS && !chromeFlags?.osDecorations && !chromeFlags?.noButtons;
  if (!show) return null;

  return (
    <div className="ml-1 flex items-center">
      <button
        type="button"
        onPointerDown={(e) => e.stopPropagation()}
        onClick={async (e) => {
          e.stopPropagation();
          await getCurrentWindow().minimize();
        }}
        className="flex h-8 w-10 items-center justify-center text-fg/45 transition hover:bg-fg/10 hover:text-fg"
        aria-label="Minimize"
      >
        <Minus size={15} strokeWidth={1.5} />
      </button>
      <button
        type="button"
        onPointerDown={(e) => e.stopPropagation()}
        onClick={async (e) => {
          e.stopPropagation();
          await getCurrentWindow().toggleMaximize();
        }}
        className="flex h-8 w-10 items-center justify-center text-fg/45 transition hover:bg-fg/10 hover:text-fg"
        aria-label="Maximize"
      >
        <Square size={12} strokeWidth={1.5} />
      </button>
      <button
        type="button"
        onPointerDown={(e) => e.stopPropagation()}
        onClick={async (e) => {
          e.stopPropagation();
          await getCurrentWindow().close();
        }}
        className="flex h-8 w-10 items-center justify-center text-fg/45 transition hover:bg-red-500/80 hover:text-white"
        aria-label="Close"
      >
        <X size={15} strokeWidth={1.5} />
      </button>
    </div>
  );
}

/** Returns props to spread onto a header element to make it a drag region. */
export function useDragRegionProps(): Record<string, string> {
  const chromeFlags = useChromeFlags();
  const showDrag = isDesktop && !chromeFlags?.osDecorations;
  return showDrag ? { "data-tauri-drag-region": "" } : {};
}

/**
 * Compact window controls strip for pages that don't render TopNav.
 * Renders as a thin bar in document flow (not floating) with drag region + buttons.
 */
export function WindowControls() {
  const chromeFlags = useChromeFlags();
  const showButtons =
    isDesktop && !isMacOS && !chromeFlags?.osDecorations && !chromeFlags?.noButtons;
  const showDrag = isDesktop && !chromeFlags?.osDecorations;

  if (!showButtons && !showDrag) return null;

  return (
    <div
      className="pointer-events-none fixed top-0 right-0 z-50 flex h-10 items-center justify-end pr-1"
      {...(showDrag ? { "data-tauri-drag-region": "" } : {})}
    >
      {showButtons && (
        <div className="pointer-events-auto flex items-center">
          <button
            type="button"
            onPointerDown={(e) => e.stopPropagation()}
            onClick={async (e) => {
              e.stopPropagation();
              await getCurrentWindow().minimize();
            }}
            className="flex h-7 w-10 items-center justify-center text-fg/45 transition hover:bg-fg/10 hover:text-fg"
            aria-label="Minimize"
          >
            <Minus size={15} strokeWidth={1.5} />
          </button>
          <button
            type="button"
            onPointerDown={(e) => e.stopPropagation()}
            onClick={async (e) => {
              e.stopPropagation();
              await getCurrentWindow().toggleMaximize();
            }}
            className="flex h-7 w-10 items-center justify-center text-fg/45 transition hover:bg-fg/10 hover:text-fg"
            aria-label="Maximize"
          >
            <Square size={12} strokeWidth={1.5} />
          </button>
          <button
            type="button"
            onPointerDown={(e) => e.stopPropagation()}
            onClick={async (e) => {
              e.stopPropagation();
              await getCurrentWindow().close();
            }}
            className="flex h-7 w-10 items-center justify-center text-fg/45 transition hover:bg-red-500/80 hover:text-white"
            aria-label="Close"
          >
            <X size={15} strokeWidth={1.5} />
          </button>
        </div>
      )}
    </div>
  );
}
