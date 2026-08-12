import { useEffect, useState } from "react";

const STORAGE_KEY = "chat.authorNote.inlineEditor";
const EVENT_NAME = "chat.authorNote.inlineEditor.change";

function readSetting(): boolean {
  if (typeof window === "undefined") return false;
  try {
    return window.localStorage.getItem(STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

export function setAuthorNoteInlineEditor(enabled: boolean) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(STORAGE_KEY, enabled ? "1" : "0");
  } catch {
    return;
  }
  window.dispatchEvent(new CustomEvent(EVENT_NAME, { detail: enabled }));
}

export function useAuthorNoteInlineEditor(): boolean {
  const [enabled, setEnabled] = useState<boolean>(() => readSetting());

  useEffect(() => {
    const handleChange = () => setEnabled(readSetting());
    window.addEventListener(EVENT_NAME, handleChange);
    window.addEventListener("storage", handleChange);
    return () => {
      window.removeEventListener(EVENT_NAME, handleChange);
      window.removeEventListener("storage", handleChange);
    };
  }, []);

  return enabled;
}
