import { invoke } from "@tauri-apps/api/core";
import type { AccessibilitySettings } from "../storage/schemas";

export type AccessibilitySoundType = "send" | "success" | "failure";

type AccessibilitySoundBase64 = Record<AccessibilitySoundType, string>;
type AccessibilitySoundUrls = Record<AccessibilitySoundType, string>;

const SOUND_MIME_TYPE = "audio/mpeg";

let soundUrls: AccessibilitySoundUrls | null = null;

function base64ToObjectUrl(base64: string, mimeType: string): string {
  const byteString = atob(base64);
  const bytes = new Uint8Array(byteString.length);
  for (let i = 0; i < byteString.length; i += 1) {
    bytes[i] = byteString.charCodeAt(i);
  }
  return URL.createObjectURL(new Blob([bytes], { type: mimeType }));
}

async function loadSoundUrls(): Promise<AccessibilitySoundUrls> {
  if (soundUrls) return soundUrls;

  const base64 = await invoke<AccessibilitySoundBase64>("accessibility_sound_base64");
  soundUrls = {
    send: base64ToObjectUrl(base64.send, SOUND_MIME_TYPE),
    success: base64ToObjectUrl(base64.success, SOUND_MIME_TYPE),
    failure: base64ToObjectUrl(base64.failure, SOUND_MIME_TYPE),
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
  try {
    const cfg = settings?.[type];
    if (!cfg?.enabled) return;

    const urls = await loadSoundUrls();
    const audio = new Audio(urls[type]);

    audio.volume = clampVolume(cfg.volume);
    audio.preload = "auto";

    await audio.play().catch(() => undefined);
  } catch (error) {
    console.warn("Failed to play accessibility sound:", error);
  }
}
