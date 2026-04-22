import { useMemo, useState, useEffect, useCallback } from "react";
import {
  ArrowLeft,
  MessageSquarePlus,
  Cpu,
  ChevronRight,
  History,
  User,
  SlidersHorizontal,
  Edit2,
  Trash2,
  Sparkles,
  TriangleAlert,
  Upload,
  NotebookPen,
} from "lucide-react";
import { useNavigate, useParams } from "react-router-dom";
import { motion } from "framer-motion";
import type {
  AdvancedModelSettings,
  Character,
  Model,
  Persona,
  Session,
} from "../../../core/storage/schemas";
import { createDefaultAdvancedModelSettings } from "../../../core/storage/schemas";
import {
  readSettings,
  saveCharacter,
  createSession,
  listPersonas,
  getSessionMeta,
  saveSession,
  deletePersona,
  getSessionMessageCount,
} from "../../../core/storage/repo";
import { BottomMenu, MenuSection } from "../../components";
import { ModelSelectionBottomMenu } from "../../components/ModelSelectionBottomMenu";
import { SessionAdvancedSettings } from "./components/SessionAdvancedSettings";
import { ProviderParameterSupportInfo } from "../../components/ProviderParameterSupportInfo";
import { AvatarImage } from "../../components/AvatarImage";
import { Switch } from "../../components/Switch";
import { useAvatar } from "../../hooks/useAvatar";
import { useChatLayoutContext } from "./ChatLayout";
import {
  formatAdvancedModelSettingsSummary,
  sanitizeAdvancedModelSettings,
} from "../../components/AdvancedModelSettingsForm";
import { typography, radius, spacing, interactive, cn, colors } from "../../design-tokens";
import { WindowControlButtons, useDragRegionProps, hasCustomWindowControls } from "../../components/App/TopNav";
import { Routes, useNavigationManager } from "../../navigation";
import { PersonaSelector } from "../group-chats/components/settings";
import { storageBridge } from "../../../core/storage/files";
import { ChatTemplateSelector } from "./components/ChatTemplateSelector";
import { AuthorNoteBottomMenu } from "./components/AuthorNoteBottomMenu";
import { useI18n } from "../../../core/i18n/context";
import { isRenderableImageUrl } from "../../../core/utils/image";

function isImageLike(value?: string) {
  return isRenderableImageUrl(value);
}

interface SettingsButtonProps {
  icon: React.ReactNode;
  title: string;
  subtitle: string;
  onClick: () => void;
  disabled?: boolean;
}

function SettingsButton({ icon, title, subtitle, onClick, disabled = false }: SettingsButtonProps) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "group flex w-full min-h-14 items-center justify-between",
        radius.md,
        "border p-4 text-left",
        interactive.transition.default,
        interactive.active.scale,
        disabled
          ? "border-fg/6 bg-surface-el/60 opacity-50 cursor-not-allowed"
          : "border-fg/10 bg-surface-el text-fg hover:border-fg/20 hover:bg-fg/6",
      )}
    >
      <div className="flex items-center gap-3 min-w-0">
        <div
          className={cn(
            "flex h-10 w-10 items-center justify-center",
            radius.full,
            "border border-fg/15 bg-fg/8 text-fg/80",
          )}
        >
          {icon}
        </div>
        <div className="min-w-0 flex-1">
          <div
            className={cn(
              typography.overline.size,
              typography.overline.weight,
              typography.overline.tracking,
              typography.overline.transform,
              "text-fg/50",
            )}
          >
            {title}
          </div>
          <div className={cn(typography.bodySmall.size, "text-fg truncate")}>{subtitle}</div>
        </div>
      </div>
      <ChevronRight className="h-4 w-4 text-fg/40 transition-colors group-hover:text-fg/80" />
    </button>
  );
}

function SectionHeader({ title, subtitle }: { title: string; subtitle?: string }) {
  return (
    <div className="flex items-end justify-between gap-3">
      <div className="min-w-0">
        <h2 className={cn(typography.h2.size, typography.h2.weight, "text-fg truncate")}>
          {title}
        </h2>
        {subtitle ? (
          <p className={cn(typography.bodySmall.size, "text-fg/50 mt-0.5 truncate")}>{subtitle}</p>
        ) : null}
      </div>
    </div>
  );
}

