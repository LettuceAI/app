import { invoke } from "@tauri-apps/api/core";

export type AudioProviderType = "gemini_tts" | "elevenlabs" | "openai_tts" | "kokoro";

export interface AudioProvider {
  id: string;
  providerType: AudioProviderType;
  label: string;
  apiKey?: string;
  projectId?: string; // Gemini only
  location?: string; // Gemini only
  baseUrl?: string; // OpenAI-compatible TTS only
  requestPath?: string; // OpenAI-compatible TTS only
  kokoroVariant?: KokoroModelVariant; // Kokoro only
  assetRoot?: string; // Kokoro only
  createdAt?: number;
  updatedAt?: number;
  isSystem?: boolean;
}

export interface AudioModel {
  id: string;
  name: string;
  providerType: string;
}

export interface CachedVoice {
  id: string;
  providerId: string;
  voiceId: string;
  name: string;
  previewUrl?: string;
  labels: Record<string, string>;
  cachedAt: number;
}

export interface UserVoice {
  id: string;
  providerId: string;
  name: string;
  modelId: string;
  voiceId: string;
  prompt?: string;
  createdAt?: number;
  updatedAt?: number;
}

export interface TtsPreviewResponse {
  audioBase64: string;
  format: string;
}

export type KokoroModelVariant = "fp32" | "fp16" | "int8";

export interface KokoroSupportedVariant {
  id: KokoroModelVariant;
  label: string;
  filename: string;
  sizeMb: number;
  mobileSupported: boolean;
}

export interface KokoroInstalledVoice {
  id: string;
  path: string;
}

export interface KokoroVoiceBlendEntry {
  voiceId: string;
  weight: number;
}

export interface KokoroAssetStatus {
  variant: KokoroModelVariant;
  variantAllowedOnPlatform: boolean;
  resolvedModelPath: string | null;
  installedVoices: KokoroInstalledVoice[];
  selectedVoiceInstalled: boolean | null;
}

export interface KokoroAvailableVoice {
  id: string;
  installed: boolean;
}

export interface KokoroQueuedInstall {
  installId: string;
  queueIds: string[];
}

export interface KokoroTokenizePreviewSegment {
  kind: string;
  sourceText: string;
  ipa: string;
  tokenIds: number[];
}

export interface KokoroTokenizePreview {
  language: string;
  primaryVoiceId: string;
  voiceBlend: KokoroVoiceBlendEntry[];
  normalizedText: string;
  effectiveText: string;
  lexiconPath: string;
  lexiconEntryCount: number;
  usedLexiconEntries: string[];
  tokenIds: number[];
  tokenCount: number;
  chunkLengths: number[];
  warnings: string[];
  segments: KokoroTokenizePreviewSegment[];
}

export async function listAudioProviders(): Promise<AudioProvider[]> {
  return invoke<AudioProvider[]>("audio_provider_list");
}

export async function upsertAudioProvider(provider: AudioProvider): Promise<AudioProvider> {
  return invoke<AudioProvider>("audio_provider_upsert", {
    providerJson: JSON.stringify(provider),
  });
}

export async function deleteAudioProvider(id: string): Promise<void> {
  return invoke("audio_provider_delete", { id });
}

export async function verifyAudioProvider(
  providerType: AudioProviderType,
  apiKey: string,
  projectId?: string,
): Promise<boolean> {
  return invoke<boolean>("audio_provider_verify", {
    providerType,
    apiKey,
    projectId,
  });
}

export async function listAudioModels(providerType: AudioProviderType): Promise<AudioModel[]> {
  return invoke<AudioModel[]>("audio_models_list", { providerType });
}

export async function listVoiceDesignModels(
  providerType: AudioProviderType,
): Promise<AudioModel[]> {
  return invoke<AudioModel[]>("audio_voice_design_models_list", { providerType });
}

export async function getProviderVoices(providerId: string): Promise<CachedVoice[]> {
  return invoke<CachedVoice[]>("audio_provider_voices", { providerId });
}

export async function refreshProviderVoices(providerId: string): Promise<CachedVoice[]> {
  return invoke<CachedVoice[]>("audio_provider_refresh_voices", { providerId });
}

export async function listUserVoices(): Promise<UserVoice[]> {
  return invoke<UserVoice[]>("user_voice_list");
}

