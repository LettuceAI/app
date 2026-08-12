import { getAppStateSummary, resetAppState } from "./appState";
import { storageBridge } from "./files";

export class ResetManager {
  static async clearAppFiles(): Promise<void> {
    try {
      await storageBridge.clearAll();
      console.log("Encrypted storage cleared successfully");
    } catch (error) {
      console.error("Failed to clear storage:", error);
      throw new Error("Failed to clear app files. Please try again.");
    }
  }

  static async clearAppState(): Promise<void> {
    try {
      await resetAppState();
      console.log("App state cleared successfully");
    } catch (error) {
      console.error("Failed to clear app state:", error);
      throw new Error("Failed to clear app state. Please try again.");
    }
  }

  static async resetAllData(): Promise<void> {
    try {
      // Reset the database - this will delete and recreate app.db
      await storageBridge.resetDatabase();

      // Reset app state to defaults
      await this.clearAppState();

      console.log("All app data reset successfully");
    } catch (error) {
      console.error("Failed to reset all data:", error);
      throw error;
    }
  }

  static async getResetSummary(): Promise<{
    appState: Awaited<ReturnType<typeof getAppStateSummary>>;
    fileCount: number;
    estimatedSessions: number;
  }> {
    const appState = await getAppStateSummary();
    const usage = await storageBridge.usageSummary().catch(() => ({ fileCount: 0, estimatedSessions: 0, lastUpdatedMs: null }));

    return {
      appState,
      fileCount: usage.fileCount,
      estimatedSessions: usage.estimatedSessions,
    };
  }
}
