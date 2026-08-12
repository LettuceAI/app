import { useEffect, useRef, useState, useCallback, useMemo } from "react";
import { useSearchParams } from "react-router-dom";
import { Bot, User, RefreshCw, Eye } from "lucide-react";
import {
  AppearanceTabBar,
  ChatAppearanceForm,
  type AppearanceKey,
  type AppearanceTab,
} from "../chats/components/appearance/ChatAppearanceForm";
import {
  readSettings,
  saveAdvancedSettings,
  listCharacters,
  getDefaultPersona,
  updateCharacterChatAppearance,
} from "../../../core/storage/repo";
import {
  createDefaultChatAppearanceSettings,
  type ChatAppearanceSettings,
  type ChatAppearanceOverride,
  type Character,
  type Persona,
  mergeChatAppearance,
} from "../../../core/storage/schemas";
import { cn } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import { useAvatar } from "../../hooks/useAvatar";
import { useImageData } from "../../hooks/useImageData";
import { AvatarImage } from "../../components/AvatarImage";
import { toast } from "../../components/toast";
import { MarkdownRenderer } from "../chats/components/MarkdownRenderer";
import {
  colorToLuminance,
  computeBubbleTextClass,
  normalizeHexColor,
} from "../../../core/utils/imageAnalysis";
import { AnimatePresence, motion } from "framer-motion";

const SAMPLE_MESSAGES: { role: "assistant" | "user"; text: string }[] = [
  {
    role: "assistant",
    text: "Hey! How are you doing today? You seemed *really* busy earlier, so I didn't want to interrupt.",
  },
  {
    role: "user",
    text: 'I\'m doing **great**, thanks for asking! Just needed a minute to finish a few things before I could "relax."',
  },
  {
    role: "assistant",
    text: 'That\'s good to hear. I was thinking about the trip we mentioned last time *(the lake cabin plan)* and wanted to revisit it.\n\n> You said you wanted somewhere "quiet" and close to the water.',
  },
  {
    role: "user",
    text: 'Oh right, that one. Did you find anything *actually* "quiet", or just the **usual crowded spots** people keep recommending? I can check the `route` after dinner.',
  },
  {
    role: "assistant",
    text: 'I found a place that looks **"perfect."** It\'s small, close to the water, and the view in the morning looks *incredible* from the deck.',
  },
  {
    role: "user",
    text: 'That sounds amazing. Send me the **details** when you can, and I\'ll check the route tonight *(plus the weather and traffic)*.\n\n> If the road is clear, we could leave "early Saturday."',
  },
];

function normalizeOverride(override: ChatAppearanceOverride): ChatAppearanceOverride {
  const normalized = { ...override } as ChatAppearanceOverride;
  normalized.userBubbleColorHex = normalizeHexColor(override.userBubbleColorHex);
  normalized.assistantBubbleColorHex = normalizeHexColor(override.assistantBubbleColorHex);
  normalized.messageTextColorHex = normalizeHexColor(override.messageTextColorHex);
  normalized.plainTextColorHex = normalizeHexColor(override.plainTextColorHex);
  normalized.italicTextColorHex = normalizeHexColor(override.italicTextColorHex);
  normalized.quotedTextColorHex = normalizeHexColor(override.quotedTextColorHex);
  normalized.inlineCodeTextColorHex = normalizeHexColor(override.inlineCodeTextColorHex);
  return Object.fromEntries(
    Object.entries(normalized)
      .filter(([_, value]) => value !== undefined)
      .sort(([a], [b]) => a.localeCompare(b)),
  ) as ChatAppearanceOverride;
}

function deriveOverrideFromSettings(
  global: ChatAppearanceSettings,
  effective: ChatAppearanceSettings,
): ChatAppearanceOverride {
  const next: Record<string, unknown> = {};

  for (const key of Object.keys(effective) as AppearanceKey[]) {
    if (JSON.stringify(effective[key]) === JSON.stringify(global[key])) continue;
    next[key] = effective[key];
  }

  return normalizeOverride(next as ChatAppearanceOverride);
}

function areSettingsEqual(a: ChatAppearanceSettings, b: ChatAppearanceSettings): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