export async function upsertUserVoice(voice: UserVoice): Promise<UserVoice> {
  return invoke<UserVoice>("user_voice_upsert", {
    voiceJson: JSON.stringify(voice),
  });
}

export async function deleteUserVoice(id: string): Promise<void> {
  return invoke("user_voice_delete", { id });
}

export async function generateTtsPreview(
  providerId: string,
  modelId: string,
  voiceId: string,
  text: string,
  prompt?: string,
  requestId?: string,
): Promise<TtsPreviewResponse> {
  return invoke<TtsPreviewResponse>("tts_preview", {
    providerId,
    modelId,
    voiceId,
    prompt,
    text,
    requestId: requestId ?? null,
  });
}

export async function kokoroSupportedVariants(): Promise<KokoroSupportedVariant[]> {
  return invoke<KokoroSupportedVariant[]>("kokoro_supported_variants");
}

export async function kokoroDefaultAssetRoot(): Promise<string> {
  return invoke<string>("kokoro_default_asset_root");
}

export async function kokoroValidateAssets(
  assetRoot: string,
  variant: KokoroModelVariant,
  selectedVoiceId?: string,
): Promise<KokoroAssetStatus> {
  return invoke<KokoroAssetStatus>("kokoro_validate_assets", {
    assetRoot,
    variant,
    selectedVoiceId: selectedVoiceId ?? null,
  });
}

export async function kokoroListInstalledVoices(
  assetRoot: string,
): Promise<KokoroInstalledVoice[]> {
  return invoke<KokoroInstalledVoice[]>("kokoro_list_installed_voices", {
    assetRoot,
  });
}

export async function kokoroListAvailableVoices(
  assetRoot: string,
): Promise<KokoroAvailableVoice[]> {
  return invoke<KokoroAvailableVoice[]>("kokoro_list_available_voices", {
    assetRoot,
  });
}

export async function kokoroInstallModel(
  assetRoot: string,
  variant: KokoroModelVariant,
): Promise<KokoroQueuedInstall> {
  return invoke<KokoroQueuedInstall>("kokoro_install_model", {
    assetRoot,
    variant,
  });
}

export async function kokoroInstallVoice(
  assetRoot: string,
  voiceId: string,
): Promise<KokoroQueuedInstall> {
  return invoke<KokoroQueuedInstall>("kokoro_install_voice", {
    assetRoot,
    voiceId,
  });
}

export async function kokoroUninstallModel(
  assetRoot: string,
  variant: KokoroModelVariant,
): Promise<boolean> {
  return invoke<boolean>("kokoro_uninstall_model", { assetRoot, variant });
}

export async function kokoroUninstallVoice(
  assetRoot: string,
  voiceId: string,
): Promise<boolean> {
  return invoke<boolean>("kokoro_uninstall_voice", { assetRoot, voiceId });
}

export async function kokoroInstallVoices(
  assetRoot: string,
  voiceIds: string[],
): Promise<KokoroQueuedInstall> {
  return invoke<KokoroQueuedInstall>("kokoro_install_voices", { assetRoot, voiceIds });
}

export interface KokoroStorageStats {
  modelBytes: number;
  voicesBytes: number;
  totalBytes: number;
  voiceCount: number;
}

export async function kokoroStorageStats(assetRoot: string): Promise<KokoroStorageStats> {
  return invoke<KokoroStorageStats>("kokoro_storage_stats", { assetRoot });
}

export async function kokoroTokenizePreview(
  assetRoot: string,
  voiceBlend: KokoroVoiceBlendEntry[],
  text: string,
  espeakBinPath?: string,
  espeakDataPath?: string,
): Promise<KokoroTokenizePreview> {
  return invoke<KokoroTokenizePreview>("kokoro_tokenize_preview", {
    assetRoot,
    voiceBlend,
    text,
    espeakBinPath: espeakBinPath ?? null,
    espeakDataPath: espeakDataPath ?? null,
  });
}

export async function kokoroPreview(
  assetRoot: string,
  variant: KokoroModelVariant,
  voiceBlend: KokoroVoiceBlendEntry[],
  text: string,
  speed?: number,
  espeakBinPath?: string,
  espeakDataPath?: string,
): Promise<TtsPreviewResponse> {
  return invoke<TtsPreviewResponse>("kokoro_preview", {
    assetRoot,
    variant,
    voiceBlend,
    text,
    speed: speed ?? null,
    espeakBinPath: espeakBinPath ?? null,
    espeakDataPath: espeakDataPath ?? null,
  });
}

