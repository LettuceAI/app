import { useEffect, useState } from "react";
import { getAppState } from "../../../../core/storage/appState";

let cachedShowNsfw: boolean | null = null;

/**
 * NSFW content is filtered Rust-side by Pure mode; cards with isNsfw only
 * reach the UI when the user allows them. This controls whether those cards
 * render unblurred.
 */
export function useShowNsfwImages(): boolean {
  const [show, setShow] = useState(cachedShowNsfw ?? false);

  useEffect(() => {
    let cancelled = false;
    getAppState()
      .then((state) => {
        cachedShowNsfw = !state.pureModeEnabled;
        if (!cancelled) setShow(cachedShowNsfw);
      })
      .catch(() => {
        // keep the safe default (blurred)
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return show;
}
