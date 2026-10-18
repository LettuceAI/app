import type { LibraryVoicePreference } from "@/api/generated/bindings";

export function voiceLabel(voice: LibraryVoicePreference | null, fallback: string): string | null {
  if (!voice) return null;
  return voice.kind === "provider" ? (voice.value.voice_name ?? voice.value.voice_id) : fallback;
}
