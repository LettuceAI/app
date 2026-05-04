import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { LayoutGroup, motion, AnimatePresence } from "framer-motion";
import {
  AudioLines,
  BookOpen,
  CheckCircle2,
  ChevronRight,
  Copy,
  Download,
  Folder,
  HardDrive,
  Languages,
  ListFilter,
  Loader2,
  Mic,
  Pause,
  Pencil,
  Play,
  Plus,
  RefreshCw,
  Settings2,
  Sparkles,
  Square,
  Trash2,
  Upload,
  Wand2,
  Waves,
  X,
} from "lucide-react";
import { save as saveDialog, open as openDialog } from "@tauri-apps/plugin-dialog";
import { mkdir, readTextFile, writeFile, writeTextFile } from "@tauri-apps/plugin-fs";
import { join as joinPath, tempDir } from "@tauri-apps/api/path";
import { cn, radius, interactive, typography, animations } from "../../design-tokens";
import { Switch } from "../../components/Switch";
import { BottomMenu } from "../../components/BottomMenu";
import { GuidedTour, useGuidedTour } from "../../components/GuidedTour";
import { getPlatform } from "../../../core/utils/platform";
import { toast } from "../../components/toast";
import { useDownloadQueueOptional } from "../../../core/downloads/DownloadQueueContext";
import {
  asrBuildPrompt,
  asrCorrectionDelete,
  asrCorrectionUpsert,
  asrCorrectionsList,
  asrExportLibrary,
  asrImportLibrary,
  asrVocabularyDelete,
  asrVocabularyList,
  asrVocabularyUpsert,
  asrVoiceExampleDelete,
  asrVoiceExampleSuggestCorrection,
  asrVoiceExampleUpsert,
  asrVoiceExamplesList,
  asrWhisperRuntimeClearCache,
  asrWhisperDeleteInstalledModel,
  asrWhisperGetModelsDir,
  asrWhisperListAvailableModels,
  asrWhisperListInstalledModels,
  asrWhisperQueueModelDownload,
  asrWhisperRuntimePreloadModel,
  asrWhisperTranscribeFile,
  asrWhisperTranscribePcm,
  getStoredMicDeviceId,
  micConstraintsWithStoredDevice,
  setStoredMicDeviceId,
  type AsrCorrection,
  type AsrExportBundle,
  type AsrInstalledWhisperModel,
  type AsrLearnedSuggestion,
  type AsrVocabularyTerm,
  type AsrWhisperModelPreset,
  type AsrWhisperTranscriptionResponse,
  type AsrVoiceExample,
} from "../../../core/asr";

type LibraryTab = "vocabulary" | "corrections" | "voiceExamples";

const LIBRARY_TABS: {
  key: LibraryTab;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  hint: string;
}[] = [
    {
      key: "vocabulary",
      label: "Vocabulary",
      icon: BookOpen,
      hint: "Names, jargon, and product terms biased into Whisper's prompt.",
    },
    {
      key: "corrections",
      label: "Corrections",
      icon: Wand2,
      hint: "Replace consistent mistakes after Whisper finishes.",
    },
    {
      key: "voiceExamples",
      label: "Voice examples",
      icon: AudioLines,
      hint: "Pair recordings with the text they should produce.",
    },
  ];

const SECTION_TITLE = cn(typography.h3.size, typography.h3.weight, "text-fg");
const SECTION_SUB = cn(typography.bodySmall.size, "text-fg/55 leading-relaxed");
const OVERLINE = cn(
  typography.overline.size,
  typography.overline.weight,
  typography.overline.tracking,
  typography.overline.transform,
  "text-fg/40",
);

function inputClass(extra?: string) {
  return cn(
    "w-full rounded-lg border border-fg/15 bg-fg/5 px-3 py-2 text-sm text-fg",
    "placeholder:text-fg/30",
    "focus:border-accent/45 focus:bg-fg/8 focus:outline-none focus:ring-2 focus:ring-accent/25",
    interactive.transition.fast,
    extra,
  );
}

function ghostButton(extra?: string) {
  return cn(
    "inline-flex h-9 items-center gap-1.5 rounded-full border border-fg/15 bg-fg/5 px-3 text-xs font-medium text-fg/75",
    "hover:border-fg/25 hover:bg-fg/10 hover:text-fg",
    interactive.transition.fast,
    interactive.active.scale,
    extra,
  );
}

function accentButton(extra?: string) {
  return cn(
    "inline-flex h-9 items-center gap-1.5 rounded-full border border-accent/40 bg-accent/15 px-3 text-xs font-semibold text-accent",
    "hover:border-accent/60 hover:bg-accent/20",
    interactive.transition.fast,
    interactive.active.scale,
    extra,
  );
}

function panelClass(extra?: string) {
  return cn(
    "rounded-2xl border border-fg/10 bg-fg/[0.04] p-4",
    interactive.transition.default,
    extra,
  );
}

function subCardClass(extra?: string) {
  return cn(
    "rounded-xl border border-fg/10 bg-fg/5 px-3.5 py-3",
    interactive.transition.fast,
    extra,
  );
}

function formatBytes(bytes: number) {
  if (!bytes || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / Math.pow(1024, index);
  return `${value.toFixed(index > 1 ? 1 : 0)} ${units[index]}`;
}

function formatDuration(ms: number) {
  if (!ms || ms <= 0) return "0.0s";
  return `${(ms / 1000).toFixed(ms >= 10_000 ? 0 : 1)}s`;
}

function micErrorMessage(error: unknown) {
  if (error instanceof DOMException) {
    if (error.name === "NotAllowedError") {
      return "Microphone permission denied. Allow microphone access for Lettuce and try again.";
    }
    if (error.name === "NotFoundError") {
      return "No microphone was found on this device.";
    }
  }
  return String(error);
}

function mergeFloat32Chunks(chunks: Float32Array[]) {
  const total = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
  const merged = new Float32Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    merged.set(chunk, offset);
    offset += chunk.length;
  }
  return merged;
}

function encodeWavMono16(pcm: Float32Array, sampleRate: number): Uint8Array {
  const bytesPerSample = 2;
  const numChannels = 1;
  const blockAlign = numChannels * bytesPerSample;
  const byteRate = sampleRate * blockAlign;
  const dataLength = pcm.length * bytesPerSample;
  const buffer = new ArrayBuffer(44 + dataLength);
  const view = new DataView(buffer);

  const writeString = (offset: number, value: string) => {
    for (let i = 0; i < value.length; i++) view.setUint8(offset + i, value.charCodeAt(i));
  };

  writeString(0, "RIFF");
  view.setUint32(4, 36 + dataLength, true);
  writeString(8, "WAVE");
  writeString(12, "fmt ");
  view.setUint32(16, 16, true); // PCM chunk size
  view.setUint16(20, 1, true); // PCM format
  view.setUint16(22, numChannels, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, byteRate, true);
  view.setUint16(32, blockAlign, true);
  view.setUint16(34, 16, true); // bits per sample
  writeString(36, "data");
  view.setUint32(40, dataLength, true);

  let offset = 44;
  for (let i = 0; i < pcm.length; i++) {
    const sample = Math.max(-1, Math.min(1, pcm[i]));
    view.setInt16(offset, sample < 0 ? sample * 0x8000 : sample * 0x7fff, true);
    offset += 2;
  }
  return new Uint8Array(buffer);
}

type RecorderSession = {
  stream: MediaStream;
  audioContext: AudioContext;
  source: MediaStreamAudioSourceNode;
  processor: ScriptProcessorNode;
  chunks: Float32Array[];
  sampleRate: number;
  startedAt: number;
};

type PlaybackSession = {
  audioContext: AudioContext;
  source: AudioBufferSourceNode;
};

interface FilterState {
  language: string;
  scope: string;
}

const DEFAULT_FILTER: FilterState = { language: "", scope: "" };
const ALL_ASR_SCOPES = ["global", "project", "character", "conversation"];

const WHISPER_LANGUAGES: { code: string; name: string }[] = [
  { code: "af", name: "Afrikaans" },
  { code: "am", name: "Amharic" },
  { code: "ar", name: "Arabic" },
  { code: "as", name: "Assamese" },
  { code: "az", name: "Azerbaijani" },
  { code: "ba", name: "Bashkir" },
  { code: "be", name: "Belarusian" },
  { code: "bg", name: "Bulgarian" },
  { code: "bn", name: "Bengali" },
  { code: "bo", name: "Tibetan" },
  { code: "br", name: "Breton" },
  { code: "bs", name: "Bosnian" },
  { code: "ca", name: "Catalan" },
  { code: "cs", name: "Czech" },
  { code: "cy", name: "Welsh" },
  { code: "da", name: "Danish" },
  { code: "de", name: "German" },
  { code: "el", name: "Greek" },
  { code: "en", name: "English" },
  { code: "es", name: "Spanish" },
  { code: "et", name: "Estonian" },
  { code: "eu", name: "Basque" },
  { code: "fa", name: "Persian" },
  { code: "fi", name: "Finnish" },
  { code: "fo", name: "Faroese" },
  { code: "fr", name: "French" },
  { code: "gl", name: "Galician" },
  { code: "gu", name: "Gujarati" },
  { code: "ha", name: "Hausa" },
  { code: "haw", name: "Hawaiian" },
  { code: "he", name: "Hebrew" },
  { code: "hi", name: "Hindi" },
  { code: "hr", name: "Croatian" },
  { code: "ht", name: "Haitian Creole" },
  { code: "hu", name: "Hungarian" },
  { code: "hy", name: "Armenian" },
  { code: "id", name: "Indonesian" },
  { code: "is", name: "Icelandic" },
  { code: "it", name: "Italian" },
  { code: "ja", name: "Japanese" },
  { code: "jw", name: "Javanese" },
  { code: "ka", name: "Georgian" },
  { code: "kk", name: "Kazakh" },
  { code: "km", name: "Khmer" },
  { code: "kn", name: "Kannada" },
  { code: "ko", name: "Korean" },
  { code: "la", name: "Latin" },
  { code: "lb", name: "Luxembourgish" },
  { code: "ln", name: "Lingala" },
  { code: "lo", name: "Lao" },
  { code: "lt", name: "Lithuanian" },
  { code: "lv", name: "Latvian" },
  { code: "mg", name: "Malagasy" },
  { code: "mi", name: "Maori" },
  { code: "mk", name: "Macedonian" },
  { code: "ml", name: "Malayalam" },
  { code: "mn", name: "Mongolian" },
  { code: "mr", name: "Marathi" },
  { code: "ms", name: "Malay" },
  { code: "mt", name: "Maltese" },
  { code: "my", name: "Burmese" },
  { code: "ne", name: "Nepali" },
  { code: "nl", name: "Dutch" },
  { code: "nn", name: "Norwegian Nynorsk" },
  { code: "no", name: "Norwegian" },
  { code: "oc", name: "Occitan" },
  { code: "pa", name: "Punjabi" },
  { code: "pl", name: "Polish" },
  { code: "ps", name: "Pashto" },
  { code: "pt", name: "Portuguese" },
  { code: "ro", name: "Romanian" },
  { code: "ru", name: "Russian" },
  { code: "sa", name: "Sanskrit" },
  { code: "sd", name: "Sindhi" },
  { code: "si", name: "Sinhala" },
  { code: "sk", name: "Slovak" },
  { code: "sl", name: "Slovenian" },
  { code: "sn", name: "Shona" },
  { code: "so", name: "Somali" },
  { code: "sq", name: "Albanian" },
  { code: "sr", name: "Serbian" },
  { code: "su", name: "Sundanese" },
  { code: "sv", name: "Swedish" },
  { code: "sw", name: "Swahili" },
  { code: "ta", name: "Tamil" },
  { code: "te", name: "Telugu" },
  { code: "tg", name: "Tajik" },
  { code: "th", name: "Thai" },
  { code: "tk", name: "Turkmen" },
  { code: "tl", name: "Tagalog" },
  { code: "tr", name: "Turkish" },
  { code: "tt", name: "Tatar" },
  { code: "uk", name: "Ukrainian" },
  { code: "ur", name: "Urdu" },
  { code: "uz", name: "Uzbek" },
  { code: "vi", name: "Vietnamese" },
  { code: "yi", name: "Yiddish" },
  { code: "yo", name: "Yoruba" },
  { code: "yue", name: "Cantonese" },
  { code: "zh", name: "Chinese" },
];

const FRIENDLY_MODEL_COPY: Record<string, { title: string; summary: string }> = {
  "tiny.en": {
    title: "Fastest English",
    summary: "Lowest memory use. Best for quick voice notes on weaker devices.",
  },
  tiny: {
    title: "Fastest Multilingual",
    summary: "Very light model with broad language support and the lowest quality ceiling.",
  },
  "base.en-q5_1": {
    title: "Fast English",
    summary: "Quick and compact. Good default for mobile if you only need English.",
  },
  "base-q5_1": {
    title: "Fast Multilingual",
    summary: "Compact multilingual option for phones and lower-power laptops.",
  },
  "base.en": {
    title: "Balanced English",
    summary: "Better accuracy than the fast presets without a large jump in size.",
  },
  base: {
    title: "Balanced Multilingual",
    summary: "Solid middle ground for multilingual use on modest hardware.",
  },
  "small.en-q5_1": {
    title: "Better English",
    summary: "A step up in quality for English while staying fairly lightweight.",
  },
  "small-q5_1": {
    title: "Better Multilingual",
    summary: "Higher quality multilingual transcription for capable mobile devices.",
  },
  "small.en": {
    title: "Desktop Balanced English",
    summary: "Good desktop default when you want speed and quality to stay balanced.",
  },
  small: {
    title: "Desktop Balanced Multilingual",
    summary: "General-purpose multilingual desktop option.",
  },
  "medium.en-q5_0": {
    title: "High Accuracy English",
    summary: "Slower, but noticeably stronger for English dictation and cleanup.",
  },
  "medium-q5_0": {
    title: "High Accuracy Multilingual",
    summary: "Strong multilingual quality for desktops without going to the largest models.",
  },
  "medium.en": {
    title: "Best English",
    summary: "Highest English quality in this curated set, with heavier memory use.",
  },
  medium: {
    title: "Best Multilingual",
    summary: "Highest general multilingual quality before the turbo-large tier.",
  },
  "large-v3-turbo-q5_0": {
    title: "Studio Multilingual",
    summary: "Fast high-end desktop option with strong overall quality.",
  },
  "large-v3-turbo": {
    title: "Studio Multilingual Max",
    summary: "Largest curated option for the best results on powerful desktops.",
  },
};

