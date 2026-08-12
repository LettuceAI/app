import { invoke } from "@tauri-apps/api/core";
import type { AccessibilitySettings } from "../storage/schemas";

export type AccessibilitySoundType = "send" | "success" | "failure";

type AccessibilitySoundBase64 = Record<AccessibilitySoundType, string>;
type AccessibilitySoundUrls = Record<AccessibilitySoundType, string>;

const SOUND_MIME_TYPE = "audio/mpeg";

let soundUrls: AccessibilitySoundUrls | null = null;

async function loadSoundUrls(): Promise<AccessibilitySoundUrls> {
  if (soundUrls) return soundUrls;

  const base64 = await invoke<AccessibilitySoundBase64>("accessibility_sound_base64");
  soundUrls = {
    send: `data:${SOUND_MIME_TYPE};base64,${base64.send}`,
    success: `data:${SOUND_MIME_TYPE};base64,${base64.success}`,
    failure: `data:${SOUND_MIME_TYPE};base64,${base64.failure}`,
  };

  return soundUrls;
}

function clampVolume(v: number) {
  return Number.isNaN(v) ? 0 : Math.min(1, Math.max(0, v));
}

export async function playAccessibilitySound(
  type: AccessibilitySoundType,
  settings?: AccessibilitySettings,
) {
  const cfg = settings?.[type];
  if (!cfg?.enabled) return;

  const urls = await loadSoundUrls();
  const audio = new Audio(urls[type]);

  audio.volume = clampVolume(cfg.volume);
  audio.preload = "auto";

  await audio.play().catch(() => undefined);
}
