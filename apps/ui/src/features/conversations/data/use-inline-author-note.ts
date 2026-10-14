import { useEffect, useState } from "react";

const storageKey = "chat.authorNote.inlineEditor";
const changeEvent = "chat.authorNote.inlineEditor.change";

function readPreference(): boolean {
  try {
    return window.localStorage.getItem(storageKey) === "1";
  } catch {
    return false;
  }
}

export function setInlineAuthorNote(enabled: boolean): void {
  try {
    window.localStorage.setItem(storageKey, enabled ? "1" : "0");
  } catch {
    window.dispatchEvent(new CustomEvent(changeEvent, { detail: enabled }));
    return;
  }
  window.dispatchEvent(new CustomEvent(changeEvent, { detail: enabled }));
}

export function useInlineAuthorNote(): boolean {
  const [enabled, setEnabled] = useState(readPreference);
  useEffect(() => {
    const changed = (event: Event) => setEnabled((event as CustomEvent<boolean>).detail);
    const stored = () => setEnabled(readPreference());
    window.addEventListener(changeEvent, changed);
    window.addEventListener("storage", stored);
    return () => {
      window.removeEventListener(changeEvent, changed);
      window.removeEventListener("storage", stored);
    };
  }, []);
  return enabled;
}