function getFriendlyModelCopy(modelId: string, fallbackLabel: string) {
  return FRIENDLY_MODEL_COPY[modelId] ?? {
    title: fallbackLabel,
    summary: "Official whisper.cpp model build.",
  };
}

function filterPayload(filter: FilterState) {
  return {
    language: filter.language.trim() ? filter.language.trim() : null,
    scopes: filter.scope.trim() ? [filter.scope.trim()] : ALL_ASR_SCOPES,
  };
}

export function SpeechRecognitionPage() {
  const downloadQueue = useDownloadQueueOptional();
  const { shouldShow: showTour, dismiss: dismissTour } = useGuidedTour("speechRecognition");
  const [tab, setTab] = useState<LibraryTab>("vocabulary");
  const [filter, setFilter] = useState<FilterState>(DEFAULT_FILTER);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [vocabulary, setVocabulary] = useState<AsrVocabularyTerm[]>([]);
  const [corrections, setCorrections] = useState<AsrCorrection[]>([]);
  const [examples, setExamples] = useState<AsrVoiceExample[]>([]);
  const [modelsDir, setModelsDir] = useState("");
  const [availableModels, setAvailableModels] = useState<AsrWhisperModelPreset[]>([]);
  const [installedModels, setInstalledModels] = useState<AsrInstalledWhisperModel[]>([]);
  const [previewPrompt, setPreviewPrompt] = useState<string>("");
  const [showApprovedOnly, setShowApprovedOnly] = useState(false);
  const [loading, setLoading] = useState(true);
  const [loadingWhisperModels, setLoadingWhisperModels] = useState(true);
  const [loadingInstalledModels, setLoadingInstalledModels] = useState(true);
  const [queueingModelId, setQueueingModelId] = useState<string | null>(null);
  const [deletingModelPath, setDeletingModelPath] = useState<string | null>(null);
  const [selectedTestModelPath, setSelectedTestModelPath] = useState("");
  const [testPrompt, setTestPrompt] = useState("");
  const [testLanguage, setTestLanguage] = useState("");
  const [testUseGpu, setTestUseGpu] = useState(true);
  const [testKeepModelLoaded, setTestKeepModelLoaded] = useState(true);
  const [modelPickerOpen, setModelPickerOpen] = useState(false);
  const [pendingModelId, setPendingModelId] = useState<string | null>(null);
  const [recordingElapsedMs, setRecordingElapsedMs] = useState(0);
  const [isRecording, setIsRecording] = useState(false);
  const [isTranscribingTest, setIsTranscribingTest] = useState(false);
  const [isPreloadingModel, setIsPreloadingModel] = useState(false);
  const [hasLastRecording, setHasLastRecording] = useState(false);
  const [isPlayingLastRecording, setIsPlayingLastRecording] = useState(false);
  const [testResult, setTestResult] = useState<AsrWhisperTranscriptionResponse | null>(null);
  const [editor, setEditor] = useState<EditorState>({ kind: "none" });
  const recorderRef = useRef<RecorderSession | null>(null);
  const lastRecordingRef = useRef<{ pcm: Float32Array; sampleRate: number } | null>(null);
  const playbackRef = useRef<PlaybackSession | null>(null);

  const reload = useCallback(async () => {
    setLoading(true);
    const payload = filterPayload(filter);
    try {
      const [voc, corr, ex, prompt] = await Promise.all([
        asrVocabularyList(payload),
        asrCorrectionsList({ ...payload, userApprovedOnly: showApprovedOnly ? true : null }),
        asrVoiceExamplesList(payload),
        asrBuildPrompt(payload),
      ]);
      setVocabulary(voc);
      setCorrections(corr);
      setExamples(ex);
      setPreviewPrompt(prompt);
    } catch (error) {
      console.error("Failed to load ASR library:", error);
      toast.error("Failed to load speech recognition data", String(error));
    } finally {
      setLoading(false);
    }
  }, [filter, showApprovedOnly]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const reloadInstalledModels = useCallback(async () => {
    setLoadingInstalledModels(true);
    try {
      const [dir, installed] = await Promise.all([
        asrWhisperGetModelsDir(),
        asrWhisperListInstalledModels(),
      ]);
      setModelsDir(dir);
      setInstalledModels(installed);
    } catch (error) {
      console.error("Failed to load installed Whisper models:", error);
      toast.error("Failed to load installed Whisper models", String(error));
    } finally {
      setLoadingInstalledModels(false);
    }
  }, []);

  const reloadWhisperCatalog = useCallback(async () => {
    setLoadingWhisperModels(true);
    try {
      setAvailableModels(await asrWhisperListAvailableModels());
    } catch (error) {
      console.error("Failed to load Whisper models:", error);
      toast.error("Failed to load official Whisper model list", String(error));
    } finally {
      setLoadingWhisperModels(false);
    }
  }, []);

  useEffect(() => {
    void reloadInstalledModels();
    void reloadWhisperCatalog();
  }, [reloadInstalledModels, reloadWhisperCatalog]);

  useEffect(() => {
    const handler = () => {
      void reload();
      void reloadInstalledModels();
      void reloadWhisperCatalog();
    };
    window.addEventListener("asr:refresh", handler);
    return () => window.removeEventListener("asr:refresh", handler);
  }, [reload, reloadInstalledModels, reloadWhisperCatalog]);

  const promptedNoModelsRef = useRef(false);
  useEffect(() => {
    if (loadingInstalledModels) return;
    if (promptedNoModelsRef.current) return;
    if (installedModels.length === 0) {
      promptedNoModelsRef.current = true;
      setModelPickerOpen(true);
    } else {
      promptedNoModelsRef.current = true;
    }
  }, [loadingInstalledModels, installedModels.length]);

  useEffect(() => {
    if (!pendingModelId) return;
    const installed = installedModels.find((m) => m.id === pendingModelId);
    if (installed) {
      setSelectedTestModelPath(installed.path);
      setPendingModelId(null);
      toast.success("Model ready", installed.label);
    }
  }, [pendingModelId, installedModels]);

  useEffect(() => {
    if (!selectedTestModelPath && installedModels.length > 0) {
      setSelectedTestModelPath(installedModels[0]?.path ?? "");
    } else if (
      selectedTestModelPath &&
      !installedModels.some((model) => model.path === selectedTestModelPath)
    ) {
      setSelectedTestModelPath(installedModels[0]?.path ?? "");
    }
  }, [installedModels, selectedTestModelPath]);

  useEffect(() => {
    if (!testLanguage && filter.language.trim()) {
      setTestLanguage(filter.language.trim());
    }
  }, [filter.language, testLanguage]);

  useEffect(() => {
    if (!isRecording) return;
    const timer = window.setInterval(() => {
      const startedAt = recorderRef.current?.startedAt;
      if (startedAt) {
        setRecordingElapsedMs(Date.now() - startedAt);
      }
    }, 200);
    return () => window.clearInterval(timer);
  }, [isRecording]);

  useEffect(() => {
    return () => {
      const session = recorderRef.current;
      if (!session) return;
      session.processor.disconnect();
      session.source.disconnect();
      session.stream.getTracks().forEach((track) => track.stop());
      void session.audioContext.close();
      recorderRef.current = null;
    };
  }, []);

  useEffect(() => {
    return () => {
      const playback = playbackRef.current;
      if (!playback) return;
      playback.source.stop();
      void playback.audioContext.close();
      playbackRef.current = null;
    };
  }, []);

  const handleExport = useCallback(async () => {
    try {
      const bundle = await asrExportLibrary(filterPayload(filter));
      const target = await saveDialog({
        title: "Export speech library",
        defaultPath: `asr-library-${new Date().toISOString().slice(0, 10)}.json`,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!target) return;
      await writeTextFile(target, JSON.stringify(bundle, null, 2));
      toast.success(
        "Exported",
        `${bundle.vocabulary.length} terms, ${bundle.corrections.length} corrections, ${bundle.voiceExamples.length} examples`,
      );
    } catch (error) {
      console.error("Failed to export ASR library:", error);
      toast.error("Export failed", String(error));
    }
  }, [filter]);

  const handleImport = useCallback(async () => {
    try {
      const path = await openDialog({
        title: "Import speech library",
        multiple: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!path || typeof path !== "string") return;
      const text = await readTextFile(path);
      const bundle = JSON.parse(text) as AsrExportBundle;
      const counts = await asrImportLibrary(bundle);
      toast.success(
        "Imported",
        `${counts.vocabulary ?? 0} terms, ${counts.corrections ?? 0} corrections, ${counts.voiceExamples ?? 0} examples`,
      );
      void reload();
    } catch (error) {
      console.error("Failed to import ASR library:", error);
      toast.error("Import failed", String(error));
    }
  }, [reload]);

  const handleClearCache = useCallback(async () => {
    try {
      const cleared = await asrWhisperRuntimeClearCache();
      toast.success("Whisper cache cleared", `${cleared} context${cleared === 1 ? "" : "s"}`);
    } catch (error) {
      console.error("Failed to clear whisper cache:", error);
      toast.error("Failed to clear cache", String(error));
    }
  }, []);

  const handleCopy = useCallback(async (value: string, label: string) => {
    try {
      await navigator.clipboard.writeText(value);
      toast.success("Copied", label);
    } catch (error) {
      toast.error("Copy failed", String(error));
    }
  }, []);

  const handleQueueModel = useCallback(async (modelId: string) => {
    setQueueingModelId(modelId);
    try {
      await asrWhisperQueueModelDownload(modelId);
      toast.success("Download queued", modelId);
    } catch (error) {
      console.error("Failed to queue Whisper model:", error);
      toast.error("Failed to queue download", String(error));
    } finally {
      setQueueingModelId(null);
    }
  }, []);

  const handleDeleteInstalledModel = useCallback(
    async (model: AsrInstalledWhisperModel) => {
      setDeletingModelPath(model.path);
      try {
        await asrWhisperDeleteInstalledModel(model.path);
        toast.success("Model deleted", model.filename);
        await reloadInstalledModels();
      } catch (error) {
        console.error("Failed to delete Whisper model:", error);
        toast.error("Failed to delete model", String(error));
      } finally {
        setDeletingModelPath(null);
      }
    },
    [reloadInstalledModels],
  );

  const runTestTranscription = useCallback(
    async (pcm: Float32Array, sampleRateHz: number) => {
      if (!selectedTestModelPath) {
        toast.warning("Choose a Whisper model first");
        return;
      }

      setIsTranscribingTest(true);
      setTestResult(null);
      try {
        const pcmBytes = new Uint8Array(pcm.buffer, pcm.byteOffset, pcm.byteLength);
        const result = await asrWhisperTranscribePcm({
          modelPath: selectedTestModelPath,
          pcmBytes,
          sampleRateHz,
          channels: 1,
          language: testLanguage.trim() || filter.language.trim() || null,
          scopes: filter.scope.trim() ? [filter.scope.trim()] : null,
          initialPrompt: testPrompt.trim() || null,
          useGpu: testUseGpu,
          forceCpu: !testUseGpu,
          keepModelLoaded: testKeepModelLoaded,
        });
        setTestResult(result);
        toast.success("Transcription complete", result.correctedText || result.rawText || "No text");
      } catch (error) {
        console.error("Failed to transcribe test audio:", error);
        toast.error("Transcription failed", String(error));
      } finally {
        setIsTranscribingTest(false);
      }
    },
    [
      filter.language,
      filter.scope,
      selectedTestModelPath,
      testKeepModelLoaded,
      testLanguage,
      testPrompt,
      testUseGpu,
    ],
  );

  const handlePreloadSelectedModel = useCallback(async () => {
    if (!selectedTestModelPath) {
      toast.warning("Choose a Whisper model first");
      return;
    }

    setIsPreloadingModel(true);
    try {
      await asrWhisperRuntimePreloadModel({
        modelPath: selectedTestModelPath,
        useGpu: testUseGpu,
        forceCpu: !testUseGpu,
      });
      toast.success("Model preloaded", "Whisper context is warm and ready.");
    } catch (error) {
      console.error("Failed to preload Whisper model:", error);
      toast.error("Failed to preload model", String(error));
    } finally {
      setIsPreloadingModel(false);
    }
  }, [selectedTestModelPath, testUseGpu]);

  const stopRecording = useCallback(async () => {
    const session = recorderRef.current;
    if (!session) return;

    recorderRef.current = null;
    setIsRecording(false);
    setRecordingElapsedMs(Date.now() - session.startedAt);

    session.processor.disconnect();
    session.source.disconnect();
    session.stream.getTracks().forEach((track) => track.stop());
    await session.audioContext.close();

    const merged = mergeFloat32Chunks(session.chunks);
    if (merged.length === 0) {
      toast.warning("No audio captured");
      return;
    }

    lastRecordingRef.current = { pcm: merged, sampleRate: session.sampleRate };
    setHasLastRecording(true);
    await runTestTranscription(merged, session.sampleRate);
  }, [runTestTranscription]);

  const rerunLastRecording = useCallback(() => {
    const last = lastRecordingRef.current;
    if (!last) return;
    void runTestTranscription(last.pcm, last.sampleRate);
  }, [runTestTranscription]);

  const stopPlayback = useCallback(() => {
    const playback = playbackRef.current;
    if (!playback) return;
    playback.source.onended = null;
    playback.source.stop();
    void playback.audioContext.close();
    playbackRef.current = null;
    setIsPlayingLastRecording(false);
  }, []);

  const playLastRecording = useCallback(async () => {
    const last = lastRecordingRef.current;
    if (!last) {
      toast.warning("Record a sample first");
      return;
    }
    if (isPlayingLastRecording) {
      stopPlayback();
      return;
    }

    try {
      const audioContext = new AudioContext();
      const buffer = audioContext.createBuffer(1, last.pcm.length, last.sampleRate);
      buffer.copyToChannel(new Float32Array(last.pcm), 0);
      const source = audioContext.createBufferSource();
      source.buffer = buffer;
      source.connect(audioContext.destination);
      source.onended = () => {
        if (playbackRef.current?.source === source) {
          void audioContext.close();
          playbackRef.current = null;
          setIsPlayingLastRecording(false);
        }
      };

      playbackRef.current = { audioContext, source };
      setIsPlayingLastRecording(true);
      source.start();
    } catch (error) {
      console.error("Failed to play last recording:", error);
      toast.error("Playback failed", String(error));
      stopPlayback();
    }
  }, [isPlayingLastRecording, stopPlayback]);

  const startRecording = useCallback(async () => {
    if (isRecording) return;
    if (!selectedTestModelPath) {
      toast.warning("Install or choose a Whisper model first");
      return;
    }

    try {
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: micConstraintsWithStoredDevice({
          channelCount: 1,
          echoCancellation: true,
          noiseSuppression: true,
          autoGainControl: true,
        }),
      });
      const audioContext = new AudioContext();
      const source = audioContext.createMediaStreamSource(stream);
      const processor = audioContext.createScriptProcessor(4096, 1, 1);
      const chunks: Float32Array[] = [];
      processor.onaudioprocess = (event) => {
        const channel = event.inputBuffer.getChannelData(0);
        chunks.push(new Float32Array(channel));
      };
      source.connect(processor);
      processor.connect(audioContext.destination);

      recorderRef.current = {
        stream,
        audioContext,
        source,
        processor,
        chunks,
        sampleRate: audioContext.sampleRate,
        startedAt: Date.now(),
      };
      stopPlayback();
      setTestResult(null);
      setRecordingElapsedMs(0);
      setIsRecording(true);
    } catch (error) {
      const message = micErrorMessage(error);
      if (error instanceof DOMException && error.name === "NotAllowedError") {
        console.warn("Microphone access denied:", error);
        toast.warning("Microphone access denied", message);
      } else {
        console.error("Failed to start microphone capture:", error);
        toast.error("Microphone access failed", message);
      }
    }
  }, [isRecording, selectedTestModelPath, stopPlayback]);

  const whisperQueue = useMemo(
    () => (downloadQueue?.queue ?? []).filter((item) => item.queueKind === "whisper"),
    [downloadQueue],
  );
  const previousWhisperQueueRef = useRef<string>("");
  useEffect(() => {
    const snapshot = JSON.stringify(
      whisperQueue.map((item) => ({
        id: item.id,
        status: item.status,
        variant: item.variant ?? null,
      })),
    );
    if (snapshot === previousWhisperQueueRef.current) {
      return;
    }

    const previousById = new Map(
      (previousWhisperQueueRef.current
        ? (JSON.parse(previousWhisperQueueRef.current) as Array<{
            id: string;
            status: string;
            variant: string | null;
          }>)
        : []
      ).map((item) => [item.id, item]),
    );

    previousWhisperQueueRef.current = snapshot;

    let shouldReloadInstalledModels = false;
    for (const item of whisperQueue) {
      const previous = previousById.get(item.id);
      if (!previous || previous.status === item.status) {
        continue;
      }
      if (item.status === "complete") {
        shouldReloadInstalledModels = true;
      }
      if (
        pendingModelId &&
        item.variant === pendingModelId &&
        (item.status === "error" || item.status === "cancelled")
      ) {
        setPendingModelId(null);
      }
    }

    if (shouldReloadInstalledModels) {
      void reloadInstalledModels();
    }
  }, [pendingModelId, reloadInstalledModels, whisperQueue]);
  const filterActive = filter.language.trim().length > 0 || filter.scope.trim().length > 0;
  const activeTab = LIBRARY_TABS.find((entry) => entry.key === tab)!;
  const tabCount = useMemo(() => {
    if (tab === "vocabulary") return vocabulary.length;
    if (tab === "corrections") return corrections.length;
    return examples.length;
  }, [tab, vocabulary.length, corrections.length, examples.length]);

  return (
    <motion.div
      initial={animations.fadeInFast.initial}
      animate={animations.fadeInFast.animate}
      transition={animations.fadeInFast.transition}
      className="flex h-full flex-col"
    >
      <div className="flex-1 overflow-y-auto">
        <div className="mx-auto w-full max-w-[1280px] px-3 pb-24 pt-3 sm:px-6 sm:pt-5 lg:px-8 space-y-4 sm:space-y-6">
          <div className="grid grid-cols-1 gap-4 sm:gap-5 lg:gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(340px,380px)]">
            <div className="min-w-0 space-y-4 sm:space-y-6">
              <div data-tour-id="asr-active-model">
                <ActiveModelCard
                  installedModels={installedModels}
                  availableModels={availableModels}
                  selectedTestModelPath={selectedTestModelPath}
                  pendingModelId={pendingModelId}
                  whisperQueue={whisperQueue}
                  onOpen={() => setModelPickerOpen(true)}
                  onClearPending={() => setPendingModelId(null)}
                />
              </div>

          <section data-tour-id="asr-library" className="space-y-3">
            <div className="flex flex-col gap-2 sm:flex-row sm:items-end sm:justify-between sm:gap-3">
              <div className="min-w-0">
                <h2 className={SECTION_TITLE}>Library</h2>
                <p className={cn("mt-0.5", SECTION_SUB)}>
                  Three layers shape Whisper output: the prompt prefix, post-processing
                  corrections, and labelled audio examples.
                </p>
              </div>
              <button
                type="button"
                onClick={() => setFiltersOpen((v) => !v)}
                className={cn(
                  ghostButton("self-start sm:self-auto"),
                  filterActive && "border-accent/40 bg-accent/10 text-accent",
                )}
              >
                <ListFilter className="h-3.5 w-3.5" />
                Filter
                {filterActive && (
                  <span className="ml-1 inline-block h-1.5 w-1.5 rounded-full bg-accent" />
                )}
              </button>
            </div>

            <AnimatePresence initial={false}>
              {filtersOpen && (
                <motion.div
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: "auto" }}
                  exit={{ opacity: 0, height: 0 }}
                  transition={{ duration: 0.18 }}
                  className="overflow-hidden"
                >
                  <FilterPanel
                    filter={filter}
                    setFilter={setFilter}
                    onReset={() => setFilter(DEFAULT_FILTER)}
                  />
                </motion.div>
              )}
            </AnimatePresence>

            <LayoutGroup id="asr-library-tabs">
              <div className="flex gap-1 overflow-x-auto rounded-2xl border border-fg/10 bg-fg/[0.04] p-1 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
                {LIBRARY_TABS.map((entry) => {
                  const Icon = entry.icon;
                  const active = tab === entry.key;
                  const count =
                    entry.key === "vocabulary"
                      ? vocabulary.length
                      : entry.key === "corrections"
                        ? corrections.length
                        : examples.length;
                  return (
                    <button
                      key={entry.key}
                      type="button"
                      onClick={() => setTab(entry.key)}
                      className={cn(
                        "relative flex flex-1 min-w-fit items-center justify-center gap-1.5 rounded-xl px-3 py-2 text-xs font-semibold",
                        interactive.transition.fast,
                        active ? "text-fg" : "text-fg/55 hover:text-fg/85",
                      )}
                    >
                      {active && (
                        <motion.span
                          layoutId="asr-tab-pill"
                          className="absolute inset-0 rounded-xl bg-fg/10 shadow-[0_1px_0_rgba(255,255,255,0.04)_inset]"
                          transition={{ type: "spring", stiffness: 420, damping: 36, mass: 0.7 }}
                        />
                      )}
                      <span className="relative flex items-center gap-1.5">
                        <Icon className="h-3.5 w-3.5" />
                        <span>{entry.label}</span>
                        <motion.span
                          layout
                          transition={{ type: "spring", stiffness: 420, damping: 36, mass: 0.7 }}
                          className={cn(
                            "ml-1 rounded-md px-1.5 py-0.5 text-[10px] font-bold leading-none",
                            active ? "bg-accent/15 text-accent" : "bg-fg/8 text-fg/45",
                          )}
                        >
                          {count}
                        </motion.span>
                      </span>
                    </button>
                  );
                })}
              </div>
            </LayoutGroup>

            <AnimatePresence mode="wait" initial={false}>
              <motion.p
                key={activeTab.key}
                initial={{ opacity: 0, y: -4 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: 4 }}
                transition={{ duration: 0.15 }}
                className="px-1 text-[11px] text-fg/40"
              >
                {activeTab.hint}
              </motion.p>
            </AnimatePresence>

            {tab === "vocabulary" && (
              <PromptPreviewCard prompt={previewPrompt} />
            )}

            {tab === "vocabulary" && (
              <VocabularySection
                items={vocabulary}
                count={tabCount}
                loading={loading}
                onAdd={() =>
                  setEditor({
                    kind: "vocabulary",
                    value: {
                      term: "",
                      category: "",
                      priority: 50,
                      language: filter.language,
                      scope: filter.scope,
                    },
                  })
                }
                onEdit={(term) => setEditor({ kind: "vocabulary", value: term })}
                onDelete={async (term) => {
                  if (term.id == null) return;
                  try {
                    await asrVocabularyDelete(term.id);
                    toast.success("Deleted vocabulary term");
                    void reload();
                  } catch (error) {
                    toast.error("Failed to delete", String(error));
                  }
                }}
              />
            )}

            {tab === "corrections" && (
              <CorrectionsSection
                items={corrections}
                count={tabCount}
                loading={loading}
                approvedOnly={showApprovedOnly}
                onToggleApprovedOnly={setShowApprovedOnly}
                onAdd={() =>
                  setEditor({
                    kind: "correction",
                    value: {
                      wrong: "",
                      correct: "",
                      confidence: 0.9,
                      userApproved: true,
                      language: filter.language,
                      scope: filter.scope,
                    },
                  })
                }
                onEdit={(item) => setEditor({ kind: "correction", value: item })}
                onDelete={async (item) => {
                  if (item.id == null) return;
                  try {
                    await asrCorrectionDelete(item.id);
                    toast.success("Deleted correction");
                    void reload();
                  } catch (error) {
                    toast.error("Failed to delete", String(error));
                  }
                }}
              />
            )}

            {tab === "voiceExamples" && (
              <VoiceExamplesSection
                items={examples}
                count={tabCount}
                loading={loading}
                onAdd={() =>
                  setEditor({
                    kind: "voiceExample",
                    value: {
                      audioPath: "",
                      expectedText: "",
                      whisperOutput: "",
                      language: filter.language,
                      scope: filter.scope,
                    },
                  })
                }
                onEdit={(item) => setEditor({ kind: "voiceExample", value: item })}
                onDelete={async (item) => {
                  if (item.id == null) return;
                  try {
                    await asrVoiceExampleDelete(item.id);
                    toast.success("Deleted voice example");
                    void reload();
                  } catch (error) {
                    toast.error("Failed to delete", String(error));
                  }
                }}
              />
            )}
          </section>

              <div data-tour-id="asr-runtime">
                <RuntimeSection
                  selectedTestModelPath={selectedTestModelPath}
                  modelsDir={modelsDir}
                  testUseGpu={testUseGpu}
                  setTestUseGpu={setTestUseGpu}
                  testKeepModelLoaded={testKeepModelLoaded}
                  setTestKeepModelLoaded={setTestKeepModelLoaded}
                  isPreloadingModel={isPreloadingModel}
                  onPreload={() => void handlePreloadSelectedModel()}
                  onClearCache={() => void handleClearCache()}
                  onCopyDir={() => void handleCopy(modelsDir, modelsDir)}
                  onLibraryImport={handleImport}
                  onLibraryExport={handleExport}
                />
              </div>
            </div>

            <aside
              data-tour-id="asr-mic-test"
              className="min-w-0 lg:sticky lg:top-4 lg:self-start"
            >
              <MicTestSection
                installedModels={installedModels}
                selectedTestModelPath={selectedTestModelPath}
                pendingModelId={pendingModelId}
                onOpenPicker={() => setModelPickerOpen(true)}
                testLanguage={testLanguage}
                setTestLanguage={setTestLanguage}
                testPrompt={testPrompt}
                setTestPrompt={setTestPrompt}
                isRecording={isRecording}
                recordingElapsedMs={recordingElapsedMs}
                isTranscribingTest={isTranscribingTest}
                isPlayingLastRecording={isPlayingLastRecording}
                hasLastRecording={hasLastRecording}
                testResult={testResult}
                onStart={() => void startRecording()}
                onStop={() => void stopRecording()}
                onRerun={rerunLastRecording}
                onPlay={() => void playLastRecording()}
              />
            </aside>
          </div>
        </div>
      </div>

      <ModelPickerSheet
        isOpen={modelPickerOpen}
        onClose={() => setModelPickerOpen(false)}
        installedModels={installedModels}
        availableModels={availableModels}
        whisperQueue={whisperQueue}
        queueingModelId={queueingModelId}
        deletingModelPath={deletingModelPath}
        selectedTestModelPath={selectedTestModelPath}
        pendingModelId={pendingModelId}
        loadingCatalog={loadingWhisperModels}
        loadingInstalled={loadingInstalledModels}
        onSelectInstalled={(model) => {
          setSelectedTestModelPath(model.path);
          setPendingModelId(null);
        }}
        onSelectAndQueue={async (model) => {
          setPendingModelId(model.id);
          await handleQueueModel(model.id);
        }}
        onDelete={(model) => void handleDeleteInstalledModel(model)}
      />

      <AnimatePresence>
        {editor.kind === "vocabulary" && (
          <VocabularyEditor
            initial={editor.value}
            onClose={() => setEditor({ kind: "none" })}
            onSaved={() => {
              setEditor({ kind: "none" });
              void reload();
            }}
          />
        )}
        {editor.kind === "correction" && (
          <CorrectionEditor
            initial={editor.value}
            onClose={() => setEditor({ kind: "none" })}
            onSaved={() => {
              setEditor({ kind: "none" });
              void reload();
            }}
          />
        )}
        {editor.kind === "voiceExample" && (
          <VoiceExampleEditor
            initial={editor.value}
            activeModelPath={selectedTestModelPath}
            onClose={() => setEditor({ kind: "none" })}
            onSaved={() => {
              setEditor({ kind: "none" });
              void reload();
            }}
          />
        )}
      </AnimatePresence>

      {showTour && <GuidedTour tour="speechRecognition" onDismiss={dismissTour} />}
    </motion.div>
  );
}

