import { useBlocker } from "@tanstack/react-router";
import { dismissTopLayer } from "@lettuceai/crisp";

export function useOverlayBack(open: boolean, close: () => void) {
  useBlocker({
    shouldBlockFn: ({ action }) => {
      if (action !== "BACK") return false;
      if (dismissTopLayer()) return true;
      if (!open) return false;
      close();
      return true;
    },
    enableBeforeUnload: false,
  });
}
