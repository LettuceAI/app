import { useMemo, useState, useEffect, useCallback } from "react";
import {
  ArrowLeft,
  MessageSquarePlus,
  Cpu,
  ChevronRight,
  Check,
  History,
  User,
  SlidersHorizontal,
  Edit2,
  Trash2,
  Info,
  Sparkles,
  TriangleAlert,
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
import { getProviderIcon } from "../../../core/utils/providerIcons";
import { BottomMenu, MenuSection } from "../../components";
import { ProviderParameterSupportInfo } from "../../components/ProviderParameterSupportInfo";
import { AvatarImage } from "../../components/AvatarImage";
import { useAvatar } from "../../hooks/useAvatar";
import { useChatLayoutContext } from "./ChatLayout";
import {
  ADVANCED_TEMPERATURE_RANGE,
  ADVANCED_TOP_P_RANGE,
  ADVANCED_MAX_TOKENS_RANGE,
  ADVANCED_FREQUENCY_PENALTY_RANGE,
  ADVANCED_PRESENCE_PENALTY_RANGE,
  ADVANCED_TOP_K_RANGE,
  formatAdvancedModelSettingsSummary,
  sanitizeAdvancedModelSettings,
} from "../../components/AdvancedModelSettingsForm";
import { typography, radius, spacing, interactive, cn, colors } from "../../design-tokens";
import { Routes, useNavigationManager } from "../../navigation";
import { PersonaSelector } from "../group-chats/components/settings";

function isImageLike(value?: string) {
  if (!value) return false;
  const lower = value.toLowerCase();
  return (
    lower.startsWith("http://") || lower.startsWith("https://") || lower.startsWith("data:image")
  );
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
          ? "border-white/5 bg-[#0c0d13]/50 opacity-50 cursor-not-allowed"
          : "border-white/10 bg-[#0c0d13]/85 text-white hover:border-white/20 hover:bg-white/10",
      )}
    >
      <div className="flex items-center gap-3 min-w-0">
        <div
          className={cn(
            "flex h-10 w-10 items-center justify-center",
            radius.full,
            "border border-white/15 bg-white/10 text-white/80",
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
              "text-white/50",
            )}
          >
            {title}
          </div>
          <div className={cn(typography.bodySmall.size, "text-white truncate")}>{subtitle}</div>
        </div>
      </div>
      <ChevronRight className="h-4 w-4 text-gray-500 transition-colors group-hover:text-white" />
    </button>
  );
}