type EditorState =
  | { kind: "none" }
  | { kind: "vocabulary"; value: AsrVocabularyTerm }
  | { kind: "correction"; value: AsrCorrection }
  | { kind: "voiceExample"; value: AsrVoiceExample };

interface SectionProps<T> {
  items: T[];
  count: number;
  loading: boolean;
  onAdd: () => void;
  onEdit: (item: T) => void;
  onDelete: (item: T) => void | Promise<void>;
}


function FilterPanel({
  filter,
  setFilter,
  onReset,
}: {
  filter: FilterState;
  setFilter: React.Dispatch<React.SetStateAction<FilterState>>;
  onReset: () => void;
}) {
  return (
    <div className={panelClass("mt-1 space-y-3")}>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <FieldShell label="Language" hint="BCP-47 code, e.g. en, en-US, tr.">
          <input
            type="text"
            className={inputClass()}
            placeholder="any"
            value={filter.language}
            onChange={(e) => setFilter((prev) => ({ ...prev, language: e.target.value }))}
          />
        </FieldShell>
        <FieldShell label="Scope" hint="Limit to a workspace or context.">
          <input
            type="text"
            className={inputClass()}
            placeholder="global"
            value={filter.scope}
            onChange={(e) => setFilter((prev) => ({ ...prev, scope: e.target.value }))}
          />
        </FieldShell>
      </div>
      <div className="flex justify-end">
        <button type="button" className={ghostButton()} onClick={onReset}>
          <X className="h-3.5 w-3.5" />
          Reset
        </button>
      </div>
    </div>
  );
}