function areOverridesEqual(a: ChatAppearanceOverride, b: ChatAppearanceOverride): boolean {
  return JSON.stringify(normalizeOverride(a)) === JSON.stringify(normalizeOverride(b));
}

function normalizeSettings(settings: ChatAppearanceSettings): ChatAppearanceSettings {
  return {
    ...settings,
    userBubbleColorHex: normalizeHexColor(settings.userBubbleColorHex),
    assistantBubbleColorHex: normalizeHexColor(settings.assistantBubbleColorHex),
    messageTextColorHex: normalizeHexColor(settings.messageTextColorHex),
    plainTextColorHex: normalizeHexColor(settings.plainTextColorHex),
    italicTextColorHex: normalizeHexColor(settings.italicTextColorHex),
    quotedTextColorHex: normalizeHexColor(settings.quotedTextColorHex),
    inlineCodeTextColorHex: normalizeHexColor(settings.inlineCodeTextColorHex),
  };
}

function CharacterAvatar({ character, size }: { character: Character; size: string }) {
  const avatarUrl = useAvatar("character", character.id, character.avatarPath, "round");
  if (avatarUrl) {
    return (
      <AvatarImage src={avatarUrl} alt={character.name} crop={character.avatarCrop} applyCrop />
    );
  }
  return (
    <span className={cn("flex items-center justify-center text-[10px] font-bold text-fg/60", size)}>
      {character.name.slice(0, 2).toUpperCase()}
    </span>
  );
}

function PersonaAvatar({ persona }: { persona: Persona }) {
  const avatarUrl = useAvatar("persona", persona.id, persona.avatarPath);
  if (avatarUrl) {
    return <AvatarImage src={avatarUrl} alt={persona.title} crop={persona.avatarCrop} applyCrop />;
  }
  return <User size={12} className="text-fg/50" />;
}

