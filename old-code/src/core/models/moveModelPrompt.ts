const STORAGE_KEY = "models.moveToLibraryDismissed";

function readDismissed(): string[] {
  if (typeof localStorage === "undefined") return [];
  const raw = localStorage.getItem(STORAGE_KEY);
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((v): v is string => typeof v === "string") : [];
  } catch {
    return [];
  }
}

export function isMovePromptDismissed(path: string): boolean {
  const trimmed = path.trim();
  if (!trimmed) return false;
  return readDismissed().includes(trimmed);
}

export function dismissMovePrompt(path: string): void {
  if (typeof localStorage === "undefined") return;
  const trimmed = path.trim();
  if (!trimmed) return;
  const dismissed = readDismissed();
  if (dismissed.includes(trimmed)) return;
  dismissed.push(trimmed);
  localStorage.setItem(STORAGE_KEY, JSON.stringify(dismissed));
}

export function clearMovePromptDismissal(path: string): void {
  if (typeof localStorage === "undefined") return;
  const trimmed = path.trim();
  if (!trimmed) return;
  const remaining = readDismissed().filter((entry) => entry !== trimmed);
  localStorage.setItem(STORAGE_KEY, JSON.stringify(remaining));
}
