import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Download, Edit3, RotateCcw, Trash2, Upload } from "lucide-react";
import { useTheme } from "../../../core/theme/ThemeContext";
import {
  getCustomColorPresets,
  getCustomColors,
  getSettingsCardOpacity,
  setCustomColors as persistCustomColors,
  setCustomColorPresets,
  setSettingsCardOpacity as persistSettingsCardOpacity,
} from "../../../core/storage/appState";
import type { CustomColorPreset, CustomColors } from "../../../core/storage/schemas";
import { cn, interactive, radius } from "../../design-tokens";
import { toast } from "../../components/toast";
import { useI18n } from "../../../core/i18n/context";
import { Switch } from "../../components/Switch";

const SETTINGS_CARD_OPACITY_DEFAULT = 5;

const COLOR_TOKENS = [
  {
    key: "surface" as const,
    label: "Surface",
    description: "Page backgrounds",
    defaultValue: "#050505",
    group: "backgrounds",
  },
  {
    key: "surfaceEl" as const,
    label: "Surface Elevated",
    description: "Cards, modals, raised elements",
    defaultValue: "#0a0a0a",
    group: "backgrounds",
  },
  {
    key: "nav" as const,
    label: "Navigation",
    description: "Top & bottom bars",
    defaultValue: "#0a0a0a",
    group: "backgrounds",
  },
  {
    key: "fg" as const,
    label: "Foreground",
    description: "Borders, overlays, navigation, UI chrome",
    defaultValue: "#ffffff",
    group: "content",
  },
  {
    key: "appText" as const,
    label: "App Text",
    description: "Primary text and interface labels",
    defaultValue: "#ffffff",
    group: "content",
  },
  {
    key: "appTextMuted" as const,
    label: "Muted Text",
    description: "Secondary text and support copy",
    defaultValue: "#adadad",
    group: "content",
  },
  {
    key: "appTextSubtle" as const,
    label: "Subtle Text",
    description: "Hints, helper text, placeholders",
    defaultValue: "#737373",
    group: "content",
  },
  {
    key: "accent" as const,
    label: "Accent",
    description: "Primary actions, success",
    defaultValue: "#34d399",
    group: "semantic",
  },
  {
    key: "info" as const,
    label: "Info",
    description: "Informational states, links",
    defaultValue: "#3b82f6",
    group: "semantic",
  },
  {
    key: "warning" as const,
    label: "Warning",
    description: "Caution states, alerts",
    defaultValue: "#f59e0b",
    group: "semantic",
  },
  {
    key: "danger" as const,
    label: "Danger",
    description: "Destructive actions, errors",
    defaultValue: "#ef4444",
    group: "semantic",
  },
  {
    key: "secondary" as const,
    label: "Secondary",
    description: "AI features, creative tools",
    defaultValue: "#a78bfa",
    group: "semantic",
  },
] as const;

type ColorKey = (typeof COLOR_TOKENS)[number]["key"];
type DerivedColorKey = "appTextMuted" | "appTextSubtle";
type BaseColorKey = Exclude<ColorKey, DerivedColorKey>;

const TOKEN_GROUPS = [
  { id: "backgrounds", label: "Backgrounds" },
  { id: "content", label: "Content" },
  { id: "semantic", label: "Semantic" },
] as const;

const DEFAULTS = Object.fromEntries(COLOR_TOKENS.map((t) => [t.key, t.defaultValue])) as Record<
  ColorKey,
  string
>;

interface Preset {
  name: string;
  colors: Record<ColorKey, string>;
  settingsCardOpacity?: number;
}

interface BasePreset {
  name: string;
  colors: Record<BaseColorKey, string>;
  settingsCardOpacity?: number;
}