// Mini preview component showing sample messages
function LivePreview({
  settings,
  character,
  persona,
  liveMode,
  backgroundUrl,
}: {
  settings: ChatAppearanceSettings;
  character?: Character | null;
  persona?: Persona | null;
  liveMode?: boolean;
  backgroundUrl?: string;
}) {
  const fontSize =
    settings.fontSize === "small"
      ? "text-xs"
      : settings.fontSize === "large"
        ? "text-base"
        : settings.fontSize === "xlarge"
          ? "text-lg"
          : "text-sm";
  const lineSpacing =
    settings.lineSpacing === "tight"
      ? "leading-snug"
      : settings.lineSpacing === "relaxed"
        ? "leading-relaxed"
        : "leading-normal";
  const bubbleRadius =
    settings.bubbleRadius === "sharp"
      ? "rounded-md"
      : settings.bubbleRadius === "pill"
        ? "rounded-2xl"
        : "rounded-lg";
  const padding =
    settings.bubblePadding === "compact"
      ? "px-2.5 py-1"
      : settings.bubblePadding === "spacious"
        ? "px-4 py-3"
        : "px-3 py-2";
  const maxW =
    settings.bubbleMaxWidth === "compact"
      ? "max-w-[70%]"
      : settings.bubbleMaxWidth === "wide"
        ? "max-w-[92%]"
        : "max-w-[82%]";
  const gap =
    settings.messageGap === "tight"
      ? "gap-1"
      : settings.messageGap === "relaxed"
        ? "gap-4"
        : "gap-2";
  const blur =
    settings.bubbleBlur === "light"
      ? "backdrop-blur-sm"
      : settings.bubbleBlur === "medium"
        ? "backdrop-blur-md"
        : settings.bubbleBlur === "heavy"
          ? "backdrop-blur-lg"
          : "";
  const avatarSize =
    settings.avatarSize === "small"
      ? "h-5 w-5"
      : settings.avatarSize === "large"
        ? "h-8 w-8"
        : "h-6 w-6";
  const avatarShape =
    settings.avatarShape === "rounded"
      ? "rounded-md"
      : settings.avatarShape === "hidden"
        ? ""
        : "rounded-full";
  const showAvatars = settings.avatarShape !== "hidden";
  const isBordered = settings.bubbleStyle === "bordered";
  const isMinimal = settings.bubbleStyle === "minimal";
  const opacity = Math.max(0, Math.min(100, settings.bubbleOpacity));
  const userHex = normalizeHexColor(settings.userBubbleColorHex);
  const assistantHex = normalizeHexColor(settings.assistantBubbleColorHex);
  const resolveTokenColor = (
    token: "accent" | "info" | "secondary" | "warning" | "neutral",
  ): string => {
    const name = token === "neutral" ? "fg" : token;
    return getComputedStyle(document.documentElement).getPropertyValue(`--color-${name}`).trim();
  };
  const userColor = userHex ?? resolveTokenColor(settings.userBubbleColor);
  const assistantColor = assistantHex ?? resolveTokenColor(settings.assistantBubbleColor);
  const useLive = liveMode && character;
  const hasBg = useLive && backgroundUrl;
  const userBubbleStyle = isMinimal
    ? undefined
    : {
        backgroundColor: `color-mix(in oklab, ${userColor} ${opacity}%, transparent)`,
        borderColor: `color-mix(in oklab, ${userColor} 50%, transparent)`,
      };
  const assistantBubbleStyle = isMinimal
    ? undefined
    : settings.assistantBubbleColor === "neutral" && !assistantHex
      ? hasBg
        ? {
            backgroundColor: `rgba(0, 0, 0, ${opacity / 100})`,
            borderColor: "rgba(0, 0, 0, 0.4)",
          }
        : {
            backgroundColor: "color-mix(in oklab, var(--color-fg) 5%, transparent)",
            borderColor: "color-mix(in oklab, var(--color-fg) 10%, transparent)",
          }
      : {
          backgroundColor: `color-mix(in oklab, ${assistantColor} ${opacity}%, transparent)`,
          borderColor: `color-mix(in oklab, ${assistantColor} 50%, transparent)`,
        };
  const opacity01 = opacity / 100;
  const userTextClass = computeBubbleTextClass(
    null,
    colorToLuminance(userColor),
    opacity01,
    settings.textMode,
  );
  const assistantTextClass =
    settings.assistantBubbleColor === "neutral" && !assistantHex && settings.textMode === "auto"
      ? hasBg
        ? "text-white/95"
        : "text-fg"
      : computeBubbleTextClass(
          null,
          settings.assistantBubbleColor === "neutral" && !assistantHex
            ? hasBg
              ? 0
              : colorToLuminance("color-mix(in oklab, var(--color-fg) 5%, transparent)")
            : colorToLuminance(assistantColor),
          opacity01,
          settings.textMode,
        );
  const textColors = {
    texts: settings.messageTextColorHex ?? settings.plainTextColorHex ?? "currentColor",
    plain: settings.plainTextColorHex ?? "currentColor",
    italic: settings.italicTextColorHex ?? "currentColor",
    quoted: settings.quotedTextColorHex ?? "currentColor",
    code: settings.inlineCodeTextColorHex ?? "currentColor",
  };

  return (
    <div
      className={cn(
        "relative overflow-hidden rounded-xl border border-fg/10 p-3 pb-5",
        !hasBg && "bg-fg/5",
      )}
    >
      {hasBg && (
        <div
          className="absolute inset-0"
          style={{
            backgroundImage: `url(${backgroundUrl})`,
            backgroundSize: "cover",
            backgroundPosition: "center",
            filter: settings.backgroundBlur > 0 ? `blur(${settings.backgroundBlur}px)` : undefined,
            transform: settings.backgroundBlur > 0 ? "scale(1.06)" : undefined,
          }}
        />
      )}
      {hasBg && settings.backgroundDim > 0 && (
        <div
          className="absolute inset-0"
          style={{ backgroundColor: `rgba(0, 0, 0, ${settings.backgroundDim / 100})` }}
        />
      )}
      <div className={cn("relative flex flex-col", gap)}>
        {SAMPLE_MESSAGES.map((msg, i) =>
          msg.role === "assistant" ? (
            <div key={i} className="flex items-end gap-1.5">
              {showAvatars && (
                <div
                  className={cn(
                    "flex shrink-0 items-center justify-center overflow-hidden border border-fg/10 bg-fg/10",
                    avatarSize,
                    avatarShape,
                  )}
                >
                  {useLive ? (
                    <CharacterAvatar character={character} size={avatarSize} />
                  ) : (
                    <Bot size={12} className="text-fg/50" />
                  )}
                </div>
              )}
              <div
                className={cn(
                  maxW,
                  padding,
                  bubbleRadius,
                  fontSize,
                  lineSpacing,
                  blur,
                  isBordered && "border",
                  assistantTextClass,
                )}
                style={assistantBubbleStyle}
              >
                <MarkdownRenderer
                  content={msg.text}
                  className="text-inherit leading-[inherit] [&_a]:text-info [&_code]:bg-black/30"
                  textColors={textColors}
                />
              </div>
            </div>
          ) : (
            <div key={i} className="flex items-end justify-end gap-1.5">
              <div
                className={cn(
                  maxW,
                  padding,
                  bubbleRadius,
                  fontSize,
                  lineSpacing,
                  blur,
                  isBordered && "border",
                  userTextClass,
                )}
                style={userBubbleStyle}
              >
                <MarkdownRenderer
                  content={msg.text}
                  className="text-inherit leading-[inherit] [&_a]:text-info [&_code]:bg-black/30"
                  textColors={textColors}
                />
              </div>
              {showAvatars && (
                <div
                  className={cn(
                    "flex shrink-0 items-center justify-center overflow-hidden border border-fg/10 bg-fg/10",
                    avatarSize,
                    avatarShape,
                  )}
                >
                  {useLive && persona ? (
                    <PersonaAvatar persona={persona} />
                  ) : (
                    <User size={12} className="text-fg/50" />
                  )}
                </div>
              )}
            </div>
          ),
        )}
      </div>
    </div>
  );
}

