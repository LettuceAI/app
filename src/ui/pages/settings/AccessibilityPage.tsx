import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Volume2, Play, Smartphone, Palette, MessageSquare, ChevronRight } from "lucide-react";
import { type as getPlatform } from "@tauri-apps/plugin-os";
import { impactFeedback } from "@tauri-apps/plugin-haptics";
import { readSettings, saveAdvancedSettings } from "../../../core/storage/repo";
import {
  createDefaultAccessibilitySettings,
  type AccessibilitySettings,
} from "../../../core/storage/schemas";
import { playAccessibilitySound } from "../../../core/utils/accessibilityAudio";
import { cn, radius, colors, interactive } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import { LocaleSelector } from "../../components/LocaleSelector";
import { Switch } from "../../components/Switch";

const SOUND_KEYS = ["send", "success", "failure"] as const;

type SoundKey = (typeof SOUND_KEYS)[number];

const HAPTIC_INTENSITIES = [
  { value: "light", labelKey: "accessibility.haptics.light" as const },
  { value: "medium", labelKey: "accessibility.haptics.medium" as const },
  { value: "heavy", labelKey: "accessibility.haptics.heavy" as const },
  { value: "soft", labelKey: "accessibility.haptics.soft" as const },
  { value: "rigid", labelKey: "accessibility.haptics.rigid" as const },
] as const;

type HapticIntensity = (typeof HAPTIC_INTENSITIES)[number]["value"];

function volumeToPercent(value: number): number {
  return Math.round(Math.max(0, Math.min(1, value)) * 100);
}

function percentToVolume(value: number): number {
  return Math.max(0, Math.min(1, value / 100));
}

