export function cleanupOldDrafts(daysOld: number = 30) {
  const threshold = Date.now() - daysOld * 24 * 60 * 60 * 1000;
  const keysToRemove: string[] = [];

  for (let i = 0; i < localStorage.length; i++) {
    const key = localStorage.key(i);
    if (key?.startsWith("chat_draft_")) {
      try {
        const value = localStorage.getItem(key);
        if (value) {
          const parsed = JSON.parse(value);
          if (parsed.updatedAt && parsed.updatedAt < threshold) {
            keysToRemove.push(key);
          }
        }
      } catch (e) {
        // If it's not JSON (legacy or corrupted), we can either leave it or remove it.
        // Let's remove it if it's very old or just let it be for now.
        // For now, let's just keep it to be safe, or mark it for removal if it doesn't look like JSON.
      }
    }
  }

  keysToRemove.forEach((key) => localStorage.removeItem(key));
  if (keysToRemove.length > 0) {
    console.log(`Cleaned up ${keysToRemove.length} old chat drafts.`);
  }
}