export function ChatAppearancePage() {
  const [searchParams] = useSearchParams();
  const { t } = useI18n();
  const characterId = searchParams.get("characterId") ?? undefined;
  const mode = characterId ? "character" : "global";

  const [globalSettings, setGlobalSettings] = useState<ChatAppearanceSettings>(
    createDefaultChatAppearanceSettings(),
  );
  const [initialGlobalSettings, setInitialGlobalSettings] = useState<ChatAppearanceSettings>(
    createDefaultChatAppearanceSettings(),
  );
  const [characterOverride, setCharacterOverride] = useState<ChatAppearanceOverride>({});
  const [initialCharacterOverride, setInitialCharacterOverride] = useState<ChatAppearanceOverride>(
    {},
  );
  const [character, setCharacter] = useState<Character | null>(null);
  const [persona, setPersona] = useState<Persona | null>(null);
  const [livePreview, setLivePreview] = useState(false);
  const [mobilePreviewOpen, setMobilePreviewOpen] = useState(false);
  const [activeTab, setActiveTab] = useState<AppearanceTab>("typography");
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [isMobileViewport, setIsMobileViewport] = useState(() =>
    typeof window !== "undefined" ? window.matchMedia("(max-width: 1023px)").matches : false,
  );
  const backgroundUrl = useImageData(livePreview ? character?.backgroundImagePath : undefined);

  // The effective settings (global merged with character override)
  const effectiveSettings = useMemo(
    () =>
      mode === "character"
        ? mergeChatAppearance(globalSettings, characterOverride)
        : globalSettings,
    [mode, globalSettings, characterOverride],
  );

  useEffect(() => {
    const load = async () => {
      try {
        const settings = await readSettings();
        const global = normalizeSettings(
          settings.advancedSettings?.chatAppearance ?? createDefaultChatAppearanceSettings(),
        );
        setGlobalSettings(global);
        setInitialGlobalSettings(global);

        if (characterId) {
          const [chars, defaultPersona] = await Promise.all([
            listCharacters(),
            getDefaultPersona(),
          ]);
          const match = chars.find((c) => c.id === characterId) ?? null;
          setCharacter(match);
          const loadedOverride = normalizeOverride(match?.chatAppearance ?? {});
          setCharacterOverride(loadedOverride);
          setInitialCharacterOverride(loadedOverride);
          setPersona(defaultPersona);
        }
      } catch (err) {
        console.error("Failed to load chat appearance settings:", err);
      } finally {
        setIsLoading(false);
      }
    };
    void load();
  }, [characterId]);

  const persistGlobal = useCallback(async (next: ChatAppearanceSettings) => {
    const settings = await readSettings();
    const normalized = normalizeSettings(next);
    await saveAdvancedSettings({
      ...(settings.advancedSettings ?? {}),
      creationHelperEnabled: settings.advancedSettings?.creationHelperEnabled ?? false,
      helpMeReplyEnabled: settings.advancedSettings?.helpMeReplyEnabled ?? true,
      chatAppearance: normalized,
    });
  }, []);

  const persistCharacter = useCallback(
    async (next: ChatAppearanceSettings) => {
      if (!character) throw new Error("Character not loaded");
      const normalized = deriveOverrideFromSettings(globalSettings, next);
      return updateCharacterChatAppearance(
        character.id,
        Object.keys(normalized).length > 0 ? normalized : null,
      );
    },
    [character, globalSettings],
  );

  const updateField = useCallback(
    <K extends AppearanceKey>(key: K, value: ChatAppearanceSettings[K]) => {
      if (mode === "global") {
        setGlobalSettings((prev) => ({ ...prev, [key]: value }));
      } else {
        setCharacterOverride((prev) => ({ ...prev, [key]: value }));
      }
    },
    [mode],
  );

  const resetField = useCallback(
    (key: AppearanceKey) => {
      if (mode !== "character") return;
      setCharacterOverride((prev) => {
        const next = { ...prev };
        delete next[key];
        return next;
      });
    },
    [mode],
  );

  const isOverridden = useCallback(
    (key: AppearanceKey): boolean => {
      if (mode !== "character") return false;
      return key in characterOverride && characterOverride[key] !== undefined;
    },
    [mode, characterOverride],
  );

  const resetAll = useCallback(() => {
    const defaults = createDefaultChatAppearanceSettings();
    if (mode === "global") {
      setGlobalSettings(defaults);
    } else {
      setCharacterOverride({});
    }
  }, [mode]);

  const isDirty = useMemo(() => {
    if (mode === "character") {
      return !areOverridesEqual(characterOverride, initialCharacterOverride);
    }
    return !areSettingsEqual(globalSettings, initialGlobalSettings);
  }, [mode, characterOverride, initialCharacterOverride, globalSettings, initialGlobalSettings]);

  const handleSave = useCallback(async () => {
    if (!isDirty || isSaving) return;
    setIsSaving(true);
    try {
      if (mode === "character") {
        const derivedOverride = deriveOverrideFromSettings(globalSettings, effectiveSettings);
        const saved = await persistCharacter(effectiveSettings);
        setCharacter({ ...saved, chatAppearance: derivedOverride });
        setCharacterOverride(derivedOverride);
        setInitialCharacterOverride(derivedOverride);
        toast.success("Saved", "Character chat appearance updated.");
      } else {
        const normalizedGlobal = normalizeSettings(globalSettings);
        await persistGlobal(normalizedGlobal);
        setGlobalSettings(normalizedGlobal);
        setInitialGlobalSettings(normalizedGlobal);
        toast.success("Saved", "Global chat appearance updated.");
      }
    } catch (err) {
      console.error("Failed to save chat appearance:", err);
      toast.error("Save failed", err instanceof Error ? err.message : String(err));
    } finally {
      setIsSaving(false);
    }
  }, [
    isDirty,
    isSaving,
    mode,
    effectiveSettings,
    globalSettings,
    persistCharacter,
    persistGlobal,
  ]);

  const handleDiscard = useCallback(() => {
    if (!isDirty) return;
    if (mode === "character") {
      setCharacterOverride(initialCharacterOverride);
    } else {
      setGlobalSettings(initialGlobalSettings);
    }
  }, [isDirty, mode, initialCharacterOverride, initialGlobalSettings]);

  useEffect(() => {
    const handleDiscardEvent = () => {
      handleDiscard();
    };
    window.addEventListener("unsaved:discard", handleDiscardEvent);
    return () => window.removeEventListener("unsaved:discard", handleDiscardEvent);
  }, [handleDiscard]);

  useEffect(() => {
    const globalWindow = window as any;
    globalWindow.__saveChatAppearance = () => {
      void handleSave();
    };
    globalWindow.__saveChatAppearanceCanSave = isDirty;
    globalWindow.__saveChatAppearanceSaving = isSaving;
    return () => {
      delete globalWindow.__saveChatAppearance;
      delete globalWindow.__saveChatAppearanceCanSave;
      delete globalWindow.__saveChatAppearanceSaving;
    };
  }, [handleSave, isDirty, isSaving]);

  useEffect(() => {
    if (typeof window === "undefined") return;

    const mediaQuery = window.matchMedia("(max-width: 1023px)");
    const syncViewport = () => {
      const matches = mediaQuery.matches;
      setIsMobileViewport(matches);
      if (!matches) {
        setMobilePreviewOpen(false);
      }
    };

    syncViewport();
    mediaQuery.addEventListener("change", syncViewport);
    return () => mediaQuery.removeEventListener("change", syncViewport);
  }, []);

  useEffect(() => {
    const globalWindow = window as any;
    globalWindow.__openChatAppearancePreview = () => {
      setMobilePreviewOpen(true);
    };
    globalWindow.__closeChatAppearancePreview = () => {
      setMobilePreviewOpen(false);
    };

    return () => {
      delete globalWindow.__openChatAppearancePreview;
      delete globalWindow.__closeChatAppearancePreview;
    };
  }, []);

  // JS-based sticky for the preview panel (CSS sticky broken by framer-motion ancestor transforms).
  // Uses getBoundingClientRect each frame — no pre-measurement, no scroll-container guessing.
  const previewRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (isLoading) return;
    const preview = previewRef.current;
    if (!preview) return;

    const mq = window.matchMedia("(min-width: 1024px)");
    let raf = 0;

    // Where the preview should stick (px from viewport top)
    const mainEl = preview.closest("main") as HTMLElement | null;
    const headerH = parseFloat(getComputedStyle(mainEl ?? document.body).paddingTop) || 72;
    const targetTop = headerH + 16;

    const tick = () => {
      if (!mq.matches) {
        preview.style.transform = "";
        return;
      }
      // Subtract any existing translateY to recover natural position
      const m = preview.style.transform.match(/translateY\((-?[\d.]+)px\)/);
      const curTy = m ? parseFloat(m[1]) : 0;
      const naturalTop = preview.getBoundingClientRect().top - curTy;

      if (naturalTop < targetTop) {
        preview.style.transform = `translateY(${targetTop - naturalTop}px)`;
      } else {
        preview.style.transform = "";
      }
    };

    const onScroll = () => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(tick);
    };

    // Capture-phase listener on document catches scroll from ANY element
    document.addEventListener("scroll", onScroll, { passive: true, capture: true });
    window.addEventListener("resize", onScroll, { passive: true });
    mq.addEventListener("change", onScroll);
    requestAnimationFrame(tick);

    return () => {
      cancelAnimationFrame(raf);
      document.removeEventListener("scroll", onScroll, { capture: true });
      window.removeEventListener("resize", onScroll);
      mq.removeEventListener("change", onScroll);
      preview.style.transform = "";
    };
  }, [isLoading]);

  if (isLoading) return null;

  const previewHeader = character ? (
    <div className="flex items-center justify-end gap-3">
      <button
        type="button"
        onClick={() => setLivePreview((v) => !v)}
        className={cn(
          "flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[10px] font-medium transition-all",
          livePreview
            ? "border-accent/40 bg-accent/15 text-accent"
            : "border-fg/10 bg-fg/5 text-fg/40 hover:text-fg/60",
        )}
      >
        <Eye size={11} />
        {livePreview ? t("chatAppearance.preview.live") : t("chatAppearance.preview.generic")}
      </button>
    </div>
  ) : null;

  const previewSurface = (
    <div className="space-y-3">
      {character && previewHeader}
      <LivePreview
        settings={effectiveSettings}
        character={character}
        persona={persona}
        liveMode={livePreview}
        backgroundUrl={backgroundUrl}
      />
    </div>
  );


  return (
    <div className="px-3 pt-4 pb-24 lg:px-8 lg:pt-6 lg:pb-12">
      {mode === "character" && character && (
        <div className="mb-5 rounded-lg border border-accent/20 bg-accent/5 px-3 py-2 text-xs text-fg/60 lg:max-w-5xl lg:mx-auto">
          Customizing chat appearance for{" "}
          <span className="font-semibold text-fg">{character.name}</span>. Only changed settings
          override the global defaults.
        </div>
      )}

      {/* Desktop: two-column layout with sticky preview on the right */}
      <div className="lg:flex lg:items-start lg:gap-8 lg:max-w-5xl lg:mx-auto">
        {/* Settings column */}
        <div className="flex-1 min-w-0 space-y-4">
          <button
            type="button"
            onClick={resetAll}
            className={cn(
              "flex w-full items-center justify-center gap-2 rounded-xl border py-2.5 text-xs font-medium transition-all",
              "border-fg/10 bg-fg/5 text-fg/50 hover:border-fg/20 hover:bg-fg/10 hover:text-fg/70",
            )}
          >
            <RefreshCw size={13} />
            {mode === "character" ? "Clear all overrides" : "Reset all to defaults"}
          </button>

          <AppearanceTabBar activeTab={activeTab} onChange={setActiveTab} />

          <ChatAppearanceForm
            settings={effectiveSettings}
            mode={mode}
            activeTab={activeTab}
            onUpdate={updateField}
            onResetField={resetField}
            isOverridden={isOverridden}
          />
        </div>

        {!isMobileViewport && (
          <div
            ref={previewRef}
            className="mb-5 lg:mb-0 lg:w-130 lg:shrink-0 lg:will-change-transform lg:max-h-[calc(100vh-8rem)] lg:overflow-y-auto"
          >
            {previewSurface}
          </div>
        )}
      </div>

      <AnimatePresence>
        {isMobileViewport && mobilePreviewOpen && (
          <motion.div
            className="fixed inset-x-0 bottom-0 top-[var(--titlebar-h,0px)] z-50 flex flex-col bg-surface"
            style={{ paddingTop: "env(safe-area-inset-top)" }}
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 20 }}
            transition={{ duration: 0.2 }}
          >
            <div className="flex items-center justify-between border-b border-fg/10 px-4 py-3">
              <div className="text-base font-semibold text-fg">
                {t("chatAppearance.preview.label")}
              </div>
              <div className="flex items-center gap-2">
                {character && (
                  <button
                    type="button"
                    onClick={() => setLivePreview((v) => !v)}
                    className={cn(
                      "rounded-full border px-3 py-1.5 text-xs font-medium transition",
                      livePreview
                        ? "border-accent/40 bg-accent/15 text-accent"
                        : "border-fg/10 text-fg/70 hover:bg-fg/10 hover:text-fg",
                    )}
                  >
                    {livePreview
                      ? t("chatAppearance.preview.live")
                      : t("chatAppearance.preview.generic")}
                  </button>
                )}
                <button
                  type="button"
                  onClick={() => setMobilePreviewOpen(false)}
                  className="rounded-full border border-fg/10 px-3 py-1.5 text-xs font-medium text-fg/70 transition hover:bg-fg/10 hover:text-fg"
                >
                  Close
                </button>
              </div>
            </div>

            <div className="flex-1 overflow-y-auto px-4 pb-6 pt-4">
              <div className="space-y-4">
                <div className="space-y-2">
                  <LivePreview
                    settings={effectiveSettings}
                    character={character}
                    persona={persona}
                    liveMode={livePreview}
                    backgroundUrl={backgroundUrl}
                  />
                  <p className="text-[11px] text-fg/40">
                    See your appearance changes without leaving the current controls.
                  </p>
                </div>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
