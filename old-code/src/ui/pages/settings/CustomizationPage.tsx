import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  Volume2,
  Play,
  Smartphone,
  Palette,
  MessageSquare,
  ChevronRight,
  Sparkles,
} from "lucide-react";
import { type as getPlatform } from "@tauri-apps/plugin-os";
import { impactFeedback } from "@tauri-apps/plugin-haptics";
import { readSettings, saveAdvancedSettings } from "../../../core/storage/repo";
import {
  createDefaultAccessibilitySettings,
  type AccessibilitySettings,
  type HeaderStyle,
  type NavigationSide,
  type NavigationStyle,
  type NavItemId,
  type NavAlign,
  type NavEdge,
} from "../../../core/storage/schemas";
import { playAccessibilitySound } from "../../../core/utils/accessibilityAudio";
import { cn, radius, colors, interactive } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import { LocaleSelector } from "../../components/LocaleSelector";
import { Switch } from "../../components/Switch";
import { NavItemsEditor } from "../../components/App/NavItemsEditor";
import {
  readTitleBarDesign,
  setTitleBarDesign,
  readTitleBarSide,
  setTitleBarSide,
  readTitleBarSize,
  setTitleBarSize,
  readWindowCorners,
  setWindowCorners,
  type TitleBarDesign,
  type TitleBarSide,
  type TitleBarSize,
  type WindowCorners,
} from "../../components/App/TitleBar";

const TITLE_BAR_OPTIONS = [
  {
    value: "classic" as const,
    labelKey: "accessibility.titleBar.classic" as const,
    descKey: "accessibility.titleBar.classicDesc" as const,
  },
  {
    value: "lights" as const,
    labelKey: "accessibility.titleBar.lights" as const,
    descKey: "accessibility.titleBar.lightsDesc" as const,
  },
  {
    value: "lights_dimmed" as const,
    labelKey: "accessibility.titleBar.lightsDimmed" as const,
    descKey: "accessibility.titleBar.lightsDimmedDesc" as const,
  },
  {
    value: "minimal" as const,
    labelKey: "accessibility.titleBar.minimal" as const,
    descKey: "accessibility.titleBar.minimalDesc" as const,
  },
  {
    value: "native" as const,
    labelKey: "accessibility.titleBar.native" as const,
    descKey: "accessibility.titleBar.nativeDesc" as const,
  },
] as const;

const NAV_STYLE_OPTIONS = [
  {
    value: "bottom" as const,
    labelKey: "accessibility.navigation.bottom" as const,
    descKey: "accessibility.navigation.bottomDesc" as const,
  },
  {
    value: "bottomLabels" as const,
    labelKey: "accessibility.navigation.bottomLabels" as const,
    descKey: "accessibility.navigation.bottomLabelsDesc" as const,
  },
  {
    value: "dock" as const,
    labelKey: "accessibility.navigation.dock" as const,
    descKey: "accessibility.navigation.dockDesc" as const,
  },
  {
    value: "sidebar" as const,
    labelKey: "accessibility.navigation.sidebar" as const,
    descKey: "accessibility.navigation.sidebarDesc" as const,
  },
  {
    value: "floatingSidebar" as const,
    labelKey: "accessibility.navigation.floatingSidebar" as const,
    descKey: "accessibility.navigation.floatingSidebarDesc" as const,
  },
  {
    value: "header" as const,
    labelKey: "accessibility.navigation.insideHeader" as const,
    descKey: "accessibility.navigation.insideHeaderDesc" as const,
  },
] as const;

const HEADER_STYLE_OPTIONS = ["auto", "attached", "floating", "inline"] as const;

const SIDEBAR_NAV_STYLES: readonly NavigationStyle[] = ["sidebar", "floatingSidebar"];
const DESKTOP_ONLY_NAV_STYLES: readonly NavigationStyle[] = [
  "sidebar",
  "floatingSidebar",
  "header",
];

