import { useEffect, useRef, useState, useCallback, useMemo } from "react";
import { useSearchParams } from "react-router-dom";
import { RotateCcw, Bot, User, RefreshCw, Eye, ChevronDown } from "lucide-react";
import {
  readSettings,
  saveAdvancedSettings,
  saveCharacter,
  listCharacters,
  getDefaultPersona,
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
import { useAvatar } from "../../hooks/useAvatar";
import { useImageData } from "../../hooks/useImageData";
import { AvatarImage } from "../../components/AvatarImage";
import { toast } from "../../components/toast";
import { normalizeHexColor } from "../../../core/utils/imageAnalysis";

type AppearanceKey = keyof ChatAppearanceSettings;

function normalizeOverride(override: ChatAppearanceOverride): ChatAppearanceOverride {
  const normalized = { ...override } as ChatAppearanceOverride;
  normalized.userBubbleColorHex = normalizeHexColor(override.userBubbleColorHex);
  normalized.assistantBubbleColorHex = normalizeHexColor(override.assistantBubbleColorHex);
  return Object.fromEntries(
    Object.entries(normalized)
      .filter(([_, value]) => value !== undefined)
      .sort(([a], [b]) => a.localeCompare(b)),
  ) as ChatAppearanceOverride;
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
  };
}

// Option grid component for enum-based settings
function OptionGrid<T extends string>({
  label,
  value,
  options,
  onChange,
  overridden,
  onReset,
}: {
  label: string;
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
  overridden?: boolean;
  onReset?: () => void;
}) {
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium text-fg/60">{label}</span>
        {overridden && onReset && (
          <button
            type="button"
            onClick={onReset}
            className="flex items-center gap-1 text-[10px] text-accent/70 hover:text-accent"
          >
            <RotateCcw size={10} />
            Reset
          </button>
        )}
      </div>
      <div className={`grid gap-1.5 ${options.length <= 3 ? "grid-cols-3" : "grid-cols-4"}`}>
        {options.map((opt) => (
          <button
            key={opt.value}
            type="button"
            onClick={() => onChange(opt.value)}
            className={cn(
              "rounded-lg border py-2 text-[11px] font-medium transition-all",
              value === opt.value
                ? "border-accent/50 bg-accent/10 text-accent"
                : "border-fg/5 bg-fg/5 text-fg/40 hover:bg-fg/10",
            )}
          >
            {opt.label}
          </button>
        ))}
      </div>
    </div>
  );
}

// Slider component for numeric settings
function SliderControl({
  label,
  value,
  min,
  max,
  step = 1,
  unit = "",
  onChange,
  overridden,
  onReset,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  unit?: string;
  onChange: (v: number) => void;
  overridden?: boolean;
  onReset?: () => void;
}) {
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium text-fg/60">{label}</span>
        <div className="flex items-center gap-2">
          {overridden && onReset && (
            <button
              type="button"
              onClick={onReset}
              className="flex items-center gap-1 text-[10px] text-accent/70 hover:text-accent"
            >
              <RotateCcw size={10} />
              Reset
            </button>
          )}
          <span className="text-[11px] text-fg/50">
            {value}
            {unit}
          </span>
        </div>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="w-full accent-accent"
      />
    </div>
  );
}

