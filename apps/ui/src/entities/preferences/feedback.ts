import { ApiFailure } from "@/api/client";
import send from "@/shared/assets/sounds/send.mp3";
import success from "@/shared/assets/sounds/success.mp3";
import failure from "@/shared/assets/sounds/failure.mp3";
import { object, type Preferences } from "./context";

export type SoundKey = "send" | "success" | "failure";
export const SOUND_DEFAULTS = {
  send: { enabled: false, volume: 0.5 },
  success: { enabled: false, volume: 0.6 },
  failure: { enabled: false, volume: 0.6 },
};
const sources = { send, success, failure };
const playing = new Set<HTMLAudioElement>();

export function soundSettings(preferences: Preferences, key: SoundKey) {
  const value = object(object(preferences.accessibility)[key]);
  return {
    enabled: typeof value.enabled === "boolean" ? value.enabled : SOUND_DEFAULTS[key].enabled,
    volume:
      typeof value.volume === "number"
        ? Math.min(1, Math.max(0, value.volume))
        : SOUND_DEFAULTS[key].volume,
  };
}

export async function playSound(
  key: SoundKey,
  preferences: Preferences,
  preview = false,
): Promise<void> {
  const settings = soundSettings(preferences, key);
  if (!settings.enabled && !preview) return;
  const audio = new Audio(sources[key]);
  audio.volume = settings.volume;
  playing.add(audio);
  const release = () => {
    playing.delete(audio);
    audio.removeEventListener("ended", release);
    audio.removeEventListener("error", release);
  };
  audio.addEventListener("ended", release);
  audio.addEventListener("error", release);
  try {
    await audio.play();
  } catch (cause) {
    release();
    throw new ApiFailure("unavailable", "sound cue could not be played", null, { cause });
  }
}
