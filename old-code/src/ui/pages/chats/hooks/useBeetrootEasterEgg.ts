import { useEffect, useRef, useState } from "react";

const BEET_REGEX = /\b(beet|beets|beetroot|beetroots)\b/i;
const STORAGE_KEY = "lettuce.easterEggs.beetroot";
const EVENT_NAME = "lettuce:easterEggs:beetroot";

interface BeetrootMessage {
  id: string;
  content: string;
}

interface UseBeetrootEasterEggOptions {
  messages: ReadonlyArray<BeetrootMessage>;
  fire: () => void;
}

function readStoredEnabled(): boolean {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === null) return true;
    return stored === "true";
  } catch {
    return true;
  }
}

/**
 * Watches the messages list (including streaming chunks) and fires the supplied
 * callback the first time a message's content contains a beet-flavored token.
 * Each message ID can only trigger once for its lifetime, and the whole effect
 * is gated by the user-controllable toggle in Security settings.
 */
export function useBeetrootEasterEgg({ messages, fire }: UseBeetrootEasterEggOptions): void {
  const firedRef = useRef<Set<string>>(new Set());
  const fireRef = useRef(fire);
  const [enabled, setEnabled] = useState(readStoredEnabled);

  useEffect(() => {
    fireRef.current = fire;
  }, [fire]);

  useEffect(() => {
    const handleStorage = (event: StorageEvent) => {
      if (event.key === STORAGE_KEY) {
        setEnabled(event.newValue === null ? true : event.newValue === "true");
      }
    };
    const handleToggleEvent = (event: Event) => {
      const detail = (event as CustomEvent<boolean>).detail;
      if (typeof detail === "boolean") setEnabled(detail);
      else setEnabled(readStoredEnabled());
    };
    window.addEventListener("storage", handleStorage);
    window.addEventListener(EVENT_NAME, handleToggleEvent);
    return () => {
      window.removeEventListener("storage", handleStorage);
      window.removeEventListener(EVENT_NAME, handleToggleEvent);
    };
  }, []);

  useEffect(() => {
    if (!enabled) return;
    for (const message of messages) {
      if (firedRef.current.has(message.id)) continue;
      const content = message.content ?? "";
      if (!content) continue;
      if (BEET_REGEX.test(content)) {
        firedRef.current.add(message.id);
        fireRef.current();
      }
    }
  }, [messages, enabled]);
}
