import { useEffect, useState } from "react";
import type { AppPlatform } from "@/api/generated/bindings";
import { createWindowController, type WindowController } from "@/api/window-controls";

export function useWindowController(platform: AppPlatform) {
  const [controller, setController] = useState<WindowController | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    void createWindowController(platform).then(async (window) => {
      if (disposed) return;
      setController(window);
      setExpanded(false);
      setFailed(false);
      if (!window) return;
      stop = await window.observeExpanded((value) => { if (!disposed) setExpanded(value); }, () => { if (!disposed) setFailed(true); });
      if (disposed) stop();
    }).catch(() => { if (!disposed) setFailed(true); });
    return () => { disposed = true; stop?.(); };
  }, [platform]);
  const perform = (action: () => Promise<void>) => {
    setFailed(false);
    void action().catch(() => setFailed(true));
  };
  return { controller, expanded, failed, perform };
}