const BASE_PRESETS: BasePreset[] = [
  {
    name: "Default Dark",
    colors: {
      surface: "#050505",
      surfaceEl: "#0a0a0a",
      nav: "#0a0a0a",
      fg: "#ffffff",
      appText: "#ffffff",
      accent: "#34d399",
      info: "#3b82f6",
      warning: "#f59e0b",
      danger: "#ef4444",
      secondary: "#a78bfa",
    },
  },
  {
    name: "Midnight Blue",
    colors: {
      surface: "#0a0e1a",
      surfaceEl: "#111827",
      nav: "#0d1120",
      fg: "#e2e8f0",
      appText: "#e2e8f0",
      accent: "#60a5fa",
      info: "#818cf8",
      warning: "#fbbf24",
      danger: "#f87171",
      secondary: "#c084fc",
    },
  },
  {
    name: "Warm Earth",
    colors: {
      surface: "#1a1410",
      surfaceEl: "#231c15",
      nav: "#1a1410",
      fg: "#f5e6d3",
      appText: "#f5e6d3",
      accent: "#d4a574",
      info: "#7cb4c4",
      warning: "#e6a23c",
      danger: "#c45c5c",
      secondary: "#c4a0e0",
    },
  },
  {
    name: "Purple Haze",
    colors: {
      surface: "#0d0815",
      surfaceEl: "#150f20",
      nav: "#0d0815",
      fg: "#e8dff5",
      appText: "#e8dff5",
      accent: "#a78bfa",
      info: "#67e8f9",
      warning: "#fcd34d",
      danger: "#fb7185",
      secondary: "#c4b5fd",
    },
  },
  {
    name: "Rose Pine",
    colors: {
      surface: "#191724",
      surfaceEl: "#1f1d2e",
      nav: "#191724",
      fg: "#e0def4",
      appText: "#e0def4",
      accent: "#c4a7e7",
      info: "#9ccfd8",
      warning: "#f6c177",
      danger: "#eb6f92",
      secondary: "#c4a7e7",
    },
  },
  {
    name: "Tokyo Night",
    colors: {
      surface: "#1a1b26",
      surfaceEl: "#24283b",
      nav: "#1a1b26",
      fg: "#c0caf5",
      appText: "#c0caf5",
      accent: "#7aa2f7",
      info: "#2ac3de",
      warning: "#e0af68",
      danger: "#f7768e",
      secondary: "#bb9af7",
    },
  },
  {
    name: "Catppuccin",
    colors: {
      surface: "#1e1e2e",
      surfaceEl: "#313244",
      nav: "#1e1e2e",
      fg: "#cdd6f4",
      appText: "#cdd6f4",
      accent: "#a6e3a1",
      info: "#89b4fa",
      warning: "#f9e2af",
      danger: "#f38ba8",
      secondary: "#cba6f7",
    },
  },
  {
    name: "Gruvbox",
    colors: {
      surface: "#1d2021",
      surfaceEl: "#282828",
      nav: "#1d2021",
      fg: "#ebdbb2",
      appText: "#ebdbb2",
      accent: "#b8bb26",
      info: "#83a598",
      warning: "#fabd2f",
      danger: "#fb4934",
      secondary: "#d3869b",
    },
  },
  {
    name: "Nord",
    colors: {
      surface: "#2e3440",
      surfaceEl: "#3b4252",
      nav: "#2e3440",
      fg: "#eceff4",
      appText: "#eceff4",
      accent: "#a3be8c",
      info: "#88c0d0",
      warning: "#ebcb8b",
      danger: "#bf616a",
      secondary: "#b48ead",
    },
  },
  {
    name: "Dracula",
    colors: {
      surface: "#282a36",
      surfaceEl: "#44475a",
      nav: "#282a36",
      fg: "#f8f8f2",
      appText: "#f8f8f2",
      accent: "#50fa7b",
      info: "#8be9fd",
      warning: "#f1fa8c",
      danger: "#ff5555",
      secondary: "#bd93f9",
    },
  },
  {
    name: "Solarized",
    colors: {
      surface: "#002b36",
      surfaceEl: "#073642",
      nav: "#002b36",
      fg: "#fdf6e3",
      appText: "#fdf6e3",
      accent: "#859900",
      info: "#268bd2",
      warning: "#b58900",
      danger: "#dc322f",
      secondary: "#6c71c4",
    },
  },
  {
    name: "Ayu Dark",
    colors: {
      surface: "#0d1017",
      surfaceEl: "#131721",
      nav: "#0d1017",
      fg: "#bfbdb6",
      appText: "#bfbdb6",
      accent: "#e6b450",
      info: "#59c2ff",
      warning: "#ffb454",
      danger: "#d95757",
      secondary: "#d2a6ff",
    },
  },
  {
    name: "One Dark",
    colors: {
      surface: "#21252b",
      surfaceEl: "#282c34",
      nav: "#21252b",
      fg: "#abb2bf",
      appText: "#abb2bf",
      accent: "#98c379",
      info: "#61afef",
      warning: "#e5c07b",
      danger: "#e06c75",
      secondary: "#c678dd",
    },
  },
  {
    name: "Vesper",
    colors: {
      surface: "#101010",
      surfaceEl: "#1c1c1c",
      nav: "#101010",
      fg: "#b0b0b0",
      appText: "#b0b0b0",
      accent: "#ffc799",
      info: "#8eb8e2",
      warning: "#deb887",
      danger: "#d08770",
      secondary: "#c4a0e0",
    },
  },
  {
    name: "Cyber Neon",
    colors: {
      surface: "#0a0a12",
      surfaceEl: "#12121e",
      nav: "#0a0a12",
      fg: "#e4e4f0",
      appText: "#e4e4f0",
      accent: "#00ffaa",
      info: "#00d4ff",
      warning: "#ffe600",
      danger: "#ff2e6c",
      secondary: "#bf5af2",
    },
  },
  {
    name: "Monochrome",
    colors: {
      surface: "#111111",
      surfaceEl: "#1a1a1a",
      nav: "#111111",
      fg: "#e0e0e0",
      appText: "#e0e0e0",
      accent: "#ffffff",
      info: "#a0a0a0",
      warning: "#c8c8c8",
      danger: "#808080",
      secondary: "#b0b0b0",
    },
  },
];

interface PresetExportPayload {
  type: "color-preset";
  version: 1;
  preset: {
    name: string;
    colors: Record<ColorKey, string>;
    settingsCardOpacity?: number;
  };
}

function isValidHex(value: string): boolean {
  return /^#[0-9a-fA-F]{6}$/.test(value);
}

function normalizeSettingsCardOpacity(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return SETTINGS_CARD_OPACITY_DEFAULT;
  }
  return Math.max(0, Math.min(100, Math.round(value)));
}

function hexToRgb(value: string): [number, number, number] {
  const match = /^#([0-9a-fA-F]{6})$/.exec(value);
  if (!match) return [0, 0, 0];
  const raw = match[1];
  return [
    Number.parseInt(raw.slice(0, 2), 16),
    Number.parseInt(raw.slice(2, 4), 16),
    Number.parseInt(raw.slice(4, 6), 16),
  ];
}

function rgbToHex([r, g, b]: [number, number, number]): string {
  return `#${[r, g, b]
    .map((channel) =>
      Math.max(0, Math.min(255, Math.round(channel)))
        .toString(16)
        .padStart(2, "0"),
    )
    .join("")}`;
}

function mixHex(base: string, overlay: string, overlayWeight: number): string {
  const weight = Math.max(0, Math.min(1, overlayWeight));
  const [baseR, baseG, baseB] = hexToRgb(base);
  const [overlayR, overlayG, overlayB] = hexToRgb(overlay);
  return rgbToHex([
    baseR * (1 - weight) + overlayR * weight,
    baseG * (1 - weight) + overlayG * weight,
    baseB * (1 - weight) + overlayB * weight,
  ]);
}

