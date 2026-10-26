import { ApiFailure, isInsideShell } from "./client";

export async function revealLocalModel(path: string): Promise<void> {
  if (!isInsideShell())
    throw new ApiFailure("unsupported", "file reveal requires the native shell", null);
  try {
    const { revealItemsInDir } = await import("@tauri-apps/plugin-opener");
    await revealItemsInDir(path);
  } catch (cause) {
    throw new ApiFailure("unavailable", "model file could not be revealed", null, { cause });
  }
}
