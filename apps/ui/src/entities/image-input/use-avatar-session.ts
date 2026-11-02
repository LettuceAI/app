import { useLayoutEffect, useRef, useState, useSyncExternalStore } from "react";
import type { ApiClient } from "@/api/client";
import { AvatarSession, type AvatarContext, type AvatarSessionState } from "./avatar-session";

const idle: AvatarSessionState = { options: null, loadingOptions: true, phase: "idle", variants: [], selectedId: null, job: null, error: null, errorSource: null, pendingRequest: false };
const noSubscribe = () => () => undefined;
const idleSnapshot = () => idle;

/**
 * One independent session for as long as the panel is open. It is created when the panel opens, with the form values
 * of that moment, and disposed when it closes, when the owner changes, or when the database is reloaded. The pair is
 * matched inside one effect, so development replay never leaves a disposed session mounted.
 */
export function useAvatarSession(api: ApiClient, open: boolean, readContext: () => AvatarContext, onReset: () => void): { session: AvatarSession | null; state: AvatarSessionState } {
  const [session, setSession] = useState<AvatarSession | null>(null);
  const latest = useRef({ readContext, onReset });
  useLayoutEffect(() => {
    latest.current = { readContext, onReset };
  });
  useLayoutEffect(() => {
    if (!open) return;
    const next = new AvatarSession(api, latest.current.readContext());
    setSession(next);
    void next.loadOptions();
    let active = true;
    let stop: (() => void) | undefined;
    void api.subscribe((event) => {
      if (event.type === "database_reloaded") latest.current.onReset();
    }).then((unsubscribe) => {
      if (active) stop = unsubscribe;
      else unsubscribe();
    }).catch(() => undefined);
    return () => {
      active = false;
      stop?.();
      next.dispose();
    };
  }, [api, open]);
  const state = useSyncExternalStore(session?.subscribe ?? noSubscribe, session?.getSnapshot ?? idleSnapshot, session?.getSnapshot ?? idleSnapshot);
  return { session, state };
}
