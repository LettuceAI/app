import type { ChatAppearanceSettings } from "../storage/schemas";
import { createDefaultChatAppearanceSettings } from "../storage/schemas";

/**
 * Analyzes an image and returns the average brightness (0–255).
 */
export async function analyzeImageBrightness(imageSrc: string): Promise<number> {
  return new Promise((resolve) => {
    const img = new Image();
    img.crossOrigin = "Anonymous";

    img.onload = () => {
      try {
        const canvas = document.createElement("canvas");
        const ctx = canvas.getContext("2d");

        if (!ctx) {
          console.warn("Could not get canvas context");
          resolve(0);
          return;
        }

        const sampleSize = 100;
        canvas.width = sampleSize;
        canvas.height = sampleSize;

        ctx.drawImage(img, 0, 0, sampleSize, sampleSize);

        const imageData = ctx.getImageData(0, 0, sampleSize, sampleSize);
        const data = imageData.data;

        let totalBrightness = 0;
        let pixelCount = 0;

        for (let i = 0; i < data.length; i += 4) {
          const r = data[i];
          const g = data[i + 1];
          const b = data[i + 2];
          const a = data[i + 3];

          if (a < 128) continue;

          const brightness = 0.299 * r + 0.587 * g + 0.114 * b;
          totalBrightness += brightness;
          pixelCount++;
        }

        if (pixelCount === 0) {
          resolve(0);
          return;
        }
        const avgBrightness = totalBrightness / pixelCount;
        console.log(`[Image Analysis] Average brightness: ${avgBrightness.toFixed(1)}`);
        resolve(avgBrightness);
      } catch (error) {
        console.error("Error analyzing image:", error);
        resolve(0);
      }
    };

    img.onerror = () => {
      console.error("Error loading image for analysis");
      resolve(0);
    };

    img.src = imageSrc;
  });
}

/**
 * Returns true if the image is predominantly light.
 */
export async function isImageLight(imageSrc: string): Promise<boolean> {
  const brightness = await analyzeImageBrightness(imageSrc);
  return brightness > 127.5;
}

/**
 * Reads a CSS custom property from :root and returns its trimmed value.
 */
export function resolveCSSColor(tokenName: string): string {
  return getComputedStyle(document.documentElement)
    .getPropertyValue("--color-" + tokenName)
    .trim();
}

const HEX_COLOR_RE = /^#?([0-9a-fA-F]{3}|[0-9a-fA-F]{4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/;

export function normalizeHexColor(value?: string | null): string | undefined {
  const trimmed = value?.trim();
  if (!trimmed || !HEX_COLOR_RE.test(trimmed)) return undefined;
  const withHash = trimmed.startsWith("#") ? trimmed : `#${trimmed}`;
  return withHash.toUpperCase();
}

/**
 * Resolve the CSS color for a bubble color token name.
 */
function resolveBubbleTokenCSS(tokenName: string): string {
  if (tokenName === "neutral") {
    return resolveCSSColor("fg");
  }
  return resolveCSSColor(tokenName);
}

/**
 * Theme colors returned from the Rust backend.
 */
export interface ThemeColors {
  assistantBg: string;
  assistantBorder: string;
  assistantText: string;
  userBg: string;
  userBorder: string;
  userText: string;
  assistantBgColor: string;
  assistantBorderColor: string;
  userBgColor: string;
  userBorderColor: string;
  headerOverlay: string;
  footerOverlay: string;
  contentOverlay: string;
}

function colorToLuminance(color: string): number {
  const trimmed = color.trim();

  if (trimmed.startsWith("#")) {
    const hex = trimmed.slice(1);
    const expanded =
      hex.length === 3
        ? `${hex[0]}${hex[0]}${hex[1]}${hex[1]}${hex[2]}${hex[2]}`
        : hex.length === 6 || hex.length === 8
          ? hex.slice(0, 6)
          : "";
    if (expanded.length === 6) {
      const r = parseInt(expanded.slice(0, 2), 16) / 255;
      const g = parseInt(expanded.slice(2, 4), 16) / 255;
      const b = parseInt(expanded.slice(4, 6), 16) / 255;
      return 0.299 * r + 0.587 * g + 0.114 * b;
    }
  }

  if (trimmed.startsWith("rgb")) {
    const inner = trimmed.replace(/^rgba?\(/, "").replace(/\)$/, "");
    const parts = inner
      .split(/[,\s/]+/)
      .map((v) => v.trim())
      .filter(Boolean)
      .map((v) => Number.parseFloat(v));
    if (parts.length >= 3 && parts.every((n) => Number.isFinite(n))) {
      const r = parts[0] / 255;
      const g = parts[1] / 255;
      const b = parts[2] / 255;
      return 0.299 * r + 0.587 * g + 0.114 * b;
    }
  }

  if (trimmed.startsWith("oklch(")) {
    const inner = trimmed.slice("oklch(".length, -1).trim();
    const first = inner.split(/\s+/)[0] ?? "0.5";
    if (first.endsWith("%")) {
      const parsed = Number.parseFloat(first.slice(0, -1));
      return Number.isFinite(parsed) ? Math.min(1, Math.max(0, parsed / 100)) : 0.5;
    }
    const parsed = Number.parseFloat(first);
    return Number.isFinite(parsed) ? Math.min(1, Math.max(0, parsed)) : 0.5;
  }

  return 0.5;
}

