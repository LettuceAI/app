import { useMemo, useState, useEffect, useLayoutEffect, useCallback, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { resolveBackTarget } from "../../navigation";
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
  RefreshCw,
} from "lucide-react";
import { motion } from "framer-motion";
import { typography, interactive, cn } from "../../design-tokens";
import { dragRegionAttr } from "./TitleBar";
import { TabItem } from "./NavItem";
import { resolveCreateAction, resolveNavEntries } from "./navDestinations";
import {
  CONTENT_COLUMN_MAX_W_LG,
  CONTENT_COLUMN_ROUTES,
  CONTENT_INNER_MAX_W_LG,
} from "./navPrefs";
import type { NavItemId } from "../../../core/storage/schemas";
import { toast } from "../toast";
import { openDocs } from "../../../core/utils/docs";
import { type TranslationKey, useI18n } from "../../../core/i18n/context";
import { getPlatform } from "../../../core/utils/platform";

interface TopNavProps {
  currentPath: string;
  onBackOverride?: () => void;
  titleOverride?: string;
  rightAction?: React.ReactNode;
  floating?: boolean;
  showNavItems?: boolean;
  onCreateClick?: () => void;
  navItems?: readonly NavItemId[] | null;
}

const appPlatform = getPlatform();
const isDesktop = appPlatform.type === "desktop";
const isMacOS = appPlatform.os === "macos";

