import { queryOptions, useQuery } from "@tanstack/react-query";
import { hexToRgb } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { CharacterSummary } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

export interface CardColors {
  css: string;
  fg: string | null;
  fgMuted: string | null;
}

const lightText = { fg: "#ffffff", fgMuted: "rgba(255, 255, 255, 0.7)" };
const darkText = { fg: "#1a1a1a", fgMuted: "rgba(0, 0, 0, 0.6)" };

/** A CSS gradient from custom colours: one is a flat colour, three or more run first, third, second. */
export function customGradientCss(colors: readonly string[]): string {
  if (colors.length === 1) return colors[0] ?? "";
  if (colors.length === 2) return `linear-gradient(135deg, ${colors[0]}, ${colors[1]})`;
  return `linear-gradient(135deg, ${[colors[0], colors[2], colors[1]].join(", ")})`;
}

/** Dark text on a bright set of colours, light text otherwise, from their average perceived brightness. */
export function textOnColors(colors: readonly string[]): { fg: string; fgMuted: string } {
  if (colors.length === 0) return lightText;
  let total = 0;
  for (const color of colors) {
    const rgb = hexToRgb(color);
    if (rgb) total += 0.299 * rgb.r + 0.587 * rgb.g + 0.114 * rgb.b;
  }
  return total / colors.length > 140 ? darkText : lightText;
}

export function avatarGradientQuery(api: ApiClient, assetId: string) {
  return queryOptions({
    queryKey: queryKeys.media.avatarGradient(assetId),
    queryFn: () => api.call("avatarGradient", { asset_id: assetId, force: false }),
    staleTime: Infinity,
  });
}

/** The colours behind a character's card, or null when it has none or they are still loading. */
export function useCardColors(api: ApiClient, character: CharacterSummary): CardColors | null {
  const { gradient, avatar } = character;
  const fromAvatar = gradient.type === "avatar" && avatar !== null;
  const avatarGradient = useQuery({ ...avatarGradientQuery(api, avatar?.asset_id ?? ""), enabled: fromAvatar });
  if (gradient.type === "none") return { css: "var(--color-surface-1)", fg: null, fgMuted: null };
  if (gradient.type === "custom") {
    const auto = textOnColors(gradient.colors);
    return { css: customGradientCss(gradient.colors), fg: gradient.text_color ?? auto.fg, fgMuted: gradient.text_secondary ?? auto.fgMuted };
  }
  if (!fromAvatar || !avatarGradient.data) return null;
  return { css: avatarGradient.data.gradient_css, fg: avatarGradient.data.text_color, fgMuted: avatarGradient.data.text_secondary };
}
