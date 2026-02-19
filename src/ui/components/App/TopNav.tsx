import { useMemo, useState, useEffect, useCallback } from "react";
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
} from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { typography, interactive, cn } from "../../design-tokens";
import { toast } from "../toast";
import { openDocs } from "../../../core/utils/docs";

interface TopNavProps {
  currentPath: string;
  onBackOverride?: () => void;
  titleOverride?: string;
  rightAction?: React.ReactNode;
}

export function TopNav({ currentPath, onBackOverride, titleOverride, rightAction }: TopNavProps) {
  const navigate = useNavigate();
  const basePath = useMemo(() => currentPath.split("?")[0], [currentPath]);
  const hasAdvancedView = useMemo(() => currentPath.includes("view=advanced"), [currentPath]);

  const title = useMemo(() => {
    if (titleOverride) return titleOverride;

    const rules: Array<{
      match: (path: string) => boolean;
      title: string;
    }> = [
      { match: (p) => p === "/settings/providers", title: "Providers" },
      { match: (p) => p.includes("view=advanced"), title: "Response Style" },
      {
        match: (p) => p === "/settings/models" || p.startsWith("/settings/models/"),
        title: "Models",
      },
      { match: (p) => p === "/settings/security", title: "Security" },
      { match: (p) => p === "/settings/accessibility", title: "Accessibility" },
      { match: (p) => p === "/settings/reset", title: "Reset" },
      { match: (p) => p === "/settings/backup", title: "Backup & Restore" },
      { match: (p) => p === "/settings/convert", title: "Convert Files" },
      { match: (p) => p === "/settings/usage", title: "Usage Analytics" },
      { match: (p) => p === "/settings/changelog", title: "Changelog" },
      { match: (p) => p === "/settings/prompts/new", title: "Create System Prompt" },
      { match: (p) => p.startsWith("/settings/prompts/"), title: "Edit System Prompt" },
      { match: (p) => p === "/settings/prompts", title: "System Prompts" },
      { match: (p) => p === "/settings/developer", title: "Developer" },
      { match: (p) => p === "/settings/advanced", title: "Advanced" },
      { match: (p) => p === "/settings/characters", title: "Characters" },
      { match: (p) => p.includes("/lorebook"), title: "Lorebooks" },
      { match: (p) => p === "/settings/personas", title: "Personas" },
      { match: (p) => p === "/settings/advanced/memory", title: "Dynamic Memory" },
      { match: (p) => p === "/settings/advanced/creation-helper", title: "Creation Helper" },
      { match: (p) => p === "/settings/advanced/help-me-reply", title: "Help Me Reply" },
      {
        match: (p) => p.startsWith("/settings/personas/") && p.endsWith("/edit"),
        title: "Edit Persona",
      },
      {
        match: (p) => p.startsWith("/settings/characters/") && p.endsWith("/edit"),
        title: "Edit Character",
      },
      { match: (p) => p === "/settings/sync", title: "Sync" },
      {
        match: (p) => p.startsWith("/settings/engine/") && p.includes("/character/new"),
        title: "New Character",
      },
      {
        match: (p) => p.startsWith("/settings/engine/") && p.endsWith("/setup"),
        title: "Engine Setup",
      },
      {
        match: (p) => p.startsWith("/settings/engine/") && p.endsWith("/providers"),
        title: "LLM Providers",
      },
      {
        match: (p) => p.startsWith("/settings/engine/") && p.endsWith("/settings"),
        title: "Engine Settings",
      },
      { match: (p) => p.startsWith("/settings/engine/"), title: "Lettuce Engine" },
      { match: (p) => p.startsWith("/settings"), title: "Settings" },
      { match: (p) => p.startsWith("/create"), title: "Create" },
      { match: (p) => p.startsWith("/onboarding"), title: "Setup" },
      { match: (p) => p.startsWith("/welcome"), title: "Welcome" },
      { match: (p) => p.startsWith("/chat/"), title: "Conversation" },
      { match: (p) => p === "/library", title: "Library" },
      { match: (p) => p === "/group-chats", title: "Group Chats" },
      { match: (p) => p.startsWith("/group-chats/"), title: "Group Chat" },
    ];

    const rule = rules.find((r) => r.match(basePath));
    return rule?.title ?? "Chats";
  }, [basePath, titleOverride]);

  const showBackButton = useMemo(() => {
    if (basePath.startsWith("/settings/") || basePath === "/settings") return true;
    if (basePath.startsWith("/create/")) return true;
    if (basePath.startsWith("/library/lorebooks")) return true;
    if (basePath === "/group-chats/new") return true;
    return false;
  }, [basePath]);

  const showFilterButton = useMemo(() => {
    return (
      basePath === "/settings/usage" ||
      basePath === "/settings/changelog" ||
      basePath === "/library" ||
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

  const showAddButton = useMemo(() => {
    if (basePath.startsWith("/settings/providers")) return true;
    // Only show + on models list page, not on edit pages (/settings/models/xxx)
    if (basePath === "/settings/models" && !hasAdvancedView) return true;
    if (basePath === "/settings/prompts") return true;
    if (basePath.includes("/lorebook")) return true;
    return false;
  }, [basePath, hasAdvancedView]);

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
    return basePath.startsWith("/settings");
  }, [basePath]);

  const isCharacterEdit = useMemo(
    () => /^\/settings\/characters\/[^/]+\/edit$/.test(basePath),
    [basePath],
  );
  const isPersonaEdit = useMemo(
    () => /^\/settings\/personas\/[^/]+\/edit$/.test(basePath),
    [basePath],
  );
  const isModelEdit = useMemo(
    () => /^\/settings\/models\/[^/]+$/.test(basePath) && basePath !== "/settings/models/new",
    [basePath],
  );
  const isModelNew = useMemo(() => basePath === "/settings/models/new", [basePath]);
  const isPromptEdit = useMemo(
    () => /^\/settings\/prompts\/[^/]+$/.test(basePath) && basePath !== "/settings/prompts/new",
    [basePath],
  );
  const isPromptNew = useMemo(() => basePath === "/settings/prompts/new", [basePath]);
  const showSaveButton =
    isCharacterEdit || isPersonaEdit || isModelEdit || isModelNew || isPromptEdit || isPromptNew;

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
        "Unsaved changes",
        "Save or discard your changes before leaving.",
        "Discard",
        () => window.dispatchEvent(new CustomEvent("unsaved:discard")),
        "unsaved-changes",
      );
    }
  }, []);

  useEffect(() => {
    if (isUnsaved) {
      ensureUnsavedToast();
    } else {
      toast.dismiss("unsaved-changes");
    }
  }, [isUnsaved, ensureUnsavedToast]);

  useEffect(() => {
    if (!isUnsaved) return;
    const handleInput = () => ensureUnsavedToast();
    document.addEventListener("input", handleInput, true);
    return () => document.removeEventListener("input", handleInput, true);
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
    } else if (basePath === "/library") {
      window.dispatchEvent(new CustomEvent("library:openFilter"));
    } else if (typeof window !== "undefined") {
      const globalWindow = window as any;
      if (typeof globalWindow.__openUsageFilters === "function") {
        globalWindow.__openUsageFilters();
      } else {
        window.dispatchEvent(new CustomEvent("usage:filters"));
      }
    }
  };

  return (
    <header
      /* Changed: added bg-opacity (bg-[#0F0F0F]/80) and increased blur to md for a premium feel */
      className="fixed top-0 left-0 right-0 z-30 border-b border-fg/10 backdrop-blur-md bg-nav/80"
      style={{
        paddingTop: "calc(env(safe-area-inset-top) + 12px)",
        paddingBottom: "12px",
      }}
    >
      <div className="relative mx-auto flex w-full max-w-md lg:max-w-none items-center justify-between px-3 lg:px-8 h-10">
        {/* Left side: */}
        <div className="flex items-center gap-1 overflow-hidden h-full">
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
                  aria-label="Go back"
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
            className={cn(
              typography.h1.size,
              "font-bold text-fg tracking-tight truncate leading-none",
              isCenteredTitle && "absolute left-1/2 -translate-x-1/2 w-auto",
            )}
          >
            {title}
          </motion.h1>
        </div>

        <div className="flex items-center justify-end gap-1 shrink-0 min-w-10 h-full">
          {showSearchButton && (
            <button
              onClick={() => navigate("/search")}
              className={cn(
                "flex items-center px-[0.6em] py-[0.3em] justify-center rounded-full",
                "text-fg/70 hover:text-fg hover:bg-fg/10",
                interactive.transition.fast,
                interactive.active.scale,
              )}
              aria-label="Search"
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
              aria-label="Settings"
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
              aria-label="Help"
            >
              <HelpCircle size={20} strokeWidth={2.5} className="text-fg/50" />
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
              aria-label="Add"
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
              aria-label="Open filters"
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
              aria-label="Save"
            >
              {isSaving ? <Loader2 size={14} className="animate-spin" /> : <Check size={14} />}
              <span className="text-xs font-medium">Save</span>
            </button>
          )}
          {rightAction}
        </div>
      </div>
    </header>
  );
}
