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

export function themeFromPreferences(value: Preferences): Theme {
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
    if (typeof color === "string" && /^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i.test(color))
      colors[key] = color;
  }
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
