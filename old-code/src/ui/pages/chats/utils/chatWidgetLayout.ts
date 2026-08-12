import { useEffect, useState } from "react";
import type { ChatAppearanceSettings } from "../../../../core/storage/schemas";

export const WIDGET_PANEL_MIN_WIDTH = 120;
export const WIDGET_PANEL_GAP = 0;
const LG_BREAKPOINT = 1024;

const PRESET_PX: Record<
  Exclude<ChatAppearanceSettings["chatColumnWidth"], "full" | "custom">,
  number
> = {
  narrow: 640,
  normal: 800,
  wide: 1024,
  xl: 1280,
};

export function resolveChatColumnPx(
  appearance: ChatAppearanceSettings,
): number | null {
  const width = appearance.chatColumnWidth;
  if (width === "full") return null;
  if (width === "custom") {
    return Math.min(2400, Math.max(400, appearance.chatColumnWidthPx ?? 800));
  }
  return PRESET_PX[width];
}

export interface ChatWidgetLayout {
  enabled: boolean;
  showLeft: boolean;
  showRight: boolean;
  panelMinWidth: number;
  gap: number;
  columnPx: number | null;
}

export function getChatWidgetLayout(
  appearance: ChatAppearanceSettings,
  viewportWidth: number,
): ChatWidgetLayout {
  const columnPx = resolveChatColumnPx(appearance);
  const base: ChatWidgetLayout = {
    enabled: false,
    showLeft: false,
    showRight: false,
    panelMinWidth: WIDGET_PANEL_MIN_WIDTH,
    gap: WIDGET_PANEL_GAP,
    columnPx,
  };
  if (!appearance.chatWidgetAreaEnabled) return base;
  if (viewportWidth < LG_BREAKPOINT) return base;
  if (columnPx == null) return base;
  const spare = viewportWidth - columnPx;
  const align = appearance.chatColumnAlign;
  if (align === "center") {
    const mode = appearance.chatWidgetCenterMode;
    if (mode === "both") {
      if (spare / 2 >= WIDGET_PANEL_MIN_WIDTH) {
        return { ...base, enabled: true, showLeft: true, showRight: true };
      }
      return base;
    }
    if (spare / 2 < WIDGET_PANEL_MIN_WIDTH) return base;
    return {
      ...base,
      enabled: true,
      showLeft: mode === "left",
      showRight: mode === "right",
    };
  }
  if (spare < WIDGET_PANEL_MIN_WIDTH) return base;
  if (align === "left") {
    return { ...base, enabled: true, showLeft: false, showRight: true };
  }
  return { ...base, enabled: true, showLeft: true, showRight: false };
}

export function useViewportWidth(): number {
  const [width, setWidth] = useState<number>(() =>
    typeof window === "undefined" ? 1280 : window.innerWidth,
  );
  useEffect(() => {
    const onResize = () => setWidth(window.innerWidth);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);
  return width;
}
