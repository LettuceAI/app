import { createContext } from "react";
import { DEFAULT_THEME, type Theme } from "@lettuceai/crisp";
import type { UiPreferenceChange_Deserialize } from "@/api/generated/bindings";
import { appThemes } from "./themes";
export type Preferences = Record<string, unknown>;
export type PreferenceEdit = (latest: Preferences) => UiPreferenceChange_Deserialize[];
export const apply = (
  value: Preferences,
  changes: UiPreferenceChange_Deserialize[],
): Preferences => {
  const next = { ...value };
  for (const change of changes) {
    if (change.value === null) delete next[change.key];
    else next[change.key] = change.value;
  }
  return next;
};

export function object(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

export type ThemeMode = "system" | "dark" | "light";
export type ThemeKind = "dark" | "light";
export const DEFAULT_PRESET_BY_KIND: Record<ThemeKind, string> = {
  dark: "midnight",
  light: "paper",
};
const HEX = /^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i;
export const isHexColor = (value: unknown): value is string =>
  typeof value === "string" && HEX.test(value);

/** `null` means `themeMode` was never set: the stored preset alone decides, as before modes existed. */
export function themeModeOf(value: Preferences): ThemeMode | null {
  return value.themeMode === "system" || value.themeMode === "dark" || value.themeMode === "light"
    ? value.themeMode
    : null;
}

export const kindOfMode = (mode: ThemeMode, systemDark: boolean): ThemeKind =>
  mode === "system" ? (systemDark ? "dark" : "light") : mode;

export const rememberedKey = (kind: ThemeKind) =>
  kind === "dark" ? ("darkThemePreset" as const) : ("lightThemePreset" as const);

/**
 * With a `themeMode`, the preset comes from the remembered preset of the active kind
 * (`darkThemePreset` or `lightThemePreset`), not from `themePreset`. The stored
 * `themePreset` and `customColors` still win when they name that same preset, which keeps
 * a "custom" theme and a saved theme's colors intact; otherwise the remembered preset is
 * read from the built-in list or the saved themes, and an unknown id falls back to the
 * kind's default. `themeAccent` is applied on top by `themeFromPreferences`.
 */
function appliedPreferences(value: Preferences, systemDark: boolean): Preferences {
  const mode = themeModeOf(value);
  if (!mode) return value;
  const kind = kindOfMode(mode, systemDark);
  const remembered = value[rememberedKey(kind)];
  const fallback = DEFAULT_PRESET_BY_KIND[kind];
  const id = typeof remembered === "string" ? remembered : fallback;
  if (id === value.themePreset) return { ...value, theme: kind };
  const saved = Array.isArray(value.customColorPresets)
    ? value.customColorPresets.map(object).find((theme) => theme.id === id)
    : undefined;
  if (saved) return { ...value, themePreset: id, theme: kind, customColors: saved.colors };
  const builtIn = { ...value };
  delete builtIn.customColors;
  return {
    ...builtIn,
    themePreset: appThemes.some((theme) => theme.id === id) ? id : fallback,
    theme: kind,
  };
}

export function themeFromPreferences(stored: Preferences, systemDark = false): Theme {
  const value = appliedPreferences(stored, systemDark);
  const preset =
    appThemes.find((theme) => theme.id === value.themePreset) ??
    (value.theme === "light" ? appThemes.find((theme) => theme.id === "paper") : undefined) ??
    DEFAULT_THEME;
  const colors = { ...preset.colors };
  const custom = object(value.customColors);
  for (const key of [
    "surface",
    "fg",
    "accent",
    "nav",
    "danger",
    "warning",
    "info",
    "secondary",
  ] as const) {
    const color = custom[key];
    if (isHexColor(color)) colors[key] = color;
  }
  if (isHexColor(value.themeAccent)) colors.accent = value.themeAccent;
  const saved = Array.isArray(value.customColorPresets)
    ? value.customColorPresets.map(object).find((theme) => theme.id === value.themePreset)
    : undefined;
  const radius =
    value.themeRadius === "sharp" || value.themeRadius === "round" ? value.themeRadius : "default";
  const density = value.themeDensity === "compact" ? "compact" : "default";
  return {
    ...preset,
    id: typeof value.themePreset === "string" ? value.themePreset : preset.id,
    name: typeof saved?.name === "string" ? saved.name : preset.name,
    colors,
    radius,
    density,
  };
}

type State = {
  value: Preferences;
  ready: boolean;
  error: unknown;
  retry: () => void;
  reset: () => void;
  update: (edit: PreferenceEdit, coalesceKey?: string) => Promise<boolean>;
};
export const PreferencesContext = createContext<State | null>(null);

import { CHAT_DEFAULTS } from "./chat";
export const ChatAppearanceContext = createContext(CHAT_DEFAULTS);