function computeTextClass(
  bgBrightness: number | null,
  bubbleLuminance: number,
  bubbleOpacity01: number,
  textMode: "auto" | "light" | "dark",
): string {
  if (textMode === "light") return "text-white";
  if (textMode === "dark") return "text-gray-900";

  const effectiveLum =
    bgBrightness === null
      ? bubbleLuminance
      : bubbleOpacity01 * bubbleLuminance + (1 - bubbleOpacity01) * (bgBrightness / 255);

  return effectiveLum > 0.45 ? "text-gray-900" : "text-white/95";
}

/**
 * Computes chat theme colors by invoking the Rust backend.
 * Resolves CSS colors from the DOM first, then delegates computation to Rust.
 *
 * bgBrightness: null when no background image, 0–255 otherwise.
 */
export async function computeChatTheme(
  settings: ChatAppearanceSettings,
  bgBrightness: number | null,
): Promise<ThemeColors> {
  const userColorCSS =
    normalizeHexColor(settings.userBubbleColorHex) ??
    resolveBubbleTokenCSS(settings.userBubbleColor);
  const assistantColorCSS =
    normalizeHexColor(settings.assistantBubbleColorHex) ??
    resolveBubbleTokenCSS(settings.assistantBubbleColor);
  const opacity = Math.max(0, Math.min(100, settings.bubbleOpacity));
  const opacity01 = opacity / 100;
  const borderPct = 50;
  const textMode = settings.textMode as "auto" | "light" | "dark";

  const userLum = colorToLuminance(userColorCSS);
  const assistantLum = colorToLuminance(assistantColorCSS);
  const isNeutralAssistant =
    settings.assistantBubbleColor === "neutral" &&
    !normalizeHexColor(settings.assistantBubbleColorHex);
  const isLightBg = bgBrightness !== null && bgBrightness > 127.5;

  const userText = computeTextClass(bgBrightness, userLum, opacity01, textMode);
  const assistantText =
    isNeutralAssistant && bgBrightness === null
      ? "text-fg"
      : computeTextClass(
          bgBrightness,
          isNeutralAssistant && bgBrightness !== null ? (isLightBg ? 0 : 0.35) : assistantLum,
          opacity01,
          textMode,
        );

  const userBgColor = `color-mix(in oklab, ${userColorCSS} ${opacity}%, transparent)`;
  const userBorderColor = `color-mix(in oklab, ${userColorCSS} ${borderPct}%, transparent)`;

  const assistantBgColor =
    isNeutralAssistant && bgBrightness === null
      ? "color-mix(in oklab, var(--color-fg) 5%, transparent)"
      : isNeutralAssistant && isLightBg
        ? `rgba(0, 0, 0, ${opacity01})`
        : isNeutralAssistant
          ? `rgba(75, 85, 99, ${Math.max(0, Math.min(1, (opacity * 0.85) / 100))})`
          : `color-mix(in oklab, ${assistantColorCSS} ${opacity}%, transparent)`;

  const assistantBorderColor =
    isNeutralAssistant && bgBrightness === null
      ? "color-mix(in oklab, var(--color-fg) 10%, transparent)"
      : isNeutralAssistant && isLightBg
        ? "rgba(0, 0, 0, 0.4)"
        : isNeutralAssistant
          ? "rgba(156, 163, 175, 0.4)"
          : `color-mix(in oklab, ${assistantColorCSS} ${borderPct}%, transparent)`;

  return {
    userBg: "",
    userBorder: "",
    userText,
    assistantBg: "",
    assistantBorder: "",
    assistantText,
    userBgColor,
    userBorderColor,
    assistantBgColor,
    assistantBorderColor,
    headerOverlay: bgBrightness === null ? "" : isLightBg ? "bg-white/45" : "bg-[#050505]/40",
    footerOverlay: bgBrightness === null ? "" : isLightBg ? "bg-white/50" : "bg-[#050505]/45",
    contentOverlay:
      bgBrightness === null ? "" : isLightBg ? "rgba(255, 255, 255, 0.20)" : "rgba(5, 5, 5, 0.15)",
  };
}

/**
 * Legacy API — computes theme using default settings + boolean brightness.
 */
export async function getThemeForBackground(isLight: boolean): Promise<ThemeColors> {
  const defaults = createDefaultChatAppearanceSettings();
  return computeChatTheme(defaults, isLight ? 200 : 50);
}

/**
 * Synchronous fallback theme (no Rust invoke) for initial render before async resolves.
 */
export function getDefaultThemeSync(): ThemeColors {
  return {
    assistantBg: "",
    assistantBorder: "",
    assistantText: "text-fg",
    userBg: "",
    userBorder: "",
    userText: "text-white/95",
    assistantBgColor: "color-mix(in oklab, var(--color-fg) 5%, transparent)",
    assistantBorderColor: "color-mix(in oklab, var(--color-fg) 10%, transparent)",
    userBgColor: "color-mix(in oklab, var(--color-accent) 35%, transparent)",
    userBorderColor: "color-mix(in oklab, var(--color-accent) 50%, transparent)",
    headerOverlay: "",
    footerOverlay: "",
    contentOverlay: "",
  };
}