function NavStylePreview({ style, selected }: { style: NavigationStyle; selected: boolean }) {
  const strip = selected ? "bg-accent/30 border-accent/30" : "bg-fg/15 border-fg/20";
  const pill = selected ? "bg-accent/50" : "bg-fg/30";
  const dot = selected ? "bg-accent/80" : "bg-fg/50";
  return (
    <span
      className={cn(
        "relative block h-16 w-24 overflow-hidden rounded-lg border",
        selected ? "border-accent/30 bg-accent/[0.04]" : "border-fg/15 bg-fg/[0.03]",
      )}
    >
      <span className="absolute left-1.5 right-1.5 top-1.5 h-1 rounded-full bg-fg/10" />
      {style === "header" ? (
        <span
          className={cn(
            "absolute left-0 right-0 top-0 flex h-4 items-center justify-center gap-1 border-b",
            strip,
          )}
        >
          <span className={cn("h-1 w-1 rounded-full", dot)} />
          <span className={cn("h-1 w-1 rounded-full", dot)} />
          <span className={cn("h-1 w-1 rounded-full", dot)} />
        </span>
      ) : style === "sidebar" ? (
        <span
          className={cn(
            "absolute bottom-0 left-0 top-0 flex w-4 flex-col items-center justify-center gap-1 border-r",
            strip,
          )}
        >
          <span className={cn("h-1 w-1 rounded-full", dot)} />
          <span className={cn("h-1 w-1 rounded-full", dot)} />
          <span className={cn("h-1 w-1 rounded-full", dot)} />
        </span>
      ) : style === "floatingSidebar" ? (
        <span className={cn("absolute left-2 top-1/2 h-9 w-2.5 -translate-y-1/2 rounded-full", pill)} />
      ) : style === "dock" ? (
        <span className={cn("absolute bottom-2 left-1/2 h-2.5 w-12 -translate-x-1/2 rounded-full", pill)} />
      ) : (
        <span
          className={cn(
            "absolute bottom-0 left-0 right-0 flex items-center justify-center gap-1 border-t",
            style === "bottomLabels" ? "h-5" : "h-4",
            strip,
          )}
        >
          <span className={cn("h-1 w-1 rounded-full", dot)} />
          <span className={cn("h-1 w-1 rounded-full", dot)} />
          <span className={cn("h-1 w-1 rounded-full", dot)} />
        </span>
      )}
    </span>
  );
}

function TitleBarPreviewCard({
  design,
  selected,
}: {
  design: TitleBarDesign;
  selected: boolean;
}) {
  return (
    <span
      className={cn(
        "relative block h-16 w-24 overflow-hidden rounded-lg border",
        selected ? "border-accent/30 bg-accent/[0.04]" : "border-fg/15 bg-fg/[0.03]",
      )}
    >
      <span
        className={cn(
          "absolute left-0 right-0 top-0 flex h-6 items-center justify-end border-b px-1.5",
          selected ? "border-accent/30 bg-accent/10" : "border-fg/20 bg-fg/10",
        )}
      >
        <span className="scale-[0.8]">
          <TitleBarDesignPreview design={design} />
        </span>
      </span>
      <span className="absolute left-1.5 right-8 top-8 h-1 rounded-full bg-fg/10" />
      <span className="absolute left-1.5 right-12 top-11 h-1 rounded-full bg-fg/10" />
    </span>
  );
}