function PromptPreviewCard({ prompt }: { prompt: string }) {
  const [open, setOpen] = useState(false);
  const empty = !prompt;
  return (
    <div className={panelClass("space-y-3")}>
      <div className="flex items-start justify-between gap-3">
        <div className="flex min-w-0 items-start gap-3">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-info/30 bg-info/10 text-info">
            <Sparkles className="h-4 w-4" />
          </div>
          <div className="min-w-0">
            <div className="text-sm font-semibold text-fg">Initial prompt preview</div>
            <div className="mt-0.5 text-[11px] text-fg/50">
              Built from your vocabulary. Length is bounded, so high-priority terms appear first.
            </div>
          </div>
        </div>
        <button type="button" className={ghostButton()} onClick={() => setOpen((v) => !v)}>
          {open ? "Hide" : "Show"}
        </button>
      </div>
      <AnimatePresence initial={false}>
        {open && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            transition={{ duration: 0.18 }}
            className="overflow-hidden"
          >
            <div
              className={cn(
                "rounded-xl border border-fg/10 bg-bg-1/50 px-3 py-2.5 text-xs leading-relaxed",
                empty ? "italic text-fg/35" : "text-fg/85",
              )}
            >
              {empty
                ? "No vocabulary terms yet. Add some below to influence Whisper."
                : prompt}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}


type WhisperQueueItems = ReturnType<typeof useDownloadQueueOptional> extends infer Q
  ? Q extends { queue: infer Items }
    ? Items
    : never
  : never;

interface ActiveModelCardProps {
  installedModels: AsrInstalledWhisperModel[];
  availableModels: AsrWhisperModelPreset[];
  selectedTestModelPath: string;
  pendingModelId: string | null;
  whisperQueue: WhisperQueueItems;
  onOpen: () => void;
  onClearPending: () => void;
}

function ActiveModelCard({
  installedModels,
  availableModels,
  selectedTestModelPath,
  pendingModelId,
  whisperQueue,
  onOpen,
  onClearPending,
}: ActiveModelCardProps) {
  const installed = useMemo(
    () => installedModels.find((m) => m.path === selectedTestModelPath) ?? null,
    [installedModels, selectedTestModelPath],
  );
  const pendingMeta = useMemo(
    () =>
      pendingModelId ? availableModels.find((m) => m.id === pendingModelId) ?? null : null,
    [pendingModelId, availableModels],
  );
  const pendingQueueEntry = useMemo(
    () =>
      pendingModelId
        ? (whisperQueue as Array<{
            variant?: string | null;
            status?: string;
            downloaded: number;
            total: number;
            speedBytesPerSec?: number;
            filename?: string;
          }>).find((q) => q.variant === pendingModelId)
        : undefined,
    [pendingModelId, whisperQueue],
  );
  const pendingPct =
    pendingQueueEntry && pendingQueueEntry.total > 0
      ? Math.min(100, Math.round((pendingQueueEntry.downloaded / pendingQueueEntry.total) * 100))
      : 0;

  if (pendingModelId) {
    const filename =
      pendingQueueEntry?.filename ??
      (pendingMeta ? `ggml-${pendingMeta.id}.bin` : pendingModelId);
    const downloading = pendingQueueEntry?.status === "downloading";
    return (
      <div
        className={cn(
          "rounded-xl border border-accent/20 bg-accent/5 px-4 py-3",
          interactive.transition.fast,
        )}
      >
        <div className="flex items-center gap-3">
          {downloading ? (
            <Download size={14} className="shrink-0 text-accent/70 animate-pulse" />
          ) : (
            <Loader2 size={14} className="shrink-0 text-fg/30 animate-spin" />
          )}
          <div className="min-w-0 flex-1">
            <p className="truncate text-[12px] font-medium text-fg/80">{filename}</p>
            <p className="truncate text-[10px] text-fg/40">
              {pendingMeta?.label ?? pendingModelId}
            </p>
          </div>
          <button
            onClick={onClearPending}
            className={cn(
              "flex items-center justify-center rounded-lg p-1.5 text-fg/25",
              interactive.transition.fast,
              "hover:bg-fg/10 hover:text-danger/70 active:scale-90",
            )}
            title="Cancel"
            aria-label="Cancel download"
          >
            <X size={13} />
          </button>
        </div>
        {downloading ? (
          <div className="mt-2 flex items-center gap-2">
            <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-fg/10">
              <div
                className="h-full rounded-full bg-accent transition-all duration-300"
                style={{ width: `${pendingPct}%` }}
              />
            </div>
            <span className="shrink-0 text-[10px] tabular-nums text-fg/40">
              {pendingPct}%
              {pendingQueueEntry && pendingQueueEntry.total > 0 && (
                <span className="ml-1 text-fg/25">
                  {formatBytes(pendingQueueEntry.downloaded)}/{formatBytes(pendingQueueEntry.total)}
                </span>
              )}
              {pendingQueueEntry && (pendingQueueEntry.speedBytesPerSec ?? 0) > 0 && (
                <span className="ml-1 text-fg/25">
                  · {formatBytes(pendingQueueEntry.speedBytesPerSec ?? 0)}/s
                </span>
              )}
            </span>
          </div>
        ) : (
          <p className="mt-1.5 text-[10px] text-fg/30">Waiting in queue...</p>
        )}
      </div>
    );
  }

  const hasModel = !!installed;
  return (
    <button
      type="button"
      onClick={onOpen}
      className={cn(
        "group relative block w-full overflow-hidden rounded-2xl border px-4 py-3.5 text-left sm:px-5 sm:py-4",
        interactive.transition.fast,
        interactive.active.scale,
        hasModel
          ? "border-accent/25 bg-gradient-to-br from-accent/[0.07] via-fg/[0.03] to-fg/[0.02] hover:border-accent/40"
          : "border-dashed border-fg/20 bg-fg/[0.03] hover:border-fg/30 hover:bg-fg/[0.05]",
      )}
    >
      <div className="flex items-center gap-3 sm:gap-4">
        <div
          className={cn(
            "flex h-11 w-11 shrink-0 items-center justify-center rounded-xl border sm:h-12 sm:w-12",
            hasModel
              ? "border-accent/30 bg-accent/12 text-accent"
              : "border-fg/15 bg-fg/8 text-fg/55",
          )}
        >
          {hasModel ? <Waves className="h-5 w-5" /> : <Download className="h-5 w-5" />}
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="text-[10px] font-semibold uppercase tracking-[0.14em] text-fg/40">
              Active model
            </span>
            {hasModel && (
              <span className="relative flex h-1.5 w-1.5">
                <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-accent/50" />
                <span className="relative inline-flex h-1.5 w-1.5 rounded-full bg-accent" />
              </span>
            )}
          </div>
          <div className="mt-0.5 truncate text-sm font-semibold text-fg sm:text-[15px]">
            {installed?.label ?? "No model selected"}
          </div>
          <div className="mt-1 flex flex-wrap items-center gap-1.5">
            {hasModel ? (
              <>
                {installed.sizeBytes > 0 && (
                  <span className="inline-flex items-center rounded-md border border-fg/10 bg-fg/8 px-1.5 py-0.5 text-[10px] font-medium text-fg/65 tabular-nums">
                    {formatBytes(installed.sizeBytes)}
                  </span>
                )}
                {installed.quantized && (
                  <span className="inline-flex items-center rounded-md border border-info/25 bg-info/10 px-1.5 py-0.5 text-[10px] font-semibold text-info">
                    quantized
                  </span>
                )}
                <span className="inline-flex items-center rounded-md border border-fg/10 bg-fg/8 px-1.5 py-0.5 text-[10px] font-medium text-fg/55">
                  {installed.englishOnly ? "EN only" : "multilingual"}
                </span>
              </>
            ) : (
              <span className="text-[11px] text-fg/50">
                Pick a Whisper model to enable speech recognition.
              </span>
            )}
          </div>
        </div>
        <div
          className={cn(
            "flex shrink-0 items-center gap-1 rounded-full border px-2.5 py-1 text-[11px] font-semibold",
            interactive.transition.fast,
            hasModel
              ? "border-fg/15 bg-fg/5 text-fg/70 group-hover:border-accent/40 group-hover:bg-accent/10 group-hover:text-accent"
              : "border-accent/40 bg-accent/15 text-accent",
          )}
        >
          {hasModel ? "Change" : "Choose"}
          <ChevronRight className="h-3.5 w-3.5" />
        </div>
      </div>
    </button>
  );
}

interface ModelPickerSheetProps {
  isOpen: boolean;
  onClose: () => void;
  installedModels: AsrInstalledWhisperModel[];
  availableModels: AsrWhisperModelPreset[];
  whisperQueue: WhisperQueueItems;
  queueingModelId: string | null;
  deletingModelPath: string | null;
  selectedTestModelPath: string;
  pendingModelId: string | null;
  loadingCatalog: boolean;
  loadingInstalled: boolean;
  onSelectInstalled: (model: AsrInstalledWhisperModel) => void;
  onSelectAndQueue: (model: AsrWhisperModelPreset) => void | Promise<void>;
  onDelete: (model: AsrInstalledWhisperModel) => void;
}

function ModelPickerSheet({
  isOpen,
  onClose,
  installedModels,
  availableModels,
  whisperQueue,
  queueingModelId,
  deletingModelPath,
  selectedTestModelPath,
  pendingModelId,
  loadingCatalog,
  loadingInstalled,
  onSelectInstalled,
  onSelectAndQueue,
  onDelete,
}: ModelPickerSheetProps) {
  const [downloadedOnly, setDownloadedOnly] = useState(false);
  const [showAllPresets, setShowAllPresets] = useState(false);
  const [search, setSearch] = useState("");
  const isMobile = useMemo(() => getPlatform().type === "mobile", []);

  const installedById = useMemo(() => {
    const map = new Map<string, AsrInstalledWhisperModel>();
    for (const m of installedModels) map.set(m.id, m);
    return map;
  }, [installedModels]);

  const recommendedForPlatform = (model: AsrWhisperModelPreset) =>
    isMobile ? model.recommendedForMobile : model.recommendedForDesktop;

  const rows = useMemo(() => {
    const term = search.trim().toLowerCase();
    return availableModels
      .map((model) => ({
        model,
        installed: installedById.get(model.id) ?? null,
      }))
      .filter(({ model, installed }) => {
        if (downloadedOnly && !installed) return false;
        if (!showAllPresets && !installed && !recommendedForPlatform(model)) return false;
        if (!term) return true;
        return (
          model.label.toLowerCase().includes(term) ||
          model.id.toLowerCase().includes(term) ||
          model.filename.toLowerCase().includes(term)
        );
      });
  }, [availableModels, installedById, downloadedOnly, search, showAllPresets, isMobile]);

  return (
    <BottomMenu isOpen={isOpen} onClose={onClose} title="Choose a Whisper model">
      <div className="space-y-3 pb-2">
        {installedModels.length === 0 && !loadingInstalled && (
          <div className="flex items-start gap-3 rounded-xl border border-warning/30 bg-warning/8 px-3 py-2.5">
            <Download className="mt-0.5 h-4 w-4 shrink-0 text-warning" />
            <div className="text-[12px] leading-relaxed text-warning/90">
              You need to download a Whisper model before you can use speech recognition. Pick one
              from the list below. We recommend the {isMobile ? "mobile" : "desktop"} options.
            </div>
          </div>
        )}
        <p className="text-[12px] leading-relaxed text-fg/55">
          Showing models recommended for {isMobile ? "mobile" : "desktop"}. Tap one to use it; if
          it isn't downloaded yet, it'll queue and activate automatically when ready.
        </p>

        <div className="space-y-2">
          <input
            type="text"
            className={inputClass()}
            placeholder="Search models"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          <div className="flex flex-wrap items-center justify-between gap-2">
            <span className="text-[11px] text-fg/55">
              {rows.length} {rows.length === 1 ? "model" : "models"}
              {downloadedOnly ? ", downloaded only" : ""}
              {!showAllPresets && ` · for ${isMobile ? "mobile" : "desktop"}`}
            </span>
            <div className="flex items-center gap-3">
              <label className="flex cursor-pointer items-center gap-2 text-[11px] text-fg/65">
                <Switch
                  id="asr-picker-downloaded-only"
                  checked={downloadedOnly}
                  onChange={setDownloadedOnly}
                />
                <span>Downloaded only</span>
              </label>
              <label className="flex cursor-pointer items-center gap-2 text-[11px] text-fg/65">
                <Switch
                  id="asr-picker-show-all"
                  checked={showAllPresets}
                  onChange={setShowAllPresets}
                />
                <span>Show all</span>
              </label>
            </div>
          </div>
        </div>

        <div className="space-y-2">
          {(loadingCatalog || loadingInstalled) && rows.length === 0 ? (
            <SkeletonList rows={4} />
          ) : rows.length === 0 ? (
            <EmptyState
              icon={Sparkles}
              title="No models match"
              description="Try clearing the search or toggling Downloaded only off."
            />
          ) : (
            rows.map(({ model, installed }) => {
              const isActive =
                installed != null && installed.path === selectedTestModelPath;
              const isPending = pendingModelId === model.id;
              const busy = isBusy(queueingModelId, whisperQueue, model.id);
              return (
                <PickerRow
                  key={model.id}
                  model={model}
                  installed={installed}
                  isActive={isActive}
                  isPending={isPending}
                  busy={busy}
                  deleting={installed != null && deletingModelPath === installed.path}
                  onSelect={() => {
                    if (installed) {
                      onSelectInstalled(installed);
                      onClose();
                    } else {
                      void onSelectAndQueue(model);
                    }
                  }}
                  onDelete={() => installed && onDelete(installed)}
                />
              );
            })
          )}
        </div>
      </div>
    </BottomMenu>
  );
}

function PickerRow({
  model,
  installed,
  isActive,
  isPending,
  busy,
  deleting,
  onSelect,
  onDelete,
}: {
  model: AsrWhisperModelPreset;
  installed: AsrInstalledWhisperModel | null;
  isActive: boolean;
  isPending: boolean;
  busy: boolean;
  deleting: boolean;
  onSelect: () => void;
  onDelete: () => void;
}) {
  const friendly = getFriendlyModelCopy(model.id, model.label);
  return (
    <div
      className={cn(
        "rounded-xl border px-3.5 py-3",
        interactive.transition.fast,
        isActive
          ? "border-accent/40 bg-accent/8"
          : isPending
            ? "border-info/30 bg-info/5"
            : "border-fg/10 bg-fg/5 hover:border-fg/20 hover:bg-fg/8",
      )}
    >
      <div className="flex items-start justify-between gap-3">
        <button
          type="button"
          onClick={onSelect}
          className="flex min-w-0 flex-1 items-start gap-3 text-left"
        >
          <div
            className={cn(
              "flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border",
              isActive
                ? "border-accent/40 bg-accent/15 text-accent"
                : installed
                  ? "border-fg/15 bg-fg/8 text-fg/70"
                  : "border-fg/10 bg-fg/5 text-fg/55",
            )}
          >
            {isActive ? (
              <CheckCircle2 className="h-4 w-4" />
            ) : isPending ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : installed ? (
              <HardDrive className="h-4 w-4" />
            ) : (
              <Download className="h-4 w-4" />
            )}
          </div>
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <span className="truncate text-sm font-semibold text-fg">{friendly.title}</span>
              {isActive && (
                <span className="rounded-md border border-accent/30 bg-accent/10 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-accent">
                  active
                </span>
              )}
              {!isActive && installed && (
                <span className="rounded-md border border-fg/15 bg-fg/8 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-fg/55">
                  installed
                </span>
              )}
              {isPending && (
                <span className="rounded-md border border-info/30 bg-info/10 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-info">
                  downloading
                </span>
              )}
            </div>
            <div className="mt-0.5 line-clamp-2 text-[11px] text-fg/55">{friendly.summary}</div>
            <ModelMeta model={installed ?? model} />
          </div>
        </button>
        <div className="flex shrink-0 flex-col items-end gap-1.5">
          {!installed && (
            <button
              type="button"
              onClick={onSelect}
              disabled={busy || isPending}
              className={cn(
                accentButton("h-8 px-3"),
                (busy || isPending) && "cursor-not-allowed opacity-60",
              )}
            >
              {busy || isPending ? (
                <Loader2 className="h-3 w-3 animate-spin" />
              ) : (
                <Download className="h-3 w-3" />
              )}
              {isPending ? "Queued" : "Get"}
            </button>
          )}
          {installed && !isActive && (
            <button type="button" onClick={onSelect} className={ghostButton("h-8 px-3")}>
              Use
            </button>
          )}
          {installed && (
            <button
              type="button"
              onClick={onDelete}
              disabled={deleting}
              className={cn(
                "flex h-8 w-8 shrink-0 items-center justify-center rounded-full",
                "border border-danger/25 bg-danger/10 text-danger",
                "hover:border-danger/45 hover:bg-danger/15",
                interactive.transition.fast,
                interactive.active.scale,
                deleting && "cursor-not-allowed opacity-60",
              )}
              aria-label="Delete model"
              title="Delete model"
            >
              {deleting ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <Trash2 className="h-3.5 w-3.5" />
              )}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}


function isBusy(
  queueingModelId: string | null,
  queue: { variant?: string | null; status?: string }[],
  modelId: string,
) {
  return (
    queueingModelId === modelId ||
    queue.some(
      (item) =>
        item.variant === modelId &&
        (item.status === "queued" || item.status === "downloading"),
    )
  );
}

function ModelMeta({
  model,
}: {
  model: AsrWhisperModelPreset | AsrInstalledWhisperModel;
}) {
  return (
    <div className="mt-1 flex flex-wrap items-center gap-x-2.5 gap-y-1 text-[11px] text-fg/50">
      <span className="tabular-nums">{formatBytes(model.sizeBytes)}</span>
      <span className="opacity-40">·</span>
      <span>{model.englishOnly ? "English only" : "Multilingual"}</span>
      {model.quantized && (
        <>
          <span className="opacity-40">·</span>
          <span>Quantized</span>
        </>
      )}
    </div>
  );
}


interface MicTestSectionProps {
  installedModels: AsrInstalledWhisperModel[];
  selectedTestModelPath: string;
  pendingModelId: string | null;
  onOpenPicker: () => void;
  testLanguage: string;
  setTestLanguage: (v: string) => void;
  testPrompt: string;
  setTestPrompt: (v: string) => void;
  isRecording: boolean;
  recordingElapsedMs: number;
  isTranscribingTest: boolean;
  isPlayingLastRecording: boolean;
  hasLastRecording: boolean;
  testResult: AsrWhisperTranscriptionResponse | null;
  onStart: () => void;
  onStop: () => void;
  onRerun: () => void;
  onPlay: () => void;
}

function MicTestSection({
  installedModels,
  selectedTestModelPath,
  pendingModelId,
  onOpenPicker,
  testLanguage,
  setTestLanguage,
  testPrompt,
  setTestPrompt,
  isRecording,
  recordingElapsedMs,
  isTranscribingTest,
  isPlayingLastRecording,
  hasLastRecording,
  testResult,
  onStart,
  onStop,
  onRerun,
  onPlay,
}: MicTestSectionProps) {
  const noModels = installedModels.length === 0;
  const activeModel = useMemo(
    () => installedModels.find((m) => m.path === selectedTestModelPath) ?? null,
    [installedModels, selectedTestModelPath],
  );

  return (
    <section className="space-y-3">
      <div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-0.5">
        <h2 className={SECTION_TITLE}>Mic test</h2>
        {(activeModel || pendingModelId) && (
          <span
            className={cn(
              "inline-flex min-w-0 max-w-full items-center gap-1.5 truncate rounded-md border px-1.5 py-0.5 text-[10px] font-medium",
              pendingModelId
                ? "border-warning/25 bg-warning/10 text-warning/90"
                : "border-fg/10 bg-fg/8 text-fg/60",
            )}
          >
            {pendingModelId ? (
              <Loader2 className="h-3 w-3 shrink-0 animate-spin" />
            ) : (
              <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-accent" />
            )}
            <span className="truncate">
              {pendingModelId ? "waiting for download" : activeModel?.label}
            </span>
          </span>
        )}
      </div>

      <div className={panelClass("space-y-3")}>
        <FieldShell label="Language">
          <select
            className={inputClass()}
            value={testLanguage}
            onChange={(event) => setTestLanguage(event.target.value)}
          >
            <option value="">Auto-detect</option>
            {WHISPER_LANGUAGES.map((lang) => (
              <option key={lang.code} value={lang.code}>
                {lang.name} ({lang.code})
              </option>
            ))}
          </select>
        </FieldShell>
        <FieldShell label="Extra prompt" hint="Appended to the vocabulary prompt for this test only.">
          <input
            type="text"
            className={inputClass()}
            placeholder="Names, products, acronyms..."
            value={testPrompt}
            onChange={(event) => setTestPrompt(event.target.value)}
          />
        </FieldShell>

        <div
          className={cn(
            "flex flex-col items-center gap-3 rounded-xl border px-4 py-6 sm:py-7",
            isRecording
              ? "border-danger/25 bg-gradient-to-b from-danger/8 via-bg-1/40 to-bg-1/30"
              : "border-fg/10 bg-gradient-to-b from-fg/[0.04] via-bg-1/30 to-bg-1/40",
          )}
        >
          <RecordButton
            isRecording={isRecording}
            disabled={isTranscribingTest || !activeModel}
            onStart={onStart}
            onStop={onStop}
          />
          <div className="text-center">
            {isRecording ? (
              <div className="flex items-center justify-center gap-1.5 text-sm font-semibold text-danger">
                <span className="relative flex h-2 w-2">
                  <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-danger opacity-60" />
                  <span className="relative inline-flex h-2 w-2 rounded-full bg-danger" />
                </span>
                <span className="tabular-nums">
                  Recording {formatDuration(recordingElapsedMs)}
                </span>
              </div>
            ) : isTranscribingTest ? (
              <div className="flex items-center justify-center gap-1.5 text-sm font-medium text-info">
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
                Transcribing...
              </div>
            ) : pendingModelId ? (
              <div className="text-xs text-warning/85">
                Waiting for selected model to finish downloading.
              </div>
            ) : noModels ? (
              <button
                type="button"
                onClick={onOpenPicker}
                className={cn(accentButton(), "text-xs")}
              >
                <Download className="h-3.5 w-3.5" />
                Choose a model
              </button>
            ) : !activeModel ? (
              <button type="button" onClick={onOpenPicker} className={ghostButton("text-xs")}>
                Pick a model to enable recording
              </button>
            ) : (
              <div className="text-xs text-fg/55">
                Tap to record. Streams PCM directly to Whisper on stop.
              </div>
            )}
          </div>
          {hasLastRecording && !isRecording && (
            <div className="flex flex-wrap items-center justify-center gap-2">
              <button
                type="button"
                className={ghostButton()}
                onClick={onRerun}
                disabled={isTranscribingTest}
              >
                <RefreshCw className="h-3.5 w-3.5" />
                Run again
              </button>
              <button type="button" className={ghostButton()} onClick={onPlay}>
                {isPlayingLastRecording ? (
                  <>
                    <Pause className="h-3.5 w-3.5" />
                    Stop playback
                  </>
                ) : (
                  <>
                    <Play className="h-3.5 w-3.5" />
                    Play recording
                  </>
                )}
              </button>
            </div>
          )}
        </div>

        {testResult && <TranscriptionResultCard result={testResult} />}
      </div>
    </section>
  );
}

function RecordButton({
  isRecording,
  disabled,
  onStart,
  onStop,
}: {
  isRecording: boolean;
  disabled: boolean;
  onStart: () => void;
  onStop: () => void;
}) {
  return (
    <button
      type="button"
      onClick={isRecording ? onStop : onStart}
      disabled={disabled}
      className={cn(
        "group relative flex h-20 w-20 items-center justify-center rounded-full",
        interactive.transition.default,
        interactive.active.scale,
        disabled && "cursor-not-allowed opacity-50",
        isRecording
          ? "border-2 border-danger/60 bg-danger/15 text-danger shadow-[0_0_24px_-4px_rgba(248,113,113,0.4)]"
          : "border-2 border-accent/40 bg-accent/15 text-accent hover:border-accent/60 hover:bg-accent/20",
      )}
      aria-label={isRecording ? "Stop recording" : "Start recording"}
    >
      {isRecording && (
        <span className="absolute inset-0 animate-ping rounded-full border-2 border-danger/40" />
      )}
      {isRecording ? (
        <Square className="h-5 w-5" fill="currentColor" />
      ) : (
        <Mic className="h-7 w-7" />
      )}
    </button>
  );
}

function TranscriptionResultCard({ result }: { result: AsrWhisperTranscriptionResponse }) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.2 }}
      className="space-y-2.5"
    >
      <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
        <div className="rounded-xl border border-fg/10 bg-bg-1/40 px-3 py-3">
          <div className={OVERLINE}>Raw</div>
          <div className="mt-1.5 text-sm text-fg/85">
            {result.rawText || <span className="italic text-fg/35">No text</span>}
          </div>
        </div>
        <div className="rounded-xl border border-accent/25 bg-accent/8 px-3 py-3">
          <div className={cn(OVERLINE, "text-accent/80")}>Corrected</div>
          <div className="mt-1.5 text-sm font-medium text-fg">
            {result.correctedText || <span className="italic text-fg/35">No text</span>}
          </div>
        </div>
      </div>
      <div className="grid grid-cols-3 gap-1.5 sm:gap-2">
        <Stat label="Language">
          <span className="inline-flex items-center gap-1">
            <Languages className="h-3 w-3 text-fg/40" />
            {result.detectedLanguage || "auto"}
          </span>
        </Stat>
        <Stat label="Corrections">
          {result.appliedCorrections.length}
        </Stat>
        <Stat label="Segments">{result.segments.length}</Stat>
      </div>

      {result.appliedCorrections.length > 0 && (
        <div className="rounded-xl border border-fg/10 bg-fg/5 px-3 py-3">
          <div className={OVERLINE}>Applied corrections</div>
          <div className="mt-2 space-y-1">
            {result.appliedCorrections.map((item, index) => (
              <div
                key={`${item.correctionId}-${index}`}
                className="flex items-center gap-2 text-sm text-fg/80"
              >
                <span className="font-medium text-danger/80">{item.matchText}</span>
                <span className="text-fg/35">→</span>
                <span className="font-medium">{item.correct}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {result.segments.length > 0 && (
        <details className="rounded-xl border border-fg/10 bg-fg/5 px-3 py-2.5 group">
          <summary className="flex cursor-pointer items-center justify-between text-xs font-semibold text-fg/70">
            <span>Segments ({result.segments.length})</span>
            <span className="text-[11px] text-fg/40 group-open:hidden">Show</span>
            <span className="hidden text-[11px] text-fg/40 group-open:inline">Hide</span>
          </summary>
          <div className="mt-2 space-y-1.5">
            {result.segments.map((segment) => (
              <div key={`${segment.index}-${segment.startMs}`} className="text-sm text-fg/80">
                <span className="mr-2 font-mono text-[10px] tabular-nums text-fg/45">
                  {formatDuration(segment.startMs)}–{formatDuration(segment.endMs)}
                </span>
                <span>{segment.text}</span>
              </div>
            ))}
          </div>
        </details>
      )}

      {result.prompt && (
        <details className="rounded-xl border border-fg/10 bg-fg/5 px-3 py-2.5 group">
          <summary className="flex cursor-pointer items-center justify-between text-xs font-semibold text-fg/70">
            <span>Prompt used</span>
            <span className="text-[11px] text-fg/40 group-open:hidden">Show</span>
            <span className="hidden text-[11px] text-fg/40 group-open:inline">Hide</span>
          </summary>
          <p className="mt-2 whitespace-pre-wrap text-[11px] leading-relaxed text-fg/65">
            {result.prompt}
          </p>
        </details>
      )}
    </motion.div>
  );
}

function RuntimeSection({
  selectedTestModelPath,
  modelsDir,
  testUseGpu,
  setTestUseGpu,
  testKeepModelLoaded,
  setTestKeepModelLoaded,
  isPreloadingModel,
  onPreload,
  onClearCache,
  onCopyDir,
  onLibraryImport,
  onLibraryExport,
}: {
  selectedTestModelPath: string;
  modelsDir: string;
  testUseGpu: boolean;
  setTestUseGpu: (value: boolean) => void;
  testKeepModelLoaded: boolean;
  setTestKeepModelLoaded: (value: boolean) => void;
  isPreloadingModel: boolean;
  onPreload: () => void;
  onClearCache: () => void;
  onCopyDir: () => void;
  onLibraryImport: () => void;
  onLibraryExport: () => void;
}) {
  const isMobile = useMemo(() => getPlatform().type === "mobile", []);
  return (
    <section className="space-y-3">
      <div>
        <h2 className={SECTION_TITLE}>Runtime</h2>
        <p className={cn("mt-0.5", SECTION_SUB)}>
          Control how Whisper loads the active model and where library data lives on disk.
        </p>
      </div>

      <div className={panelClass("space-y-3")}>
        {!isMobile && (
          <RuntimeRow
            icon={<Sparkles className="h-4 w-4" />}
            title="Use GPU"
            description="Use CUDA, Vulkan, ROCm, or Metal acceleration when this build supports it. Off forces CPU."
            right={<Switch checked={testUseGpu} onChange={setTestUseGpu} />}
          />
        )}
        <RuntimeRow
          icon={<Waves className="h-4 w-4" />}
          title="Keep model loaded"
          description="Reuse the loaded Whisper context between transcriptions for lower latency."
          right={<Switch checked={testKeepModelLoaded} onChange={setTestKeepModelLoaded} />}
        />
        <MicInputRow />
        <RuntimeRow
          icon={<Settings2 className="h-4 w-4" />}
          title="Whisper context cache"
          description={
            selectedTestModelPath
              ? "Preload the active model now, or clear the cache to force a cold reload later."
              : "Pick a model above to preload it into the Whisper runtime cache."
          }
          right={
            <div className="flex flex-wrap justify-end gap-2">
              <button
                type="button"
                className={accentButton()}
                onClick={onPreload}
                disabled={!selectedTestModelPath || isPreloadingModel}
              >
                {isPreloadingModel ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <Waves className="h-3.5 w-3.5" />
                )}
                Preload
              </button>
              <button type="button" className={ghostButton()} onClick={onClearCache}>
                <Trash2 className="h-3.5 w-3.5" />
                Clear
              </button>
            </div>
          }
        />
        <RuntimeRow
          icon={<Folder className="h-4 w-4" />}
          title="Models folder"
          description={modelsDir || "Loading..."}
          descriptionMono
          right={
            modelsDir ? (
              <button type="button" className={ghostButton()} onClick={onCopyDir}>
                <Copy className="h-3.5 w-3.5" />
                Copy
              </button>
            ) : null
          }
        />
        <RuntimeRow
          icon={<Upload className="h-4 w-4" />}
          title="Library data"
          description="Back up vocabulary, corrections, and voice examples as JSON, or restore from a previous export."
          right={
            <div className="flex flex-wrap justify-end gap-2">
              <button type="button" className={ghostButton()} onClick={onLibraryImport}>
                <Upload className="h-3.5 w-3.5" />
                Import
              </button>
              <button type="button" className={ghostButton()} onClick={onLibraryExport}>
                <Download className="h-3.5 w-3.5" />
                Export
              </button>
            </div>
          }
        />
      </div>
    </section>
  );
}

function RuntimeRow({
  icon,
  title,
  description,
  descriptionMono,
  right,
}: {
  icon: React.ReactNode;
  title: string;
  description: string;
  descriptionMono?: boolean;
  right: React.ReactNode;
}) {
  return (
    <div className="flex flex-col gap-3 rounded-xl border border-fg/10 bg-fg/5 px-3.5 py-3 sm:flex-row sm:items-start sm:justify-between">
      <div className="flex min-w-0 items-start gap-3">
        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-fg/12 bg-fg/8 text-fg/75">
          {icon}
        </div>
        <div className="min-w-0">
          <div className="text-sm font-semibold text-fg">{title}</div>
          <div
            className={cn(
              "mt-0.5 text-[11px] leading-relaxed text-fg/55",
              descriptionMono && "break-all font-mono",
            )}
          >
            {description}
          </div>
        </div>
      </div>
      {right && (
        <div className="flex shrink-0 flex-wrap justify-end gap-2 pl-12 sm:pl-0">{right}</div>
      )}
    </div>
  );
}

function MicInputRow() {
  const [devices, setDevices] = useState<MediaDeviceInfo[]>([]);
  const [selectedId, setSelectedId] = useState<string>(getStoredMicDeviceId() ?? "");
  const [needsPermission, setNeedsPermission] = useState(false);
  const [refreshing, setRefreshing] = useState(false);

  const refresh = useCallback(async () => {
    if (typeof navigator === "undefined" || !navigator.mediaDevices?.enumerateDevices) {
      return;
    }
    setRefreshing(true);
    try {
      const list = await navigator.mediaDevices.enumerateDevices();
      const inputs = list.filter((d) => d.kind === "audioinput");
      setDevices(inputs);
      setNeedsPermission(inputs.some((d) => !d.label));
    } catch (error) {
      console.error("Failed to enumerate audio devices:", error);
    } finally {
      setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    if (typeof navigator === "undefined" || !navigator.mediaDevices) return;
    const handler = () => void refresh();
    navigator.mediaDevices.addEventListener("devicechange", handler);
    return () => navigator.mediaDevices.removeEventListener("devicechange", handler);
  }, [refresh]);

  useEffect(() => {
    if (!selectedId) return;
    if (devices.length === 0) return;
    if (!devices.some((d) => d.deviceId === selectedId)) {
      setSelectedId("");
      setStoredMicDeviceId(null);
    }
  }, [devices, selectedId]);

  const requestPermission = async () => {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      stream.getTracks().forEach((track) => track.stop());
      await refresh();
    } catch (error) {
      console.error("Failed to request microphone permission:", error);
      toast.error("Microphone permission denied", micErrorMessage(error));
    }
  };

  const handleChange = (next: string) => {
    setSelectedId(next);
    setStoredMicDeviceId(next || null);
  };

  return (
    <div className="flex flex-col gap-3 rounded-xl border border-fg/10 bg-fg/5 px-3.5 py-3 sm:flex-row sm:flex-wrap sm:items-center">
      <div className="flex min-w-0 flex-1 items-center gap-3">
        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-fg/10 bg-fg/5 text-fg/70">
          <Mic className="h-4 w-4" />
        </div>
        <div className="min-w-0">
          <div className="text-sm font-semibold text-fg">Microphone</div>
          <div className="text-[11px] leading-snug text-fg/55">
            Used by the chat footer and Mic test.
          </div>
        </div>
      </div>
      <div className="flex items-center gap-2 pl-12 sm:pl-0 sm:shrink-0">
        <select
          className={cn(inputClass(), "h-9 min-w-0 flex-1 py-0 sm:w-56 sm:flex-none sm:max-w-[60vw]")}
          value={selectedId}
          onChange={(event) => handleChange(event.target.value)}
          disabled={devices.length === 0}
        >
          <option value="">System default</option>
          {devices.map((device) => (
            <option key={device.deviceId} value={device.deviceId}>
              {device.label || `Microphone (${device.deviceId.slice(0, 6)}...)`}
            </option>
          ))}
        </select>
        <button
          type="button"
          onClick={() => void refresh()}
          className={ghostButton("h-9 w-9 px-0 justify-center")}
          title="Refresh device list"
          aria-label="Refresh device list"
          disabled={refreshing}
        >
          {refreshing ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : (
            <RefreshCw className="h-3.5 w-3.5" />
          )}
        </button>
      </div>
      {needsPermission && (
        <button
          type="button"
          onClick={() => void requestPermission()}
          className={cn(accentButton("h-8 px-3"), "text-xs")}
        >
          <Mic className="h-3.5 w-3.5" />
          Grant access to see device names
        </button>
      )}
    </div>
  );
}

function Stat({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="min-w-0 rounded-xl border border-fg/10 bg-fg/5 px-2 py-2 sm:px-3">
      <div className={cn(OVERLINE, "truncate")}>{label}</div>
      <div className="mt-1 truncate text-sm font-semibold text-fg/85 tabular-nums">{children}</div>
    </div>
  );
}


function LibraryHeader({
  title,
  count,
  onAdd,
  addLabel,
}: {
  title: string;
  count: number;
  onAdd: () => void;
  addLabel: string;
}) {
  return (
    <div className="flex items-center justify-between px-1">
      <div className="flex items-center gap-2">
        <span className="text-sm font-semibold text-fg">{title}</span>
        <span className="rounded-md bg-fg/10 px-1.5 py-0.5 text-[10px] font-bold text-fg/55 tabular-nums">
          {count}
        </span>
      </div>
      <button type="button" className={accentButton()} onClick={onAdd}>
        <Plus className="h-3.5 w-3.5" />
        {addLabel}
      </button>
    </div>
  );
}

function VocabularySection({
  items,
  count,
  loading,
  onAdd,
  onEdit,
  onDelete,
}: SectionProps<AsrVocabularyTerm>) {
  return (
    <div className="space-y-2">
      <LibraryHeader title="Custom vocabulary" count={count} onAdd={onAdd} addLabel="Add term" />
      {loading && items.length === 0 ? (
        <SkeletonList />
      ) : items.length === 0 ? (
        <EmptyState
          icon={BookOpen}
          title="No custom vocabulary"
          description="Add names, jargon, or product terms so Whisper has a chance to recognise them."
          action={
            <button type="button" className={accentButton()} onClick={onAdd}>
              <Plus className="h-3.5 w-3.5" />
              Add first term
            </button>
          }
        />
      ) : (
        <div className="space-y-2">
          {items.map((item) => (
            <div
              key={item.id ?? `${item.term}-${item.scope}`}
              className={subCardClass("group hover:border-fg/20")}
            >
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="truncate text-sm font-semibold text-fg">{item.term}</span>
                    {typeof item.priority === "number" && (
                      <PriorityBadge priority={item.priority} />
                    )}
                  </div>
                  <ChipRow
                    chips={[
                      item.category && { label: "category", value: item.category },
                      item.language && { label: "lang", value: item.language },
                      { label: "scope", value: item.scope ?? "global" },
                      typeof item.useCount === "number" && item.useCount > 0
                        ? { label: "uses", value: String(item.useCount) }
                        : null,
                    ]}
                  />
                </div>
                <RowActions onEdit={() => onEdit(item)} onDelete={() => void onDelete(item)} />
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function PriorityBadge({ priority }: { priority: number }) {
  const tone =
    priority >= 75
      ? "border-warning/30 bg-warning/10 text-warning"
      : priority >= 40
        ? "border-info/30 bg-info/10 text-info"
        : "border-fg/15 bg-fg/10 text-fg/55";
  return (
    <span
      className={cn(
        "rounded-md border px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider tabular-nums",
        tone,
      )}
    >
      P{priority}
    </span>
  );
}

interface CorrectionsSectionProps extends SectionProps<AsrCorrection> {
  approvedOnly: boolean;
  onToggleApprovedOnly: (next: boolean) => void;
}

const CORRECTIONS_PAGE_SIZE = 10;

function CorrectionsSection({
  items,
  count,
  loading,
  approvedOnly,
  onToggleApprovedOnly,
  onAdd,
  onEdit,
  onDelete,
}: CorrectionsSectionProps) {
  const [page, setPage] = useState(0);
  const totalPages = Math.max(1, Math.ceil(items.length / CORRECTIONS_PAGE_SIZE));

  useEffect(() => {
    if (page > totalPages - 1) setPage(Math.max(0, totalPages - 1));
  }, [page, totalPages]);

  useEffect(() => {
    setPage(0);
  }, [approvedOnly]);

  const start = page * CORRECTIONS_PAGE_SIZE;
  const visible = items.slice(start, start + CORRECTIONS_PAGE_SIZE);
  const showingFrom = items.length === 0 ? 0 : start + 1;
  const showingTo = Math.min(items.length, start + CORRECTIONS_PAGE_SIZE);

  return (
    <div className="space-y-2">
      <LibraryHeader
        title="Learned corrections"
        count={count}
        onAdd={onAdd}
        addLabel="Add correction"
      />
      <div className={subCardClass("flex items-center justify-between")}>
        <div className="min-w-0">
          <div className="text-sm font-semibold text-fg">Approved only</div>
          <div className="mt-0.5 text-[11px] text-fg/50">
            Hide auto-suggested entries that have not been reviewed.
          </div>
        </div>
        <Switch id="asr-approved-only" checked={approvedOnly} onChange={onToggleApprovedOnly} />
      </div>
      {loading && items.length === 0 ? (
        <SkeletonList />
      ) : items.length === 0 ? (
        <EmptyState
          icon={Wand2}
          title="No corrections yet"
          description="A correction rewrites Whisper output. Add a wrong → right pair to clean up consistent mistakes."
          action={
            <button type="button" className={accentButton()} onClick={onAdd}>
              <Plus className="h-3.5 w-3.5" />
              Add first correction
            </button>
          }
        />
      ) : (
        <div className="space-y-2">
          {visible.map((item) => (
            <div
              key={item.id ?? `${item.wrong}->${item.correct}`}
              className={subCardClass("hover:border-fg/20")}
            >
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2 text-sm">
                    <span className="truncate font-semibold text-danger/80 line-through decoration-danger/40">
                      {item.wrong}
                    </span>
                    <span className="text-fg/40">→</span>
                    <span className="truncate font-semibold text-fg">{item.correct}</span>
                  </div>
                  <ChipRow
                    chips={[
                      {
                        label: item.userApproved ? "approved" : "suggested",
                        value: "",
                        tone: item.userApproved ? "info" : "muted",
                      },
                      typeof item.confidence === "number"
                        ? { label: "conf", value: item.confidence.toFixed(2) }
                        : null,
                      item.language && { label: "lang", value: item.language },
                      { label: "scope", value: item.scope ?? "global" },
                      typeof item.useCount === "number" && item.useCount > 0
                        ? { label: "uses", value: String(item.useCount) }
                        : null,
                      typeof item.acceptedCount === "number" && item.acceptedCount > 0
                        ? { label: "accepted", value: String(item.acceptedCount), tone: "info" }
                        : null,
                      typeof item.rejectedCount === "number" && item.rejectedCount > 0
                        ? { label: "ignored", value: String(item.rejectedCount), tone: "muted" }
                        : null,
                      typeof item.seenCount === "number" && item.seenCount > 0
                        ? { label: "seen", value: String(item.seenCount) }
                        : null,
                      item.lastSeenAt ? { label: "last", value: item.lastSeenAt } : null,
                    ]}
                  />
                </div>
                <RowActions onEdit={() => onEdit(item)} onDelete={() => void onDelete(item)} />
              </div>
            </div>
          ))}
        </div>
      )}
      {items.length > CORRECTIONS_PAGE_SIZE && (
        <Pagination
          page={page}
          totalPages={totalPages}
          showingFrom={showingFrom}
          showingTo={showingTo}
          total={items.length}
          onPrev={() => setPage((p) => Math.max(0, p - 1))}
          onNext={() => setPage((p) => Math.min(totalPages - 1, p + 1))}
        />
      )}
    </div>
  );
}

function Pagination({
  page,
  totalPages,
  showingFrom,
  showingTo,
  total,
  onPrev,
  onNext,
}: {
  page: number;
  totalPages: number;
  showingFrom: number;
  showingTo: number;
  total: number;
  onPrev: () => void;
  onNext: () => void;
}) {
  const atStart = page === 0;
  const atEnd = page >= totalPages - 1;
  return (
    <div className="flex items-center justify-between gap-2 rounded-xl border border-fg/10 bg-fg/5 px-3 py-2">
      <span className="text-[11px] tabular-nums text-fg/55">
        <span className="font-semibold text-fg/75">
          {showingFrom}–{showingTo}
        </span>{" "}
        of {total}
      </span>
      <div className="flex items-center gap-1.5">
        <button
          type="button"
          onClick={onPrev}
          disabled={atStart}
          className={cn(
            "flex h-8 w-8 items-center justify-center rounded-lg border border-fg/10 bg-fg/5 text-fg/70",
            interactive.transition.fast,
            interactive.active.scale,
            "hover:border-fg/25 hover:bg-fg/10 hover:text-fg",
            "disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:border-fg/10 disabled:hover:bg-fg/5 disabled:hover:text-fg/70",
          )}
          aria-label="Previous page"
        >
          <ChevronRight className="h-3.5 w-3.5 rotate-180" />
        </button>
        <span className="min-w-[3.5rem] text-center text-[11px] font-semibold tabular-nums text-fg/70">
          {page + 1} / {totalPages}
        </span>
        <button
          type="button"
          onClick={onNext}
          disabled={atEnd}
          className={cn(
            "flex h-8 w-8 items-center justify-center rounded-lg border border-fg/10 bg-fg/5 text-fg/70",
            interactive.transition.fast,
            interactive.active.scale,
            "hover:border-fg/25 hover:bg-fg/10 hover:text-fg",
            "disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:border-fg/10 disabled:hover:bg-fg/5 disabled:hover:text-fg/70",
          )}
          aria-label="Next page"
        >
          <ChevronRight className="h-3.5 w-3.5" />
        </button>
      </div>
    </div>
  );
}

function VoiceExamplesSection({
  items,
  count,
  loading,
  onAdd,
  onEdit,
  onDelete,
}: SectionProps<AsrVoiceExample>) {
  return (
    <div className="space-y-2">
      <LibraryHeader title="Voice examples" count={count} onAdd={onAdd} addLabel="Add example" />
      {loading && items.length === 0 ? (
        <SkeletonList />
      ) : items.length === 0 ? (
        <EmptyState
          icon={AudioLines}
          title="No voice examples"
          description="Pair an audio clip with the text it should produce. Useful for tuning corrections from real recordings."
          action={
            <button type="button" className={accentButton()} onClick={onAdd}>
              <Plus className="h-3.5 w-3.5" />
              Add first example
            </button>
          }
        />
      ) : (
        <div className="space-y-2">
          {items.map((item) => (
            <div key={item.id ?? item.audioPath} className={subCardClass("hover:border-fg/20")}>
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-semibold text-fg">{item.expectedText}</div>
                  <div className="mt-1 truncate font-mono text-[10px] text-fg/45">
                    {item.audioPath}
                  </div>
                  {item.whisperOutput && (
                    <div className="mt-1.5 rounded-md border border-fg/10 bg-bg-1/40 px-2 py-1 text-[11px] leading-relaxed text-fg/70">
                      <span className="text-fg/40">heard: </span>
                      <span>{item.whisperOutput}</span>
                    </div>
                  )}
                  <ChipRow
                    chips={[
                      item.language && { label: "lang", value: item.language },
                      { label: "scope", value: item.scope ?? "global" },
                      item.correctionId != null
                        ? { label: "correction", value: `#${item.correctionId}` }
                        : null,
                      item.termId != null ? { label: "term", value: `#${item.termId}` } : null,
                    ]}
                  />
                </div>
                <RowActions onEdit={() => onEdit(item)} onDelete={() => void onDelete(item)} />
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

type ChipEntry = { label: string; value: string; tone?: "info" | "muted" | "default" };
type Chip = ChipEntry | null | false | undefined | "";

function ChipRow({ chips }: { chips: Chip[] }) {
  const visible = chips.filter(Boolean) as ChipEntry[];
  if (visible.length === 0) return null;
  return (
    <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
      {visible.map((chip, idx) => {
        const tone = chip.tone ?? "default";
        const toneClass =
          tone === "info"
            ? "border-info/30 bg-info/10 text-info"
            : tone === "muted"
              ? "border-fg/10 bg-fg/8 text-fg/50"
              : "border-fg/10 bg-fg/8 text-fg/55";
        return (
          <span
            key={`${chip.label}-${idx}`}
            className={cn(
              "inline-flex items-center gap-1 rounded-md border px-1.5 py-0.5 text-[10px] font-medium",
              toneClass,
            )}
          >
            <span className="opacity-60">{chip.label}</span>
            {chip.value && <span className="font-semibold">{chip.value}</span>}
          </span>
        );
      })}
    </div>
  );
}

function RowActions({ onEdit, onDelete }: { onEdit: () => void; onDelete: () => void }) {
  return (
    <div className="flex shrink-0 gap-1">
      <button
        type="button"
        onClick={onEdit}
        className={cn(
          "flex h-8 w-8 items-center justify-center rounded-lg border border-fg/10 bg-fg/5 text-fg/65",
          "hover:border-fg/25 hover:bg-fg/10 hover:text-fg",
          interactive.transition.fast,
          interactive.active.scale,
        )}
        aria-label="Edit"
      >
        <Pencil className="h-3.5 w-3.5" />
      </button>
      <button
        type="button"
        onClick={onDelete}
        className={cn(
          "flex h-8 w-8 items-center justify-center rounded-lg border border-danger/20 bg-danger/10 text-danger",
          "hover:border-danger/40 hover:bg-danger/20",
          interactive.transition.fast,
          interactive.active.scale,
        )}
        aria-label="Delete"
      >
        <Trash2 className="h-3.5 w-3.5" />
      </button>
    </div>
  );
}

function EmptyState({
  icon: Icon,
  title,
  description,
  action,
}: {
  icon?: React.ComponentType<{ className?: string }>;
  title: string;
  description: string;
  action?: React.ReactNode;
}) {
  return (
    <div
      className={cn(
        "flex flex-col items-center gap-2 rounded-2xl border border-dashed border-fg/15 bg-fg/[0.03] px-4 py-8 text-center",
      )}
    >
      {Icon && (
        <div className="flex h-10 w-10 items-center justify-center rounded-full border border-fg/10 bg-fg/5 text-fg/55">
          <Icon className="h-5 w-5" />
        </div>
      )}
      <div className="text-sm font-semibold text-fg/75">{title}</div>
      <div className="max-w-sm text-[11px] leading-relaxed text-fg/50">{description}</div>
      {action && <div className="mt-1">{action}</div>}
    </div>
  );
}

function SkeletonList({ rows = 3 }: { rows?: number }) {
  return (
    <div className="space-y-2">
      {Array.from({ length: rows }).map((_, idx) => (
        <div
          key={idx}
          className={cn(
            "h-16 w-full animate-pulse",
            radius.lg,
            "border border-fg/5 bg-fg/5",
          )}
        />
      ))}
    </div>
  );
}


interface EditorShellProps {
  title: string;
  subtitle?: string;
  onClose: () => void;
  onSubmit: () => void | Promise<void>;
  submitLabel?: string;
  busy?: boolean;
  children: React.ReactNode;
}

function EditorSheet({
  title,
  subtitle,
  onClose,
  onSubmit,
  submitLabel = "Save",
  busy,
  children,
}: EditorShellProps) {
  return (
    <BottomMenu isOpen onClose={onClose} title={title}>
      <div className="space-y-4 pb-2">
        {subtitle && <p className="text-[12px] leading-relaxed text-fg/55">{subtitle}</p>}
        {children}
        <div className="flex gap-3 pt-1">
          <button
            type="button"
            onClick={onClose}
            disabled={busy}
            className="flex-1 rounded-lg border border-fg/10 bg-fg/5 px-4 py-2 text-sm font-medium text-fg/70 transition hover:border-fg/20 hover:bg-fg/10 hover:text-fg disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => void onSubmit()}
            disabled={busy}
            className="flex flex-1 items-center justify-center gap-1.5 rounded-lg border border-accent/40 bg-accent/20 px-4 py-2 text-sm font-semibold text-accent/90 transition hover:border-accent/60 hover:bg-accent/30 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {busy && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
            {submitLabel}
          </button>
        </div>
      </div>
    </BottomMenu>
  );
}

function FieldShell({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <label className="block">
      <span className={cn("mb-1 block px-0.5", OVERLINE)}>{label}</span>
      {children}
      {hint && <span className="mt-1 block px-0.5 text-[11px] text-fg/45">{hint}</span>}
    </label>
  );
}

function VocabularyEditor({
  initial,
  onClose,
  onSaved,
}: {
  initial: AsrVocabularyTerm;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [draft, setDraft] = useState<AsrVocabularyTerm>(initial);
  const [busy, setBusy] = useState(false);
  const isEdit = initial.id != null;

  const submit = async () => {
    if (!draft.term.trim()) {
      toast.warning("Term required");
      return;
    }
    setBusy(true);
    try {
      await asrVocabularyUpsert({
        ...draft,
        term: draft.term.trim(),
        category: draft.category?.trim() || null,
        language: draft.language?.trim() || null,
        scope: draft.scope?.trim() || null,
      });
      toast.success(isEdit ? "Updated" : "Added");
      onSaved();
    } catch (error) {
      toast.error("Failed to save", String(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <EditorSheet
      title={isEdit ? "Edit vocabulary term" : "Add vocabulary term"}
      subtitle="Bias Whisper toward names, jargon, and product terms."
      onClose={onClose}
      onSubmit={submit}
      busy={busy}
    >
      <FieldShell label="Term">
        <input
          autoFocus
          type="text"
          className={inputClass()}
          value={draft.term}
          onChange={(e) => setDraft((p) => ({ ...p, term: e.target.value }))}
        />
      </FieldShell>
      <FieldShell label="Category" hint="Optional grouping label, e.g. names, places, jargon.">
        <input
          type="text"
          className={inputClass()}
          value={draft.category ?? ""}
          onChange={(e) => setDraft((p) => ({ ...p, category: e.target.value }))}
        />
      </FieldShell>
      <div className="grid grid-cols-2 gap-3">
        <FieldShell label="Language">
          <input
            type="text"
            placeholder="any"
            className={inputClass()}
            value={draft.language ?? ""}
            onChange={(e) => setDraft((p) => ({ ...p, language: e.target.value }))}
          />
        </FieldShell>
        <FieldShell label="Scope">
          <input
            type="text"
            placeholder="global"
            className={inputClass()}
            value={draft.scope ?? ""}
            onChange={(e) => setDraft((p) => ({ ...p, scope: e.target.value }))}
          />
        </FieldShell>
      </div>
      <FieldShell label="Priority" hint="Higher values appear earlier in the prompt (default 50).">
        <input
          type="number"
          min={0}
          max={100}
          className={inputClass()}
          value={draft.priority ?? 50}
          onChange={(e) =>
            setDraft((p) => ({
              ...p,
              priority: Number.isFinite(Number(e.target.value)) ? Number(e.target.value) : 50,
            }))
          }
        />
      </FieldShell>
    </EditorSheet>
  );
}

function CorrectionEditor({
  initial,
  onClose,
  onSaved,
}: {
  initial: AsrCorrection;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [draft, setDraft] = useState<AsrCorrection>(initial);
  const [busy, setBusy] = useState(false);
  const isEdit = initial.id != null;

  const submit = async () => {
    if (!draft.wrong.trim() || !draft.correct.trim()) {
      toast.warning("Both wrong and correct text are required");
      return;
    }
    setBusy(true);
    try {
      await asrCorrectionUpsert({
        ...draft,
        wrong: draft.wrong.trim(),
        correct: draft.correct.trim(),
        language: draft.language?.trim() || null,
        scope: draft.scope?.trim() || null,
      });
      toast.success(isEdit ? "Updated" : "Added");
      onSaved();
    } catch (error) {
      toast.error("Failed to save", String(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <EditorSheet
      title={isEdit ? "Edit correction" : "Add correction"}
      subtitle="Replace consistent Whisper mistakes."
      onClose={onClose}
      onSubmit={submit}
      busy={busy}
    >
      <FieldShell label="Wrong (heard)">
        <input
          autoFocus
          type="text"
          className={inputClass()}
          value={draft.wrong}
          onChange={(e) => setDraft((p) => ({ ...p, wrong: e.target.value }))}
        />
      </FieldShell>
      <FieldShell label="Correct (replacement)">
        <input
          type="text"
          className={inputClass()}
          value={draft.correct}
          onChange={(e) => setDraft((p) => ({ ...p, correct: e.target.value }))}
        />
      </FieldShell>
      <div className="grid grid-cols-2 gap-3">
        <FieldShell label="Language">
          <input
            type="text"
            placeholder="any"
            className={inputClass()}
            value={draft.language ?? ""}
            onChange={(e) => setDraft((p) => ({ ...p, language: e.target.value }))}
          />
        </FieldShell>
        <FieldShell label="Scope">
          <input
            type="text"
            placeholder="global"
            className={inputClass()}
            value={draft.scope ?? ""}
            onChange={(e) => setDraft((p) => ({ ...p, scope: e.target.value }))}
          />
        </FieldShell>
      </div>
      <FieldShell label="Confidence (0-1)">
        <input
          type="number"
          min={0}
          max={1}
          step={0.05}
          className={inputClass()}
          value={draft.confidence ?? 0.9}
          onChange={(e) =>
            setDraft((p) => ({
              ...p,
              confidence: Number.isFinite(Number(e.target.value)) ? Number(e.target.value) : 0.9,
            }))
          }
        />
      </FieldShell>
      <div className="flex items-center justify-between rounded-xl border border-fg/10 bg-fg/5 px-3 py-2.5">
        <div>
          <div className="text-sm font-semibold text-fg">User approved</div>
          <div className="text-[11px] text-fg/50">Approved corrections take precedence.</div>
        </div>
        <Switch
          id="asr-correction-approved"
          checked={!!draft.userApproved}
          onChange={(next) => setDraft((p) => ({ ...p, userApproved: next }))}
        />
      </div>
    </EditorSheet>
  );
}

function VoiceExampleEditor({
  initial,
  onClose,
  onSaved,
  activeModelPath,
}: {
  initial: AsrVoiceExample;
  onClose: () => void;
  onSaved: () => void;
  activeModelPath: string;
}) {
  const [draft, setDraft] = useState<AsrVoiceExample>(initial);
  const [busy, setBusy] = useState(false);
  const [suggestion, setSuggestion] = useState<AsrLearnedSuggestion | null>(null);
  const [suggesting, setSuggesting] = useState(false);
  const [transcribing, setTranscribing] = useState(false);
  const [isRecording, setIsRecording] = useState(false);
  const [recordingElapsedMs, setRecordingElapsedMs] = useState(0);
  const [savingRecording, setSavingRecording] = useState(false);
  const recorderRef = useRef<RecorderSession | null>(null);
  const isEdit = initial.id != null;

  useEffect(() => {
    if (!isRecording) return;
    const timer = window.setInterval(() => {
      const startedAt = recorderRef.current?.startedAt;
      if (startedAt) setRecordingElapsedMs(Date.now() - startedAt);
    }, 200);
    return () => window.clearInterval(timer);
  }, [isRecording]);

  useEffect(() => {
    return () => {
      const session = recorderRef.current;
      if (!session) return;
      try {
        session.processor.disconnect();
        session.source.disconnect();
        session.stream.getTracks().forEach((t) => t.stop());
        void session.audioContext.close();
      } catch {}
      recorderRef.current = null;
    };
  }, []);

  const startRecording = async () => {
    if (isRecording || savingRecording) return;
    try {
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: micConstraintsWithStoredDevice({
          channelCount: 1,
          echoCancellation: true,
          noiseSuppression: true,
          autoGainControl: true,
        }),
      });
      const audioContext = new AudioContext();
      const source = audioContext.createMediaStreamSource(stream);
      const processor = audioContext.createScriptProcessor(4096, 1, 1);
      const chunks: Float32Array[] = [];
      processor.onaudioprocess = (event) => {
        chunks.push(new Float32Array(event.inputBuffer.getChannelData(0)));
      };
      source.connect(processor);
      processor.connect(audioContext.destination);
      recorderRef.current = {
        stream,
        audioContext,
        source,
        processor,
        chunks,
        sampleRate: audioContext.sampleRate,
        startedAt: Date.now(),
      };
      setRecordingElapsedMs(0);
      setIsRecording(true);
    } catch (error) {
      console.error("Failed to start voice example recording:", error);
      toast.error("Microphone access failed", String(error));
    }
  };

  const stopRecording = async () => {
    const session = recorderRef.current;
    if (!session) return;
    recorderRef.current = null;
    setIsRecording(false);
    try {
      session.processor.disconnect();
      session.source.disconnect();
      session.stream.getTracks().forEach((t) => t.stop());
      await session.audioContext.close();
    } catch (err) {
      console.warn("Failed to cleanly stop recorder:", err);
    }
    const merged = mergeFloat32Chunks(session.chunks);
    if (merged.length === 0) {
      toast.warning("No audio captured");
      return;
    }
    setSavingRecording(true);
    try {
      const wav = encodeWavMono16(merged, session.sampleRate);
      const baseAbs = await tempDir();
      const dirAbs = await joinPath(baseAbs, "lettuce-asr-voice-examples");
      await mkdir(dirAbs, { recursive: true });
      const filename = `recording-${Date.now()}.wav`;
      const absolutePath = await joinPath(dirAbs, filename);
      await writeFile(absolutePath, wav);
      setDraft((p) => ({ ...p, audioPath: absolutePath }));
      toast.success("Recording saved", absolutePath);
    } catch (error) {
      console.error("Failed to save voice example recording:", error);
      toast.error("Failed to save recording", String(error));
    } finally {
      setSavingRecording(false);
    }
  };

  const cancelRecording = () => {
    const session = recorderRef.current;
    if (!session) return;
    recorderRef.current = null;
    try {
      session.processor.disconnect();
      session.source.disconnect();
      session.stream.getTracks().forEach((t) => t.stop());
      void session.audioContext.close();
    } catch (err) {
      console.warn("Failed to cancel recorder:", err);
    }
    setIsRecording(false);
    setRecordingElapsedMs(0);
  };

  const transcribeAudio = async () => {
    if (!draft.audioPath.trim()) {
      toast.warning("Pick an audio file first");
      return;
    }
    if (!activeModelPath) {
      toast.warning("No active Whisper model", "Choose a model from the active model card first.");
      return;
    }
    setTranscribing(true);
    try {
      const result = await asrWhisperTranscribeFile({
        modelPath: activeModelPath,
        audioPath: draft.audioPath.trim(),
        language: draft.language?.trim() || null,
        scopes: draft.scope?.trim() ? [draft.scope.trim()] : null,
      });
      const text = result.rawText || result.correctedText || "";
      setDraft((p) => ({ ...p, whisperOutput: text }));
      toast.success("Transcribed", text || "Whisper returned no text");
    } catch (error) {
      console.error("Failed to transcribe audio:", error);
      toast.error("Transcription failed", String(error));
    } finally {
      setTranscribing(false);
    }
  };

  const canSuggest = useMemo(
    () => Boolean(draft.expectedText.trim() && draft.whisperOutput?.trim()),
    [draft.expectedText, draft.whisperOutput],
  );

  const pickAudio = async () => {
    try {
      const path = await openDialog({
        title: "Select audio file",
        multiple: false,
        filters: [{ name: "Audio", extensions: ["wav", "mp3", "flac", "m4a", "ogg"] }],
      });
      if (path && typeof path === "string") {
        setDraft((p) => ({ ...p, audioPath: path }));
      }
    } catch (error) {
      toast.error("Failed to pick file", String(error));
    }
  };

  const fetchSuggestion = async () => {
    if (!canSuggest) return;
    setSuggesting(true);
    try {
      const result = await asrVoiceExampleSuggestCorrection({
        whisperOutput: draft.whisperOutput ?? "",
        expectedText: draft.expectedText,
        language: draft.language ?? null,
        scope: draft.scope ?? null,
      });
      setSuggestion(result);
      if (!result) {
        toast.info("No suggested correction", "Whisper output is close enough to expected text.");
      }
    } catch (error) {
      toast.error("Suggestion failed", String(error));
    } finally {
      setSuggesting(false);
    }
  };

  const promoteSuggestion = async () => {
    if (!suggestion) return;
    setBusy(true);
    try {
      const correction = await asrCorrectionUpsert({
        wrong: suggestion.wrong,
        correct: suggestion.correct,
        confidence: suggestion.confidence,
        language: suggestion.language ?? null,
        scope: suggestion.scope,
        userApproved: true,
      });
      setDraft((p) => ({ ...p, correctionId: correction.id ?? null }));
      toast.success("Saved as approved correction");
      setSuggestion(null);
    } catch (error) {
      toast.error("Failed to save correction", String(error));
    } finally {
      setBusy(false);
    }
  };

  const submit = async () => {
    if (!draft.audioPath.trim() || !draft.expectedText.trim()) {
      toast.warning("Audio path and expected text are required");
      return;
    }
    setBusy(true);
    try {
      await asrVoiceExampleUpsert({
        ...draft,
        audioPath: draft.audioPath.trim(),
        expectedText: draft.expectedText.trim(),
        whisperOutput: draft.whisperOutput?.trim() || null,
        language: draft.language?.trim() || null,
        scope: draft.scope?.trim() || null,
      });
      toast.success(isEdit ? "Updated" : "Added");
      onSaved();
    } catch (error) {
      toast.error("Failed to save", String(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <EditorSheet
      title={isEdit ? "Edit voice example" : "Add voice example"}
      subtitle="Pair an audio clip with the text it should produce."
      onClose={onClose}
      onSubmit={submit}
      busy={busy}
    >
      <FieldShell
        label="Audio path"
        hint="Pick an existing audio file, or record a new clip and we'll save a WAV under the app's data directory."
      >
        <div className="space-y-2">
          {isRecording ? (
            <div className="flex items-center justify-between gap-3 rounded-lg border border-danger/25 bg-danger/8 px-3 py-2">
              <div className="flex items-center gap-2">
                <span className="relative flex h-2 w-2">
                  <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-danger opacity-60" />
                  <span className="relative inline-flex h-2 w-2 rounded-full bg-danger" />
                </span>
                <span className="text-sm font-semibold text-danger">
                  Recording {formatDuration(recordingElapsedMs)}
                </span>
              </div>
              <div className="flex gap-2">
                <button type="button" className={ghostButton()} onClick={cancelRecording}>
                  <X className="h-3.5 w-3.5" />
                  Discard
                </button>
                <button
                  type="button"
                  className={accentButton()}
                  onClick={() => void stopRecording()}
                >
                  <CheckCircle2 className="h-3.5 w-3.5" />
                  Stop & save
                </button>
              </div>
            </div>
          ) : (
            <div className="flex gap-2">
              <input
                type="text"
                className={inputClass("flex-1")}
                value={draft.audioPath}
                placeholder="/path/to/clip.wav"
                onChange={(e) => setDraft((p) => ({ ...p, audioPath: e.target.value }))}
                disabled={savingRecording}
              />
              <button
                type="button"
                className={ghostButton()}
                onClick={pickAudio}
                disabled={savingRecording}
              >
                <Folder className="h-3.5 w-3.5" />
                Browse
              </button>
              <button
                type="button"
                className={ghostButton()}
                onClick={() => void startRecording()}
                disabled={savingRecording}
              >
                {savingRecording ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <Mic className="h-3.5 w-3.5" />
                )}
                {savingRecording ? "Saving..." : "Record"}
              </button>
            </div>
          )}
        </div>
      </FieldShell>
      <FieldShell label="Expected text" hint="What the recording should transcribe to.">
        <textarea
          rows={2}
          className={inputClass("resize-none")}
          value={draft.expectedText}
          onChange={(e) => setDraft((p) => ({ ...p, expectedText: e.target.value }))}
        />
      </FieldShell>
      <FieldShell
        label="Whisper output"
        hint="Optional. Paste the raw transcription, or run Whisper on the file above."
      >
        <div className="space-y-2">
          <textarea
            rows={2}
            className={inputClass("resize-none")}
            value={draft.whisperOutput ?? ""}
            onChange={(e) => setDraft((p) => ({ ...p, whisperOutput: e.target.value }))}
          />
          <div className="flex justify-end">
            <button
              type="button"
              onClick={() => void transcribeAudio()}
              disabled={transcribing || !draft.audioPath.trim() || !activeModelPath}
              className={cn(
                ghostButton(),
                (transcribing || !draft.audioPath.trim() || !activeModelPath) &&
                  "cursor-not-allowed opacity-60",
              )}
              title={
                !activeModelPath
                  ? "Pick an active Whisper model first"
                  : !draft.audioPath.trim()
                    ? "Pick an audio file first"
                    : undefined
              }
            >
              {transcribing ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <Wand2 className="h-3.5 w-3.5" />
              )}
              {transcribing ? "Transcribing..." : "Transcribe with active model"}
            </button>
          </div>
        </div>
      </FieldShell>
      <div className="grid grid-cols-2 gap-3">
        <FieldShell label="Language">
          <input
            type="text"
            placeholder="any"
            className={inputClass()}
            value={draft.language ?? ""}
            onChange={(e) => setDraft((p) => ({ ...p, language: e.target.value }))}
          />
        </FieldShell>
        <FieldShell label="Scope">
          <input
            type="text"
            placeholder="global"
            className={inputClass()}
            value={draft.scope ?? ""}
            onChange={(e) => setDraft((p) => ({ ...p, scope: e.target.value }))}
          />
        </FieldShell>
      </div>

      <div className="rounded-xl border border-fg/10 bg-fg/5 px-3 py-3">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0">
            <div className="text-sm font-semibold text-fg">Suggest correction</div>
            <div className="mt-0.5 text-[11px] text-fg/50">
              Diffs Whisper output against expected text and proposes a correction pair.
            </div>
          </div>
          <button
            type="button"
            disabled={!canSuggest || suggesting}
            onClick={fetchSuggestion}
            className={cn(
              ghostButton(),
              (!canSuggest || suggesting) && "cursor-not-allowed opacity-60",
            )}
          >
            {suggesting ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <Sparkles className="h-3.5 w-3.5" />
            )}
            Preview
          </button>
        </div>
        {suggestion && (
          <motion.div
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            className="mt-3 rounded-lg border border-accent/30 bg-accent/10 px-3 py-2.5"
          >
            <div className="flex flex-wrap items-center gap-2 text-sm">
              <span className="font-semibold text-danger/80 line-through decoration-danger/40">
                {suggestion.wrong}
              </span>
              <span className="text-fg/40">→</span>
              <span className="font-semibold text-fg">{suggestion.correct}</span>
              <span className="ml-auto text-[10px] font-bold tabular-nums text-fg/45">
                conf {suggestion.confidence.toFixed(2)}
              </span>
            </div>
            <div className="mt-2 flex justify-end">
              <button
                type="button"
                onClick={() => void promoteSuggestion()}
                className={accentButton()}
                disabled={busy}
              >
                <CheckCircle2 className="h-3.5 w-3.5" />
                Save as approved
              </button>
            </div>
          </motion.div>
        )}
      </div>
    </EditorSheet>
  );
}
