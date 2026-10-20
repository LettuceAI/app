import type { OpenedItem } from "./generated/bindings";
import { isInsideShell } from "./client";

export interface OpenedFilesInbox {
  take(): Promise<OpenedItem[]>;
  listen(onOpened: () => void): Promise<() => void>;
}

export async function createOpenedFilesInbox(): Promise<OpenedFilesInbox | null> {
  if (!isInsideShell()) return null;
  const { commands, events } = await import("./generated/bindings");
  return {
    take: () => commands.openedFilesTake(),
    listen: async (onOpened) => {
      const unlisten = await events.openedFiles.listen(() => onOpened());
      return () => { unlisten(); };
    },
  };
}