function QuickChip({
  icon,
  label,
  value,
  onClick,
  disabled = false,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "group flex w-full min-h-14 items-center justify-between",
        radius.md,
        "border p-4 text-left",
        interactive.transition.default,
        interactive.active.scale,
        disabled
          ? "border-fg/6 bg-surface-el/60 opacity-50 cursor-not-allowed"
          : "border-fg/10 bg-surface-el hover:border-fg/20 hover:bg-fg/6",
      )}
    >
      <div className="flex items-center gap-3 min-w-0">
        <div
          className={cn(
            "flex h-10 w-10 items-center justify-center",
            radius.full,
            "border border-fg/15 bg-fg/8 text-fg/80",
          )}
        >
          {icon}
        </div>
        <div className="min-w-0 flex-1">
          <div
            className={cn(
              typography.overline.size,
              typography.overline.weight,
              typography.overline.tracking,
              typography.overline.transform,
              "text-fg/50",
            )}
          >
            {label}
          </div>
          <div className={cn(typography.bodySmall.size, "text-fg truncate")}>{value}</div>
        </div>
      </div>
      <ChevronRight className="h-4 w-4 text-fg/40 transition-colors group-hover:text-fg/80" />
    </button>
  );
}
/*
interface ModelOptionProps {
  model: Model;
  isSelected: boolean;
  isGlobalDefault: boolean;
  isCharacterDefault: boolean;
  onClick: () => void;
}

function ModelOption({
  model,
  isSelected,
  isGlobalDefault,
  isCharacterDefault,
  onClick,
}: ModelOptionProps) {
  const defaultBadge = isCharacterDefault
    ? {
        label: "Character default",
        color: "text-emerald-200 border-emerald-400/40 bg-emerald-400/10",
      }
    : isGlobalDefault
      ? { label: "App default", color: "text-blue-200 border-blue-400/30 bg-blue-400/10" }
      : null;

  return (
    <button
      onClick={onClick}
      className={cn(
        "group relative flex w-full items-center justify-between gap-3",
        radius.lg,
        "p-4 text-left",
        interactive.transition.default,
        interactive.active.scale,
        isSelected
          ? "border border-emerald-400/40 bg-emerald-400/15 ring-2 ring-emerald-400/30 text-emerald-100"
          : "border border-white/10 bg-white/5 text-white hover:border-white/20 hover:bg-white/10",
      )}
      aria-pressed={isSelected}
    >
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <div className={cn(typography.body.size, typography.h3.weight, "truncate", "py-0.5")}>
            {model.displayName}
          </div>
          {defaultBadge && (
            <span
              className={cn(
                "shrink-0 rounded-full border px-2 text-[10px] font-medium",
                defaultBadge.color,
              )}
            >
              {defaultBadge.label}
            </span>
          )}
        </div>
        <div className={cn(typography.caption.size, "mt-1 truncate text-gray-400")}>
          {model.name}
        </div>
      </div>

      <div
        className={cn(
          "flex h-8 w-8 items-center justify-center rounded-full",
          "border", // always have border to keep size
          isSelected
            ? "bg-emerald-500/20 border-emerald-400/50 text-emerald-300"
            : "bg-white/5 border-white/10 text-white/70 group-hover:border-white/20",
        )}
        aria-hidden="true"
      >
        {isSelected ? <Check className="h-4 w-4" /> : <span className="h-4 w-4" />}
      </div>
    </button>
  );
}*/