function getResolvedDefaultColor(key: ColorKey, colors?: Partial<CustomColors>): string {
  if (key === "appTextMuted") {
    return mixHex(
      colors?.surface ?? DEFAULTS.surface,
      colors?.appText ?? colors?.fg ?? DEFAULTS.appText,
      0.68,
    );
  }
  if (key === "appTextSubtle") {
    return mixHex(
      colors?.surface ?? DEFAULTS.surface,
      colors?.appText ?? colors?.fg ?? DEFAULTS.appText,
      0.45,
    );
  }
  return DEFAULTS[key];
}

function withDerivedTextColors(preset: BasePreset): Preset {
  return {
    ...preset,
    colors: {
      ...preset.colors,
      appTextMuted: getResolvedDefaultColor("appTextMuted", preset.colors),
      appTextSubtle: getResolvedDefaultColor("appTextSubtle", preset.colors),
    },
  };
}

const PRESETS: Preset[] = BASE_PRESETS.map(withDerivedTextColors);

function cssVar(key: string): string {
  return `--color-${key.replace(/[A-Z]/g, (m) => "-" + m.toLowerCase())}`;
}

function normalizeCustomColors(colors?: CustomColors): CustomColors {
  const next: CustomColors = {};
  for (const token of COLOR_TOKENS) {
    const value = colors?.[token.key];
    if (value && isValidHex(value)) {
      next[token.key] = value;
    }
  }
  return next;
}

function normalizePreset(preset: CustomColorPreset): CustomColorPreset {
  return {
    ...preset,
    name: preset.name.trim(),
    colors: normalizeCustomColors(preset.colors),
    settingsCardOpacity:
      preset.settingsCardOpacity === undefined
        ? undefined
        : normalizeSettingsCardOpacity(preset.settingsCardOpacity),
  };
}

function presetsEqual(a: CustomColorPreset[], b: CustomColorPreset[]): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i += 1) {
    const left = normalizePreset(a[i]);
    const right = normalizePreset(b[i]);
    if (
      left.id !== right.id ||
      left.name !== right.name ||
      left.createdAt !== right.createdAt ||
      left.settingsCardOpacity !== right.settingsCardOpacity ||
      JSON.stringify(left.colors) !== JSON.stringify(right.colors)
    ) {
      return false;
    }
  }
  return true;
}

function colorsEqual(a: CustomColors, b: CustomColors): boolean {
  return JSON.stringify(normalizeCustomColors(a)) === JSON.stringify(normalizeCustomColors(b));
}

function buildPresetColors(colors: CustomColors): Record<ColorKey, string> {
  return Object.fromEntries(
    COLOR_TOKENS.map((token) => [
      token.key,
      colors[token.key] ?? getResolvedDefaultColor(token.key, colors),
    ]),
  ) as Record<ColorKey, string>;
}

function parseImportedPreset(
  value: unknown,
): { name: string; colors: CustomColors; settingsCardOpacity?: number } | null {
  if (!value || typeof value !== "object") return null;

  const data = value as Record<string, unknown>;
  const preset =
    data.preset && typeof data.preset === "object"
      ? (data.preset as Record<string, unknown>)
      : data;

  const name = typeof preset.name === "string" ? preset.name.trim() : "";
  if (!name) return null;

  const rawColors = preset.colors;
  if (!rawColors || typeof rawColors !== "object") return null;

  const colors = normalizeCustomColors(rawColors as CustomColors);
  const rawOpacity = preset.settingsCardOpacity;
  return {
    name,
    colors,
    settingsCardOpacity:
      typeof rawOpacity === "number" ? normalizeSettingsCardOpacity(rawOpacity) : undefined,
  };
}

function applyColorsToDocument(colors: CustomColors) {
  const root = document.documentElement;
  for (const token of COLOR_TOKENS) {
    const value = colors[token.key];
    if (value) {
      root.style.setProperty(cssVar(token.key), value);
    } else {
      root.style.removeProperty(cssVar(token.key));
    }
  }
}

function applySettingsCardOpacityToDocument(opacity: number) {
  const normalized = normalizeSettingsCardOpacity(opacity);
  const root = document.documentElement;
  root.style.setProperty("--settings-card-opacity", `${normalized}%`);
  root.style.setProperty("--settings-card-opacity-mid", `${Math.min(normalized + 2, 100)}%`);
  root.style.setProperty("--settings-card-opacity-hover", `${Math.min(normalized + 4, 100)}%`);
}

function slugify(value: string): string {
  return value
    .replace(/[^a-z0-9]+/gi, "_")
    .replace(/^_+|_+$/g, "")
    .toLowerCase();
}

const TOKEN_LABEL_KEYS: Record<ColorKey, { label: string; desc: string }> = {
  surface: {
    label: "colorCustomization.tokens.surface",
    desc: "colorCustomization.tokens.surfaceDesc",
  },
  surfaceEl: {
    label: "colorCustomization.tokens.surfaceEl",
    desc: "colorCustomization.tokens.surfaceElDesc",
  },
  nav: { label: "colorCustomization.tokens.nav", desc: "colorCustomization.tokens.navDesc" },
  fg: {
    label: "colorCustomization.tokens.foreground",
    desc: "colorCustomization.tokens.foregroundDesc",
  },
  appText: {
    label: "colorCustomization.tokens.appText",
    desc: "colorCustomization.tokens.appTextDesc",
  },
  appTextMuted: {
    label: "colorCustomization.tokens.appTextMuted",
    desc: "colorCustomization.tokens.appTextMutedDesc",
  },
  appTextSubtle: {
    label: "colorCustomization.tokens.appTextSubtle",
    desc: "colorCustomization.tokens.appTextSubtleDesc",
  },
  accent: {
    label: "colorCustomization.tokens.accent",
    desc: "colorCustomization.tokens.accentDesc",
  },
  info: { label: "colorCustomization.tokens.info", desc: "colorCustomization.tokens.infoDesc" },
  warning: {
    label: "colorCustomization.tokens.warning",
    desc: "colorCustomization.tokens.warningDesc",
  },
  danger: {
    label: "colorCustomization.tokens.danger",
    desc: "colorCustomization.tokens.dangerDesc",
  },
  secondary: {
    label: "colorCustomization.tokens.secondary",
    desc: "colorCustomization.tokens.secondaryDesc",
  },
};

