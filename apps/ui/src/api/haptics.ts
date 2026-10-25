import { ApiFailure, isInsideShell } from "./client";
import type {
  AppPlatform,
  SettingsUiAccessibilitySettingsHapticIntensity,
} from "./generated/bindings";

export async function typingHaptic(
  intensity: SettingsUiAccessibilitySettingsHapticIntensity,
  platform: AppPlatform | undefined,
): Promise<void> {
  if (!isInsideShell() || (platform !== "android" && platform !== "ios")) return;
  try {
    const { impactFeedback } = await import("@tauri-apps/plugin-haptics");
    const outcome = await impactFeedback(intensity);
    if (outcome.status === "error") throw outcome.error;
  } catch (cause) {
    throw new ApiFailure("unavailable", "haptic feedback could not be played", null, { cause });
  }
}
