import { useCallback, useLayoutEffect, useRef, useState } from "react";
import type { ChatLook } from "@/entities/preferences";

export const WIDGET_PANEL_MIN = 220;

const PRESET_PX = { narrow: 640, normal: 800, wide: 1024, xl: 1280 } as const;

export interface WidgetLayout {
  enabled: boolean;
  showLeft: boolean;
  showRight: boolean;
  columnPx: number | null;
}

/** The pixel width the chat column takes; none when it fills the row. */
function columnWidth(look: ChatLook): number | null {
  if (look.chatColumnWidth === "full") return null;
  if (look.chatColumnWidth === "custom") return Math.min(2400, Math.max(400, look.chatColumnWidthPx));
  return PRESET_PX[look.chatColumnWidth];
}

/**
 * Which side panels fit beside the chat column: both when each has room for the minimum, one when only one does, none on a phone, with a full-width column, or when the row is too narrow. `areaWidth` is the width of the row the column and panels share.
 */
export function widgetLayout(look: ChatLook, areaWidth: number, phone: boolean): WidgetLayout {
  const columnPx = columnWidth(look);
  const none = { enabled: false, showLeft: false, showRight: false, columnPx };
  if (!look.chatWidgetAreaEnabled || phone || columnPx === null) return none;
  const spare = areaWidth - columnPx;
  if (look.chatColumnAlign === "center") {
    if (spare / 2 < WIDGET_PANEL_MIN) return none;
    const mode = look.chatWidgetCenterMode;
    return { enabled: true, showLeft: mode !== "right", showRight: mode !== "left", columnPx };
  }
  if (spare < WIDGET_PANEL_MIN) return none;
  return { enabled: true, showLeft: look.chatColumnAlign === "right", showRight: look.chatColumnAlign === "left", columnPx };
}

/** The measured width of an element, kept current by a ResizeObserver. */
export function useAreaWidth<T extends HTMLElement>() {
  const [width, setWidth] = useState(0);
  const observer = useRef<ResizeObserver | null>(null);
  const ref = useCallback((element: T | null) => {
    observer.current?.disconnect();
    observer.current = null;
    if (!element) return;
    setWidth(element.getBoundingClientRect().width);
    observer.current = new ResizeObserver((entries) => {
      const next = entries[entries.length - 1]?.contentRect.width;
      if (next !== undefined) setWidth(next);
    });
    observer.current.observe(element);
  }, []);
  useLayoutEffect(() => () => observer.current?.disconnect(), []);
  return [ref, width] as const;
}
