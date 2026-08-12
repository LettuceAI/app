import { invoke } from "@tauri-apps/api/core";

export interface AsrVocabularyTerm {
  id?: number | null;
  term: string;
  normalizedTerm?: string | null;
  language?: string | null;
  category?: string | null;
  scope?: string | null;
  priority?: number | null;
  useCount?: number | null;
  createdAt?: string | null;
  updatedAt?: string | null;
}

export interface AsrCorrection {
  id?: number | null;
  wrong: string;
  normalizedWrong?: string | null;
  correct: string;
  normalizedCorrect?: string | null;
  language?: string | null;
  scope?: string | null;
  confidence?: number | null;
  useCount?: number | null;
  acceptedCount?: number | null;
  rejectedCount?: number | null;
  seenCount?: number | null;
  lastSeenAt?: string | null;
  userApproved?: boolean | null;
  createdAt?: string | null;
  updatedAt?: string | null;
}

export interface AsrVoiceExample {
  id?: number | null;
  audioPath: string;
  expectedText: string;
  normalizedExpectedText?: string | null;
  whisperOutput?: string | null;
  normalizedWhisperOutput?: string | null;
  language?: string | null;
  scope?: string | null;
  termId?: number | null;
  correctionId?: number | null;
  createdAt?: string | null;
}

export interface AsrLearnedSuggestion {
  wrong: string;
  normalizedWrong: string;
  correct: string;
  normalizedCorrect: string;
  language?: string | null;
  scope: string;
  confidence: number;
  acceptedCount: number;
  rejectedCount: number;
  seenCount: number;
}

export interface AsrIgnoredSuggestion {
  id?: number | null;
  wrong: string;
  normalizedWrong?: string | null;
  correct: string;
  normalizedCorrect?: string | null;
  language?: string | null;
  scope?: string | null;
  ignoredCount?: number | null;
  lastIgnoredAt?: string | null;
  createdAt?: string | null;
  updatedAt?: string | null;
}

export interface AsrCorrectionApplication {
  correctionId: number;
  wrong: string;
  correct: string;
  matchText: string;
}

export interface AsrExportBundle {
  version: number;
  vocabulary: AsrVocabularyTerm[];
  corrections: AsrCorrection[];
  voiceExamples: AsrVoiceExample[];
  ignoredSuggestions?: AsrIgnoredSuggestion[];
}

export interface AsrWhisperSegment {
  index: number;
  startMs: number;
  endMs: number;
  text: string;
  noSpeechProbability: number;
  speakerTurnNext: boolean;
}

export interface AsrWhisperTranscribePcmRequest {
  modelPath: string;
  pcmBytes: Uint8Array;
  sampleRateHz: number;
  channels?: number | null;
  language?: string | null;
  scopes?: string[] | null;
  initialPrompt?: string | null;
  translate?: boolean | null;
  detectLanguage?: boolean | null;
  noContext?: boolean | null;
  singleSegment?: boolean | null;
  tokenTimestamps?: boolean | null;
  splitOnWord?: boolean | null;
  maxLen?: number | null;
  maxTokens?: number | null;
  offsetMs?: number | null;
  durationMs?: number | null;
  threads?: number | null;
  bestOf?: number | null;
  temperature?: number | null;
  temperatureInc?: number | null;
  useGpu?: boolean | null;
  forceCpu?: boolean | null;
  keepModelLoaded?: boolean | null;
  flashAttn?: boolean | null;
  gpuDevice?: number | null;
}

export interface AsrWhisperTranscribeRequest {
  modelPath: string;
  audioPath: string;
  language?: string | null;
  scopes?: string[] | null;
  initialPrompt?: string | null;
  translate?: boolean | null;
  detectLanguage?: boolean | null;
  noContext?: boolean | null;
  singleSegment?: boolean | null;
  tokenTimestamps?: boolean | null;
  splitOnWord?: boolean | null;
  maxLen?: number | null;
  maxTokens?: number | null;
  offsetMs?: number | null;
  durationMs?: number | null;
  threads?: number | null;
  bestOf?: number | null;
  temperature?: number | null;
  temperatureInc?: number | null;
  useGpu?: boolean | null;
  forceCpu?: boolean | null;
  keepModelLoaded?: boolean | null;
  flashAttn?: boolean | null;
  gpuDevice?: number | null;
}

export interface AsrWhisperRuntimeLoadRequest {
  modelPath: string;
  useGpu?: boolean | null;
  forceCpu?: boolean | null;
  flashAttn?: boolean | null;
  gpuDevice?: number | null;
}

export interface AsrWhisperTranscriptionResponse {
  audioPath: string;
  modelPath: string;
  sampleRateHz: number;
  prompt: string;
  rawText: string;
  correctedText: string;
  detectedLanguage?: string | null;
  segments: AsrWhisperSegment[];
  appliedCorrections: AsrCorrectionApplication[];
}

export interface AsrWhisperModelPreset {
  id: string;
  filename: string;
  label: string;
  repo: string;
  downloadUrl: string;
  sizeBytes: number;
  englishOnly: boolean;
  quantized: boolean;
  recommended: boolean;
  recommendedForMobile: boolean;
  recommendedForDesktop: boolean;
}

export interface AsrInstalledWhisperModel {
  id: string;
  filename: string;
  label: string;
  path: string;
  sizeBytes: number;
  englishOnly: boolean;
  quantized: boolean;
}