export function TopNav({
  currentPath,
  onBackOverride,
  titleOverride,
  rightAction,
  floating = false,
  showNavItems = false,
  onCreateClick,
  navItems,
}: TopNavProps) {
  const navigate = useNavigate();
  const { t } = useI18n();
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
      { match: (p) => p === "/settings/customization", titleKey: "common.nav.accessibility" },
      {
        match: (p) => p === "/settings/speech-recognition",
        titleKey: "common.nav.speechRecognition",
      },
      { match: (p) => p === "/settings/reset", titleKey: "common.nav.reset" },
      { match: (p) => p === "/settings/backup", titleKey: "common.nav.backupRestore" },
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
      {
        match: (p) => p === "/settings/advanced/lorebooks",
        titleKey: "common.nav.lorebooks",
      },
      {
        match: (p) => p === "/settings/advanced/companion-soul-writer",
        titleKey: "common.nav.companionSoulWriter",
      },
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
        match: (p) => p === "/settings/advanced/group-chats",
        titleKey: "common.nav.groupChats",
      },
      {
        match: (p) => p === "/settings/advanced/host-api",
        titleKey: "common.nav.hostApi",
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
      { match: (p) => p === "/settings/image-generation", titleKey: "common.nav.imageGeneration" },
      { match: (p) => p === "/settings/voices", titleKey: "common.nav.voices" },
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

  const alignToContentColumn = useMemo(
    () => CONTENT_COLUMN_ROUTES.includes(basePath),
    [basePath],
  );

  const showLayoutToggle = useMemo(() => {
    return (
      basePath === "/chat" ||
      basePath === "/" ||
      basePath === "/group-chats" ||
      basePath === "/settings/models" ||
      basePath === "/settings/models/browse"
    );
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
      if (basePath === "/settings/models/browse") {
        const mode =
          (window as any).__hfBrowserViewMode || window.localStorage.getItem("hfBrowser:viewMode");
        if (mode) setLayoutViewMode(mode);
        return;
      }
      if (basePath === "/group-chats") {
        const mode = (window as any).__groupChatsViewMode;
        if (mode) setLayoutViewMode(mode);
        return;
      }
      const mode = (window as any).__chatsViewMode;
      if (mode) setLayoutViewMode(mode);
    };
    sync();
    const eventName =
      basePath === "/settings/models"
        ? "models:viewModeChanged"
        : basePath === "/settings/models/browse"
          ? "hfBrowser:viewModeChanged"
          : basePath === "/group-chats"
            ? "groupChats:viewModeChanged"
            : "chats:viewModeChanged";
    window.addEventListener(eventName, sync);
    return () => window.removeEventListener(eventName, sync);
  }, [basePath, showLayoutToggle]);

  const LayoutToggleIcon =
    basePath === "/settings/models"
      ? layoutViewMode === "grid"
        ? LayoutList
        : LayoutGrid
      : basePath === "/settings/models/browse"
        ? layoutViewMode === "list"
          ? LayoutList
          : layoutViewMode === "grid"
            ? LayoutGrid
            : Grid3X3
        : basePath === "/group-chats"
          ? layoutViewMode === "detailed"
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
    if (basePath === "/settings/advanced/lorebooks") return false;
    if (basePath === "/settings/advanced/companion-soul-writer") return false;
    if (basePath === "/library/lorebook/generate") return false;
    if (basePath.includes("/lorebook")) return true;
    return false;
  }, [basePath, hasAdvancedView]);

  const showImportButton = useMemo(
    () =>
      basePath === "/settings/models" ||
      /^\/settings\/characters\/[^/]+\/templates$/.test(basePath),
    [basePath],
  );

  const showRefreshButton = useMemo(() => basePath === "/settings/speech-recognition", [basePath]);

  const showInstalledModelsButton = useMemo(
    () => basePath === "/settings/models/browse",
    [basePath],
  );

  // Map paths to docs keys for contextual help
  const docsKeyForPath = useMemo(() => {
    if (basePath.includes("/lorebook")) return "lorebooks";
    if (basePath === "/settings/providers") return "providers";
    if (basePath === "/settings/models/browse" || basePath === "/settings/models/installed")
      return "modelBrowser";
    if (basePath === "/settings/models" || basePath.startsWith("/settings/models/"))
      return "models";
    if (basePath === "/settings/prompts" || basePath.startsWith("/settings/prompts/"))
      return "systemPrompts";
    if (
      basePath === "/settings/characters" ||
      (basePath.startsWith("/settings/characters/") && basePath.endsWith("/edit"))
    )
      return "characters";
    if (basePath === "/create/character/helper") return "smartCreator";
    if (basePath === "/create/character") return "characters";
    if (basePath === "/create/persona") return "personas";
    if (
      (basePath.startsWith("/personas/") && basePath.endsWith("/edit")) ||
      basePath === "/settings/personas" ||
      (basePath.startsWith("/settings/personas/") && basePath.endsWith("/edit"))
    )
      return "personas";
    if (basePath.startsWith("/settings/customization")) return "accessibility";
    if (basePath === "/settings/sync") return "sync";
    if (basePath === "/settings/security") return "security";
    if (basePath.startsWith("/settings/usage")) return "usage";
    if (basePath === "/settings/backup") return "backupRestore";
    if (basePath === "/settings/image-generation") return "imagegen";
    if (basePath === "/settings/voices") return "textToSpeech";
    if (basePath === "/settings/speech-recognition") return "speechRecognition";
    if (basePath === "/settings/advanced/host-api") return "hostApi";
    if (basePath === "/settings/advanced/help-me-reply") return "helpMeReply";
    if (basePath === "/settings/advanced/companions") return "companionMode";
    if (basePath === "/settings/advanced/creation-helper") return "smartCreator";
    if (basePath === "/settings/advanced/memory") return "memorySystem";
    if (basePath.startsWith("/group-chats")) return "groupChats";
    if (basePath.startsWith("/discover")) return "discovery";
    if (basePath.endsWith("/tree")) return "branching";
    if (basePath.endsWith("/memories")) return "memorySystem";
    return null;
  }, [basePath]);

  const showHelpButton = useMemo(() => docsKeyForPath !== null, [docsKeyForPath]);

  const isCenteredTitle = useMemo(() => {
    return (
      (basePath.startsWith("/settings") &&
        (!basePath.includes("/lorebook") ||
          basePath === "/settings/advanced/lorebooks" ||
          basePath === "/settings/advanced/companion-soul-writer")) ||
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
    () => basePath === "/settings/customization/chat",
    [basePath],
  );
  const isColorCustomizationEdit = useMemo(
    () => basePath === "/settings/customization/colors",
    [basePath],
  );
  const isTemplateEdit = useMemo(
    () => /^\/settings\/characters\/[^/]+\/templates\/[^/]+$/.test(basePath),
    [basePath],
  );
  const isOnboardingModelEditor = useMemo(
    () =>
      (isModelEdit || isModelNew) &&
      (currentPath.includes("returnTo=/onboarding") ||
        currentPath.includes("returnTo=%2Fonboarding")),
    [currentPath, isModelEdit, isModelNew],
  );
  const showSaveButton =
    !isOnboardingModelEditor &&
    (isCharacterEdit ||
      isPersonaEdit ||
      isModelEdit ||
      isModelNew ||
      isPromptEdit ||
      isPromptNew ||
      isChatAppearanceEdit ||
      isColorCustomizationEdit ||
      isTemplateEdit);

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

  const triggerActiveSave = useCallback(() => {
    const globalWindow = window as any;
    if (isCharacterEdit && typeof globalWindow.__saveCharacter === "function") {
      globalWindow.__saveCharacter();
    } else if (isPersonaEdit && typeof globalWindow.__savePersona === "function") {
      globalWindow.__savePersona();
    } else if ((isModelEdit || isModelNew) && typeof globalWindow.__saveModel === "function") {
      globalWindow.__saveModel();
    } else if (isPromptEdit || isPromptNew) {
      window.dispatchEvent(new CustomEvent("prompt:save"));
    } else if (isChatAppearanceEdit && typeof globalWindow.__saveChatAppearance === "function") {
      globalWindow.__saveChatAppearance();
    } else if (
      isColorCustomizationEdit &&
      typeof globalWindow.__saveColorCustomization === "function"
    ) {
      globalWindow.__saveColorCustomization();
    } else if (isTemplateEdit && typeof globalWindow.__saveCharacter === "function") {
      globalWindow.__saveCharacter();
    }
  }, [
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
    const handler = () => triggerActiveSave();
    window.addEventListener("unsaved:save", handler);
    return () => window.removeEventListener("unsaved:save", handler);
  }, [triggerActiveSave]);

  const ensureUnsavedToast = useCallback(() => {
    if (!toast.isVisible("unsaved-changes")) {
      toast.warningSticky(
        t("topNav.unsavedChangesTitle"),
        t("topNav.unsavedChangesMessage"),
        t("common.buttons.discard"),
        () => window.dispatchEvent(new CustomEvent("unsaved:discard")),
        "unsaved-changes",
        {
          label: t("topNav.save"),
          onAction: () => triggerActiveSave(),
        },
      );
    }
  }, [t, triggerActiveSave]);

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
    if (basePath.startsWith("/settings/")) {
      const segments = basePath.split("/").filter(Boolean);
      if (segments.length <= 2) {
        navigate("/settings");
        return;
      }
      const templateEditorMatch = basePath.match(
        /^\/settings\/characters\/([^/]+)\/templates\/.+$/,
      );
      if (templateEditorMatch) {
        navigate(`/settings/characters/${templateEditorMatch[1]}/templates`);
        return;
      }
      const templateListMatch = basePath.match(/^\/settings\/characters\/([^/]+)\/templates$/);
      if (templateListMatch) {
        navigate(`/settings/characters/${templateListMatch[1]}/edit`);
        return;
      }
      const kokoroBlendMatch = basePath.match(/^\/settings\/voices\/kokoro\/([^/]+)\/blend/);
      if (kokoroBlendMatch) {
        navigate(`/settings/voices/kokoro/${kokoroBlendMatch[1]}`);
        return;
      }
      if (/^\/settings\/voices\/kokoro\/[^/]+$/.test(basePath)) {
        navigate("/settings/providers?tab=audio");
        return;
      }
      const mapped = resolveBackTarget(currentPath);
      if (mapped && mapped.startsWith("/settings")) {
        navigate(mapped);
        return;
      }
      segments.pop();
      navigate("/" + segments.join("/"));
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

  const handleRefreshClick = () => {
    if (basePath === "/settings/speech-recognition") {
      window.dispatchEvent(new CustomEvent("asr:refresh"));
    }
  };

  const headerRef = useRef<HTMLElement | null>(null);
  useLayoutEffect(() => {
    const el = headerRef.current;
    if (!el) return;
    const gap = floating ? 12 : 0;
    const publish = () => {
      document.documentElement.style.setProperty("--topnav-h", `${el.offsetHeight + gap}px`);
    };
    publish();
    const ro = new ResizeObserver(publish);
    ro.observe(el);
    return () => {
      ro.disconnect();
      document.documentElement.style.setProperty("--topnav-h", "0px");
    };
  }, [floating]);

  return (
    <header
      ref={headerRef}
      className={
        floating
          ? cn(
            "fixed left-[calc(var(--appnav-top-w,0px)+2rem)] right-[calc(var(--appnav-top-wr,0px)+2rem)]",
            "lg:left-[calc(var(--appnav-top-w,0px)+3rem)] lg:right-[calc(var(--appnav-top-wr,0px)+3rem)] lg:mx-auto",
            CONTENT_INNER_MAX_W_LG,
            "top-[calc(var(--titlebar-h,0px)+12px)] z-40 rounded-full border border-fg/10 shadow-[0_12px_32px_rgba(0,0,0,0.35)] backdrop-blur-md bg-nav/90",
          )
          : "fixed left-[var(--appnav-top-w,0px)] right-[var(--appnav-top-wr,0px)] top-[var(--titlebar-h,0px)] z-40 border-b border-fg/10 backdrop-blur-md bg-nav/80 lg:px-4"
      }
      style={{
        paddingTop: isDesktop ? "8px" : "calc(env(safe-area-inset-top) + 12px)",
        paddingBottom: isDesktop ? "8px" : "12px",
      }}
      {...dragRegionAttr}
    >
      <div
        className={cn(
          "relative mx-auto flex h-10 w-full max-w-md items-center justify-between px-3",
          floating ? "lg:px-6" : "lg:px-8",
          !floating && alignToContentColumn ? CONTENT_COLUMN_MAX_W_LG : "lg:max-w-none",
        )}
        style={isMacOS ? { paddingLeft: "72px" } : undefined}
        {...dragRegionAttr}
      >
        {showNavItems && (
          <div className="absolute left-1/2 top-1/2 z-10 flex -translate-x-1/2 -translate-y-1/2 items-center gap-2">
            {resolveNavEntries(navItems).map((entry, index) =>
              entry.kind === "create" ? (
                <button
                  key={`create-${index}`}
                  onClick={() => resolveCreateAction(basePath, () => onCreateClick?.())}
                  data-tour-id="nav-create"
                  className="mx-0.5 flex h-9 w-10 items-center justify-center rounded-xl border border-fg/15 bg-fg/10 text-fg transition hover:border-fg/25 hover:bg-fg/20"
                  aria-label={t("common.bottomNav.create")}
                >
                  <Plus size={18} />
                </button>
              ) : (
                <TabItem
                  key={entry.destination.id}
                  to={entry.destination.to}
                  icon={entry.destination.icon}
                  label={t(entry.destination.labelKey)}
                  active={entry.destination.isActive(basePath)}
                  className="h-9 w-11"
                  dataTourId={entry.destination.dataTourId}
                  layoutId="activeTabHeader"
                  rounded="rounded-xl"
                  iconSize={20}
                />
              ),
            )}
          </div>
        )}
        {/* Left side: */}
        <div className="flex items-center gap-1 overflow-hidden h-full" {...dragRegionAttr}>
          {showBackButton && (
            <div className="flex w-10 shrink-0 items-center justify-center">
              <button
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
              </button>
            </div>
          )}

          <motion.h1
            key={title}
            initial={{ opacity: 0, y: 5 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.3, ease: "easeOut" }}
            {...dragRegionAttr}
            className={cn(
              typography.h1.size,
              "font-bold text-fg tracking-tight truncate leading-none",
              isCenteredTitle && "absolute left-1/2 -translate-x-1/2 w-auto",
            )}
          >
            {title}
          </motion.h1>
        </div>

        <div
          className="flex items-center justify-end gap-1 shrink-0 min-w-10 h-full"
          {...dragRegionAttr}
        >
          {showLayoutToggle && (
            <button
              onClick={() =>
                window.dispatchEvent(
                  new CustomEvent(
                    basePath === "/settings/models"
                      ? "models:cycleViewMode"
                      : basePath === "/settings/models/browse"
                        ? "hfBrowser:cycleViewMode"
                        : basePath === "/group-chats"
                          ? "groupChats:cycleViewMode"
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
              data-tour-id="top-search"
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
          {showSettingsButton && (
            <button
              onClick={() => navigate("/settings")}
              data-tour-id="top-settings"
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
          {showInstalledModelsButton && (
            <button
              onClick={() => navigate("/settings/models/installed")}
              className={cn(
                "flex items-center px-[0.6em] py-[0.3em] justify-center rounded-full",
                "text-fg/70 hover:text-fg hover:bg-fg/10",
                interactive.transition.fast,
                interactive.active.scale,
              )}
              aria-label={t("topNav.extra.installedModels")}
              title={t("topNav.extra.installedModels")}
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
          {showRefreshButton && (
            <button
              data-tour-id="asr-refresh"
              onClick={handleRefreshClick}
              className={cn(
                "flex items-center px-[0.6em] py-[0.3em] justify-center rounded-full",
                "text-fg/70 hover:text-fg hover:bg-fg/10",
                interactive.transition.fast,
                interactive.active.scale,
              )}
              aria-label={t("topNav.extra.refresh")}
            >
              <RefreshCw size={18} strokeWidth={2.4} className="text-fg" />
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
              onClick={() => triggerActiveSave()}
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
        </div>
      </div>
    </header>
  );
}
