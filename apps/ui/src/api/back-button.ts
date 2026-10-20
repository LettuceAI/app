import type { AppPlatform } from "./generated/bindings";
import { isInsideShell } from "./client";

export interface BackButton {
  listen(onBack: (canGoBack: boolean) => void): Promise<() => void>;
  exit(): Promise<void>;
}

export async function createBackButton(platform: AppPlatform): Promise<BackButton | null> {
  if (!isInsideShell() || platform !== "android") return null;
  const { exit, onBackButtonPress } = await import("@tauri-apps/api/app");
  return {
    listen: async (onBack) => {
      const listener = await onBackButtonPress(({ canGoBack }) => onBack(canGoBack));
      return () => { void listener.unregister(); };
    },
    exit: () => exit(0),
  };
}