const GROUP_LABEL_KEYS: Record<string, string> = {
  backgrounds: "colorCustomization.groups.backgrounds",
  content: "colorCustomization.groups.content",
  semantic: "colorCustomization.groups.semantic",
};

const PRESET_NAME_KEYS: Record<string, string> = {
  "Default Dark": "colorCustomization.presets.defaultDark",
  "Midnight Blue": "colorCustomization.presets.midnightBlue",
  "Warm Earth": "colorCustomization.presets.warmEarth",
  "Purple Haze": "colorCustomization.presets.purpleHaze",
  "Rose Pine": "colorCustomization.presets.rosePine",
  "Tokyo Night": "colorCustomization.presets.tokyoNight",
  Catppuccin: "colorCustomization.presets.catppuccin",
  Gruvbox: "colorCustomization.presets.gruvbox",
  Nord: "colorCustomization.presets.nord",
  Dracula: "colorCustomization.presets.dracula",
  Solarized: "colorCustomization.presets.solarized",
  "Ayu Dark": "colorCustomization.presets.ayuDark",
  "One Dark": "colorCustomization.presets.oneDark",
  Vesper: "colorCustomization.presets.vesper",
  "Cyber Neon": "colorCustomization.presets.cyberNeon",
  Monochrome: "colorCustomization.presets.monochrome",
};