function TitleBarDesignPreview({ design }: { design: TitleBarDesign }) {
  if (design === "lights") {
    return (
      <span className="flex items-center gap-1.5">
        <span className="h-3 w-3 rounded-full bg-[#ff5f57]" />
        <span className="h-3 w-3 rounded-full bg-[#febc2e]" />
        <span className="h-3 w-3 rounded-full bg-[#28c840]" />
      </span>
    );
  }
  if (design === "lights_dimmed") {
    return (
      <span className="flex items-center gap-1.5">
        <span className="h-3 w-3 rounded-full bg-fg/20" />
        <span className="h-3 w-3 rounded-full bg-fg/20" />
        <span className="h-3 w-3 rounded-full bg-fg/20" />
      </span>
    );
  }
  if (design === "minimal") {
    return (
      <span className="flex items-center gap-2 text-fg/45">
        <span className="h-px w-2.5 bg-current" />
        <span className="h-2 w-2 border border-current" />
        <span className="relative h-2.5 w-2.5">
          <span className="absolute left-0 top-1/2 h-px w-full rotate-45 bg-current" />
          <span className="absolute left-0 top-1/2 h-px w-full -rotate-45 bg-current" />
        </span>
      </span>
    );
  }
  if (design === "native") {
    return (
      <span className="rounded border border-fg/20 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wider text-fg/40">
        OS
      </span>
    );
  }
  return (
    <span className="flex items-center text-fg/45">
      <span className="flex h-5 w-6 items-center justify-center">
        <span className="h-px w-2.5 bg-current" />
      </span>
      <span className="flex h-5 w-6 items-center justify-center">
        <span className="h-2 w-2 border border-current" />
      </span>
      <span className="flex h-5 w-6 items-center justify-center bg-red-500/80">
        <span className="relative h-2.5 w-2.5">
          <span className="absolute left-0 top-1/2 h-px w-full rotate-45 bg-white" />
          <span className="absolute left-0 top-1/2 h-px w-full -rotate-45 bg-white" />
        </span>
      </span>
    </span>
  );
}

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

