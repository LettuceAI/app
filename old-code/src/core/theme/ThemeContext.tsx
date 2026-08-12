import React, { createContext, useContext, useState, useEffect, useCallback } from "react";
import {
  getTheme as getStoredTheme,
  setTheme as setStoredTheme,
  getCustomColors,
  getSettingsCardOpacity as getStoredSettingsCardOpacity,
  setCustomColors as saveCustomColors,
  setSettingsCardOpacity as saveSettingsCardOpacity,
} from "../storage/appState";
import type { CustomColors } from "../storage/schemas";

type Theme = "light" | "dark";

interface ThemeContextType {
  theme: Theme;
  toggleTheme: () => void;
  setTheme: (theme: Theme) => void;
  customColors: CustomColors | undefined;
  setCustomColors: (colors: CustomColors) => void;
  settingsCardOpacity: number;
  setSettingsCardOpacity: (opacity: number) => void;
}

const ThemeContext = createContext<ThemeContextType | undefined>(undefined);

export function useTheme(): ThemeContextType {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error("useTheme must be used within a ThemeProvider");
  }
  return context;
}

const COLOR_KEY_TO_VAR: Record<string, string> = {
  surface: "--color-surface",
  surfaceEl: "--color-surface-el",
  fg: "--color-fg",
  appText: "--color-app-text",
  appTextMuted: "--color-app-text-muted",
  appTextSubtle: "--color-app-text-subtle",
  accent: "--color-accent",
  danger: "--color-danger",
  warning: "--color-warning",
  info: "--color-info",
  secondary: "--color-secondary",
  nav: "--color-nav",
};

function applySettingsCardOpacity(opacity: number) {
  const normalized = Math.max(0, Math.min(100, Math.round(opacity)));
  const root = document.documentElement;
  root.style.setProperty("--settings-card-opacity", `${normalized}%`);
  root.style.setProperty("--settings-card-opacity-mid", `${Math.min(normalized + 2, 100)}%`);
  root.style.setProperty("--settings-card-opacity-hover", `${Math.min(normalized + 4, 100)}%`);
}

function applyCustomColors(colors: Partial<CustomColors>) {
  const root = document.documentElement;
  for (const [key, value] of Object.entries(colors)) {
    const varName = COLOR_KEY_TO_VAR[key];
    if (varName && value) {
      root.style.setProperty(varName, value);
    }
  }
}

interface ThemeProviderProps {
  children: React.ReactNode;
}

export function ThemeProvider({ children }: ThemeProviderProps) {
  const [theme, setThemeState] = useState<Theme>("light");
  const [customColorsState, setCustomColorsState] = useState<CustomColors | undefined>(undefined);
  const [settingsCardOpacityState, setSettingsCardOpacityState] = useState(5);

  useEffect(() => {
    let cancelled = false;

    (async () => {
      const [savedTheme, savedColors, savedSettingsCardOpacity] = await Promise.all([
        getStoredTheme() as Promise<Theme>,
        getCustomColors(),
        getStoredSettingsCardOpacity(),
      ]);
      if (cancelled) return;
      setThemeState(savedTheme);
      updateDocumentTheme(savedTheme);
      if (savedColors) {
        setCustomColorsState(savedColors);
        applyCustomColors(savedColors);
      }
      setSettingsCardOpacityState(savedSettingsCardOpacity);
      applySettingsCardOpacity(savedSettingsCardOpacity);
    })();

    return () => {
      cancelled = true;
    };
  }, []);

  const updateDocumentTheme = (newTheme: Theme) => {
    const root = document.documentElement;
    if (newTheme === "dark") {
      root.classList.add("dark");
    } else {
      root.classList.remove("dark");
    }
  };

  const setTheme = (newTheme: Theme) => {
    setThemeState(newTheme);
    void setStoredTheme(newTheme);
    updateDocumentTheme(newTheme);
  };

  const toggleTheme = () => {
    const newTheme = theme === "light" ? "dark" : "light";
    setTheme(newTheme);
  };

  const setCustomColors = useCallback((colors: CustomColors) => {
    setCustomColorsState(colors);
    applyCustomColors(colors);
    void saveCustomColors(colors);
  }, []);

  const setSettingsCardOpacity = useCallback((opacity: number) => {
    const normalized = Math.max(0, Math.min(100, Math.round(opacity)));
    setSettingsCardOpacityState(normalized);
    applySettingsCardOpacity(normalized);
    void saveSettingsCardOpacity(normalized);
  }, []);

  return (
    <ThemeContext.Provider
      value={{
        theme,
        toggleTheme,
        setTheme,
        customColors: customColorsState,
        setCustomColors,
        settingsCardOpacity: settingsCardOpacityState,
        setSettingsCardOpacity,
      }}
    >
      {children}
    </ThemeContext.Provider>
  );
}
