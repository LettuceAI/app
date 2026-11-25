import { useState } from "react";

/**
 * What an overlay shows: the current value while it is open, and the last one while it animates closed, so its
 * content does not vanish before the exit. `session` changes each time a value opens, including the same value
 * opened again after closing, for use as a key that resets the overlay's own state.
 */
export function useClosingValue<T>(value: T | null): { shown: T | null; session: number } {
  const [shown, setShown] = useState<T | null>(value);
  const [current, setCurrent] = useState<T | null>(value);
  const [session, setSession] = useState(0);
  if (value !== current) {
    setCurrent(value);
    if (value !== null) {
      setShown(value);
      setSession((count) => count + 1);
    }
  }
  return { shown, session };
}