export function ChatSettingsContent({
  character,
  mode = "page",
  onClose,
  onOpenAuthorNote,
}: {
  character: Character;
  mode?: "page" | "drawer";
  onClose?: () => void;
  onOpenAuthorNote?: () => void;
}) {
  const navigate = useNavigate();
  const { backOrReplace } = useNavigationManager();
  const { t } = useI18n();
  const { characterId } = useParams();
  const dragRegionProps = useDragRegionProps();
  const [models, setModels] = useState<Model[]>([]);
  const [globalDefaultModelId, setGlobalDefaultModelId] = useState<string | null>(null);
  const [currentCharacter, setCurrentCharacter] = useState<Character>(character);
  const avatarUrl = useAvatar(
    "character",
    currentCharacter?.id,
    currentCharacter?.avatarPath,
    "round",
  );
  const { backgroundImageData, reloadCharacter } = useChatLayoutContext();
  const [showModelSelector, setShowModelSelector] = useState(false);
  const [modelSelectorTarget, setModelSelectorTarget] = useState<"primary" | "fallback">("primary");
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [currentSession, setCurrentSession] = useState<Session | null>(null);
  const [showPersonaSelector, setShowPersonaSelector] = useState(false);
  const [sessionAdvancedSettings, setSessionAdvancedSettings] =
    useState<AdvancedModelSettings | null>(null);
  const [showSessionAdvancedMenu, setShowSessionAdvancedMenu] = useState(false);
  const [showParameterSupport, setShowParameterSupport] = useState(false);
  const [showChatpkgImportMenu, setShowChatpkgImportMenu] = useState(false);
  const [sessionAdvancedDraft, setSessionAdvancedDraft] = useState<AdvancedModelSettings>(
    createDefaultAdvancedModelSettings(),
  );
  const [sessionOverrideEnabled, setSessionOverrideEnabled] = useState<boolean>(false);
  const [showPersonaActions, setShowPersonaActions] = useState(false);
  const [showTemplateSelector, setShowTemplateSelector] = useState(false);
  const [showAuthorNoteMenu, setShowAuthorNoteMenu] = useState(false);
  const [selectedPersonaForActions, setSelectedPersonaForActions] = useState<Persona | null>(null);
  const [messageCount, setMessageCount] = useState<number>(0);
  const [pendingChatpkgImport, setPendingChatpkgImport] = useState<{
    path: string;
    info: any;
  } | null>(null);
  const [importingChatpkg, setImportingChatpkg] = useState(false);
  const personaForAvatar = useMemo(() => {
    if (!currentSession) return null;
    if (currentSession.personaDisabled || currentSession.personaId === "") return null;
    if (currentSession.personaId) {
      return personas.find((p) => p.id === currentSession.personaId) ?? null;
    }
    return personas.find((p) => p.isDefault) ?? null;
  }, [currentSession, personas]);
  const personaAvatarUrl = useAvatar(
    "persona",
    personaForAvatar?.id ?? "",
    personaForAvatar?.avatarPath,
    "round",
  );

  const loadModels = useCallback(async () => {
    try {
      const settings = await readSettings();
      setModels(settings.models);
      setGlobalDefaultModelId(settings.defaultModelId);
    } catch (error) {
      console.error("Failed to load models/settings:", error);
    }
  }, []);

  const loadPersonas = useCallback(async () => {
    const personaList = await listPersonas();
    setPersonas(personaList);
  }, []);

  const loadSession = useCallback(async () => {
    if (!characterId) return;
    const urlParams = new URLSearchParams(window.location.search);
    const sessionId = urlParams.get("sessionId");
    if (sessionId) {
      try {
        const session = await getSessionMeta(sessionId);
        setCurrentSession(session);
        const sessionAdvanced = session?.advancedModelSettings ?? null;
        setSessionAdvancedSettings(sessionAdvanced);

        try {
          const count = await getSessionMessageCount(sessionId);
          setMessageCount(count);
        } catch (e) {
          console.warn("Failed to load message count", e);
          setMessageCount(0);
        }
      } catch (error) {
        console.error("Failed to load session:", error);
        setCurrentSession(null);
        setSessionAdvancedSettings(null);
      }
    } else {
      setCurrentSession(null);
      setSessionAdvancedSettings(null);
    }
  }, [characterId]);

  useEffect(() => {
    loadModels();
    loadPersonas();
    loadSession();
  }, [loadModels, loadPersonas, loadSession]);

  useEffect(() => {
    setCurrentCharacter(character);
  }, [character]);

  const getEffectiveModelId = useCallback(() => {
    return currentCharacter?.defaultModelId || globalDefaultModelId || null;
  }, [currentCharacter?.defaultModelId, globalDefaultModelId]);

  const selectedModelId = currentCharacter?.defaultModelId ?? null;
  const selectedFallbackModelId = currentCharacter?.fallbackModelId ?? null;
  const effectiveModelId = getEffectiveModelId();
  const currentModel = useMemo(
    () => models.find((m) => m.id === effectiveModelId),
    [models, effectiveModelId],
  );

  const baseAdvancedSettings = useMemo(() => {
    return currentModel?.advancedModelSettings ?? createDefaultAdvancedModelSettings();
  }, [currentModel?.advancedModelSettings]);

  useEffect(() => {
    setSessionAdvancedSettings(currentSession?.advancedModelSettings ?? null);
  }, [currentSession]);

  useEffect(() => {
    if (sessionAdvancedSettings) {
      setSessionAdvancedDraft(sessionAdvancedSettings);
      setSessionOverrideEnabled(true);
    } else {
      setSessionAdvancedDraft(baseAdvancedSettings);
      setSessionOverrideEnabled(false);
    }
  }, [sessionAdvancedSettings, baseAdvancedSettings]);

  const handleNewChat = async () => {
    if (!characterId || !currentCharacter) return;

    // If character has templates, show selector
    if (currentCharacter.chatTemplates && currentCharacter.chatTemplates.length > 0) {
      setShowTemplateSelector(true);
      return;
    }

    try {
      const session = await createSession(
        characterId,
        "New Chat",
        currentCharacter.defaultSceneId ?? currentCharacter.scenes?.[0]?.id,
      );
      navigate(`/chat/${characterId}?sessionId=${session.id}`, { replace: true });
    } catch (error) {
      console.error("Failed to create new chat:", error);
    }
  };

  const handleTemplateSelected = async (templateId: string | null) => {
    if (!characterId || !currentCharacter) return;
    setShowTemplateSelector(false);
    try {
      const sceneId = templateId
        ? undefined
        : (currentCharacter.defaultSceneId ?? currentCharacter.scenes?.[0]?.id);
      const session = await createSession(
        characterId,
        "New Chat",
        sceneId,
        templateId ?? undefined,
      );
      navigate(`/chat/${characterId}?sessionId=${session.id}`, { replace: true });
    } catch (error) {
      console.error("Failed to create new chat:", error);
    }
  };

  const handleChangeModel = async (modelId: string | null) => {
    if (!characterId) return;

    try {
      const updatedCharacter = await saveCharacter({
        ...currentCharacter,
        defaultModelId: modelId,
      });
      setCurrentCharacter(updatedCharacter);
      reloadCharacter();
    } catch (error) {
      console.error("Failed to change character model:", error);
    }
  };

  const handleChangeFallbackModel = async (modelId: string | null) => {
    if (!characterId) return;

    try {
      const updatedCharacter = await saveCharacter({
        ...currentCharacter,
        fallbackModelId: modelId,
      });
      setCurrentCharacter(updatedCharacter);
      reloadCharacter();
    } catch (error) {
      console.error("Failed to change fallback model:", error);
    }
  };

  const handleChangePersona = async (personaId: string | null) => {
    if (!currentSession || !character) {
      console.log("No current session or character");
      return;
    }

    try {
      console.log("Changing persona to:", personaId);

      const disablePersona = personaId === null;
      const updatedSession = {
        ...currentSession,
        personaId: disablePersona ? null : personaId,
        personaDisabled: disablePersona,
        updatedAt: Date.now(),
      };

      console.log("Updated session:", updatedSession);
      await saveSession(updatedSession);
      console.log("Session saved successfully");
      setCurrentSession(updatedSession);
      setShowPersonaSelector(false);

      if (characterId && currentSession.id) {
        navigate(Routes.chatSession(characterId, currentSession.id), { replace: true });
      }
    } catch (error) {
      console.error("Failed to change persona:", error);
    }
  };

  const handleSaveSessionAdvancedSettings = useCallback(
    async (next: AdvancedModelSettings | null) => {
      if (!currentSession) {
        console.warn("Attempted to save session advanced settings without session");
        return;
      }

      try {
        const sanitized = next ? sanitizeAdvancedModelSettings(next) : null;
        const updatedSession: Session = {
          ...currentSession,
          advancedModelSettings: sanitized ?? undefined,
          updatedAt: Date.now(),
        };
        await saveSession(updatedSession);
        setCurrentSession(updatedSession);
        setSessionAdvancedSettings(sanitized);
        setShowSessionAdvancedMenu(false);
      } catch (error) {
        console.error("Failed to save session advanced settings:", error);
      }
    },
    [currentSession],
  );

  const handleToggleSessionVoiceAutoplay = useCallback(async () => {
    if (!currentSession) {
      return;
    }
    const fallback = currentCharacter?.voiceAutoplay ?? false;
    const currentValue = currentSession.voiceAutoplay ?? fallback;
    const updatedSession: Session = {
      ...currentSession,
      voiceAutoplay: !currentValue,
      updatedAt: Date.now(),
    };
    try {
      await saveSession(updatedSession);
      setCurrentSession(updatedSession);
    } catch (error) {
      console.error("Failed to update session voice autoplay:", error);
    }
  }, [currentCharacter?.voiceAutoplay, currentSession]);

  const handleResetSessionVoiceAutoplay = useCallback(async () => {
    if (!currentSession) {
      return;
    }
    const updatedSession: Session = {
      ...currentSession,
      voiceAutoplay: undefined,
      updatedAt: Date.now(),
    };
    try {
      await saveSession(updatedSession);
      setCurrentSession(updatedSession);
    } catch (error) {
      console.error("Failed to reset session voice autoplay:", error);
    }
  }, [currentSession]);

  const handleViewHistory = useCallback(() => {
    if (!characterId) return;
    const base = Routes.chatHistory(characterId);
    if (currentSession?.id) {
      navigate(`${base}?sessionId=${encodeURIComponent(currentSession.id)}`);
      return;
    }
    navigate(base);
  }, [characterId, currentSession?.id, navigate]);

  const handleOpenAuthorNote = useCallback(() => {
    if (onOpenAuthorNote) {
      onOpenAuthorNote();
      return;
    }
    setShowAuthorNoteMenu(true);
  }, [onOpenAuthorNote]);

  const handleOpenImportChatpkg = useCallback(async () => {
    if (!characterId) return;
    try {
      const picked = await storageBridge.chatpkgPickFile();
      if (!picked) return;
      const info = await storageBridge.chatpkgInspect(picked.path);
      if (info?.type !== "single_chat") {
        alert("This package is not a single chat package.");
        return;
      }
      setPendingChatpkgImport({ path: picked.path, info });
      setShowChatpkgImportMenu(true);
    } catch (error) {
      console.error("Failed to inspect chat package:", error);
      alert(typeof error === "string" ? error : "Failed to inspect chat package");
    }
  }, [characterId]);

  const handleImportChatpkg = useCallback(async () => {
    if (!characterId || !pendingChatpkgImport) return;
    try {
      setImportingChatpkg(true);
      const result = await storageBridge.chatpkgImport(pendingChatpkgImport.path, {
        targetCharacterId: characterId,
      });
      setShowChatpkgImportMenu(false);
      setPendingChatpkgImport(null);
      const importedSessionId = result?.sessionId;
      if (typeof importedSessionId === "string" && importedSessionId.length > 0) {
        navigate(Routes.chatSession(characterId, importedSessionId), { replace: true });
      }
    } catch (error) {
      console.error("Failed to import chat package:", error);
      alert(typeof error === "string" ? error : "Failed to import chat package");
    } finally {
      setImportingChatpkg(false);
    }
  }, [characterId, navigate, pendingChatpkgImport]);

  const avatarDisplay = useMemo(() => {
    if (avatarUrl && isImageLike(avatarUrl)) {
      return (
        <div className="h-12 w-12 overflow-hidden rounded-full">
          <AvatarImage
            src={avatarUrl}
            alt={currentCharacter?.name ?? "avatar"}
            crop={currentCharacter?.avatarCrop}
            applyCrop
          />
        </div>
      );
    }
    const initials = currentCharacter?.name ? currentCharacter.name.slice(0, 2).toUpperCase() : "?";
    return (
      <div className="flex h-12 w-12 items-center justify-center rounded-full border border-white/10 bg-white/10 text-sm font-semibold text-white">
        {initials}
      </div>
    );
  }, [currentCharacter, avatarUrl]);

  const advancedDefaultsLabel = useMemo(() => {
    return currentModel?.advancedModelSettings ? "Model defaults" : "App defaults";
  }, [currentModel?.advancedModelSettings]);

  const effectiveVoiceAutoplay = useMemo(() => {
    return currentSession?.voiceAutoplay ?? currentCharacter?.voiceAutoplay ?? false;
  }, [currentCharacter?.voiceAutoplay, currentSession?.voiceAutoplay]);

  const sessionAdvancedSummary = useMemo(() => {
    if (!currentSession) {
      return "Open a chat session first";
    }
    if (!sessionAdvancedSettings) {
      return `${advancedDefaultsLabel}: ${formatAdvancedModelSettingsSummary(baseAdvancedSettings, "Default settings")}`;
    }
    return `Overrides: ${formatAdvancedModelSettingsSummary(sessionAdvancedSettings, "Overrides active")}`;
  }, [currentSession, sessionAdvancedSettings, baseAdvancedSettings, advancedDefaultsLabel]);

  const sessionAdvancedOverrideCount = useMemo(() => {
    if (!currentSession || !sessionAdvancedSettings) return 0;
    const keys: (keyof AdvancedModelSettings)[] = [
      "temperature",
      "topP",
      "topK",
      "maxOutputTokens",
      "contextLength",
      "frequencyPenalty",
      "presencePenalty",
    ];
    let count = 0;
    for (const key of keys) {
      const overrideValue = sessionAdvancedSettings[key];
      if (overrideValue === null || overrideValue === undefined) continue;
      const baseValue = baseAdvancedSettings?.[key];
      if (baseValue === null || baseValue === undefined) {
        count += 1;
        continue;
      }
      if (typeof overrideValue === "number" && typeof baseValue === "number") {
        if (Math.abs(overrideValue - baseValue) > 1e-9) count += 1;
      } else {
        count += 1;
      }
    }
    return count;
  }, [currentSession, sessionAdvancedSettings, baseAdvancedSettings]);

  const isDynamic = useMemo(() => {
    return currentCharacter?.memoryType === "dynamic";
  }, [currentCharacter?.memoryType]);

  const memorySummaryPreview = useMemo(() => {
    if (!currentSession) return "Open a chat session to view memory";
    if (!isDynamic) {
      const memoryCount = currentSession.memories?.length ?? 0;
      if (memoryCount > 0) return "Manual memories available for this session";
      return "No memories yet — add manual memories from the Memories page";
    }
    const summary = (currentSession.memorySummary ?? "").trim();
    if (summary) return summary;
    const memoryCount =
      currentSession.memoryEmbeddings?.length ?? currentSession.memories?.length ?? 0;
    if (memoryCount > 0) return "No summary yet — memories exist for this session";
    return "No memories yet — open to add summary, tags, and history";
  }, [currentSession, isDynamic]);

  const memoryMetaLine = useMemo(() => {
    if (!currentSession) return "Session required";
    const memoryCount =
      (isDynamic ? currentSession.memoryEmbeddings?.length : currentSession.memories?.length) ?? 0;
    const toolsCount = isDynamic ? (currentSession.memoryToolEvents?.length ?? 0) : 0;
    const tokenCount = isDynamic ? (currentSession.memorySummaryTokenCount ?? 0) : 0;
    const parts: string[] = [];
    parts.push(`${memoryCount.toLocaleString()} items`);
    if (toolsCount > 0) parts.push(`${toolsCount.toLocaleString()} tool events`);
    if (tokenCount > 0) parts.push(`${tokenCount.toLocaleString()} summary tokens`);
    return parts.join(" • ");
  }, [currentSession, isDynamic]);

  const handleBack = () => {
    if (mode === "drawer" && onClose) {
      onClose();
      return;
    }
    if (characterId) {
      const urlParams = new URLSearchParams(window.location.search);
      const sessionId = urlParams.get("sessionId");
      backOrReplace(Routes.chatSession(characterId, sessionId));
    } else {
      backOrReplace(Routes.chat);
    }
  };

  const getCurrentPersonaDisplay = () => {
    if (!currentSession) return "Open a chat session first";

    if (currentSession.personaDisabled || currentSession.personaId === "") return "No persona";
    const currentPersonaId = currentSession?.personaId;
    if (!currentPersonaId) {
      const defaultPersona = personas.find((p) => p.isDefault);
      if (!defaultPersona) return "No persona";
      return defaultPersona.nickname
        ? `${defaultPersona.title} (${defaultPersona.nickname}) (default)`
        : `${defaultPersona.title} (default)`;
    }
    const persona = personas.find((p) => p.id === currentPersonaId);
    if (!persona) return "Custom persona";
    return persona.nickname ? `${persona.title} (${persona.nickname})` : persona.title;
  };

  const selectedPersonaId = useMemo(() => {
    if (!currentSession) return undefined;
    if (currentSession.personaDisabled || currentSession.personaId === "") return "";
    if (currentSession.personaId) return currentSession.personaId;
    const defaultPersona = personas.find((p) => p.isDefault);
    return defaultPersona?.id;
  }, [currentSession, personas]);

  const getModelDisplay = () => {
    if (!currentModel) return "No model available";
    return currentModel.displayName + (!currentCharacter?.defaultModelId ? " (app default)" : "");
  };

  const getFallbackModelDisplay = () => {
    if (!selectedFallbackModelId) return "None";
    const fallback = models.find((m) => m.id === selectedFallbackModelId);
    return fallback?.displayName || fallback?.name || "Unknown model";
  };

  const isDrawer = mode === "drawer";

  return (
    <div
      className={cn(
        "relative flex h-full flex-col",
        colors.text.primary,
        !isDrawer && !backgroundImageData && "bg-surface",
        isDrawer && "bg-surface",
      )}
    >
      {/* Scrim overlay on top of shared background (page mode only) */}
      {!isDrawer && backgroundImageData && (
        <div className="pointer-events-none fixed inset-0 z-0 bg-black/40" aria-hidden="true" />
      )}
      {/* Header */}
      {!isDrawer && (
        <header
          className={cn(
            "z-20 shrink-0 border-b border-fg/10 pl-3 lg:pl-8",
            hasCustomWindowControls ? "pr-0" : "pr-3 lg:pr-8",
            !backgroundImageData ? "bg-surface" : "",
          )}
          style={{
            paddingTop: "calc(env(safe-area-inset-top) + 12px)",
            paddingBottom: "12px",
          }}
          {...dragRegionProps}
        >
          <div className="flex h-10 items-center justify-between" {...dragRegionProps}>
            <div className="flex items-center min-w-0">
              <button
                onClick={handleBack}
                className="flex shrink-0 items-center justify-center -ml-2 px-[0.6em] py-[0.3em] text-fg transition hover:text-fg/80"
                aria-label="Back to chat"
              >
                <ArrowLeft size={18} strokeWidth={2.5} />
              </button>
              <div className="min-w-0 text-left">
                <p className="truncate text-xl font-bold text-fg/90">Chat Settings</p>
                <p className="mt-0.5 truncate text-xs text-fg/50">Manage conversation preferences</p>
              </div>
            </div>
            <WindowControlButtons />
          </div>
        </header>
      )}

      {/* Content */}
      <main className={cn("relative z-10 flex-1 overflow-y-auto px-3 pt-4 pb-16", !isDrawer && "")}>
        <motion.div
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3, ease: "easeOut" }}
          className={spacing.section}
        >
          {/* Session Header */}
          <section
            className={cn(radius.lg, "border border-fg/10 bg-surface-el/90 p-4 backdrop-blur-sm")}
          >
            <div className="flex items-center gap-3">
              {avatarDisplay}
              <div className="min-w-0 flex-1">
                <h3 className={cn(typography.body.size, typography.h3.weight, "text-fg")}>
                  {character.name}
                </h3>
                {currentSession ? (
                  <p className={cn(typography.caption.size, "text-fg/55 mt-1 truncate")}>
                    Session: {currentSession.title || "Untitled"}
                    <span className="opacity-50 mx-1.5">•</span>
                    {messageCount} messages
                  </p>
                ) : null}
                {currentCharacter?.description || currentCharacter?.definition ? (
                  <p
                    className={cn(
                      typography.caption.size,
                      "text-fg/55 leading-relaxed line-clamp-2 mt-1",
                    )}
                  >
                    {currentCharacter.description || currentCharacter.definition}
                  </p>
                ) : null}
              </div>
            </div>
          </section>

          {/* Memory (Primary) */}
          <section className={spacing.item}>
            <SectionHeader
              title={t("chats.settings.memorySection")}
              subtitle={t("chats.settings.memorySectionDesc")}
            />
            <button
              onClick={() => {
                if (!characterId) return;
                if (!currentSession) return;
                navigate(Routes.chatMemories(characterId, currentSession.id));
              }}
              disabled={!currentSession}
              className={cn(
                "group w-full text-left",
                radius.lg,
                "border p-4",
                interactive.transition.default,
                interactive.active.scale,
                !currentSession
                  ? "border-fg/6 bg-surface-el/60 opacity-50 cursor-not-allowed"
                  : "border-accent/25 bg-surface-el hover:border-accent/40",
              )}
            >
              <div className="flex items-start justify-between gap-3">
                <div className="flex items-center gap-3 min-w-0">
                  <div
                    className={cn(
                      "flex h-10 w-10 items-center justify-center",
                      radius.full,
                      "border border-accent/30 bg-accent/15 text-accent",
                    )}
                  >
                    <Sparkles className="h-4 w-4" />
                  </div>
                  <div className="min-w-0">
                    <div
                      className={cn(
                        typography.overline.size,
                        typography.overline.weight,
                        typography.overline.tracking,
                        typography.overline.transform,
                        "text-fg/50",
                      )}
                    >
                      Memory
                    </div>
                    <div className={cn(typography.bodySmall.size, "text-fg truncate")}>
                      {memoryMetaLine}
                    </div>
                  </div>
                </div>
                <ChevronRight className="mt-1 h-4 w-4 text-fg/40 transition-colors group-hover:text-fg/80" />
              </div>
              <p
                className={cn(
                  typography.bodySmall.size,
                  "mt-3 text-fg/70 leading-relaxed line-clamp-3",
                )}
              >
                {memorySummaryPreview}
              </p>
            </button>
          </section>

          {/* Quick Settings */}
          <section className={spacing.item}>
            <SectionHeader
              title={t("chats.settings.quickSettings")}
              subtitle={t("chats.settings.quickSettingsDesc")}
            />
            <div className="grid grid-cols-1 gap-2">
              <QuickChip
                icon={
                  personaAvatarUrl ? (
                    <div className="h-full w-full overflow-hidden rounded-full">
                      <AvatarImage
                        src={personaAvatarUrl}
                        alt={personaForAvatar?.title ?? "Persona"}
                        crop={personaForAvatar?.avatarCrop}
                        applyCrop
                      />
                    </div>
                  ) : (
                    <User className="h-4 w-4" />
                  )
                }
                label={t("chats.settings.persona")}
                value={getCurrentPersonaDisplay()}
                onClick={() => setShowPersonaSelector(true)}
                disabled={!currentSession}
              />
              <QuickChip
                icon={<Cpu className="h-4 w-4" />}
                label={t("chats.settings.model")}
                value={getModelDisplay()}
                onClick={() => {
                  setModelSelectorTarget("primary");
                  setShowModelSelector(true);
                }}
              />
              <QuickChip
                icon={<TriangleAlert className="h-4 w-4" />}
                label={t("chats.settings.fallbackModel")}
                value={getFallbackModelDisplay()}
                onClick={() => {
                  setModelSelectorTarget("fallback");
                  setShowModelSelector(true);
                }}
              />
            </div>
          </section>

          {/* Voice */}
          {currentCharacter?.voiceConfig && (
            <section className={spacing.item}>
              <SectionHeader
                title={t("chats.settings.voice")}
                subtitle={t("chats.settings.voiceDesc")}
              />
              <div
                className={cn(
                  "flex items-center justify-between gap-3 rounded-xl border px-4 py-3",
                  !currentSession
                    ? "border-white/5 bg-[#0c0d13]/50 opacity-50 cursor-not-allowed"
                    : "border-white/10 bg-[#0c0d13]/85",
                )}
              >
                <div>
                  <p className="text-sm font-semibold text-white">Autoplay voice</p>
                  <p className="mt-1 text-xs text-white/50">
                    {currentSession
                      ? currentSession.voiceAutoplay == null
                        ? "Using character default"
                        : "Session override active"
                      : "Open a chat session first"}
                  </p>
                </div>
                <Switch
                  id="session-voice-autoplay"
                  checked={effectiveVoiceAutoplay}
                  onChange={() => handleToggleSessionVoiceAutoplay()}
                  disabled={!currentSession}
                />
              </div>
              {currentSession && currentSession.voiceAutoplay != null && (
                <button
                  type="button"
                  onClick={handleResetSessionVoiceAutoplay}
                  className="mt-2 w-full rounded-xl border border-white/10 bg-white/5 px-4 py-2 text-xs text-white/70 transition hover:border-white/20 hover:bg-white/10"
                >
                  Use character default
                </button>
              )}
            </section>
          )}

          {/* Advanced (Important) */}
          <section className={spacing.item}>
            <SectionHeader
              title={t("chats.settings.advanced")}
              subtitle={t("chats.settings.advancedDesc")}
            />
            <button
              onClick={() => {
                if (!currentSession) return;
                const draft = sessionAdvancedSettings ?? baseAdvancedSettings;
                setSessionAdvancedDraft(draft);
                setSessionOverrideEnabled(Boolean(sessionAdvancedSettings));
                setShowSessionAdvancedMenu(true);
              }}
              disabled={!currentSession}
              className={cn(
                "group flex w-full items-center justify-between gap-3",
                radius.lg,
                "border p-4 text-left",
                interactive.transition.default,
                interactive.active.scale,
                !currentSession
                  ? "border-fg/6 bg-surface-el/60 opacity-50 cursor-not-allowed"
                  : "border-fg/10 bg-surface-el hover:border-fg/20 hover:bg-fg/6",
              )}
            >
              <div className="flex items-start gap-3 min-w-0">
                <div
                  className={cn(
                    "flex h-10 w-10 items-center justify-center",
                    radius.full,
                    "border border-fg/15 bg-fg/8 text-fg/80",
                  )}
                >
                  <SlidersHorizontal className="h-4 w-4" />
                </div>
                <div className="min-w-0">
                  <div className="flex items-center gap-2 min-w-0">
                    <div
                      className={cn(
                        typography.overline.size,
                        typography.overline.weight,
                        typography.overline.tracking,
                        typography.overline.transform,
                        "text-fg/50 truncate",
                      )}
                    >
                      Advanced Settings
                    </div>
                    {currentSession ? (
                      <span
                        className={cn(
                          "shrink-0 rounded-full border px-2 py-0.5",
                          typography.overline.size,
                          typography.overline.weight,
                          typography.overline.tracking,
                          typography.overline.transform,
                          sessionAdvancedSettings
                            ? colors.accent.emerald.subtle
                            : "border-fg/10 bg-fg/6 text-fg/60",
                        )}
                      >
                        {sessionAdvancedSettings
                          ? `Overrides${sessionAdvancedOverrideCount ? ` (${sessionAdvancedOverrideCount})` : ""}`
                          : "Defaults"}
                      </span>
                    ) : null}
                  </div>
                  <div className={cn(typography.bodySmall.size, "text-fg mt-1 truncate")}>
                    {sessionAdvancedSummary}
                  </div>
                </div>
              </div>
              <ChevronRight className="h-4 w-4 text-fg/40 transition-colors group-hover:text-fg/80" />
            </button>
          </section>

          {/* Session Management */}
          <section className={spacing.item}>
            <SectionHeader
              title={t("chats.settings.session")}
              subtitle={t("chats.settings.sessionDesc")}
            />
            <div className={spacing.field}>
              <SettingsButton
                icon={<NotebookPen className="h-4 w-4" />}
                title="Author Note"
                subtitle={
                  currentSession?.authorNote?.trim()
                    ? "Active for this chat"
                    : "Private direction for replies"
                }
                onClick={handleOpenAuthorNote}
                disabled={!currentSession}
              />
              <SettingsButton
                icon={<MessageSquarePlus className="h-4 w-4" />}
                title={t("chats.settings.newChat")}
                subtitle={t("chats.settings.newChatDesc")}
                onClick={handleNewChat}
              />
              <SettingsButton
                icon={<History className="h-4 w-4" />}
                title={t("chats.chatHistory")}
                subtitle={t("chats.settings.chatHistoryDesc")}
                onClick={handleViewHistory}
              />
              <SettingsButton
                icon={<Upload className="h-4 w-4" />}
                title={t("chats.importChatPackage")}
                subtitle={t("chats.settings.importChatPackageDesc")}
                onClick={() => {
                  void handleOpenImportChatpkg();
                }}
              />
            </div>
          </section>
        </motion.div>
      </main>

      {/* Persona Selection */}
      <PersonaSelector
        isOpen={showPersonaSelector}
        onClose={() => setShowPersonaSelector(false)}
        personas={personas}
        selectedPersonaId={selectedPersonaId}
        onSelect={handleChangePersona}
        onLongPress={(persona) => {
          setSelectedPersonaForActions(persona);
          setShowPersonaActions(true);
        }}
      />

      <AuthorNoteBottomMenu
        isOpen={showAuthorNoteMenu}
        onClose={() => setShowAuthorNoteMenu(false)}
        session={currentSession}
        onSaved={setCurrentSession}
      />

      {/* Model Selection */}
      <ModelSelectionBottomMenu
        isOpen={showModelSelector}
        onClose={() => setShowModelSelector(false)}
        title={
          modelSelectorTarget === "fallback"
            ? t("chats.settings.selectFallbackModel")
            : t("chats.settings.selectModel")
        }
        models={models}
        selectedModelIds={
          modelSelectorTarget === "fallback"
            ? selectedFallbackModelId
              ? [selectedFallbackModelId]
              : []
            : selectedModelId
              ? [selectedModelId]
              : []
        }
        searchPlaceholder="Search models..."
        theme="dark"
        tone="emerald"
        includeExitIcon={false}
        location="bottom"
        onSelectModel={(modelId) => {
          if (modelSelectorTarget === "fallback") {
            void handleChangeFallbackModel(modelId);
          } else {
            void handleChangeModel(modelId);
          }
          setShowModelSelector(false);
        }}
        clearOption={{
          label:
            modelSelectorTarget === "fallback" ? "No fallback model" : "Use global default model",
          icon: Cpu,
          selected:
            modelSelectorTarget === "fallback" ? !selectedFallbackModelId : !selectedModelId,
          onClick: () => {
            if (modelSelectorTarget === "fallback") {
              void handleChangeFallbackModel(null);
            } else {
              void handleChangeModel(null);
            }
            setShowModelSelector(false);
          },
        }}
      />

      {/* Persona Actions */}
      <BottomMenu
        isOpen={showPersonaActions}
        onClose={() => setShowPersonaActions(false)}
        title={t("chats.settings.personaActions")}
      >
        <MenuSection>
          <div className="space-y-2">
            <button
              onClick={() => {
                if (selectedPersonaForActions) {
                  navigate(`/personas/${selectedPersonaForActions.id}/edit`);
                }
                setShowPersonaActions(false);
              }}
              className="flex w-full items-center gap-3 rounded-xl border border-white/10 bg-white/5 px-4 py-3 text-left transition hover:border-white/20 hover:bg-white/10"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-full border border-white/10 bg-white/10">
                <Edit2 className="h-4 w-4 text-white/70" />
              </div>
              <span className="text-sm font-medium text-white">Edit Persona</span>
            </button>

            <button
              onClick={async () => {
                if (selectedPersonaForActions) {
                  try {
                    await deletePersona(selectedPersonaForActions.id);
                    loadPersonas();
                  } catch (error) {
                    console.error("Failed to delete persona:", error);
                  }
                }
                setShowPersonaActions(false);
              }}
              className="flex w-full items-center gap-3 rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-left transition hover:border-red-500/50 hover:bg-red-500/20"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-full border border-red-500/30 bg-red-500/20">
                <Trash2 className="h-4 w-4 text-red-400" />
              </div>
              <span className="text-sm font-medium text-red-300">Delete Persona</span>
            </button>
          </div>
        </MenuSection>
      </BottomMenu>

      {/* Session Advanced Settings */}
      <SessionAdvancedSettings
        isOpen={showSessionAdvancedMenu}
        onClose={() => setShowSessionAdvancedMenu(false)}
        draft={sessionAdvancedDraft}
        onDraftChange={setSessionAdvancedDraft}
        overrideEnabled={sessionOverrideEnabled}
        onOverrideEnabledChange={setSessionOverrideEnabled}
        baseSettings={baseAdvancedSettings}
        onSave={handleSaveSessionAdvancedSettings}
        onShowParameterSupport={() => setShowParameterSupport(true)}
        hasSession={!!currentSession}
        providerId={currentModel?.providerId ?? "openai"}
        modelPath={currentModel?.name}
      />

      {/* Parameter Support */}
      <BottomMenu
        isOpen={showChatpkgImportMenu}
        onClose={() => {
          if (importingChatpkg) return;
          setShowChatpkgImportMenu(false);
          setPendingChatpkgImport(null);
        }}
        title={t("chats.importChatPackage")}
      >
        <MenuSection>
          <div className="space-y-4">
            <div className="rounded-xl border border-white/10 bg-white/5 p-3 text-xs text-white/70">
              Format:{" "}
              {pendingChatpkgImport?.info?.source?.format === "sillytavern"
                ? "SillyTavern format (.jsonl)"
                : "Chat package / JSONL"}
            </div>
            <div className="rounded-xl border border-white/10 bg-white/5 p-3 text-sm text-white/80">
              {pendingChatpkgImport?.info?.characterId ? (
                pendingChatpkgImport.info.characterId === characterId ? (
                  <p>{t("chats.characterSpecificMatches")}</p>
                ) : (
                  <p>{t("chats.characterSpecificMismatch", { name: currentCharacter.name })}</p>
                )
              ) : (
                <p>{t("chats.nonCharacterSpecificImport", { name: currentCharacter.name })}</p>
              )}
            </div>
            <button
              type="button"
              onClick={() => {
                void handleImportChatpkg();
              }}
              disabled={importingChatpkg}
              className="w-full rounded-xl border border-emerald-500/30 bg-emerald-500/20 py-3 text-sm font-medium text-emerald-200 hover:bg-emerald-500/30 disabled:opacity-50"
            >
              {importingChatpkg ? t("common.buttons.importing") : t("common.buttons.import")}
            </button>
          </div>
        </MenuSection>
      </BottomMenu>

      {/* Parameter Support */}
      <BottomMenu
        isOpen={showParameterSupport}
        onClose={() => setShowParameterSupport(false)}
        title="Parameter Support"
        includeExitIcon={true}
        location="bottom"
      >
        <MenuSection>
          <ProviderParameterSupportInfo
            providerId={(() => {
              const effectiveModelId = getEffectiveModelId();
              const model = models.find((m) => m.id === effectiveModelId);
              return model?.providerId || "openai";
            })()}
          />
        </MenuSection>
      </BottomMenu>

      {/* Template selector */}
      <ChatTemplateSelector
        isOpen={showTemplateSelector}
        onClose={() => setShowTemplateSelector(false)}
        templates={currentCharacter.chatTemplates ?? []}
        defaultTemplateId={currentCharacter.defaultChatTemplateId}
        onSelect={handleTemplateSelected}
      />
    </div>
  );
}

export function ChatSettingsPage() {
  const { character, characterLoading } = useChatLayoutContext();

  if (characterLoading) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-surface">
        <div className="h-10 w-10 animate-spin rounded-full border-4 border-white/10 border-t-white/60" />
      </div>
    );
  }

  if (!character) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-surface px-4">
        <div className="text-center">
          <p className="text-lg text-white">Character not found</p>
          <p className="mt-2 text-sm text-gray-400">
            The character you&apos;re looking for doesn&apos;t exist.
          </p>
        </div>
      </div>
    );
  }

  return <ChatSettingsContent character={character} />;
}
