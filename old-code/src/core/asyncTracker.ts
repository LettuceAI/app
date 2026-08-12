type PendingAsyncEntry = {
  startedAt: number;
  label: string;
};

const pendingAsync = new Map<string, PendingAsyncEntry>();

export function beginAsyncAction(id: string, label: string): void {
  pendingAsync.set(id, { startedAt: Date.now(), label });
}

export function endAsyncAction(id: string): void {
  pendingAsync.delete(id);
}

export function hasPendingAsyncActions(): boolean {
  return pendingAsync.size > 0;
}

export function listPendingAsyncActions(): Array<{
  id: string;
  label: string;
  ageMs: number;
}> {
  const now = Date.now();
  return Array.from(pendingAsync.entries()).map(([id, entry]) => ({
    id,
    label: entry.label,
    ageMs: now - entry.startedAt,
  }));
}
