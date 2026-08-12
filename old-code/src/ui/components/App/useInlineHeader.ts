import { useEffect, useState } from "react";

import { readSettingsCached, SETTINGS_UPDATED_EVENT } from "../../../core/storage/repo";
import { getPlatform } from "../../../core/utils/platform";
import { readCachedNavPrefs } from "./navPrefs";

const isDesktopPlatform = getPlatform().type === "desktop";
const INLINE_HEADER_MEDIA_QUERY = "(min-width: 1024px)";

function isInlineHeaderStyle(): boolean {
  const style = readSettingsCached()?.advancedSettings?.headerStyle ?? readCachedNavPrefs().header;
  return style === "inline";
}

function matchesInlineViewport(): boolean {
  if (typeof window === "undefined") return false;
  return window.matchMedia(INLINE_HEADER_MEDIA_QUERY).matches;
}

function isRailNavStyle(): boolean {
  const style =
    readSettingsCached()?.advancedSettings?.navigationStyle ?? readCachedNavPrefs().style;
  return style === "sidebar" || style === "floatingSidebar";
}

export function useRailSettings(): boolean {
  const [enabled, setEnabled] = useState(
    () => isDesktopPlatform && isRailNavStyle() && matchesInlineViewport(),
  );

  useEffect(() => {
    if (!isDesktopPlatform || typeof window === "undefined") return;
    const mediaQuery = window.matchMedia(INLINE_HEADER_MEDIA_QUERY);
    const sync = () => setEnabled(isRailNavStyle() && mediaQuery.matches);
    sync();
    mediaQuery.addEventListener("change", sync);
    window.addEventListener(SETTINGS_UPDATED_EVENT, sync);
    return () => {
      mediaQuery.removeEventListener("change", sync);
      window.removeEventListener(SETTINGS_UPDATED_EVENT, sync);
    };
  }, []);

  return enabled;
}

export function resolveInlineHeader(): boolean {
  return isDesktopPlatform && isInlineHeaderStyle() && matchesInlineViewport();
}

export function useInlineHeader(): boolean {
  const [enabled, setEnabled] = useState(resolveInlineHeader);

  useEffect(() => {
    if (!isDesktopPlatform || typeof window === "undefined") return;
    const mediaQuery = window.matchMedia(INLINE_HEADER_MEDIA_QUERY);
    const sync = () => setEnabled(isInlineHeaderStyle() && mediaQuery.matches);
    sync();
    mediaQuery.addEventListener("change", sync);
    window.addEventListener(SETTINGS_UPDATED_EVENT, sync);
    return () => {
      mediaQuery.removeEventListener("change", sync);
      window.removeEventListener(SETTINGS_UPDATED_EVENT, sync);
    };
  }, []);

  return enabled;
}