function SectionHeader({ title, subtitle }: { title: string; subtitle?: string }) {
  return (
    <div className="flex items-end justify-between gap-3">
      <div className="min-w-0">
        <h2 className={cn(typography.h2.size, typography.h2.weight, "text-white truncate")}>
          {title}
        </h2>
        {subtitle ? (
          <p className={cn(typography.bodySmall.size, "text-white/50 mt-0.5 truncate")}>
            {subtitle}
          </p>
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
          ? "border-white/5 bg-[#0c0d13]/50 opacity-50 cursor-not-allowed"
          : "border-white/10 bg-[#0c0d13]/85 hover:border-white/20 hover:bg-white/10",
      )}
    >
      <div className="flex items-center gap-3 min-w-0">
        <div
          className={cn(
            "flex h-10 w-10 items-center justify-center",
            radius.full,
            "border border-white/15 bg-white/10 text-white/80",
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
              "text-white/50",
            )}
          >
            {label}
          </div>
          <div className={cn(typography.bodySmall.size, "text-white truncate")}>{value}</div>
        </div>
      </div>
      <ChevronRight className="h-4 w-4 text-gray-500 transition-colors group-hover:text-white" />
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

function ChatSettingsContent({ character }: { character: Character }) {
  const navigate = useNavigate();
  const { backOrReplace } = useNavigationManager();
  const { characterId } = useParams();
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
  const [modelSearchQuery, setModelSearchQuery] = useState("");
  const [sessionAdvancedDraft, setSessionAdvancedDraft] = useState<AdvancedModelSettings>(
    createDefaultAdvancedModelSettings(),
  );
  const [sessionOverrideEnabled, setSessionOverrideEnabled] = useState<boolean>(false);
  const [showPersonaActions, setShowPersonaActions] = useState(false);
  const [selectedPersonaForActions, setSelectedPersonaForActions] = useState<Persona | null>(null);
  const [messageCount, setMessageCount] = useState<number>(0);
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

    try {
      const session = await createSession(
        characterId,
        "New Chat",
        currentCharacter.scenes && currentCharacter.scenes.length > 0
          ? currentCharacter.scenes[0].id
          : undefined,
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
      return defaultPersona ? `${defaultPersona.title} (default)` : "No persona";
    }
    const persona = personas.find((p) => p.id === currentPersonaId);
    return persona ? persona.title : "Custom persona";
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

  return (
    <div
      className={cn("relative flex min-h-screen flex-col overflow-hidden", colors.text.primary)}
      style={{ backgroundColor: backgroundImageData ? undefined : "#050505" }}
    >
      {/* Scrim overlay on top of shared background */}
      {backgroundImageData && (
        <div className="pointer-events-none fixed inset-0 z-0 bg-black/40" aria-hidden="true" />
      )}
      {/* Header */}
      <header
        className={cn(
          "z-20 shrink-0 border-b border-white/10 px-4 pb-3 pt-[calc(env(safe-area-inset-top)+12px)] sticky top-0",
          !backgroundImageData ? "bg-[#050505]" : "",
        )}
      >
        <div className="flex items-center gap-3">
          <div className="flex flex-1 items-center min-w-0">
            <button
              onClick={handleBack}
              className="flex shrink-0 px-[0.6em] py-[0.3em] items-center justify-center -ml-2 text-white transition hover:text-white/80"
              aria-label="Back to chat"
            >
              <ArrowLeft size={14} strokeWidth={2.5} />
            </button>
            <div className="min-w-0 flex-1 text-left">
              <p className="truncate text-xl font-bold text-white/90">Chat Settings</p>
              <p className="mt-0.5 truncate text-xs text-white/50">
                Manage conversation preferences
              </p>
            </div>
          </div>
        </div>
      </header>

      {/* Content */}
      <main className="relative z-10 flex-1 overflow-y-auto px-3 pt-4 pb-16">
        <motion.div
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3, ease: "easeOut" }}
          className={spacing.section}
        >
          {/* Session Header */}
          <section
            className={cn(radius.lg, "border border-white/10 bg-[#0c0d13]/85 p-4 backdrop-blur-sm")}
          >
            <div className="flex items-center gap-3">
              {avatarDisplay}
              <div className="min-w-0 flex-1">
                <h3 className={cn(typography.body.size, typography.h3.weight, "text-white")}>
                  {character.name}
                </h3>
                {currentSession ? (
                  <p className={cn(typography.caption.size, "text-gray-400 mt-1 truncate")}>
                    Session: {currentSession.title || "Untitled"}
                    <span className="opacity-50 mx-1.5">•</span>
                    {messageCount} messages
                  </p>
                ) : null}
                {currentCharacter?.description || currentCharacter?.definition ? (
                  <p
                    className={cn(
                      typography.caption.size,
                      "text-gray-400 leading-relaxed line-clamp-2 mt-1",
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
            <SectionHeader title="Memory" subtitle="Summary, tags, tool call history" />
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
                  ? "border-white/5 bg-[#0c0d13]/50 opacity-10 cursor-not-allowed"
                  : cn(
                      "border-emerald-400/20 bg-[#0c0d13]/70 hover:border-emerald-400/30",
                      colors.effects.glow,
                    ),
              )}
            >
              <div className="flex items-start justify-between gap-3">
                <div className="flex items-center gap-3 min-w-0">
                  <div
                    className={cn(
                      "flex h-10 w-10 items-center justify-center",
                      radius.full,
                      "border border-emerald-400/20 bg-emerald-400/10 text-emerald-200",
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
                        "text-white/50",
                      )}
                    >
                      Memory
                    </div>
                    <div className={cn(typography.bodySmall.size, "text-white truncate")}>
                      {memoryMetaLine}
                    </div>
                  </div>
                </div>
                <ChevronRight className="mt-1 h-4 w-4 text-white/40 transition-colors group-hover:text-white" />
              </div>
              <p
                className={cn(
                  typography.bodySmall.size,
                  "mt-3 text-white/70 leading-relaxed line-clamp-3",
                )}
              >
                {memorySummaryPreview}
              </p>
            </button>
          </section>

          {/* Quick Settings */}
          <section className={spacing.item}>
            <SectionHeader title="Quick Settings" subtitle="Most common adjustments" />
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
                label="Persona"
                value={getCurrentPersonaDisplay()}
                onClick={() => setShowPersonaSelector(true)}
                disabled={!currentSession}
              />
              <QuickChip
                icon={<Cpu className="h-4 w-4" />}
                label="Model"
                value={getModelDisplay()}
                onClick={() => {
                  setModelSelectorTarget("primary");
                  setShowModelSelector(true);
                }}
              />
              <QuickChip
                icon={<TriangleAlert className="h-4 w-4" />}
                label="Fallback Model"
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
              <SectionHeader title="Voice" subtitle="Text-to-speech playback" />
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
                <div className="flex items-center">
                  <input
                    id="session-voice-autoplay"
                    type="checkbox"
                    checked={effectiveVoiceAutoplay}
                    onChange={handleToggleSessionVoiceAutoplay}
                    disabled={!currentSession}
                    className="peer sr-only"
                  />
                  <label
                    htmlFor="session-voice-autoplay"
                    className={`relative inline-flex h-6 w-11 shrink-0 rounded-full transition-all ${
                      effectiveVoiceAutoplay ? "bg-emerald-500" : "bg-white/20"
                    } ${currentSession ? "cursor-pointer" : "cursor-not-allowed"}`}
                  >
                    <span
                      className={`inline-block h-5 w-5 mt-0.5 transform rounded-full bg-white transition ${
                        effectiveVoiceAutoplay ? "translate-x-5" : "translate-x-0.5"
                      }`}
                    />
                  </label>
                </div>
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
            <SectionHeader title="Advanced" subtitle="Override model parameters for this session" />
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
                  ? "border-white/5 bg-[#0c0d13]/50 opacity-50 cursor-not-allowed"
                  : "border-white/10 bg-[#0c0d13]/85 hover:border-white/20 hover:bg-white/10",
              )}
            >
              <div className="flex items-start gap-3 min-w-0">
                <div
                  className={cn(
                    "flex h-10 w-10 items-center justify-center",
                    radius.full,
                    "border border-white/15 bg-white/10 text-white/80",
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
                        "text-white/50 truncate",
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
                            : "border-white/10 bg-white/5 text-white/60",
                        )}
                      >
                        {sessionAdvancedSettings
                          ? `Overrides${sessionAdvancedOverrideCount ? ` (${sessionAdvancedOverrideCount})` : ""}`
                          : "Defaults"}
                      </span>
                    ) : null}
                  </div>
                  <div className={cn(typography.bodySmall.size, "text-white mt-1 truncate")}>
                    {sessionAdvancedSummary}
                  </div>
                </div>
              </div>
              <ChevronRight className="h-4 w-4 text-gray-500 transition-colors group-hover:text-white" />
            </button>
          </section>

          {/* Session Management */}
          <section className={spacing.item}>
            <SectionHeader title="Session" subtitle="Start new chats and browse history" />
            <div className={spacing.field}>
              <SettingsButton
                icon={<MessageSquarePlus className="h-4 w-4" />}
                title="New Chat"
                subtitle="Start a fresh conversation"
                onClick={handleNewChat}
              />
              <SettingsButton
                icon={<History className="h-4 w-4" />}
                title="Chat History"
                subtitle="View previous sessions"
                onClick={handleViewHistory}
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

      {/* Model Selection */}
      <BottomMenu
        isOpen={showModelSelector}
        onClose={() => {
          setShowModelSelector(false);
          setModelSearchQuery("");
        }}
        title={modelSelectorTarget === "fallback" ? "Select Fallback Model" : "Select Model"}
        includeExitIcon={false}
        location="bottom"
      >
        <div className="space-y-4">
          <div className="relative">
            <input
              type="text"
              value={modelSearchQuery}
              onChange={(e) => setModelSearchQuery(e.target.value)}
              placeholder="Search models..."
              className="w-full rounded-xl border border-white/10 bg-black/30 px-4 py-2.5 pl-10 text-sm text-white placeholder-white/40 focus:border-white/20 focus:outline-none"
            />
            <svg
              className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-white/40"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
              />
            </svg>
          </div>
          <div className="space-y-2 max-h-[50vh] overflow-y-auto">
            <button
              onClick={() => {
                if (modelSelectorTarget === "fallback") {
                  void handleChangeFallbackModel(null);
                } else {
                  void handleChangeModel(null);
                }
                setShowModelSelector(false);
                setModelSearchQuery("");
              }}
              className={cn(
                "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition",
                (modelSelectorTarget === "fallback" ? !selectedFallbackModelId : !selectedModelId)
                  ? "border-emerald-400/40 bg-emerald-400/10"
                  : "border-white/10 bg-white/5 hover:bg-white/10",
              )}
            >
              <Cpu className="h-5 w-5 text-white/40" />
              <span className="text-sm text-white">
                {modelSelectorTarget === "fallback"
                  ? "No fallback model"
                  : "Use global default model"}
              </span>
              {modelSelectorTarget === "fallback"
                ? !selectedFallbackModelId && <Check className="h-4 w-4 ml-auto text-emerald-400" />
                : !selectedModelId && <Check className="h-4 w-4 ml-auto text-emerald-400" />}
            </button>
            {models
              .filter((model) => {
                if (!modelSearchQuery) return true;
                const q = modelSearchQuery.toLowerCase();
                return (
                  model.displayName?.toLowerCase().includes(q) ||
                  model.name?.toLowerCase().includes(q)
                );
              })
              .map((model) => (
                <button
                  key={model.id}
                  onClick={() => {
                    if (modelSelectorTarget === "fallback") {
                      void handleChangeFallbackModel(model.id);
                    } else {
                      void handleChangeModel(model.id);
                    }
                    setShowModelSelector(false);
                    setModelSearchQuery("");
                  }}
                  className={cn(
                    "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition",
                    (
                      modelSelectorTarget === "fallback"
                        ? selectedFallbackModelId === model.id
                        : selectedModelId === model.id
                    )
                      ? "border-emerald-400/40 bg-emerald-400/10"
                      : "border-white/10 bg-white/5 hover:bg-white/10",
                  )}
                >
                  {getProviderIcon(model.providerId)}
                  <div className="flex-1 min-w-0">
                    <span className="block truncate text-sm text-white">
                      {model.displayName || model.name}
                    </span>
                    <span className="block truncate text-xs text-white/40">{model.name}</span>
                  </div>
                  {(modelSelectorTarget === "fallback"
                    ? selectedFallbackModelId === model.id
                    : selectedModelId === model.id) && (
                    <Check className="h-4 w-4 shrink-0 text-emerald-400" />
                  )}
                </button>
              ))}
          </div>
        </div>
      </BottomMenu>

      {/* Persona Actions */}
      <BottomMenu
        isOpen={showPersonaActions}
        onClose={() => setShowPersonaActions(false)}
        title="Persona Actions"
      >
        <MenuSection>
          <div className="space-y-2">
            <button
              onClick={() => {
                if (selectedPersonaForActions) {
                  navigate(`/settings/personas/${selectedPersonaForActions.id}/edit`);
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

      {/* Session Advanced Settings Bottom Menu */}
      <BottomMenu
        isOpen={showSessionAdvancedMenu}
        onClose={() => setShowSessionAdvancedMenu(false)}
        title="Session Advanced Settings"
        includeExitIcon={true}
        location="bottom"
      >
        <MenuSection>
          {currentSession ? (
            <div className="space-y-5">
              <div className="flex items-center justify-between rounded-xl border border-white/10 px-4 py-3">
                <div>
                  <p className="text-sm font-semibold text-white">Override defaults</p>
                  <p className="mt-1 text-xs text-white/50 leading-relaxed">
                    Override model parameters just for this conversation
                  </p>
                </div>

                <div className="flex items-center">
                  <input
                    id="use-as-default"
                    type="checkbox"
                    checked={sessionOverrideEnabled}
                    onChange={() => setSessionOverrideEnabled((value) => !value)}
                    className="peer sr-only"
                  />
                  <label
                    htmlFor="use-as-default"
                    className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full transition-all ${
                      sessionOverrideEnabled ? "bg-emerald-500" : "bg-white/20"
                    }`}
                  >
                    <span
                      className={`inline-block h-5 w-5 mt-0.5 transform rounded-full bg-white transition ${
                        sessionOverrideEnabled ? "translate-x-5" : "translate-x-0.5"
                      }`}
                    />
                  </label>
                </div>
              </div>

              {/* Advanced Settings Controls */}
              {sessionOverrideEnabled && (
                <div className="space-y-3">
                  {/* Parameter Support Info Button */}
                  <button
                    onClick={() => setShowParameterSupport(true)}
                    className="flex w-full items-center justify-center gap-2 rounded-xl border border-blue-400/30 bg-blue-400/10 px-4 py-2.5 text-sm text-blue-200 transition hover:bg-blue-400/15 active:scale-[0.99]"
                  >
                    <Info className="h-4 w-4" />
                    <span>View Parameter Support</span>
                  </button>

                  {/* Temperature */}
                  <div className="rounded-xl border border-white/10 p-4">
                    <div className="mb-3 flex items-start justify-between gap-3">
                      <div className="flex-1">
                        <label className="text-sm font-medium text-white">Temperature</label>
                        <p className="mt-0.5 text-xs text-white/50">
                          Controls randomness and creativity
                        </p>
                      </div>
                      <span className="rounded-lg bg-emerald-400/15 px-2.5 py-1 text-sm font-mono font-semibold text-emerald-200">
                        {sessionAdvancedDraft.temperature?.toFixed(2) ?? "0.70"}
                      </span>
                    </div>
                    <input
                      type="number"
                      inputMode="decimal"
                      min={ADVANCED_TEMPERATURE_RANGE.min}
                      max={ADVANCED_TEMPERATURE_RANGE.max}
                      step={0.01}
                      value={sessionAdvancedDraft.temperature ?? ""}
                      onChange={(e) => {
                        const raw = e.target.value;
                        setSessionAdvancedDraft({
                          ...sessionAdvancedDraft,
                          temperature: raw === "" ? null : Number(raw),
                        });
                      }}
                      placeholder="0.70"
                      className="w-full rounded-lg border border-white/10 bg-black/20 px-3.5 py-3 text-base text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
                    />
                    <div className="mt-2 flex items-center justify-between text-xs text-white/40">
                      <span>0 - Precise</span>
                      <span>2 - Creative</span>
                    </div>
                  </div>

                  {/* Top P */}
                  <div className="rounded-xl border border-white/10 p-4">
                    <div className="mb-3 flex items-start justify-between gap-3">
                      <div className="flex-1">
                        <label className="text-sm font-medium text-white">Top P</label>
                        <p className="mt-0.5 text-xs text-white/50">Nucleus sampling threshold</p>
                      </div>
                      <span className="rounded-lg bg-blue-400/15 px-2.5 py-1 text-sm font-mono font-semibold text-blue-200">
                        {sessionAdvancedDraft.topP?.toFixed(2) ?? "1.00"}
                      </span>
                    </div>
                    <input
                      type="number"
                      inputMode="decimal"
                      min={ADVANCED_TOP_P_RANGE.min}
                      max={ADVANCED_TOP_P_RANGE.max}
                      step={0.01}
                      value={sessionAdvancedDraft.topP ?? ""}
                      onChange={(e) => {
                        const raw = e.target.value;
                        setSessionAdvancedDraft({
                          ...sessionAdvancedDraft,
                          topP: raw === "" ? null : Number(raw),
                        });
                      }}
                      placeholder="1.00"
                      className="w-full rounded-lg border border-white/10 bg-black/20 px-3.5 py-3 text-base text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
                    />
                    <div className="mt-2 flex items-center justify-between text-xs text-white/40">
                      <span>0 - Focused</span>
                      <span>1 - Diverse</span>
                    </div>
                  </div>

                  {/* Max Tokens */}
                  <div className="rounded-xl border border-white/10 p-4">
                    <div className="mb-3">
                      <label className="text-sm font-medium text-white">Max Output Tokens</label>
                      <p className="mt-0.5 text-xs text-white/50">Maximum response length</p>
                    </div>

                    <div className="flex gap-2 mb-3">
                      <button
                        type="button"
                        onClick={() =>
                          setSessionAdvancedDraft({
                            ...sessionAdvancedDraft,
                            maxOutputTokens: null,
                          })
                        }
                        className={`flex-1 rounded-lg px-3 py-2 text-sm font-medium transition ${
                          !sessionAdvancedDraft.maxOutputTokens
                            ? "bg-purple-400/20 text-purple-200"
                            : "border border-white/10 text-white/60 hover:bg-white/5 active:bg-white/10"
                        }`}
                      >
                        Auto
                      </button>
                      <button
                        type="button"
                        onClick={() =>
                          setSessionAdvancedDraft({
                            ...sessionAdvancedDraft,
                            maxOutputTokens: 1024,
                          })
                        }
                        className={`flex-1 rounded-lg px-3 py-2 text-sm font-medium transition ${
                          sessionAdvancedDraft.maxOutputTokens
                            ? "bg-purple-400/20 text-purple-200"
                            : "border border-white/10 text-white/60 hover:bg-white/5 active:bg-white/10"
                        }`}
                      >
                        Custom
                      </button>
                    </div>

                    {sessionAdvancedDraft.maxOutputTokens !== null &&
                      sessionAdvancedDraft.maxOutputTokens !== undefined && (
                        <input
                          type="number"
                          inputMode="numeric"
                          min={ADVANCED_MAX_TOKENS_RANGE.min}
                          max={ADVANCED_MAX_TOKENS_RANGE.max}
                          value={sessionAdvancedDraft.maxOutputTokens ?? ""}
                          onChange={(e) =>
                            setSessionAdvancedDraft({
                              ...sessionAdvancedDraft,
                              maxOutputTokens: Number(e.target.value),
                            })
                          }
                          placeholder="1024"
                          className="w-full rounded-lg border border-white/10 bg-black/20 px-3.5 py-3 text-base text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
                        />
                      )}

                    <p className="mt-2 text-xs text-white/40">
                      {!sessionAdvancedDraft.maxOutputTokens
                        ? "Let the model decide the response length"
                        : `Range: ${ADVANCED_MAX_TOKENS_RANGE.min.toLocaleString()} - ${ADVANCED_MAX_TOKENS_RANGE.max.toLocaleString()}`}
                    </p>
                  </div>

                  {/* Frequency Penalty */}
                  <div className="rounded-xl border border-white/10 p-4">
                    <div className="mb-3 flex items-start justify-between gap-3">
                      <div className="flex-1">
                        <label className="text-sm font-medium text-white">Frequency Penalty</label>
                        <p className="mt-0.5 text-xs text-white/50">
                          Reduce repetition of token sequences
                        </p>
                      </div>
                      <span className="rounded-lg bg-orange-400/15 px-2.5 py-1 text-sm font-mono font-semibold text-orange-200">
                        {sessionAdvancedDraft.frequencyPenalty?.toFixed(2) ?? "0.00"}
                      </span>
                    </div>
                    <input
                      type="number"
                      inputMode="decimal"
                      min={ADVANCED_FREQUENCY_PENALTY_RANGE.min}
                      max={ADVANCED_FREQUENCY_PENALTY_RANGE.max}
                      step={0.01}
                      value={sessionAdvancedDraft.frequencyPenalty ?? ""}
                      onChange={(e) => {
                        const raw = e.target.value;
                        setSessionAdvancedDraft({
                          ...sessionAdvancedDraft,
                          frequencyPenalty: raw === "" ? null : Number(raw),
                        });
                      }}
                      placeholder="0.00"
                      className="w-full rounded-lg border border-white/10 bg-black/20 px-3.5 py-3 text-base text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
                    />
                    <div className="mt-2 flex items-center justify-between text-xs text-white/40">
                      <span>-2 - More Rep.</span>
                      <span>2 - Less Rep.</span>
                    </div>
                  </div>

                  {/* Presence Penalty */}
                  <div className="rounded-xl border border-white/10 p-4">
                    <div className="mb-3 flex items-start justify-between gap-3">
                      <div className="flex-1">
                        <label className="text-sm font-medium text-white">Presence Penalty</label>
                        <p className="mt-0.5 text-xs text-white/50">
                          Encourage discussing new topics
                        </p>
                      </div>
                      <span className="rounded-lg bg-pink-400/15 px-2.5 py-1 text-sm font-mono font-semibold text-pink-200">
                        {sessionAdvancedDraft.presencePenalty?.toFixed(2) ?? "0.00"}
                      </span>
                    </div>
                    <input
                      type="number"
                      inputMode="decimal"
                      min={ADVANCED_PRESENCE_PENALTY_RANGE.min}
                      max={ADVANCED_PRESENCE_PENALTY_RANGE.max}
                      step={0.01}
                      value={sessionAdvancedDraft.presencePenalty ?? ""}
                      onChange={(e) => {
                        const raw = e.target.value;
                        setSessionAdvancedDraft({
                          ...sessionAdvancedDraft,
                          presencePenalty: raw === "" ? null : Number(raw),
                        });
                      }}
                      placeholder="0.00"
                      className="w-full rounded-lg border border-white/10 bg-black/20 px-3.5 py-3 text-base text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
                    />
                    <div className="mt-2 flex items-center justify-between text-xs text-white/40">
                      <span>-2 - Repeat</span>
                      <span>2 - Explore</span>
                    </div>
                  </div>

                  {/* Top K */}
                  <div className="rounded-xl border border-white/10 p-4">
                    <div className="mb-3">
                      <label className="text-sm font-medium text-white">Top K</label>
                      <p className="mt-0.5 text-xs text-white/50">Limit sampling to top K tokens</p>
                    </div>
                    <input
                      type="number"
                      inputMode="numeric"
                      min={ADVANCED_TOP_K_RANGE.min}
                      max={ADVANCED_TOP_K_RANGE.max}
                      value={sessionAdvancedDraft.topK ?? ""}
                      onChange={(e) => {
                        const val = e.target.value === "" ? null : Number(e.target.value);
                        setSessionAdvancedDraft({ ...sessionAdvancedDraft, topK: val });
                      }}
                      placeholder="40"
                      className="w-full rounded-lg border border-white/10 bg-black/20 px-3.5 py-3 text-base text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
                    />
                    <p className="mt-2 text-xs text-white/40">
                      Lower values = more focused, higher = more diverse
                    </p>
                  </div>
                </div>
              )}

              <div className="flex gap-3">
                <button
                  type="button"
                  onClick={() => {
                    setSessionOverrideEnabled(false);
                    setSessionAdvancedDraft(baseAdvancedSettings);
                    handleSaveSessionAdvancedSettings(null);
                  }}
                  className="flex-1 rounded-xl border border-white/10 py-3 text-sm font-medium text-white hover:bg-white/5 active:scale-[0.99]"
                >
                  Use defaults
                </button>
                <button
                  type="button"
                  onClick={() =>
                    handleSaveSessionAdvancedSettings(
                      sessionOverrideEnabled ? sessionAdvancedDraft : null,
                    )
                  }
                  className="flex-1 rounded-xl bg-emerald-400/20 py-3 text-sm font-semibold text-emerald-100 hover:bg-emerald-400/25 active:scale-[0.99]"
                >
                  Save changes
                </button>
              </div>
            </div>
          ) : (
            <div className="rounded-2xl border border-amber-500/20 bg-amber-500/10 px-6 py-4 text-sm text-amber-200">
              Open a chat session to configure per-session settings.
            </div>
          )}
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
    </div>
  );
}

export function ChatSettingsPage() {
  const { character, characterLoading } = useChatLayoutContext();

  if (characterLoading) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-[#050505]">
        <div className="h-10 w-10 animate-spin rounded-full border-4 border-white/10 border-t-white/60" />
      </div>
    );
  }

  if (!character) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-[#050505] px-4">
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
