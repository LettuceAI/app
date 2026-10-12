import type { AppPlatform } from "./generated/bindings";
import { isInsideShell } from "./client";

export type WindowEdge = "North" | "NorthEast" | "East" | "SouthEast" | "South" | "SouthWest" | "West" | "NorthWest";

export interface WindowController {
  minimize(): Promise<void>;
  toggleMaximize(): Promise<void>;
  close(): Promise<void>;
  resize(edge: WindowEdge): Promise<void>;
  observeExpanded(onChange: (expanded: boolean) => void, onError: () => void): Promise<() => void>;
}

export async function createWindowController(platform: AppPlatform): Promise<WindowController | null> {
  if (!isInsideShell() || !["windows", "linux", "macos"].includes(platform)) return null;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const window = getCurrentWindow();
  return {
    minimize: () => window.minimize(),
    toggleMaximize: () => window.toggleMaximize(),
    close: () => window.close(),
    resize: (edge) => window.startResizeDragging(edge),
    observeExpanded: async (onChange, onError) => {
      let active = true;
      let revision = 0;
      const update = async () => {
        const current = ++revision;
        try {
          const [maximized, fullscreen] = await Promise.all([window.isMaximized(), window.isFullscreen()]);
          if (active && current === revision) onChange(maximized || fullscreen);
        } catch {
          if (active) onError();
        }
      };
      const unlisten = await window.onResized(() => { void update(); });
      await update();
      return () => { active = false; unlisten(); };
    },
  };
}