export function AccessibilityPage() {
  const navigate = useNavigate();
  const { locale, setLocale, t } = useI18n();
  const [accessibility, setAccessibility] = useState<AccessibilitySettings>(
    createDefaultAccessibilitySettings(),
  );
  const [isLoading, setIsLoading] = useState(true);
  const [platform, setPlatform] = useState<string>("");

  useEffect(() => {
    setPlatform(getPlatform());
    const loadSettings = async () => {
      try {
        const settings = await readSettings();
        const next =
          settings.advancedSettings?.accessibility ?? createDefaultAccessibilitySettings();
        setAccessibility(next);
      } catch (error) {
        console.error("Failed to load accessibility settings:", error);
      } finally {
        setIsLoading(false);
      }
    };

    void loadSettings();
  }, []);

  const isMobile = platform === "android" || platform === "ios";

  const persistAccessibility = async (next: AccessibilitySettings) => {
    try {
      const settings = await readSettings();
      const advancedSettings = {
        ...(settings.advancedSettings ?? {}),
        creationHelperEnabled: settings.advancedSettings?.creationHelperEnabled ?? false,
        helpMeReplyEnabled: settings.advancedSettings?.helpMeReplyEnabled ?? true,
        accessibility: next,
      };
      await saveAdvancedSettings(advancedSettings);
    } catch (error) {
      console.error("Failed to save accessibility settings:", error);
    }
  };

  const updateSound = (
    key: SoundKey,
    updater: (current: AccessibilitySettings[SoundKey]) => AccessibilitySettings[SoundKey],
  ) => {
    setAccessibility((prev) => {
      const next = {
        ...prev,
        [key]: updater(prev[key]),
      };
      void persistAccessibility(next);
      return next;
    });
  };

  const updateHaptics = (enabled: boolean) => {
    setAccessibility((prev) => {
      const next = {
        ...prev,
        haptics: enabled,
      };
      void persistAccessibility(next);
      return next;
    });
  };

  const handleIntensityChange = (intensity: HapticIntensity) => {
    setAccessibility((prev) => {
      const next = {
        ...prev,
        hapticIntensity: intensity,
      };
      void persistAccessibility(next);
      return next;
    });
    // Visual/Tactile preview
    if (isMobile) {
      void impactFeedback(intensity);
    }
  };

  const handleTest = (key: SoundKey) => {
    const previewSettings: AccessibilitySettings = {
      ...accessibility,
      [key]: { ...accessibility[key], enabled: true },
    };
    playAccessibilitySound(key, previewSettings);
  };

  if (isLoading) {
    return null;
  }

  return (
    <div className="flex h-full flex-col pb-16">
      <section className="flex-1 overflow-y-auto px-3 pt-3 space-y-6">
        <div>
          <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
            {t("accessibility.sectionTitles.language")}
          </h2>
          <div className={cn("rounded-xl border px-4 py-3", "border-fg/10 bg-fg/5")}>
            <LocaleSelector
              value={locale}
              onChange={setLocale}
              label={t("accessibility.language.appLanguage")}
              description={t("accessibility.language.description")}
              title={t("components.localeSelector.title")}
            />
          </div>
        </div>

        <div>
          <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
            {t("accessibility.sectionTitles.sounds")}
          </h2>
          <div className="space-y-3">
            {SOUND_KEYS.map((key) => {
              const sound = accessibility[key];
              return (
                <div
                  key={key}
                  className={cn(
                    "rounded-xl border px-4 py-3",
                    sound.enabled ? "border-accent/25 bg-fg/6" : "border-fg/10 bg-fg/5",
                  )}
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="flex items-start gap-3">
                      <div
                        className={cn(
                          "flex h-8 w-8 shrink-0 items-center justify-center rounded-full border",
                          sound.enabled ? "border-accent/40 bg-accent/15" : "border-fg/10 bg-fg/10",
                        )}
                      >
                        <Volume2 className="h-4 w-4 text-fg/70" />
                      </div>
                      <div>
                        <div className="text-sm font-medium text-fg">
                          {t(`accessibility.sounds.${key}` as const)}
                        </div>
                        <div className="mt-0.5 text-[11px] text-fg/45">
                          {t(`accessibility.sounds.${key}Description` as const)}
                        </div>
                      </div>
                    </div>
                    <Switch
                      id={`accessibility-${key}-enabled`}
                      checked={sound.enabled}
                      onChange={(next) =>
                        updateSound(key, (current) => ({ ...current, enabled: next }))
                      }
                    />
                  </div>

                  <div className="mt-3 flex items-center gap-3">
                    <input
                      type="range"
                      min={0}
                      max={100}
                      value={volumeToPercent(sound.volume)}
                      onChange={(event) => {
                        const nextVolume = percentToVolume(Number(event.target.value));
                        updateSound(key, (current) => ({ ...current, volume: nextVolume }));
                      }}
                      className="flex-1 accent-accent"
                    />
                    <span className="w-10 text-right text-[11px] text-fg/50">
                      {volumeToPercent(sound.volume)}%
                    </span>
                    <button
                      type="button"
                      onClick={() => handleTest(key)}
                      className={cn(
                        "flex h-8 items-center gap-1.5 px-3 text-xs font-medium text-fg/80",
                        radius.full,
                        "border border-fg/15 bg-fg/5",
                        interactive.transition.fast,
                        "hover:border-fg/25 hover:bg-fg/10",
                      )}
                    >
                      <Play className="h-3.5 w-3.5" />
                      {t("accessibility.sounds.testButton")}
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        {isMobile && (
          <div>
            <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
              {t("accessibility.sectionTitles.haptics")}
            </h2>
            <div className="space-y-4">
              <div
                className={cn(
                  "rounded-xl border px-4 py-4",
                  accessibility.haptics ? "border-accent/25 bg-fg/6" : "border-fg/10 bg-fg/5",
                )}
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="flex items-start gap-3">
                    <div
                      className={cn(
                        "flex h-8 w-8 shrink-0 items-center justify-center rounded-full border",
                        accessibility.haptics
                          ? "border-accent/40 bg-accent/15"
                          : "border-fg/10 bg-fg/10",
                      )}
                    >
                      <Smartphone className="h-4 w-4 text-fg/70" />
                    </div>
                    <div>
                      <div className="text-sm font-medium text-fg">
                        {t("accessibility.haptics.vibrateOnChat")}
                      </div>
                      <div className="mt-0.5 text-[11px] text-fg/45">
                        {t("accessibility.haptics.vibrateDesc")}
                      </div>
                    </div>
                  </div>
                  <Switch
                    id="accessibility-haptics-enabled"
                    checked={accessibility.haptics}
                    onChange={updateHaptics}
                  />
                </div>

                {accessibility.haptics && (
                  <div className="mt-3">
                    <div className="mb-3 text-[10px] font-semibold uppercase tracking-wider text-fg/30">
                      {t("accessibility.haptics.intensity")}
                    </div>
                    <div className="grid grid-cols-5 gap-1.5">
                      {HAPTIC_INTENSITIES.map((opt) => (
                        <button
                          key={opt.value}
                          type="button"
                          onClick={() => handleIntensityChange(opt.value)}
                          className={cn(
                            "flex flex-col items-center justify-center rounded-lg border py-2.5 transition-all",
                            accessibility.hapticIntensity === opt.value
                              ? "border-accent/50 bg-accent/10 text-accent"
                              : "border-fg/5 bg-fg/5 text-fg/40 hover:bg-fg/10",
                          )}
                        >
                          <span className="text-[10px] font-medium">{t(opt.labelKey)}</span>
                        </button>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            </div>
          </div>
        )}

        <div>
          <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
            {t("accessibility.sectionTitles.appearance")}
          </h2>
          <button
            type="button"
            onClick={() => navigate("/settings/accessibility/colors")}
            className={cn(
              "group flex w-full items-center gap-3 rounded-xl border px-4 py-3.5",
              "border-fg/10 bg-fg/5",
              interactive.transition.fast,
              "hover:border-fg/20 hover:bg-fg/10",
            )}
          >
            <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-fg/10 bg-fg/10">
              <Palette className="h-4 w-4 text-fg/70" />
            </div>
            <div className="flex-1 text-left">
              <div className="text-sm font-medium text-fg">
                {t("accessibility.appearance.customColors")}
              </div>
              <div className="mt-0.5 text-[11px] text-fg/45">
                {t("accessibility.appearance.customColorsDesc")}
              </div>
            </div>
            <ChevronRight className="h-4 w-4 shrink-0 text-fg/25 transition-colors group-hover:text-fg/50" />
          </button>
          <button
            type="button"
            onClick={() => navigate("/settings/accessibility/chat")}
            className={cn(
              "group flex w-full items-center gap-3 rounded-xl border px-4 py-3.5 mt-3",
              "border-fg/10 bg-fg/5",
              interactive.transition.fast,
              "hover:border-fg/20 hover:bg-fg/10",
            )}
          >
            <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-fg/10 bg-fg/10">
              <MessageSquare className="h-4 w-4 text-fg/70" />
            </div>
            <div className="flex-1 text-left">
              <div className="text-sm font-medium text-fg">
                {t("accessibility.appearance.chatAppearance")}
              </div>
              <div className="mt-0.5 text-[11px] text-fg/45">
                {t("accessibility.appearance.chatAppearanceDesc")}
              </div>
            </div>
            <ChevronRight className="h-4 w-4 shrink-0 text-fg/25 transition-colors group-hover:text-fg/50" />
          </button>
        </div>

        <div
          className={cn("rounded-xl border px-4 py-3 text-[11px] text-fg/45", colors.glass.subtle)}
        >
          {t("accessibility.feedbackInfo")}
          {isMobile ? ` ${t("accessibility.hapticsInfo")}` : ""}
        </div>
      </section>
    </div>
  );
}