export function ColorCustomizationPage() {
  const {
    setCustomColors: setThemeCustomColors,
    setSettingsCardOpacity: setThemeSettingsCardOpacity,
  } = useTheme();
  const { t } = useI18n();
  const appTextStyle = { color: "var(--color-app-text)" } as const;
  const appTextMutedStyle = { color: "var(--color-app-text-muted)" } as const;
  const appTextSubtleStyle = { color: "var(--color-app-text-subtle)" } as const;
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [workingColors, setWorkingColors] = useState<CustomColors>({});
  const [initialColors, setInitialColors] = useState<CustomColors>({});
  const [settingsCardOpacity, setSettingsCardOpacity] = useState(SETTINGS_CARD_OPACITY_DEFAULT);
  const [initialSettingsCardOpacity, setInitialSettingsCardOpacity] = useState(
    SETTINGS_CARD_OPACITY_DEFAULT,
  );
  const [importedPresets, setImportedPresets] = useState<CustomColorPreset[]>([]);
  const [initialImportedPresets, setInitialImportedPresets] = useState<CustomColorPreset[]>([]);
  const [appliedPresetName, setAppliedPresetName] = useState<string | null>(null);
  const [derivedPresetId, setDerivedPresetId] = useState<string | null>(null);
  const pickerRefs = useRef<Record<string, HTMLInputElement | null>>({});
  const importInputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    const load = async () => {
      try {
        const [storedColors, storedPresets, storedSettingsCardOpacity] = await Promise.all([
          getCustomColors(),
          getCustomColorPresets(),
          getSettingsCardOpacity(),
        ]);
        const normalizedColors = normalizeCustomColors(storedColors ?? {});
        const normalizedPresets = (storedPresets ?? []).map(normalizePreset);
        const normalizedSettingsCardOpacity =
          normalizeSettingsCardOpacity(storedSettingsCardOpacity);

        setWorkingColors(normalizedColors);
        setInitialColors(normalizedColors);
        setSettingsCardOpacity(normalizedSettingsCardOpacity);
        setInitialSettingsCardOpacity(normalizedSettingsCardOpacity);
        setImportedPresets(normalizedPresets);
        setInitialImportedPresets(normalizedPresets);
        applyColorsToDocument(normalizedColors);
        applySettingsCardOpacityToDocument(normalizedSettingsCardOpacity);
      } catch (error) {
        console.error("Failed to load color customization settings:", error);
        toast.error("Load failed", "Could not load saved color settings.");
      } finally {
        setIsLoading(false);
      }
    };

    void load();
  }, []);

  useEffect(() => {
    if (isLoading) return;
    applyColorsToDocument(workingColors);
  }, [workingColors, isLoading]);

  useEffect(() => {
    if (isLoading) return;
    applySettingsCardOpacityToDocument(settingsCardOpacity);
  }, [settingsCardOpacity, isLoading]);

  const isDirty = useMemo(() => {
    return (
      !colorsEqual(workingColors, initialColors) ||
      settingsCardOpacity !== initialSettingsCardOpacity ||
      !presetsEqual(importedPresets, initialImportedPresets)
    );
  }, [
    workingColors,
    initialColors,
    settingsCardOpacity,
    initialSettingsCardOpacity,
    importedPresets,
    initialImportedPresets,
  ]);

  const getEffective = (key: ColorKey): string => workingColors[key] ?? "";

  const getDraftOrValue = (key: ColorKey): string => {
    if (key in drafts) return drafts[key];
    return getEffective(key);
  };

  const getDisplayColor = (key: ColorKey): string => {
    const v = getDraftOrValue(key);
    return isValidHex(v) ? v : getResolvedDefaultColor(key, workingColors);
  };

  const getUniqueModifiedPresetName = useCallback(
    (parentName: string): string => {
      const base = `modified ${parentName}`;
      const exists = (candidate: string) => {
        const lower = candidate.toLowerCase();
        return (
          PRESETS.some((preset) => preset.name.toLowerCase() === lower) ||
          importedPresets.some((preset) => preset.name.toLowerCase() === lower)
        );
      };

      if (!exists(base)) return base;
      let index = 2;
      while (exists(`${base} #${index}`)) {
        index += 1;
      }
      return `${base} #${index}`;
    },
    [importedPresets],
  );

  const trackDerivedPreset = useCallback(
    (nextColors: CustomColors, nextOpacity: number) => {
      if (!appliedPresetName) return;

      if (derivedPresetId) {
        setImportedPresets((prev) =>
          prev.map((preset) =>
            preset.id === derivedPresetId
              ? {
                  ...preset,
                  colors: nextColors,
                  settingsCardOpacity: nextOpacity,
                }
              : preset,
          ),
        );
        return;
      }

      const createdPreset: CustomColorPreset = {
        id: crypto.randomUUID(),
        name: getUniqueModifiedPresetName(appliedPresetName),
        colors: nextColors,
        settingsCardOpacity: nextOpacity,
        createdAt: Date.now(),
      };
      setImportedPresets((prev) => [...prev, createdPreset]);
      setDerivedPresetId(createdPreset.id);
    },
    [appliedPresetName, derivedPresetId, getUniqueModifiedPresetName],
  );

  const commitWorkingState = useCallback(
    (nextColors: CustomColors, nextOpacity: number) => {
      const normalizedColors = normalizeCustomColors(nextColors);
      const normalizedOpacity = normalizeSettingsCardOpacity(nextOpacity);
      if (
        colorsEqual(normalizedColors, workingColors) &&
        normalizedOpacity === settingsCardOpacity
      ) {
        return;
      }

      trackDerivedPreset(normalizedColors, normalizedOpacity);
      setWorkingColors(normalizedColors);
      setSettingsCardOpacity(normalizedOpacity);
    },
    [settingsCardOpacity, trackDerivedPreset, workingColors],
  );

  const applyManualEdit = useCallback(
    (next: CustomColors) => {
      commitWorkingState(next, settingsCardOpacity);
    },
    [commitWorkingState, settingsCardOpacity],
  );

  const handleSettingsCardOpacityChange = useCallback(
    (value: number) => {
      commitWorkingState(workingColors, value);
    },
    [commitWorkingState, workingColors],
  );

  const handleChange = useCallback(
    (key: ColorKey, value: string) => {
      setDrafts((prev) => ({ ...prev, [key]: value }));
      if (isValidHex(value)) {
        applyManualEdit({ ...workingColors, [key]: value });
      }
    },
    [applyManualEdit, workingColors],
  );

  const handleBlur = useCallback((key: ColorKey) => {
    setDrafts((prev) => {
      const next = { ...prev };
      delete next[key];
      return next;
    });
  }, []);

  const handleReset = useCallback(
    (key: ColorKey) => {
      const next = { ...workingColors };
      delete next[key];
      applyManualEdit(next);
      setDrafts((prev) => {
        const next = { ...prev };
        delete next[key];
        return next;
      });
    },
    [applyManualEdit, workingColors],
  );

  const handleResetAll = useCallback(() => {
    commitWorkingState({}, SETTINGS_CARD_OPACITY_DEFAULT);
    setDrafts({});
  }, [commitWorkingState]);

  const applyPreset = useCallback(
    (preset: {
      name: string;
      colors: Record<ColorKey, string> | CustomColors;
      settingsCardOpacity?: number;
    }) => {
      setWorkingColors(normalizeCustomColors(preset.colors));
      setSettingsCardOpacity(normalizeSettingsCardOpacity(preset.settingsCardOpacity));
      setDrafts({});
      setAppliedPresetName(preset.name);
      setDerivedPresetId(null);
    },
    [],
  );

  const handleSave = useCallback(async () => {
    if (!isDirty || isSaving) return;
    setIsSaving(true);
    try {
      const normalizedColors = normalizeCustomColors(workingColors);
      const normalizedPresets = importedPresets.map(normalizePreset);
      const normalizedSettingsCardOpacity = normalizeSettingsCardOpacity(settingsCardOpacity);
      await persistCustomColors(normalizedColors);
      await persistSettingsCardOpacity(normalizedSettingsCardOpacity);
      await setCustomColorPresets(normalizedPresets);
      setThemeCustomColors(normalizedColors);
      setThemeSettingsCardOpacity(normalizedSettingsCardOpacity);
      setInitialColors(normalizedColors);
      setInitialSettingsCardOpacity(normalizedSettingsCardOpacity);
      setInitialImportedPresets(normalizedPresets);
      toast.success("Saved", "Color customization updated.");
    } catch (error) {
      console.error("Failed to save color customization:", error);
      toast.error("Save failed", error instanceof Error ? error.message : String(error));
    } finally {
      setIsSaving(false);
    }
  }, [
    isDirty,
    isSaving,
    workingColors,
    settingsCardOpacity,
    importedPresets,
    setThemeCustomColors,
    setThemeSettingsCardOpacity,
  ]);

  const handleDiscard = useCallback(() => {
    if (!isDirty) return;
    setWorkingColors(initialColors);
    setSettingsCardOpacity(initialSettingsCardOpacity);
    setImportedPresets(initialImportedPresets);
    setDrafts({});
    setAppliedPresetName(null);
    setDerivedPresetId(null);
  }, [isDirty, initialColors, initialImportedPresets, initialSettingsCardOpacity]);

  const handleImportJson = useCallback((rawJson: string) => {
    let parsed: unknown;
    try {
      parsed = JSON.parse(rawJson);
    } catch {
      toast.error("Import failed", "Invalid JSON file.");
      return;
    }

    const imported = parseImportedPreset(parsed);
    if (!imported) {
      toast.error(
        "Import failed",
        "Expected `{ name, colors }` or `{ preset: { name, colors } }`.",
      );
      return;
    }

    if (Object.keys(imported.colors).length === 0) {
      toast.error("Import failed", "No valid hex colors found in file.");
      return;
    }

    const createdAt = Date.now();
    const preset: CustomColorPreset = {
      id: crypto.randomUUID(),
      name: imported.name,
      colors: imported.colors,
      settingsCardOpacity: imported.settingsCardOpacity,
      createdAt,
    };

    setImportedPresets((prev) => [...prev, preset]);
    toast.success("Imported", `Added preset \"${preset.name}\".`);
  }, []);

  const handleImportClick = useCallback(() => {
    importInputRef.current?.click();
  }, []);

  const handleExport = useCallback(() => {
    const defaultName = `Custom Preset ${new Date().toISOString().slice(0, 10)}`;
    const name = window.prompt("Export preset name", defaultName)?.trim();
    if (!name) return;

    const payload: PresetExportPayload = {
      type: "color-preset",
      version: 1,
      preset: {
        name,
        colors: buildPresetColors(workingColors),
        settingsCardOpacity,
      },
    };

    const blob = new Blob([JSON.stringify(payload, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    const date = new Date().toISOString().slice(0, 10);
    link.href = url;
    link.download = `color_preset_${slugify(name) || "export"}_${date}.json`;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
  }, [settingsCardOpacity, workingColors]);

  const handleRenameImportedPreset = useCallback((preset: CustomColorPreset) => {
    const nextName = window.prompt("Rename preset", preset.name)?.trim();
    if (!nextName || nextName === preset.name) return;

    setImportedPresets((prev) =>
      prev.map((item) => (item.id === preset.id ? { ...item, name: nextName } : item)),
    );
  }, []);

  const handleDeleteImportedPreset = useCallback(
    (preset: CustomColorPreset) => {
      const confirmed = window.confirm(`Delete imported preset \"${preset.name}\"?`);
      if (!confirmed) return;

      setImportedPresets((prev) => prev.filter((item) => item.id !== preset.id));
      if (derivedPresetId === preset.id) {
        setDerivedPresetId(null);
        setAppliedPresetName(null);
      }
    },
    [derivedPresetId],
  );

  useEffect(() => {
    const handleDiscardEvent = () => {
      handleDiscard();
    };
    window.addEventListener("unsaved:discard", handleDiscardEvent);
    return () => window.removeEventListener("unsaved:discard", handleDiscardEvent);
  }, [handleDiscard]);

  useEffect(() => {
    const globalWindow = window as any;
    globalWindow.__saveColorCustomization = () => {
      void handleSave();
    };
    globalWindow.__saveColorCustomizationCanSave = isDirty;
    globalWindow.__saveColorCustomizationSaving = isSaving;
    return () => {
      delete globalWindow.__saveColorCustomization;
      delete globalWindow.__saveColorCustomizationCanSave;
      delete globalWindow.__saveColorCustomizationSaving;
    };
  }, [handleSave, isDirty, isSaving]);

  const hasAnyCustom =
    COLOR_TOKENS.some((tok) => workingColors[tok.key]) ||
    settingsCardOpacity !== SETTINGS_CARD_OPACITY_DEFAULT;

  const isPresetActive = (preset: Preset | CustomColorPreset) => {
    const presetColors = preset.colors;
    const presetOpacity = normalizeSettingsCardOpacity(preset.settingsCardOpacity);
    return (
      COLOR_TOKENS.every((tok) => {
        const current = workingColors[tok.key] ?? getResolvedDefaultColor(tok.key, workingColors);
        const candidate = presetColors[tok.key] ?? getResolvedDefaultColor(tok.key, presetColors);
        return current === candidate;
      }) && settingsCardOpacity === presetOpacity
    );
  };

  if (isLoading) return null;

  return (
    <div className="color-customization-editor-scope flex h-full flex-col pb-16">
      <section className="flex-1 min-h-0 overflow-y-auto px-3 pt-3 space-y-5">
        <input
          ref={importInputRef}
          type="file"
          accept="application/json,.json"
          className="hidden"
          onChange={async (event) => {
            const file = event.currentTarget.files?.[0];
            if (!file) return;
            try {
              const raw = await file.text();
              handleImportJson(raw);
            } catch (error) {
              console.error("Failed to import color preset:", error);
              toast.error("Import failed", "Could not read selected file.");
            } finally {
              event.currentTarget.value = "";
            }
          }}
        />

        {/* Presets */}
        <div>
          <div className="mb-2.5 flex items-center justify-between px-1 gap-2">
            <h2
              className="text-[10px] font-semibold uppercase tracking-[0.25em]"
              style={appTextSubtleStyle}
            >
              {t("colorCustomization.presetsLabel")}
            </h2>
            <div className="flex items-center gap-1.5">
              <button
                type="button"
                onClick={handleImportClick}
                className={cn(
                  "flex items-center gap-1.5 px-2.5 py-1 text-[10px] font-medium",
                  radius.full,
                  "border border-fg/10 bg-fg/5",
                  interactive.transition.fast,
                  "hover:border-fg/20 hover:text-fg/75",
                )}
                style={appTextMutedStyle}
              >
                <Upload className="h-3 w-3" />
                {t("colorCustomization.importButton")}
              </button>
              <button
                type="button"
                onClick={handleExport}
                className={cn(
                  "flex items-center gap-1.5 px-2.5 py-1 text-[10px] font-medium",
                  radius.full,
                  "border border-fg/10 bg-fg/5",
                  interactive.transition.fast,
                  "hover:border-fg/20 hover:text-fg/75",
                )}
                style={appTextMutedStyle}
              >
                <Download className="h-3 w-3" />
                {t("colorCustomization.exportButton")}
              </button>
              {hasAnyCustom && (
                <button
                  type="button"
                  onClick={handleResetAll}
                  className={cn(
                    "flex items-center gap-1.5 px-2.5 py-1 text-[10px] font-medium",
                    radius.full,
                    "border border-fg/10 bg-fg/5",
                    interactive.transition.fast,
                    "hover:border-fg/20 hover:text-fg/75",
                  )}
                  style={appTextMutedStyle}
                >
                  <RotateCcw className="h-3 w-3" />
                  {t("colorCustomization.resetAllButton")}
                </button>
              )}
            </div>
          </div>

          <div className="grid grid-cols-2 gap-2">
            {PRESETS.map((preset) => {
              const active = isPresetActive(preset);
              return (
                <button
                  key={preset.name}
                  type="button"
                  onClick={() =>
                    applyPreset({
                      name: preset.name,
                      colors: preset.colors,
                      settingsCardOpacity: preset.settingsCardOpacity,
                    })
                  }
                  className={cn(
                    "flex items-center gap-2.5 rounded-lg border px-3 py-2",
                    interactive.transition.fast,
                    active
                      ? "border-accent/40 bg-accent/10"
                      : "border-fg/10 bg-fg/5 hover:border-fg/20 hover:bg-fg/8",
                  )}
                >
                  <div className="flex gap-0.5 shrink-0">
                    {(["accent", "info", "warning", "danger"] as const).map((k) => (
                      <div
                        key={k}
                        className="h-3 w-3 rounded-full"
                        style={{ backgroundColor: preset.colors[k] }}
                      />
                    ))}
                  </div>
                  <span
                    className={cn(
                      "text-[11px] font-medium truncate",
                      active ? "text-accent" : "text-fg/60",
                    )}
                    style={active ? undefined : appTextMutedStyle}
                  >
                    {PRESET_NAME_KEYS[preset.name]
                      ? t(PRESET_NAME_KEYS[preset.name] as any)
                      : preset.name}
                  </span>
                </button>
              );
            })}
          </div>
        </div>

        {importedPresets.length > 0 && (
          <div>
            <h2
              className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em]"
              style={appTextSubtleStyle}
            >
              {t("colorCustomization.customPresetsLabel")}
            </h2>
            <div className="space-y-2">
              {importedPresets.map((preset) => {
                const active = isPresetActive(preset);
                return (
                  <div
                    key={preset.id}
                    className={cn(
                      "flex items-center gap-2 rounded-lg border px-3 py-2",
                      active ? "border-accent/40 bg-accent/10" : "border-fg/10 bg-fg/5",
                    )}
                  >
                    <button
                      type="button"
                      onClick={() =>
                        applyPreset({
                          name: preset.name,
                          colors: preset.colors,
                          settingsCardOpacity: preset.settingsCardOpacity,
                        })
                      }
                      className="flex min-w-0 flex-1 items-center gap-2.5"
                    >
                      <div className="flex gap-1 shrink-0">
                        {(["accent", "info", "warning", "danger"] as const).map((k) => (
                          <div
                            key={k}
                            className="h-3 w-3 rounded-full"
                            style={{ backgroundColor: preset.colors[k] ?? DEFAULTS[k] }}
                          />
                        ))}
                      </div>
                      <span
                        className={cn(
                          "text-[11px] font-medium truncate",
                          active ? "text-accent" : "text-fg/60",
                        )}
                        style={active ? undefined : appTextMutedStyle}
                      >
                        {preset.name}
                      </span>
                    </button>
                    <button
                      type="button"
                      onClick={() => handleRenameImportedPreset(preset)}
                      className="p-1 hover:text-fg/70"
                      style={appTextSubtleStyle}
                      title="Rename preset"
                    >
                      <Edit3 className="h-4 w-4" />
                    </button>
                    <button
                      type="button"
                      onClick={() => handleDeleteImportedPreset(preset)}
                      className="ml-1.5 p-1 hover:text-danger"
                      style={appTextSubtleStyle}
                      title="Delete preset"
                    >
                      <Trash2 className="h-4 w-4" />
                    </button>
                  </div>
                );
              })}
            </div>
          </div>
        )}

        <div>
          <h2
            className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em]"
            style={appTextSubtleStyle}
          >
            {t("colorCustomization.settingsCardsLabel")}
          </h2>
          <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
            <div className="flex items-center justify-between gap-4">
              <div className="min-w-0">
                <div className="text-sm font-medium" style={appTextStyle}>
                  {t("colorCustomization.settingsCardsOpacity")}
                </div>
                <div className="text-[11px]" style={appTextMutedStyle}>
                  {t("colorCustomization.settingsCardsOpacityDesc")}
                </div>
              </div>
              <div
                className="min-w-14 rounded-md border border-fg/10 bg-fg/5 px-2.5 py-1 text-right text-xs font-medium"
                style={appTextStyle}
              >
                {settingsCardOpacity}%
              </div>
            </div>
            <input
              type="range"
              min={0}
              max={100}
              step={1}
              value={settingsCardOpacity}
              onChange={(event) =>
                handleSettingsCardOpacityChange(Number.parseInt(event.target.value, 10))
              }
              className="mt-4 w-full accent-accent"
            />
          </div>
        </div>

        {/* Grouped token editors */}
        {TOKEN_GROUPS.map((group) => {
          const tokens = COLOR_TOKENS.filter((tok) => tok.group === group.id);
          return (
            <div key={group.id}>
              <h2
                className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em]"
                style={appTextSubtleStyle}
              >
                {t(GROUP_LABEL_KEYS[group.id] as any)}
              </h2>
              <div className="space-y-2.5">
                {tokens.map((token) => {
                  const displayColor = getDisplayColor(token.key);
                  const inputValue =
                    getDraftOrValue(token.key) || getResolvedDefaultColor(token.key, workingColors);
                  const isCustom = Boolean(workingColors[token.key]);

                  return (
                    <div
                      key={token.key}
                      className={cn(
                        "rounded-xl border px-4 py-3",
                        isCustom ? "border-accent/25 bg-fg/6" : "border-fg/10 bg-fg/5",
                      )}
                    >
                      <div className="flex items-center justify-between gap-3">
                        <div className="flex items-center gap-3">
                          <button
                            type="button"
                            className="relative h-8 w-8 shrink-0 rounded-full border border-fg/15 overflow-hidden"
                            style={{ backgroundColor: displayColor }}
                            onClick={() => pickerRefs.current[token.key]?.click()}
                          >
                            <input
                              ref={(el) => {
                                pickerRefs.current[token.key] = el;
                              }}
                              type="color"
                              value={displayColor}
                              onChange={(e) => handleChange(token.key, e.target.value)}
                              className="absolute inset-0 h-full w-full cursor-pointer opacity-0"
                              tabIndex={-1}
                            />
                          </button>
                          <div>
                            <div className="text-sm font-medium" style={appTextStyle}>
                              {t(TOKEN_LABEL_KEYS[token.key].label as any)}
                            </div>
                            <div className="text-[11px]" style={appTextMutedStyle}>
                              {t(TOKEN_LABEL_KEYS[token.key].desc as any)}
                            </div>
                          </div>
                        </div>

                        <div className="flex items-center gap-2">
                          <input
                            type="text"
                            value={inputValue}
                            onChange={(e) => handleChange(token.key, e.target.value)}
                            onBlur={() => handleBlur(token.key)}
                            placeholder={getResolvedDefaultColor(token.key, workingColors)}
                            spellCheck={false}
                            className={cn(
                              "w-22.5 rounded-lg border px-2.5 py-1.5 font-mono text-xs",
                              "border-fg/10 bg-fg/5 placeholder:text-[var(--color-app-text-subtle)]",
                              "focus:border-accent/40 focus:outline-none",
                              interactive.transition.fast,
                            )}
                            style={appTextStyle}
                          />
                          {isCustom && (
                            <button
                              type="button"
                              onClick={() => handleReset(token.key)}
                              className="hover:text-fg/60"
                              style={appTextSubtleStyle}
                              title="Reset to default"
                            >
                              <RotateCcw className="h-3.5 w-3.5" />
                            </button>
                          )}
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          );
        })}

        {/* Live preview */}
        <div>
          <h2
            className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em]"
            style={appTextSubtleStyle}
          >
            {t("colorCustomization.previewLabel")}
          </h2>
          <div className="rounded-xl border border-fg/10 bg-surface p-4 space-y-4">
            <div className="space-y-1">
              <p className="text-sm font-medium" style={{ color: "var(--color-app-text)" }}>
                Primary text
              </p>
              <p className="text-xs" style={{ color: "var(--color-app-text-muted)" }}>
                Muted text color
              </p>
              <p className="text-[11px]" style={{ color: "var(--color-app-text-subtle)" }}>
                Subtle text color
              </p>
            </div>

            <div className="flex flex-wrap gap-2">
              <span className="rounded-full border border-accent/40 bg-accent/15 px-3 py-1 text-[11px] font-medium text-accent">
                Accent
              </span>
              <span className="rounded-full border border-info/40 bg-info/15 px-3 py-1 text-[11px] font-medium text-info">
                Info
              </span>
              <span className="rounded-full border border-warning/40 bg-warning/15 px-3 py-1 text-[11px] font-medium text-warning">
                Warning
              </span>
              <span className="rounded-full border border-danger/40 bg-danger/15 px-3 py-1 text-[11px] font-medium text-danger">
                Danger
              </span>
            </div>

            <div className="rounded-lg border border-fg/10 bg-fg/5 p-3 space-y-2">
              <div className="flex items-center justify-between">
                <span className="text-xs font-medium" style={{ color: "var(--color-app-text)" }}>
                  Sample Card
                </span>
                <span className="rounded-full bg-accent/15 px-2 py-0.5 text-[10px] font-medium text-accent">
                  Active
                </span>
              </div>
              <p className="text-[11px]" style={{ color: "var(--color-app-text-subtle)" }}>
                This preview uses the current settings-card opacity and app text colors.
              </p>
              <div className="flex gap-2">
                <div className="flex-1 rounded-md bg-accent/10 px-2 py-1.5 text-center text-[10px] font-medium text-accent">
                  Confirm
                </div>
                <div className="flex-1 rounded-md bg-danger/10 px-2 py-1.5 text-center text-[10px] font-medium text-danger">
                  Delete
                </div>
              </div>
            </div>

            <div className="flex items-center justify-between rounded-lg border border-fg/10 bg-fg/5 px-3 py-2.5">
              <span className="text-xs" style={{ color: "var(--color-app-text-muted)" }}>
                Sample toggle
              </span>
              <Switch checked={true} onChange={() => {}} />
            </div>
          </div>
        </div>

        <div className="h-4" />
      </section>
    </div>
  );
}
