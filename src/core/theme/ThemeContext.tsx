import React, { createContext, useContext, useState, useEffect, useCallback } from "react";
import { getTheme as getStoredTheme, setTheme as setStoredTheme, getCustomColors, setCustomColors as saveCustomColors } from "../storage/appState";
import type { CustomColors } from "../storage/schemas";

type Theme = "light" | "dark";

interface ThemeContextType {
  theme: Theme;
  toggleTheme: () => void;
  setTheme: (theme: Theme) => void;
  customColors: CustomColors | undefined;
  setCustomColors: (colors: CustomColors) => void;
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
  accent: "--color-accent",
  danger: "--color-danger",
  warning: "--color-warning",
  info: "--color-info",
  secondary: "--color-secondary",
  nav: "--color-nav",
};

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

  useEffect(() => {
    let cancelled = false;

    (async () => {
      const [savedTheme, savedColors] = await Promise.all([
        getStoredTheme() as Promise<Theme>,
        getCustomColors(),
      ]);
      if (cancelled) return;
      setThemeState(savedTheme);
      updateDocumentTheme(savedTheme);
      if (savedColors) {
        setCustomColorsState(savedColors);
        applyCustomColors(savedColors);
      }
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

  return (
    <ThemeContext.Provider value={{ theme, toggleTheme, setTheme, customColors: customColorsState, setCustomColors }}>
      {children}
    </ThemeContext.Provider>
  );
}