export interface AsrListFilter {
  language?: string | null;
  scopes?: string[] | null;
}

export function asrVocabularyList(filter: AsrListFilter = {}): Promise<AsrVocabularyTerm[]> {
  return invoke<AsrVocabularyTerm[]>("asr_vocabulary_list", {
    language: filter.language ?? null,
    scopes: filter.scopes ?? null,
  });
}

export function asrVocabularyUpsert(term: AsrVocabularyTerm): Promise<AsrVocabularyTerm> {
  return invoke<AsrVocabularyTerm>("asr_vocabulary_upsert", { term });
}

export function asrVocabularyDelete(id: number): Promise<void> {
  return invoke<void>("asr_vocabulary_delete", { id });
}

export function asrCorrectionsList(
  filter: AsrListFilter & { userApprovedOnly?: boolean | null } = {},
): Promise<AsrCorrection[]> {
  return invoke<AsrCorrection[]>("asr_corrections_list", {
    language: filter.language ?? null,
    scopes: filter.scopes ?? null,
    userApprovedOnly: filter.userApprovedOnly ?? null,
  });
}

export function asrCorrectionUpsert(correction: AsrCorrection): Promise<AsrCorrection> {
  return invoke<AsrCorrection>("asr_correction_upsert", { correction });
}

export function asrCorrectionDelete(id: number): Promise<void> {
  return invoke<void>("asr_correction_delete", { id });
}

export function asrVoiceExamplesList(filter: AsrListFilter = {}): Promise<AsrVoiceExample[]> {
  return invoke<AsrVoiceExample[]>("asr_voice_examples_list", {
    language: filter.language ?? null,
    scopes: filter.scopes ?? null,
  });
}

export function asrVoiceExampleUpsert(example: AsrVoiceExample): Promise<AsrVoiceExample> {
  return invoke<AsrVoiceExample>("asr_voice_example_upsert", { example });
}

export function asrVoiceExampleDelete(id: number): Promise<void> {
  return invoke<void>("asr_voice_example_delete", { id });
}

export function asrBuildPrompt(filter: AsrListFilter = {}): Promise<string> {
  return invoke<string>("asr_build_prompt", {
    language: filter.language ?? null,
    scopes: filter.scopes ?? null,
  });
}

export function asrExportLibrary(filter: AsrListFilter = {}): Promise<AsrExportBundle> {
  return invoke<AsrExportBundle>("asr_export_library", {
    language: filter.language ?? null,
    scopes: filter.scopes ?? null,
  });
}

export function asrImportLibrary(bundle: AsrExportBundle): Promise<Record<string, number>> {
  return invoke<Record<string, number>>("asr_import_library", { bundle });
}

export function asrVoiceExampleSuggestCorrection(args: {
  whisperOutput: string;
  expectedText: string;
  language?: string | null;
  scope?: string | null;
}): Promise<AsrLearnedSuggestion | null> {
  return invoke<AsrLearnedSuggestion | null>("asr_voice_example_suggest_correction", {
    whisperOutput: args.whisperOutput,
    expectedText: args.expectedText,
    language: args.language ?? null,
    scope: args.scope ?? null,
  });
}

export function asrSuggestCorrectionsFromEdit(args: {
  before: string;
  after: string;
  language?: string | null;
  scope?: string | null;
}): Promise<AsrLearnedSuggestion[]> {
  return invoke<AsrLearnedSuggestion[]>("asr_suggest_corrections_from_edit", {
    before: args.before,
    after: args.after,
    language: args.language ?? null,
    scope: args.scope ?? null,
  });
}

export function asrIgnoreSuggestion(suggestion: AsrLearnedSuggestion): Promise<void> {
  return invoke<void>("asr_ignore_suggestion", { suggestion });
}

export function asrWhisperTranscribeFile(
  request: AsrWhisperTranscribeRequest,
): Promise<AsrWhisperTranscriptionResponse> {
  return invoke<AsrWhisperTranscriptionResponse>("asr_whisper_transcribe_file", { request });
}

export function asrWhisperTranscribePcm(
  request: AsrWhisperTranscribePcmRequest,
): Promise<AsrWhisperTranscriptionResponse> {
  return invoke<AsrWhisperTranscriptionResponse>("asr_whisper_transcribe_pcm", { request });
}

export function asrWhisperRuntimeClearCache(): Promise<number> {
  return invoke<number>("asr_whisper_runtime_clear_cache");
}

export function asrWhisperRuntimePreloadModel(
  request: AsrWhisperRuntimeLoadRequest,
): Promise<void> {
  return invoke<void>("asr_whisper_runtime_preload_model", { request });
}

export function asrWhisperListAvailableModels(): Promise<AsrWhisperModelPreset[]> {
  return invoke<AsrWhisperModelPreset[]>("asr_whisper_list_available_models");
}

export function asrWhisperGetModelsDir(): Promise<string> {
  return invoke<string>("asr_whisper_get_models_dir");
}

export function asrWhisperQueueModelDownload(modelId: string): Promise<string> {
  return invoke<string>("asr_whisper_queue_model_download", { modelId });
}

export function asrWhisperListInstalledModels(): Promise<AsrInstalledWhisperModel[]> {
  return invoke<AsrInstalledWhisperModel[]>("asr_whisper_list_installed_models");
}

export function asrWhisperDeleteInstalledModel(filePath: string): Promise<void> {
  return invoke<void>("asr_whisper_delete_installed_model", { filePath });
}