export function playAudioFromBase64(audioBase64: string, format: string): HTMLAudioElement {
  const audio = new Audio(`data:${format};base64,${audioBase64}`);
  void audio.play();
  return audio;
}

export async function abortAudioPreview(requestId: string): Promise<void> {
  return invoke("abort_request", { requestId });
}

export async function searchProviderVoices(
  providerId: string,
  search: string,
): Promise<CachedVoice[]> {
  return invoke<CachedVoice[]>("audio_provider_search_voices", {
    providerId,
    search,
  });
}

export interface VoiceDesignPreview {
  generatedVoiceId: string;
  audioBase64: string;
  durationSecs: number;
  mediaType: string;
}

export interface CreatedVoiceResult {
  voiceId: string;
}

export async function designVoicePreview(
  providerId: string,
  textSample: string,
  voiceDescription: string,
  modelId?: string,
  numPreviews?: number,
): Promise<VoiceDesignPreview[]> {
  return invoke<VoiceDesignPreview[]>("voice_design_preview", {
    providerId,
    textSample,
    voiceDescription,
    modelId,
    numPreviews,
  });
}

export async function createVoiceFromPreview(
  providerId: string,
  voiceName: string,
  generatedVoiceId: string,
  voiceDescription?: string,
): Promise<CreatedVoiceResult> {
  return invoke<CreatedVoiceResult>("voice_design_create", {
    providerId,
    voiceName,
    generatedVoiceId,
    voiceDescription,
  });
}

// --- TTS Audio Cache Functions ---

export interface TtsCacheStats {
  sizeBytes: number;
  count: number;
}

/**
 * Generate a cache key for TTS audio based on generation parameters.
 * This key is used to store and retrieve cached audio from disk.
 */
export async function getTtsCacheKey(
  providerId: string,
  modelId: string,
  voiceId: string,
  text: string,
  prompt?: string,
): Promise<string> {
  return invoke<string>("tts_cache_key", {
    providerId,
    modelId,
    voiceId,
    text,
    prompt: prompt ?? null,
  });
}

/**
 * Check if TTS audio exists in the disk cache.
 */
export async function ttsCacheExists(cacheKey: string): Promise<boolean> {
  return invoke<boolean>("tts_cache_exists", { cacheKey });
}

/**
 * Get cached TTS audio from disk.
 * Returns null if not found.
 */
export async function getTtsCached(cacheKey: string): Promise<TtsPreviewResponse | null> {
  return invoke<TtsPreviewResponse | null>("tts_cache_get", { cacheKey });
}

/**
 * Save TTS audio to disk cache.
 */
export async function saveTtsToCache(
  cacheKey: string,
  audioBase64: string,
  format: string,
): Promise<void> {
  return invoke("tts_cache_save", { cacheKey, audioBase64, format });
}

/**
 * Delete a specific TTS audio from disk cache.
 */
export async function deleteTtsFromCache(cacheKey: string): Promise<void> {
  return invoke("tts_cache_delete", { cacheKey });
}

/**
 * Clear all TTS audio from disk cache.
 * Returns the number of files deleted.
 */
export async function clearTtsCache(): Promise<number> {
  return invoke<number>("tts_cache_clear");
}

/**
 * Get statistics about the TTS audio cache.
 */
export async function getTtsCacheStats(): Promise<TtsCacheStats> {
  return invoke<TtsCacheStats>("tts_cache_stats");
}

/**
 * Generate TTS audio for a message, using disk cache if available.
 * This is the preferred method for message playback (not previews).
 */
export async function generateTtsForMessage(
  providerId: string,
  modelId: string,
  voiceId: string,
  text: string,
  prompt?: string,
  requestId?: string,
): Promise<TtsPreviewResponse> {
  // Generate cache key
  const cacheKey = await getTtsCacheKey(providerId, modelId, voiceId, text, prompt);

  // Check disk cache first
  const cached = await getTtsCached(cacheKey);
  if (cached) {
    return cached;
  }

  // Generate new audio
  const response = await generateTtsPreview(providerId, modelId, voiceId, text, prompt, requestId);

  // Save to disk cache (fire and forget, don't block playback)
  saveTtsToCache(cacheKey, response.audioBase64, response.format).catch((err) => {
    console.warn("Failed to save TTS audio to cache:", err);
  });

  return response;
}
