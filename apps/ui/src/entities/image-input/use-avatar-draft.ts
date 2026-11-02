import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useLocation } from "@tanstack/react-router";
import type { AvatarOwner } from "./avatar-owner-context";

const stashed = new Map<string, { routeKey: string; value: unknown }>();

export interface AvatarDraft<T> {
  restored: T | null;
  owner: AvatarOwner;
}

/**
 * Keeps a form draft in memory while its owner page visits image settings from the avatar panel. Back returns to the
 * history entry that held the open panel, and only a draft saved for that exact entry is handed back, so a form
 * opened fresh later never receives a stale one.
 */
export function useAvatarDraft<T>({ ownerId, read, beforeLeave }: { ownerId: string; read: () => T; beforeLeave: () => void }): AvatarDraft<T> {
  const routeKey = useLocation({ select: (location) => location.state.__TSR_key });
  const [restored] = useState<T | null>(() => {
    const entry = stashed.get(ownerId);
    return entry && entry.routeKey === routeKey ? (entry.value as T) : null;
  });
  useEffect(() => {
    stashed.delete(ownerId);
  }, [ownerId]);
  const latest = useRef({ read, beforeLeave });
  useLayoutEffect(() => {
    latest.current = { read, beforeLeave };
  });
  const owner = useMemo<AvatarOwner>(() => ({
    keepForSettings: (panelKey) => {
      latest.current.beforeLeave();
      if (panelKey) stashed.set(ownerId, { routeKey: panelKey, value: latest.current.read() });
    },
  }), [ownerId]);
  return { restored, owner };
}
