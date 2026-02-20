import { invoke } from "@tauri-apps/api/core";
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
  headerOverlay: string;
  footerOverlay: string;
  contentOverlay: string;
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
  const userColorCSS = resolveBubbleTokenCSS(settings.userBubbleColor);
  const assistantColorCSS = resolveBubbleTokenCSS(settings.assistantBubbleColor);

  return invoke<ThemeColors>("compute_chat_theme", {
    settings: {
      userBubbleColor: settings.userBubbleColor,
      assistantBubbleColor: settings.assistantBubbleColor,
      bubbleOpacity: settings.bubbleOpacity,
      textMode: settings.textMode,
    },
    bgBrightness,
    resolved: {
      userColorCss: userColorCSS,
      assistantColorCss: assistantColorCSS,
    },
  });
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
    assistantBg: "bg-fg/5",
    assistantBorder: "border-fg/10",
    assistantText: "text-fg",
    userBg: "bg-accent/35",
    userBorder: "border-accent/50",
    userText: "text-white/95",
    headerOverlay: "",
    footerOverlay: "",
    contentOverlay: "",
  };
}