function HexColorControl({
  label,
  value,
  onChange,
  overridden,
  onReset,
}: {
  label: string;
  value?: string;
  onChange: (v: string | undefined) => void;
  overridden?: boolean;
  onReset?: () => void;
}) {
  const [draft, setDraft] = useState(value ?? "");
  useEffect(() => {
    setDraft(value ?? "");
  }, [value]);

  const applyDraft = useCallback(() => {
    onChange(normalizeHexColor(draft));
  }, [draft, onChange]);

  const swatch = normalizeHexColor(draft) ?? "#000000";

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium text-fg/60">{label}</span>
        <div className="flex items-center gap-2">
          {overridden && onReset && (
            <button
              type="button"
              onClick={onReset}
              className="flex items-center gap-1 text-[10px] text-accent/70 hover:text-accent"
            >
              <RotateCcw size={10} />
              Reset
            </button>
          )}
          <button
            type="button"
            onClick={() => onChange(undefined)}
            className="text-[10px] text-fg/45 transition hover:text-fg/70"
          >
            Use token
          </button>
        </div>
      </div>
      <div className="flex items-center gap-2">
        <input
          type="text"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={applyDraft}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              applyDraft();
              (e.currentTarget as HTMLInputElement).blur();
            }
          }}
          placeholder="#00FFAA"
          className="h-9 flex-1 rounded-lg border border-fg/10 bg-fg/5 px-3 text-xs text-fg outline-none transition focus:border-accent/40"
        />
        <input
          type="color"
          value={swatch}
          onChange={(e) => {
            setDraft(e.target.value);
            onChange(normalizeHexColor(e.target.value));
          }}
          className="h-9 w-12 cursor-pointer rounded-md border border-fg/15 bg-fg/5 p-1"
          aria-label={`${label} picker`}
        />
      </div>
      {draft.trim().length > 0 && !normalizeHexColor(draft) && (
        <p className="text-[10px] text-warning">Use 3, 4, 6, or 8-digit hex. Example: #22CCAA</p>
      )}
    </div>
  );
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
  const userBubbleStyle = isMinimal
    ? undefined
    : {
        backgroundColor: `color-mix(in oklab, ${userColor} ${opacity}%, transparent)`,
        borderColor: `color-mix(in oklab, ${userColor} 50%, transparent)`,
      };
  const assistantBubbleStyle = isMinimal
    ? undefined
    : settings.assistantBubbleColor === "neutral" && !assistantHex
      ? {
          backgroundColor: "color-mix(in oklab, var(--color-fg) 5%, transparent)",
          borderColor: "color-mix(in oklab, var(--color-fg) 10%, transparent)",
        }
      : {
          backgroundColor: `color-mix(in oklab, ${assistantColor} ${opacity}%, transparent)`,
          borderColor: `color-mix(in oklab, ${assistantColor} 50%, transparent)`,
        };

  const sampleMessages: { role: "assistant" | "user"; text: string }[] = [
    { role: "assistant", text: "Hey! How are you doing today?" },
    { role: "user", text: "I'm doing great, thanks for asking!" },
    {
      role: "assistant",
      text: "That's wonderful to hear! I was just thinking about what we talked about last time.",
    },
    { role: "user", text: "Oh right, the trip we were planning. Did you find any good spots?" },
    {
      role: "assistant",
      text: "I did! There's a cozy little place by the lake that I think you'd absolutely love. The scenery is breathtaking.",
    },
    { role: "user", text: "That sounds amazing, let's go this weekend!" },
  ];

  const useLive = liveMode && character;
  const hasBg = useLive && backgroundUrl;

  return (
    <div
      className={cn(
        "relative overflow-hidden rounded-xl border border-fg/10 p-3",
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
          }}
        />
      )}
      {hasBg && settings.backgroundBlur > 0 && (
        <div
          className="absolute inset-0 transform-gpu backdrop-blur-md will-change-opacity"
          style={{
            opacity: Math.min(1, settings.backgroundBlur / 20),
            backgroundColor: "rgba(0, 0, 0, 0.01)",
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
        {sampleMessages.map((msg, i) =>
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
                  "text-fg",
                )}
                style={assistantBubbleStyle}
              >
                {msg.text}
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
                  "text-fg",
                )}
                style={userBubbleStyle}
              >
                {msg.text}
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
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
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
    async (next: ChatAppearanceOverride) => {
      if (!character) throw new Error("Character not loaded");
      const normalized = normalizeOverride(next);
      return saveCharacter({
        ...character,
        chatAppearance: Object.keys(normalized).length > 0 ? normalized : undefined,
      });
    },
    [character],
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
        const saved = await persistCharacter(characterOverride);
        const nextOverride = normalizeOverride(saved.chatAppearance ?? characterOverride);
        setCharacter({ ...saved, chatAppearance: nextOverride });
        setCharacterOverride(nextOverride);
        setInitialCharacterOverride(nextOverride);
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
  }, [isDirty, isSaving, mode, characterOverride, globalSettings, persistCharacter, persistGlobal]);

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

  const settingsContent = (
    <>
      {/* Reset button */}
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

      {/* Typography */}
      <div>
        <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
          Typography
        </h2>
        <div className="space-y-4 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
          <OptionGrid
            label="Font Size"
            value={effectiveSettings.fontSize}
            options={[
              { value: "small", label: "Small" },
              { value: "medium", label: "Medium" },
              { value: "large", label: "Large" },
              { value: "xlarge", label: "X-Large" },
            ]}
            onChange={(v) => updateField("fontSize", v)}
            overridden={isOverridden("fontSize")}
            onReset={mode === "character" ? () => resetField("fontSize") : undefined}
          />
          <OptionGrid
            label="Line Spacing"
            value={effectiveSettings.lineSpacing}
            options={[
              { value: "tight", label: "Tight" },
              { value: "normal", label: "Normal" },
              { value: "relaxed", label: "Relaxed" },
            ]}
            onChange={(v) => updateField("lineSpacing", v)}
            overridden={isOverridden("lineSpacing")}
            onReset={mode === "character" ? () => resetField("lineSpacing") : undefined}
          />
        </div>
      </div>

      {/* Message Bubbles */}
      <div>
        <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
          Message Bubbles
        </h2>
        <div className="space-y-4 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
          <OptionGrid
            label="Style"
            value={effectiveSettings.bubbleStyle}
            options={[
              { value: "bordered", label: "Bordered" },
              { value: "filled", label: "Filled" },
              { value: "minimal", label: "Minimal" },
            ]}
            onChange={(v) => updateField("bubbleStyle", v)}
            overridden={isOverridden("bubbleStyle")}
            onReset={mode === "character" ? () => resetField("bubbleStyle") : undefined}
          />
          <OptionGrid
            label="Corner Radius"
            value={effectiveSettings.bubbleRadius}
            options={[
              { value: "sharp", label: "Sharp" },
              { value: "rounded", label: "Rounded" },
              { value: "pill", label: "Pill" },
            ]}
            onChange={(v) => updateField("bubbleRadius", v)}
            overridden={isOverridden("bubbleRadius")}
            onReset={mode === "character" ? () => resetField("bubbleRadius") : undefined}
          />
          <OptionGrid
            label="Max Width"
            value={effectiveSettings.bubbleMaxWidth}
            options={[
              { value: "compact", label: "Compact" },
              { value: "normal", label: "Normal" },
              { value: "wide", label: "Wide" },
            ]}
            onChange={(v) => updateField("bubbleMaxWidth", v)}
            overridden={isOverridden("bubbleMaxWidth")}
            onReset={mode === "character" ? () => resetField("bubbleMaxWidth") : undefined}
          />
          <OptionGrid
            label="Padding"
            value={effectiveSettings.bubblePadding}
            options={[
              { value: "compact", label: "Compact" },
              { value: "normal", label: "Normal" },
              { value: "spacious", label: "Spacious" },
            ]}
            onChange={(v) => updateField("bubblePadding", v)}
            overridden={isOverridden("bubblePadding")}
            onReset={mode === "character" ? () => resetField("bubblePadding") : undefined}
          />
        </div>
      </div>

      {/* Layout */}
      <div>
        <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
          Layout
        </h2>
        <div className="space-y-4 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
          <OptionGrid
            label="Message Spacing"
            value={effectiveSettings.messageGap}
            options={[
              { value: "tight", label: "Tight" },
              { value: "normal", label: "Normal" },
              { value: "relaxed", label: "Relaxed" },
            ]}
            onChange={(v) => updateField("messageGap", v)}
            overridden={isOverridden("messageGap")}
            onReset={mode === "character" ? () => resetField("messageGap") : undefined}
          />
          <OptionGrid
            label="Avatar Shape"
            value={effectiveSettings.avatarShape}
            options={[
              { value: "circle", label: "Circle" },
              { value: "rounded", label: "Rounded" },
              { value: "hidden", label: "Hidden" },
            ]}
            onChange={(v) => updateField("avatarShape", v)}
            overridden={isOverridden("avatarShape")}
            onReset={mode === "character" ? () => resetField("avatarShape") : undefined}
          />
          <OptionGrid
            label="Avatar Size"
            value={effectiveSettings.avatarSize}
            options={[
              { value: "small", label: "Small" },
              { value: "medium", label: "Medium" },
              { value: "large", label: "Large" },
            ]}
            onChange={(v) => updateField("avatarSize", v)}
            overridden={isOverridden("avatarSize")}
            onReset={mode === "character" ? () => resetField("avatarSize") : undefined}
          />
        </div>
      </div>

      {/* Colors */}
      <div>
        <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
          Colors
        </h2>
        <div className="space-y-4 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
          <OptionGrid
            label="User Bubble Color"
            value={effectiveSettings.userBubbleColor}
            options={[
              { value: "accent", label: "Accent" },
              { value: "info", label: "Info" },
              { value: "secondary", label: "Secondary" },
              { value: "warning", label: "Warning" },
            ]}
            onChange={(v) => updateField("userBubbleColor", v)}
            overridden={isOverridden("userBubbleColor")}
            onReset={mode === "character" ? () => resetField("userBubbleColor") : undefined}
          />
          <OptionGrid
            label="Assistant Bubble Color"
            value={effectiveSettings.assistantBubbleColor}
            options={[
              { value: "neutral", label: "Neutral" },
              { value: "accent", label: "Accent" },
              { value: "info", label: "Info" },
              { value: "secondary", label: "Secondary" },
            ]}
            onChange={(v) => updateField("assistantBubbleColor", v)}
            overridden={isOverridden("assistantBubbleColor")}
            onReset={mode === "character" ? () => resetField("assistantBubbleColor") : undefined}
          />
          <HexColorControl
            label="User Bubble Hex Override"
            value={effectiveSettings.userBubbleColorHex}
            onChange={(v) => updateField("userBubbleColorHex", v)}
            overridden={isOverridden("userBubbleColorHex")}
            onReset={mode === "character" ? () => resetField("userBubbleColorHex") : undefined}
          />
          <HexColorControl
            label="Assistant Bubble Hex Override"
            value={effectiveSettings.assistantBubbleColorHex}
            onChange={(v) => updateField("assistantBubbleColorHex", v)}
            overridden={isOverridden("assistantBubbleColorHex")}
            onReset={mode === "character" ? () => resetField("assistantBubbleColorHex") : undefined}
          />
        </div>
      </div>

      {/* Background */}
      <div>
        <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
          Background & Transparency
        </h2>
        <div className="space-y-4 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
          <SliderControl
            label="Background Dim"
            value={effectiveSettings.backgroundDim}
            min={0}
            max={80}
            step={5}
            unit="%"
            onChange={(v) => updateField("backgroundDim", v)}
            overridden={isOverridden("backgroundDim")}
            onReset={mode === "character" ? () => resetField("backgroundDim") : undefined}
          />
          <SliderControl
            label="Background Blur"
            value={effectiveSettings.backgroundBlur}
            min={0}
            max={20}
            step={1}
            unit="px"
            onChange={(v) => updateField("backgroundBlur", v)}
            overridden={isOverridden("backgroundBlur")}
            onReset={mode === "character" ? () => resetField("backgroundBlur") : undefined}
          />
          <OptionGrid
            label="Bubble Blur"
            value={effectiveSettings.bubbleBlur}
            options={[
              { value: "none", label: "None" },
              { value: "light", label: "Light" },
              { value: "medium", label: "Medium" },
              { value: "heavy", label: "Heavy" },
            ]}
            onChange={(v) => updateField("bubbleBlur", v)}
            overridden={isOverridden("bubbleBlur")}
            onReset={mode === "character" ? () => resetField("bubbleBlur") : undefined}
          />
          <SliderControl
            label="Bubble Opacity"
            value={effectiveSettings.bubbleOpacity}
            min={20}
            max={100}
            step={5}
            unit="%"
            onChange={(v) => updateField("bubbleOpacity", v)}
            overridden={isOverridden("bubbleOpacity")}
            onReset={mode === "character" ? () => resetField("bubbleOpacity") : undefined}
          />
          <OptionGrid
            label="Text Color Mode"
            value={effectiveSettings.textMode}
            options={[
              { value: "auto", label: "Auto" },
              { value: "light", label: "Light" },
              { value: "dark", label: "Dark" },
            ]}
            onChange={(v) => updateField("textMode", v)}
            overridden={isOverridden("textMode")}
            onReset={mode === "character" ? () => resetField("textMode") : undefined}
          />
        </div>
      </div>

      <div className="h-4" />
    </>
  );

  return (
    <div className="pb-16">
      {mode === "character" && character && (
        <div className="mb-5 rounded-lg border border-accent/20 bg-accent/5 px-3 py-2 text-xs text-fg/60 lg:max-w-5xl lg:mx-auto">
          Customizing chat appearance for{" "}
          <span className="font-semibold text-fg">{character.name}</span>. Only changed settings
          override the global defaults.
        </div>
      )}

      {/* Desktop: two-column layout with sticky preview */}
      <div className="lg:flex lg:items-start lg:gap-8 lg:max-w-5xl lg:mx-auto">
        {/* Preview column — collapsible on mobile, sticky on desktop */}
        <div
          ref={previewRef}
          className="mb-5 lg:mb-0 lg:w-[520px] lg:shrink-0 lg:will-change-transform"
        >
          <div className="mb-2 flex items-center justify-between px-1">
            <button
              type="button"
              onClick={() => setMobilePreviewOpen((v) => !v)}
              className="flex items-center gap-1.5 lg:pointer-events-none"
            >
              <h2 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
                Preview
              </h2>
              <ChevronDown
                size={12}
                className={cn(
                  "text-fg/30 transition-transform lg:hidden",
                  mobilePreviewOpen && "rotate-180",
                )}
              />
            </button>
            <div className="flex items-center gap-2">
              {character && (
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
                  {livePreview ? "Live" : "Generic"}
                </button>
              )}
            </div>
          </div>
          {/* Always visible on desktop (lg+), collapsible on mobile */}
          <div className={cn(
            "overflow-hidden transition-all duration-200",
            mobilePreviewOpen ? "max-h-[2000px] opacity-100" : "max-h-0 opacity-0",
            "lg:max-h-none lg:opacity-100",
          )}>
            <LivePreview
              settings={effectiveSettings}
              character={character}
              persona={persona}
              liveMode={livePreview}
              backgroundUrl={backgroundUrl}
            />
          </div>
        </div>

        {/* Settings column */}
        <div className="flex-1 min-w-0 space-y-5">{settingsContent}</div>
      </div>
    </div>
  );
}