export function CustomizationPage() {
  const navigate = useNavigate();
  const { locale, setLocale, t } = useI18n();
  const [accessibility, setAccessibility] = useState<AccessibilitySettings>(
    createDefaultAccessibilitySettings(),
  );
  const [isLoading, setIsLoading] = useState(true);
  const [platform, setPlatform] = useState<string>("");
  const [isBeetrootEnabled, setIsBeetrootEnabled] = useState(true);
  const [titleBarDesign, setTitleBarDesignState] = useState<TitleBarDesign>(readTitleBarDesign);
  const [navStyle, setNavStyleState] = useState<NavigationStyle>("bottom");
  const [navSide, setNavSideState] = useState<NavigationSide>("left");
  const [headerStyle, setHeaderStyleState] = useState<HeaderStyle>("auto");
  const [navItems, setNavItemsState] = useState<NavItemId[] | null>(null);
  const [navAlign, setNavAlignState] = useState<NavAlign>("start");
  const [navEdge, setNavEdgeState] = useState<NavEdge>("bottom");
  const [titleBarSide, setTitleBarSideState] = useState<TitleBarSide>(readTitleBarSide);
  const [titleBarSize, setTitleBarSizeState] = useState<TitleBarSize>(readTitleBarSize);

  const handleTitleBarDesignChange = (design: TitleBarDesign) => {
    setTitleBarDesignState(design);
    setTitleBarDesign(design);
  };

  const handleTitleBarSideChange = (side: TitleBarSide) => {
    setTitleBarSideState(side);
    setTitleBarSide(side);
  };

  const handleTitleBarSizeChange = (size: TitleBarSize) => {
    setTitleBarSizeState(size);
    setTitleBarSize(size);
  };

  const [windowCorners, setWindowCornersState] = useState<WindowCorners>(readWindowCorners);

  const handleWindowCornersChange = (corners: WindowCorners) => {
    setWindowCornersState(corners);
    setWindowCorners(corners);
  };

  useEffect(() => {
    setPlatform(getPlatform());
    const loadSettings = async () => {
      try {
        const settings = await readSettings();
        const next =
          settings.advancedSettings?.accessibility ?? createDefaultAccessibilitySettings();
        setAccessibility(next);
        setNavStyleState(settings.advancedSettings?.navigationStyle ?? "bottom");
        setNavSideState(settings.advancedSettings?.navigationSide ?? "left");
        setHeaderStyleState(settings.advancedSettings?.headerStyle ?? "auto");
        setNavItemsState(settings.advancedSettings?.navItems ?? null);
        setNavAlignState(settings.advancedSettings?.navAlign ?? "start");
        setNavEdgeState(settings.advancedSettings?.navEdge ?? "bottom");
      } catch (error) {
        console.error("Failed to load accessibility settings:", error);
      } finally {
        setIsLoading(false);
      }
    };

    void loadSettings();
    try {
      const stored = localStorage.getItem("lettuce.easterEggs.beetroot");
      if (stored !== null) {
        setIsBeetrootEnabled(stored === "true");
      }
    } catch (err) {
      console.error("Failed to read beetroot setting:", err);
    }
  }, []);

  const handleBeetrootToggle = () => {
    const newValue = !isBeetrootEnabled;
    setIsBeetrootEnabled(newValue);
    try {
      localStorage.setItem("lettuce.easterEggs.beetroot", String(newValue));
      window.dispatchEvent(new CustomEvent("lettuce:easterEggs:beetroot", { detail: newValue }));
    } catch (err) {
      console.error("Failed to save beetroot setting:", err);
      setIsBeetrootEnabled(!newValue);
    }
  };

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

  const persistNavigationStyle = async (next: NavigationStyle) => {
    const previous = navStyle;
    setNavStyleState(next);
    try {
      const settings = await readSettings();
      await saveAdvancedSettings({
        ...(settings.advancedSettings ?? {}),
        navigationStyle: next,
      });
    } catch (error) {
      console.error("Failed to save navigation style:", error);
      setNavStyleState(previous);
    }
  };

  const persistNavigationSide = async (next: NavigationSide) => {
    const previous = navSide;
    setNavSideState(next);
    try {
      const settings = await readSettings();
      await saveAdvancedSettings({
        ...(settings.advancedSettings ?? {}),
        navigationSide: next,
      });
    } catch (error) {
      console.error("Failed to save navigation side:", error);
      setNavSideState(previous);
    }
  };

  const persistNavItems = async (next: NavItemId[] | null) => {
    const previous = navItems;
    setNavItemsState(next);
    try {
      const settings = await readSettings();
      await saveAdvancedSettings({
        ...(settings.advancedSettings ?? {}),
        navItems: next ?? undefined,
      });
    } catch (error) {
      console.error("Failed to save navigation items:", error);
      setNavItemsState(previous);
    }
  };

  const persistNavAlign = async (next: NavAlign) => {
    const previous = navAlign;
    setNavAlignState(next);
    try {
      const settings = await readSettings();
      await saveAdvancedSettings({
        ...(settings.advancedSettings ?? {}),
        navAlign: next,
      });
    } catch (error) {
      console.error("Failed to save navigation placement:", error);
      setNavAlignState(previous);
    }
  };

  const persistNavEdge = async (next: NavEdge) => {
    const previous = navEdge;
    setNavEdgeState(next);
    try {
      const settings = await readSettings();
      await saveAdvancedSettings({
        ...(settings.advancedSettings ?? {}),
        navEdge: next,
      });
    } catch (error) {
      console.error("Failed to save navigation edge:", error);
      setNavEdgeState(previous);
    }
  };

  const persistHeaderStyle = async (next: HeaderStyle) => {
    const previous = headerStyle;
    setHeaderStyleState(next);
    try {
      const settings = await readSettings();
      await saveAdvancedSettings({
        ...(settings.advancedSettings ?? {}),
        headerStyle: next,
      });
    } catch (error) {
      console.error("Failed to save header style:", error);
      setHeaderStyleState(previous);
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
    <div className="flex h-full flex-col">
      <section className="flex-1 overflow-y-auto px-3 pt-3 pb-6 space-y-6">
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

        {!isMobile && (
          <div>
            <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
              {t("accessibility.sectionTitles.titleBar")}
            </h2>
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
              {TITLE_BAR_OPTIONS.map((option) => {
                const selected = titleBarDesign === option.value;
                return (
                  <button
                    key={option.value}
                    type="button"
                    onClick={() => handleTitleBarDesignChange(option.value)}
                    className={cn(
                      "flex flex-col items-center gap-3 rounded-xl border px-3 pb-3.5 pt-4 text-center",
                      interactive.transition.fast,
                      selected
                        ? "border-accent/25 bg-fg/6"
                        : "border-fg/10 bg-fg/5 hover:border-fg/20",
                    )}
                  >
                    <TitleBarPreviewCard design={option.value} selected={selected} />
                    <span className="min-w-0 space-y-1">
                      <span
                        className={cn(
                          "block text-[13px] font-medium leading-tight",
                          selected ? "text-accent" : "text-fg",
                        )}
                      >
                        {t(option.labelKey)}
                      </span>
                      <span className="block text-[11px] leading-snug text-fg/45">
                        {t(option.descKey)}
                      </span>
                    </span>
                  </button>
                );
              })}
            </div>
            {titleBarDesign !== "native" && (
              <>
                <div className="mt-2 flex items-center justify-between gap-3 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
                  <div className="min-w-0">
                    <div className="text-sm font-medium text-fg">
                      {t("accessibility.titleBar.position")}
                    </div>
                    <div className="mt-0.5 text-[11px] text-fg/45">
                      {t("accessibility.titleBar.positionDesc")}
                    </div>
                  </div>
                  <div className="flex shrink-0 gap-1 rounded-lg border border-fg/10 bg-fg/5 p-1">
                    {(["left", "right"] as const).map((side) => (
                      <button
                        key={side}
                        type="button"
                        onClick={() => handleTitleBarSideChange(side)}
                        className={cn(
                          "rounded-md px-3 py-1 text-xs font-medium",
                          interactive.transition.fast,
                          titleBarSide === side
                            ? "bg-accent/20 text-accent"
                            : "text-fg/60 hover:text-fg",
                        )}
                      >
                        {t(`accessibility.titleBar.${side}` as const)}
                      </button>
                    ))}
                  </div>
                </div>
                <div className="mt-2 flex items-center justify-between gap-3 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
                  <div className="min-w-0">
                    <div className="text-sm font-medium text-fg">
                      {t("accessibility.titleBar.size")}
                    </div>
                    <div className="mt-0.5 text-[11px] text-fg/45">
                      {t("accessibility.titleBar.sizeDesc")}
                    </div>
                  </div>
                  <div className="flex shrink-0 gap-1 rounded-lg border border-fg/10 bg-fg/5 p-1">
                    {(["small", "medium", "large"] as const).map((size) => (
                      <button
                        key={size}
                        type="button"
                        onClick={() => handleTitleBarSizeChange(size)}
                        className={cn(
                          "rounded-md px-3 py-1 text-xs font-medium",
                          interactive.transition.fast,
                          titleBarSize === size
                            ? "bg-accent/20 text-accent"
                            : "text-fg/60 hover:text-fg",
                        )}
                      >
                        {t(`accessibility.titleBar.${size}` as const)}
                      </button>
                    ))}
                  </div>
                </div>
                {platform === "linux" && (
                  <div className="mt-2 flex items-center justify-between gap-3 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
                    <div className="min-w-0">
                      <div className="text-sm font-medium text-fg">
                        {t("accessibility.titleBar.corners")}
                      </div>
                      <div className="mt-0.5 text-[11px] text-fg/45">
                        {t("accessibility.titleBar.cornersDesc")}
                      </div>
                    </div>
                    <div className="flex shrink-0 gap-1 rounded-lg border border-fg/10 bg-fg/5 p-1">
                      {(["off", "small", "medium", "large"] as const).map((corners) => (
                        <button
                          key={corners}
                          type="button"
                          onClick={() => handleWindowCornersChange(corners)}
                          className={cn(
                            "rounded-md px-3 py-1 text-xs font-medium",
                            interactive.transition.fast,
                            windowCorners === corners
                              ? "bg-accent/20 text-accent"
                              : "text-fg/60 hover:text-fg",
                          )}
                        >
                          {corners === "off"
                            ? t("accessibility.titleBar.cornersOff")
                            : t(`accessibility.titleBar.${corners}` as const)}
                        </button>
                      ))}
                    </div>
                  </div>
                )}
              </>
            )}
            <p className="mt-2 px-1 text-[11px] text-fg/40">
              {t("accessibility.titleBar.flagsNote")}
            </p>
          </div>
        )}

        <div>
          <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
            {t("accessibility.sectionTitles.navigation")}
          </h2>
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
            {NAV_STYLE_OPTIONS.filter(
              (option) => !(isMobile && DESKTOP_ONLY_NAV_STYLES.includes(option.value)),
            ).map((option) => {
              const selected = navStyle === option.value;
              return (
                <button
                  key={option.value}
                  type="button"
                  onClick={() => void persistNavigationStyle(option.value)}
                  className={cn(
                    "flex flex-col items-center gap-3 rounded-xl border px-3 pb-3.5 pt-4 text-center",
                    interactive.transition.fast,
                    selected
                      ? "border-accent/25 bg-fg/6"
                      : "border-fg/10 bg-fg/5 hover:border-fg/20",
                  )}
                >
                  <NavStylePreview style={option.value} selected={selected} />
                  <span className="min-w-0 space-y-1">
                    <span
                      className={cn(
                        "block text-[13px] font-medium leading-tight",
                        selected ? "text-accent" : "text-fg",
                      )}
                    >
                      {t(option.labelKey)}
                    </span>
                    <span className="block text-[11px] leading-snug text-fg/45">
                      {t(option.descKey)}
                    </span>
                  </span>
                </button>
              );
            })}
          </div>
          {SIDEBAR_NAV_STYLES.includes(navStyle) && (
            <div className="mt-2 flex items-center justify-between gap-3 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
              <div className="min-w-0">
                <div className="text-sm font-medium text-fg">
                  {t("accessibility.navigation.side")}
                </div>
                <div className="mt-0.5 text-[11px] text-fg/45">
                  {t("accessibility.navigation.sideDesc")}
                </div>
              </div>
              <div className="flex shrink-0 gap-1 rounded-lg border border-fg/10 bg-fg/5 p-1">
                {(["left", "right"] as const).map((side) => (
                  <button
                    key={side}
                    type="button"
                    onClick={() => void persistNavigationSide(side)}
                    className={cn(
                      "rounded-md px-3 py-1 text-xs font-medium",
                      interactive.transition.fast,
                      navSide === side
                        ? "bg-accent/20 text-accent"
                        : "text-fg/60 hover:text-fg",
                    )}
                  >
                    {t(`accessibility.titleBar.${side}` as const)}
                  </button>
                ))}
              </div>
            </div>
          )}
          {!isMobile &&
            (navStyle === "dock" ||
              navStyle === "floatingSidebar" ||
              navStyle === "sidebar") && (
            <div className="mt-2 flex items-center justify-between gap-3 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
              <div className="min-w-0">
                <div className="text-sm font-medium text-fg">
                  {t("accessibility.navigation.placement")}
                </div>
                <div className="mt-0.5 text-[11px] text-fg/45">
                  {t("accessibility.navigation.placementDesc")}
                </div>
              </div>
              <div className="flex shrink-0 items-center gap-2">
                {navStyle === "dock" && (
                  <div className="flex gap-1 rounded-lg border border-fg/10 bg-fg/5 p-1">
                    {(["top", "bottom"] as const).map((edgeOption) => (
                      <button
                        key={edgeOption}
                        type="button"
                        onClick={() => void persistNavEdge(edgeOption)}
                        className={cn(
                          "rounded-md px-3 py-1 text-xs font-medium",
                          interactive.transition.fast,
                          navEdge === edgeOption
                            ? "bg-accent/20 text-accent"
                            : "text-fg/60 hover:text-fg",
                        )}
                      >
                        {edgeOption === "top"
                          ? t("accessibility.navigation.posTop")
                          : t("accessibility.navigation.posBottom")}
                      </button>
                    ))}
                  </div>
                )}
                <div className="flex gap-1 rounded-lg border border-fg/10 bg-fg/5 p-1">
                  {(["start", "center", "end"] as const).map((alignOption) => (
                    <button
                      key={alignOption}
                      type="button"
                      onClick={() => void persistNavAlign(alignOption)}
                      className={cn(
                        "rounded-md px-3 py-1 text-xs font-medium",
                        interactive.transition.fast,
                        navAlign === alignOption
                          ? "bg-accent/20 text-accent"
                          : "text-fg/60 hover:text-fg",
                      )}
                    >
                      {navStyle === "dock"
                        ? alignOption === "start"
                          ? t("accessibility.titleBar.left")
                          : alignOption === "center"
                            ? t("accessibility.navigation.posCenter")
                            : t("accessibility.titleBar.right")
                        : alignOption === "start"
                          ? t("accessibility.navigation.posTop")
                          : alignOption === "center"
                            ? t("accessibility.navigation.posMiddle")
                            : t("accessibility.navigation.posBottom")}
                    </button>
                  ))}
                </div>
              </div>
            </div>
          )}
          <div className="mt-2 flex flex-col gap-3 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="min-w-0">
              <div className="text-sm font-medium text-fg">
                {t("accessibility.navigation.header")}
              </div>
              <div className="mt-0.5 text-[11px] text-fg/45">
                {t("accessibility.navigation.headerDesc")}
              </div>
            </div>
            <div className="flex w-full shrink-0 gap-1 rounded-lg border border-fg/10 bg-fg/5 p-1 sm:w-auto">
              {HEADER_STYLE_OPTIONS.filter((style) => !isMobile || style !== "inline").map((style) => (
                <button
                  key={style}
                  type="button"
                  onClick={() => void persistHeaderStyle(style)}
                  className={cn(
                    "min-w-0 flex-1 truncate rounded-md px-2 py-1.5 text-[11px] font-medium sm:flex-none sm:px-3 sm:py-1 sm:text-xs",
                    interactive.transition.fast,
                    headerStyle === style
                      ? "bg-accent/20 text-accent"
                      : "text-fg/60 hover:text-fg",
                  )}
                >
                  {t(`accessibility.navigation.header_${style}` as const)}
                </button>
              ))}
            </div>
          </div>
          {!isMobile && (
            <div className="mt-2 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
              <NavItemsEditor
                value={navItems}
                onChange={(next) => void persistNavItems(next)}
                navStyle={navStyle}
              />
            </div>
          )}
          {!isMobile && (
            <p className="mt-2 px-1 text-[11px] text-fg/40">
              {t("accessibility.navigation.sidebarFallbackNote")}
            </p>
          )}
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
            {t("accessibility.sectionTitles.easterEggs")}
          </h2>
          <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
            <div className="flex items-start gap-3">
              <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-fg/10 bg-fg/10">
                <Sparkles className="h-4 w-4 text-fg/70" />
              </div>
              <div className="min-w-0 flex-1">
                <div className="flex items-center justify-between gap-3">
                  <div className="flex-1">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium text-fg">{t("accessibility.easterEggs.beetrootRain")}</span>
                      <span
                        className={`rounded-md border px-1.5 py-0.5 text-[10px] font-medium leading-none uppercase tracking-[0.25em] ${
                          isBeetrootEnabled
                            ? "border-info/40 bg-info/15 text-info"
                            : "border-fg/10 bg-fg/10 text-fg/60"
                        }`}
                      >
                        {isBeetrootEnabled ? t("common.labels.on") : t("common.labels.off")}
                      </span>
                    </div>
                    <div className="mt-0.5 text-[11px] text-fg/50">
                      {t("accessibility.easterEggs.beetrootDesc")}
                    </div>
                  </div>
                  <Switch
                    id="beetroot-rain"
                    checked={isBeetrootEnabled}
                    onChange={() => handleBeetrootToggle()}
                  />
                </div>
              </div>
            </div>
          </div>
        </div>

        <div>
          <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
            {t("accessibility.sectionTitles.appearance")}
          </h2>
          <button
            type="button"
            onClick={() => navigate("/settings/customization/colors")}
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
            onClick={() => navigate("/settings/customization/chat")}
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
