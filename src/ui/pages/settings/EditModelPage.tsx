import { AnimatePresence, motion } from "framer-motion";
import { useState, useEffect, useMemo, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import {
  ADVANCED_TEMPERATURE_RANGE,
  ADVANCED_TOP_P_RANGE,
  ADVANCED_MAX_TOKENS_RANGE,
  ADVANCED_CONTEXT_LENGTH_RANGE,
  ADVANCED_FREQUENCY_PENALTY_RANGE,
  ADVANCED_PRESENCE_PENALTY_RANGE,
  ADVANCED_TOP_K_RANGE,
  ADVANCED_SD_CFG_SCALE_RANGE,
  ADVANCED_SD_DENOISING_STRENGTH_RANGE,
  ADVANCED_SD_SEED_RANGE,
  ADVANCED_SD_STEPS_RANGE,
  ADVANCED_REASONING_BUDGET_RANGE,
  ADVANCED_LLAMA_GPU_LAYERS_RANGE,
  ADVANCED_LLAMA_THREADS_RANGE,
  ADVANCED_LLAMA_THREADS_BATCH_RANGE,
  ADVANCED_LLAMA_SEED_RANGE,
  ADVANCED_LLAMA_ROPE_FREQ_BASE_RANGE,
  ADVANCED_LLAMA_ROPE_FREQ_SCALE_RANGE,
  ADVANCED_LLAMA_BATCH_SIZE_RANGE,
  ADVANCED_LLAMA_DRY_MULTIPLIER_RANGE,
  ADVANCED_LLAMA_DRY_BASE_RANGE,
  ADVANCED_LLAMA_DRY_ALLOWED_LENGTH_RANGE,
  ADVANCED_LLAMA_DRY_PENALTY_LAST_N_RANGE,
  ADVANCED_OLLAMA_NUM_CTX_RANGE,
  ADVANCED_OLLAMA_NUM_PREDICT_RANGE,
  ADVANCED_OLLAMA_NUM_KEEP_RANGE,
  ADVANCED_OLLAMA_NUM_BATCH_RANGE,
  ADVANCED_OLLAMA_NUM_GPU_RANGE,
  ADVANCED_OLLAMA_NUM_THREAD_RANGE,
  ADVANCED_OLLAMA_TFS_Z_RANGE,
  ADVANCED_OLLAMA_TYPICAL_P_RANGE,
  ADVANCED_OLLAMA_MIN_P_RANGE,
  ADVANCED_OLLAMA_MIROSTAT_TAU_RANGE,
  ADVANCED_OLLAMA_MIROSTAT_ETA_RANGE,
  ADVANCED_OLLAMA_REPEAT_PENALTY_RANGE,
  ADVANCED_OLLAMA_SEED_RANGE,
} from "../../components/AdvancedModelSettingsForm";
import { BottomMenu, MenuButton, MenuSection } from "../../components/BottomMenu";
import { ModelSelectionBottomMenu } from "../../components/ModelSelectionBottomMenu";
import { NumberInput } from "../../components/NumberInput";
import {
  Info,
  Brain,
  RefreshCw,
  Check,
  ChevronDown,
  ChevronRight,
  HelpCircle,
  AlertTriangle,
  FolderOpen,
  Loader,
  HardDrive,
  ArrowRight,
  CopyCheck,
  Copy,
  Maximize2,
  SendHorizontal,
  X,
} from "lucide-react";
import { ProviderParameterSupportInfo } from "../../components/ProviderParameterSupportInfo";
import { LlamaSamplerOrderEditor } from "../../components/LlamaSamplerOrderEditor";
import { toast } from "../../components/toast";
import { useModelEditorController } from "./hooks/useModelEditorController";
import { useNavigationManager } from "../../navigation";
import { useSearchParams, useNavigate } from "react-router-dom";
import { addOrUpdateModel } from "../../../core/storage/repo";
import type { LlamaLastRuntimeReport, ReasoningSupport } from "../../../core/storage/schemas";
import {
  getProviderReasoningSupport,
  getProviderCachingSupport,
  isGeminiFamilyProvider,
} from "../../../core/storage/schemas";
import { getProviderIcon } from "../../../core/utils/providerIcons";
import {
  sdImportModel,
  sdListLocalFiles,
  sdListModels,
  sdSetModelFile,
  type SdLocalFile,
  type SdModelEntry,
  type SdModelRole,
} from "../../../core/local-diffusion";
import { cn } from "../../design-tokens";
import { openDocs } from "../../../core/utils/docs";
import { useI18n, type TranslationKey } from "../../../core/i18n/context";
import { Switch } from "../../components/Switch";

type DownloadedGgufModel = {
  modelId: string;
  filename: string;
  path: string;
  size: number;
  quantization: string;
  isMmproj?: boolean;
  isMtp?: boolean;
};

type LocalLibraryPickerMode = "model" | "mmproj" | "mtp";

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const value = bytes / Math.pow(1024, i);
  return `${value.toFixed(i > 1 ? 1 : 0)} ${units[i]}`;
}

function deriveDisplayNameFromPath(path: string): string {
  const filename = path.split(/[/\\]/).filter(Boolean).pop() || path;
  return filename
    .replace(/\.gguf$/i, "")
    .replace(/[-_]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

type LlamaCppContextInfo = {
  maxContextLength: number;
  recommendedContextLength?: number | null;
  availableMemoryBytes?: number | null;
  availableVramBytes?: number | null;
  modelSizeBytes?: number | null;
  layerCount?: number | null;
  supportsGpuOffload?: boolean | null;
};

function formatRuntimeNumber(value?: number | null): string | null {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return null;
  }
  return new Intl.NumberFormat("en-US").format(value);
}

function formatRuntimeDate(value?: number | null): string | null {
  if (!value || !Number.isFinite(value)) {
    return null;
  }
  try {
    return new Date(value).toLocaleString();
  } catch {
    return null;
  }
}

function formatRuntimeRate(value?: number | null): string | null {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return null;
  }
  return `${value.toFixed(value >= 10 ? 1 : 2)} tok/s`;
}

function getLlamaRuntimeHeadlineKey(
  report: LlamaLastRuntimeReport,
):
  | "editModel.runtime.headline.succeeded"
  | "editModel.runtime.headline.cpuFallbackSucceeded"
  | "editModel.runtime.headline.cpuFallbackFailed"
  | "editModel.runtime.headline.failed" {
  if (report.status === "succeeded") return "editModel.runtime.headline.succeeded";
  if (report.status === "cpuFallbackSucceeded") {
    return "editModel.runtime.headline.cpuFallbackSucceeded";
  }
  if (report.status === "cpuFallbackFailed") {
    return "editModel.runtime.headline.cpuFallbackFailed";
  }
  return "editModel.runtime.headline.failed";
}

function getLlamaRuntimeDetailKey(
  report: LlamaLastRuntimeReport,
):
  | "editModel.runtime.detail.succeeded"
  | "editModel.runtime.detail.cpuFallbackSucceeded"
  | "editModel.runtime.detail.cpuFallbackFailed"
  | "editModel.runtime.detail.failed" {
  if (report.status === "succeeded") return "editModel.runtime.detail.succeeded";
  if (report.status === "cpuFallbackSucceeded") {
    return "editModel.runtime.detail.cpuFallbackSucceeded";
  }
  if (report.status === "cpuFallbackFailed") {
    return "editModel.runtime.detail.cpuFallbackFailed";
  }
  return "editModel.runtime.detail.failed";
}

type EditorSectionKey =
  | "generation"
  | "runtime"
  | "configuration"
  | "reasoning"
  | "caching"
  | "capabilities";

const EDITOR_FADE_DURATION = 0.16;

const LLAMA_KV_TYPE_OPTIONS = [
  { value: "auto", labelKey: "editModel.llamaKvType.auto" },
  { value: "f16", labelKey: "editModel.llamaKvType.f16" },
  { value: "q8_0", labelKey: "editModel.llamaKvType.q8_0" },
  { value: "q8_1", labelKey: "editModel.llamaKvType.q8_1" },
  { value: "q6_k", labelKey: "editModel.llamaKvType.q6_k" },
  { value: "q5_k", labelKey: "editModel.llamaKvType.q5_k" },
  { value: "q5_1", labelKey: "editModel.llamaKvType.q5_1" },
  { value: "q5_0", labelKey: "editModel.llamaKvType.q5_0" },
  { value: "q4_k", labelKey: "editModel.llamaKvType.q4_k" },
  { value: "q4_1", labelKey: "editModel.llamaKvType.q4_1" },
  { value: "q4_0", labelKey: "editModel.llamaKvType.q4_0" },
  { value: "q3_k", labelKey: "editModel.llamaKvType.q3_k" },
  { value: "q2_k", labelKey: "editModel.llamaKvType.q2_k" },
] satisfies ReadonlyArray<{ value: string; labelKey: TranslationKey }>;

const LLAMA_CHAT_TEMPLATE_PRESET_OPTIONS = [
  { value: "auto", labelKey: "editModel.chatTemplatePreset.auto" },
  { value: "chatml", labelKey: "editModel.chatTemplatePreset.chatml" },
  { value: "llama2", labelKey: "editModel.chatTemplatePreset.llama2" },
  { value: "llama3", labelKey: "editModel.chatTemplatePreset.llama3" },
  { value: "mistral-v1", labelKey: "editModel.chatTemplatePreset.mistralV1" },
  { value: "vicuna", labelKey: "editModel.chatTemplatePreset.vicuna" },
  { value: "gemma", labelKey: "editModel.chatTemplatePreset.gemma" },
] satisfies ReadonlyArray<{ value: string; labelKey: TranslationKey }>;

const LLAMA_SAMPLER_PROFILE_OPTIONS = [
  { value: "balanced", labelKey: "editModel.samplerProfile.balanced" },
  { value: "creative", labelKey: "editModel.samplerProfile.creative" },
  { value: "stable", labelKey: "editModel.samplerProfile.stable" },
  { value: "reasoning", labelKey: "editModel.samplerProfile.reasoning" },
] satisfies ReadonlyArray<{ value: string; labelKey: TranslationKey }>;

const LLAMA_QUICK_PRESET_DETAILS = {
  balanced: [
    "editModel.quickPresetDetails.batchSize512",
    "editModel.quickPresetDetails.kvCacheQ8_0",
    "editModel.quickPresetDetails.offloadKqvOn",
    "editModel.quickPresetDetails.flashAttentionAuto",
  ],
  throughput: [
    "editModel.quickPresetDetails.batchSize1024",
    "editModel.quickPresetDetails.kvCacheF16",
    "editModel.quickPresetDetails.offloadKqvOn",
    "editModel.quickPresetDetails.flashAttentionEnabled",
  ],
  vram: [
    "editModel.quickPresetDetails.batchSize512",
    "editModel.quickPresetDetails.kvCacheQ4_k",
    "editModel.quickPresetDetails.offloadKqvOn",
    "editModel.quickPresetDetails.flashAttentionEnabled",
  ],
  cpu_ram: [
    "editModel.quickPresetDetails.batchSize256",
    "editModel.quickPresetDetails.kvCacheQ8_0",
    "editModel.quickPresetDetails.offloadKqvOff",
    "editModel.quickPresetDetails.flashAttentionAuto",
  ],
} satisfies Record<string, ReadonlyArray<TranslationKey>>;

const LLAMA_SAMPLER_PROFILE_DETAILS = {
  balanced: [
    "editModel.samplerProfileDetails.temp080",
    "editModel.samplerProfileDetails.topP095",
    "editModel.samplerProfileDetails.topK40",
    "editModel.samplerProfileDetails.minP005",
    "editModel.samplerProfileDetails.freqPen015",
  ],
  creative: [
    "editModel.samplerProfileDetails.temp095",
    "editModel.samplerProfileDetails.topP098",
    "editModel.samplerProfileDetails.topK80",
    "editModel.samplerProfileDetails.minP002",
    "editModel.samplerProfileDetails.presencePen025",
  ],
  stable: [
    "editModel.samplerProfileDetails.temp055",
    "editModel.samplerProfileDetails.topP090",
    "editModel.samplerProfileDetails.topK32",
    "editModel.samplerProfileDetails.minP008",
    "editModel.samplerProfileDetails.typicalP097",
  ],
  reasoning: [
    "editModel.samplerProfileDetails.temp035",
    "editModel.samplerProfileDetails.topP090",
    "editModel.samplerProfileDetails.topK24",
    "editModel.samplerProfileDetails.typicalP095",
    "editModel.samplerProfileDetails.freqPen010",
  ],
} satisfies Record<string, ReadonlyArray<TranslationKey>>;

const normalizeSearchText = (value?: string) =>
  (value ?? "")
    .toLowerCase()
    .replace(/[_:/.-]+/g, " ")
    .replace(/[^a-z0-9\s]/g, "")
    .replace(/\s+/g, " ")
    .trim();

const joinStringList = (value?: string[] | null) => (value?.length ? value.join(", ") : "");

const getEditDistance = (a: string, b: string) => {
  if (a === b) return 0;
  if (!a.length) return b.length;
  if (!b.length) return a.length;

  const rows = a.length + 1;
  const cols = b.length + 1;
  const dp = Array.from({ length: rows }, (_, i) => {
    const row = new Array<number>(cols).fill(0);
    row[0] = i;
    return row;
  });
  for (let j = 0; j < cols; j++) dp[0][j] = j;

  for (let i = 1; i < rows; i++) {
    for (let j = 1; j < cols; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      dp[i][j] = Math.min(dp[i - 1][j] + 1, dp[i][j - 1] + 1, dp[i - 1][j - 1] + cost);
    }
  }
  return dp[rows - 1][cols - 1];
};

function FieldBlock({
  label,
  action,
  children,
}: {
  label: string;
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="space-y-2.5">
      <div className="flex h-9 items-center justify-between gap-3">
        <label className="text-[13px] font-medium text-fg/72">{label}</label>
        {action ? <div className="shrink-0">{action}</div> : null}
      </div>
      {children}
    </div>
  );
}


export function EditModelPage() {
  const { t } = useI18n();
  const [showParameterSupport, setShowParameterSupport] = useState(false);
  const [isManualInput, setIsManualInput] = useState(false);
  const [showModelSelector, setShowModelSelector] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [debouncedSearchQuery, setDebouncedSearchQuery] = useState("");
  const [showOnlyFreeModels, setShowOnlyFreeModels] = useState(false);
  const [activePanel, setActivePanel] = useState<EditorSectionKey>("generation");
  const [selectedLlamaQuickPreset, setSelectedLlamaQuickPreset] = useState<
    "balanced" | "throughput" | "vram" | "cpu_ram" | null
  >(null);
  const [showPlatformSelector, setShowPlatformSelector] = useState(false);
  const [sdEntries, setSdEntries] = useState<SdModelEntry[] | null>(null);
  const [showSdModelPicker, setShowSdModelPicker] = useState(false);
  const [sdFilesDraft, setSdFilesDraft] = useState<Record<string, string>>({});
  const [sdMainPathDraft, setSdMainPathDraft] = useState("");
  const [sdLibraryFiles, setSdLibraryFiles] = useState<SdLocalFile[] | null>(null);
  const [sdLibraryRole, setSdLibraryRole] = useState<SdModelRole | null>(null);
  const [llamaContextInfo, setLlamaContextInfo] = useState<LlamaCppContextInfo | null>(null);
  const [llamaContextError, setLlamaContextError] = useState<string | null>(null);
  const [llamaContextLoading, setLlamaContextLoading] = useState(false);
  const [showLocalModelPicker, setShowLocalModelPicker] = useState(false);
  const [localLibraryPickerMode, setLocalLibraryPickerMode] =
    useState<LocalLibraryPickerMode>("model");
  const [downloadedModels, setDownloadedModels] = useState<DownloadedGgufModel[]>([]);
  const [loadingDownloaded, setLoadingDownloaded] = useState(false);
  const [ggufModelsDir, setGgufModelsDir] = useState<string | null>(null);
  const [showMovePrompt, setShowMovePrompt] = useState(false);
  const [movePromptSource, setMovePromptSource] = useState<"save" | "browse">("save");
  const [movingModel, setMovingModel] = useState(false);
  const [moveError, setMoveError] = useState<string | null>(null);
  const [movePromptPath, setMovePromptPath] = useState<string | null>(null);
  const [skippedMovePromptPath, setSkippedMovePromptPath] = useState<string | null>(null);
  const [pendingReturnAfterMovePrompt, setPendingReturnAfterMovePrompt] = useState<string | null>(
    null,
  );
  const [showLlamaRuntimeReport, setShowLlamaRuntimeReport] = useState(false);
  const [showTemplateOverlay, setShowTemplateOverlay] = useState(false);
  const [templateOverlayDraft, setTemplateOverlayDraft] = useState("");
  const [showEmbeddedTemplateViewer, setShowEmbeddedTemplateViewer] = useState(false);
  const [embeddedTemplateLoading, setEmbeddedTemplateLoading] = useState(false);
  const [embeddedTemplateText, setEmbeddedTemplateText] = useState("");
  const [embeddedTemplateError, setEmbeddedTemplateError] = useState<string | null>(null);
  const [runabilityScore, setRunabilityScore] = useState<{
    score: number;
    label: "excellent" | "good" | "marginal" | "poor" | "unrunnable";
    fitsInRam: boolean;
    fitsInVram: boolean;
    memoryScore: number;
    gpuScore: number;
    kvScore: number;
    quantScore: number;
    gpuMode: string;
    availableRam: number;
    availableVram: number;
    modelSize: number;
    quantization: string;
  } | null>(null);
  const [runabilityLoading, setRunabilityLoading] = useState(false);

  const {
    state: {
      loading,
      saving,
      verifying,
      fetchingModels,
      fetchedModels,
      error,
      providers,
      editorModel,
      modelAdvancedDraft,
    },
    isNew,
    canSave,
    hasUnsavedChanges,
    updateEditorModel,
    handleDisplayNameChange,
    handleModelNameChange,
    handleProviderSelection,
    setModelAdvancedDraft,
    handleTemperatureChange,
    handleTopPChange,
    handleMaxTokensChange,
    handleContextLengthChange,
    handleFrequencyPenaltyChange,
    handlePresencePenaltyChange,
    handleTopKChange,
    handleLlamaGpuLayersChange,
    handleLlamaThreadsChange,
    handleLlamaThreadsBatchChange,
    handleLlamaSeedChange,
    handleLlamaRopeFreqBaseChange,
    handleLlamaRopeFreqScaleChange,
    handleLlamaOffloadKqvChange,
    handleLlamaBatchSizeChange,
    handleLlamaKvTypeChange,
    handleLlamaFlashAttentionChange,
    handleLlamaSwaFullChange,
    handleLlamaSamplerProfileChange,
    handleLlamaSamplerOrderChange,
    handleLlamaMinPChange,
    handleLlamaTypicalPChange,
    handleLlamaDryMultiplierChange,
    handleLlamaDryBaseChange,
    handleLlamaDryAllowedLengthChange,
    handleLlamaDryPenaltyLastNChange,
    handleLlamaDrySequenceBreakersChange,
    handleLlamaChatTemplateOverrideChange,
    handleLlamaMmprojPathChange,
    handleLlamaChatTemplatePresetChange,
    handleLlamaRawCompletionFallbackChange,
    handleLlamaStrictModeChange,
    handleLlamaMtpEnabledChange,
    handleLlamaMtpDraftTokensChange,
    handleLlamaMtpModelPathChange,
    handleLlamaStreamingEnabledChange,
    handleOllamaNumCtxChange,
    handleOllamaNumPredictChange,
    handleOllamaNumKeepChange,
    handleOllamaNumBatchChange,
    handleOllamaNumGpuChange,
    handleOllamaNumThreadChange,
    handleOllamaTfsZChange,
    handleOllamaTypicalPChange,
    handleOllamaMinPChange,
    handleOllamaMirostatChange,
    handleOllamaMirostatTauChange,
    handleOllamaMirostatEtaChange,
    handleOllamaRepeatPenaltyChange,
    handleOllamaSeedChange,
    handleOllamaStopChange,
    handleReasoningEnabledChange,
    handleReasoningEffortChange,
    handleReasoningBudgetChange,
    handleForceSendThinkingStateChange,
    handlePromptCachingEnabledChange,
    handlePromptCachingTtlChange,
    applyLlamaRuntimeSuggestion,
    saveModel,
    resetToInitial,
    fetchModels,
  } = useModelEditorController();
  useNavigationManager();
  const editNavigate = useNavigate();
  const [editSearchParams] = useSearchParams();
  const returnTo = editSearchParams.get("returnTo");
  const isOnboardingReturnFlow = !!returnTo?.startsWith("/onboarding");
  const isLocalModel = editorModel?.providerId === "llamacpp";
  const isOllamaModel = editorModel?.providerId === "ollama";
  const llamaRuntimeReport = modelAdvancedDraft.llamaLastRuntimeReport ?? null;
  const llamaRuntimeFacts = useMemo(() => {
    if (!llamaRuntimeReport) {
      return [];
    }
    const fields = [
      [t("editModel.runtimeFacts.updated"), formatRuntimeDate(llamaRuntimeReport.updatedAt)],
      [t("editModel.runtimeFacts.modelPath"), llamaRuntimeReport.modelPath],
      [t("editModel.runtimeFacts.backendUsed"), llamaRuntimeReport.backendPathUsed ?? null],
      [t("editModel.runtimeFacts.failureStage"), llamaRuntimeReport.failureStage ?? null],
      [t("editModel.runtimeFacts.requestedContext"), formatRuntimeNumber(llamaRuntimeReport.requestedContext)],
      [t("editModel.runtimeFacts.recommendedContext"), formatRuntimeNumber(llamaRuntimeReport.recommendedContext)],
      [t("editModel.runtimeFacts.initialContext"), formatRuntimeNumber(llamaRuntimeReport.initialContextCandidate)],
      [t("editModel.runtimeFacts.actualContext"), formatRuntimeNumber(llamaRuntimeReport.actualContextUsed)],
      [t("editModel.runtimeFacts.requestedGpuLayers"), formatRuntimeNumber(llamaRuntimeReport.requestedGpuLayers)],
      [t("editModel.runtimeFacts.actualGpuLayers"), formatRuntimeNumber(llamaRuntimeReport.actualGpuLayersUsed)],
      [t("editModel.runtimeFacts.requestedBatch"), formatRuntimeNumber(llamaRuntimeReport.requestedBatchLimit)],
      [t("editModel.runtimeFacts.initialBatch"), formatRuntimeNumber(llamaRuntimeReport.initialBatchCandidate)],
      [t("editModel.runtimeFacts.actualBatch"), formatRuntimeNumber(llamaRuntimeReport.actualBatchUsed)],
      [
        t("editModel.runtimeFacts.smartOffloadFallback"),
        llamaRuntimeReport.smartGpuLayerFallbackActivated == null
          ? null
          : llamaRuntimeReport.smartGpuLayerFallbackActivated
            ? t("editModel.runtimeFacts.active")
            : t("editModel.runtimeFacts.notNeeded"),
      ],
      [
        t("editModel.runtimeFacts.kqvFallback"),
        llamaRuntimeReport.kqvFallbackActivated == null
          ? null
          : llamaRuntimeReport.kqvFallbackActivated
            ? t("editModel.runtimeFacts.movedToRam")
            : t("editModel.runtimeFacts.notNeeded"),
      ],
      [
        t("editModel.runtimeFacts.smartOffloadEstimate"),
        formatRuntimeNumber(llamaRuntimeReport.smartOffloadEstimatedGpuLayers),
      ],
      [
        t("editModel.runtimeFacts.smartOffloadCandidates"),
        llamaRuntimeReport.smartOffloadCandidateLayers?.length
          ? llamaRuntimeReport.smartOffloadCandidateLayers.join(", ")
          : null,
      ],
      [t("editModel.runtimeFacts.kvCache"), llamaRuntimeReport.actualKvTypeUsed ?? null],
      [t("editModel.runtimeFacts.kqvOffload"), llamaRuntimeReport.actualOffloadKqvMode ?? null],
      [t("editModel.runtimeFacts.flashAttention"), llamaRuntimeReport.flashAttentionPolicy ?? null],
      [
        t("editModel.runtimeFacts.gpuBackends"),
        llamaRuntimeReport.compiledGpuBackends?.length
          ? llamaRuntimeReport.compiledGpuBackends.join(", ")
          : null,
      ],
      [
        t("editModel.runtimeFacts.availableRam"),
        llamaRuntimeReport.availableMemoryBytes
          ? formatBytes(llamaRuntimeReport.availableMemoryBytes)
          : null,
      ],
      [
        t("editModel.runtimeFacts.availableVram"),
        llamaRuntimeReport.availableVramBytes
          ? formatBytes(llamaRuntimeReport.availableVramBytes)
          : null,
      ],
      [
        t("editModel.runtimeFacts.modelSize"),
        llamaRuntimeReport.modelSizeBytes ? formatBytes(llamaRuntimeReport.modelSizeBytes) : null,
      ],
      [t("editModel.runtimeFacts.promptTokens"), formatRuntimeNumber(llamaRuntimeReport.promptTokens)],
      [t("editModel.runtimeFacts.promptPositions"), formatRuntimeNumber(llamaRuntimeReport.promptPositions)],
      [t("editModel.runtimeFacts.targetNewTokens"), formatRuntimeNumber(llamaRuntimeReport.targetNewTokens)],
      [t("editModel.runtimeFacts.completionTokens"), formatRuntimeNumber(llamaRuntimeReport.completionTokens)],
      [t("editModel.runtimeFacts.finishReason"), llamaRuntimeReport.finishReason ?? null],
      [
        t("editModel.runtimeFacts.firstToken"),
        llamaRuntimeReport.firstTokenMs
          ? `${formatRuntimeNumber(llamaRuntimeReport.firstTokenMs)} ms`
          : null,
      ],
      [t("editModel.runtimeFacts.throughput"), formatRuntimeRate(llamaRuntimeReport.tokensPerSecond)],
      [t("editModel.runtimeFacts.promptTemplate"), llamaRuntimeReport.promptTemplateSource ?? null],
    ] as const;
    return fields.filter(([, value]) => value).map(([label, value]) => ({ label, value: value! }));
  }, [llamaRuntimeReport]);
  const handleApplyLlamaRuntimeSuggestion = async () => {
    const applied = await applyLlamaRuntimeSuggestion();
    if (applied) {
      toast.success(
        t("editModel.toasts.runtimeConfigApplied"),
        t("editModel.toasts.runtimeConfigAppliedDescription"),
      );
    }
  };

  // Fetch GGUF models directory path on mount
  useEffect(() => {
    invoke<string>("hf_get_gguf_models_dir")
      .then((dir) => setGgufModelsDir(dir))
      .catch(() => setGgufModelsDir(null));
  }, []);

  // Fetch downloaded GGUF files when a local picker is opened
  const openDownloadedLibraryPicker = async (mode: LocalLibraryPickerMode) => {
    setLocalLibraryPickerMode(mode);
    setShowLocalModelPicker(true);
    setLoadingDownloaded(true);
    try {
      const models = await invoke<DownloadedGgufModel[]>("hf_list_downloaded_models");
      setDownloadedModels(models);
    } catch (err) {
      console.error("Failed to list downloaded models", err);
      setDownloadedModels([]);
    } finally {
      setLoadingDownloaded(false);
    }
  };

  const openLocalModelPicker = async () => openDownloadedLibraryPicker("model");

  const openLocalMmprojPicker = async () => openDownloadedLibraryPicker("mmproj");

  const openLocalMtpPicker = async () => openDownloadedLibraryPicker("mtp");

  const syncImageInputScope = (mmprojPath: string | null) => {
    if (!editorModel) return;
    const currentScopes = (editorModel.inputScopes ?? ["text"]) as Array<
      "text" | "image" | "audio"
    >;
    const hasImageScope = currentScopes.includes("image");

    if (mmprojPath?.trim()) {
      if (!hasImageScope) {
        const nextScopes = [...currentScopes, "image"].filter(
          (scope, index, arr) => arr.indexOf(scope) === index,
        ) as Array<"text" | "image" | "audio">;
        updateEditorModel({
          inputScopes: nextScopes,
        });
      }
      return;
    }

    if (hasImageScope) {
      const nextScopes = currentScopes.filter((scope) => scope !== "image") as Array<
        "text" | "image" | "audio"
      >;
      updateEditorModel({
        inputScopes: nextScopes.length > 0 ? nextScopes : ["text"],
      });
    }
  };

  const handleSelectLocalLibraryFile = (model: DownloadedGgufModel) => {
    if (localLibraryPickerMode === "mmproj") {
      handleLlamaMmprojPathChange(model.path);
      syncImageInputScope(model.path);
    } else if (localLibraryPickerMode === "mtp") {
      handleLlamaMtpModelPathChange(model.path);
    } else {
      handleModelNameChange(model.path);
      if (!editorModel?.displayName?.trim()) {
        const cleanName = deriveDisplayNameFromPath(model.filename);
        handleDisplayNameChange(cleanName);
      }
    }
    setShowLocalModelPicker(false);
  };

  const handleBrowseLocalModel = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "GGUF Model", extensions: ["gguf"] }],
      });

      if (!selected || typeof selected !== "string") return;

      handleModelNameChange(selected);
      if (!editorModel?.displayName?.trim()) {
        handleDisplayNameChange(deriveDisplayNameFromPath(selected));
      }
      if (isPathOutsideGgufDir(selected) && skippedMovePromptPath !== selected) {
        setMovePromptSource("browse");
        setMovePromptPath(selected);
        setPendingReturnAfterMovePrompt(null);
        setMoveError(null);
        setShowMovePrompt(true);
      }
    } catch (error) {
      console.error("Failed to browse for local model", error);
    }
  };

  const highlightedTemplate = useMemo(() => {
    if (!embeddedTemplateText) return null;

    // Tokenize Jinja template into colored spans
    const jinjaKeywords = new Set([
      "if",
      "else",
      "elif",
      "endif",
      "for",
      "endfor",
      "block",
      "endblock",
      "macro",
      "endmacro",
      "set",
      "extends",
      "include",
      "import",
      "from",
      "not",
      "and",
      "or",
      "in",
      "is",
      "true",
      "false",
      "none",
      "True",
      "False",
      "None",
      "namespace",
    ]);

    const escapeHtml = (s: string) =>
      s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

    // Highlight the inside of a Jinja tag (block or expression)
    const highlightTagInner = (inner: string): string => {
      // Match strings, keywords, numbers, pipes/operators, and plain text
      return inner.replace(
        /("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')|(\b\d+(?:\.\d+)?\b)|(\b[a-zA-Z_]\w*\b)|([|~%^!=<>]+)|([^"'a-zA-Z_0-9|~%^!=<>]+)/g,
        (match, str, num, word, op, rest) => {
          if (str) return `<span style="color:#a8db8a">${escapeHtml(str)}</span>`;
          if (num) return `<span style="color:#d4976c">${escapeHtml(num)}</span>`;
          if (word) {
            if (jinjaKeywords.has(word))
              return `<span style="color:#c792ea;font-weight:500">${escapeHtml(word)}</span>`;
            return `<span style="color:#82aaff">${escapeHtml(word)}</span>`;
          }
          if (op) return `<span style="color:#89ddff">${escapeHtml(op)}</span>`;
          return escapeHtml(rest ?? match);
        },
      );
    };

    // Main pass: split by Jinja tags, then by plain-text tokens
    const parts: string[] = [];
    let cursor = 0;
    // Match {# ... #}, {% ... %}, {{ ... }}
    const tagRegex = /\{#[\s\S]*?#\}|\{%[-+]?[\s\S]*?[-+]?%\}|\{\{[\s\S]*?\}\}/g;
    let m: RegExpExecArray | null;

    while ((m = tagRegex.exec(embeddedTemplateText)) !== null) {
      // Plain text before this tag
      if (m.index > cursor) {
        parts.push(
          `<span style="color:rgba(255,255,255,0.55)">${escapeHtml(embeddedTemplateText.slice(cursor, m.index))}</span>`,
        );
      }

      const tag = m[0];
      if (tag.startsWith("{#")) {
        // Comment
        parts.push(
          `<span style="color:rgba(255,255,255,0.25);font-style:italic">${escapeHtml(tag)}</span>`,
        );
      } else if (tag.startsWith("{%")) {
        // Block tag
        const delimiters = tag.match(/^(\{%[-+]?)([\s\S]*)([-+]?%\})$/);
        if (delimiters) {
          parts.push(`<span style="color:#c792ea">${escapeHtml(delimiters[1])}</span>`);
          parts.push(highlightTagInner(delimiters[2]));
          parts.push(`<span style="color:#c792ea">${escapeHtml(delimiters[3])}</span>`);
        } else {
          parts.push(`<span style="color:#c792ea">${escapeHtml(tag)}</span>`);
        }
      } else {
        // Expression {{ ... }}
        const delimiters = tag.match(/^(\{\{)([\s\S]*)(\}\})$/);
        if (delimiters) {
          parts.push(`<span style="color:#89ddff">${escapeHtml(delimiters[1])}</span>`);
          parts.push(highlightTagInner(delimiters[2]));
          parts.push(`<span style="color:#89ddff">${escapeHtml(delimiters[3])}</span>`);
        } else {
          parts.push(`<span style="color:#89ddff">${escapeHtml(tag)}</span>`);
        }
      }
      cursor = m.index + tag.length;
    }

    // Remaining plain text
    if (cursor < embeddedTemplateText.length) {
      parts.push(
        `<span style="color:rgba(255,255,255,0.55)">${escapeHtml(embeddedTemplateText.slice(cursor))}</span>`,
      );
    }

    return parts.join("");
  }, [embeddedTemplateText]);

  const toggleEmbeddedTemplate = async () => {
    if (showEmbeddedTemplateViewer) {
      setShowEmbeddedTemplateViewer(false);
      return;
    }

    if (!editorModel?.name?.trim()) {
      toast.warning(
        t("editModel.toasts.modelPathRequired"),
        t("editModel.toasts.modelPathRequiredDescription"),
      );
      return;
    }

    setShowEmbeddedTemplateViewer(true);
    setEmbeddedTemplateLoading(true);
    setEmbeddedTemplateError(null);
    setEmbeddedTemplateText("");

    try {
      const template = await invoke<string>("llamacpp_embedded_chat_template", {
        modelPath: editorModel.name.trim(),
      });
      setEmbeddedTemplateText(template);
    } catch (error) {
      setEmbeddedTemplateError(typeof error === "string" ? error : String(error));
    } finally {
      setEmbeddedTemplateLoading(false);
    }
  };

  const handleUseEmbeddedTemplate = () => {
    if (!embeddedTemplateText.trim()) return;
    setTemplateOverlayDraft(embeddedTemplateText);
    setShowEmbeddedTemplateViewer(false);
    toast.success(t("editModel.toasts.embeddedTemplatePasted"));
  };

  const openTemplateOverlay = () => {
    setTemplateOverlayDraft(modelAdvancedDraft.llamaChatTemplateOverride ?? "");
    setShowTemplateOverlay(true);
  };

  const saveTemplateOverlay = () => {
    handleLlamaChatTemplateOverrideChange(
      templateOverlayDraft.trim() === "" ? null : templateOverlayDraft,
    );
    setShowTemplateOverlay(false);
    setShowEmbeddedTemplateViewer(false);
  };

  const cancelTemplateOverlay = () => {
    setShowTemplateOverlay(false);
    setShowEmbeddedTemplateViewer(false);
  };

  // Check if a path is outside the GGUF models dir
  const isPathOutsideGgufDir = (path: string): boolean => {
    if (!ggufModelsDir || !path.trim()) return false;
    return !path.startsWith(ggufModelsDir);
  };

  // Intercept save for llamacpp models to check if move prompt is needed
  const handleSaveWithMoveCheck = async (options?: { navigateAfterSave?: boolean }) => {
    const shouldNavigateAfterSave = options?.navigateAfterSave && !!returnTo;
    if (!isLocalModel || !editorModel?.name?.trim()) {
      const success = await saveModel();
      if (success && shouldNavigateAfterSave) {
        editNavigate(returnTo!);
      }
      return;
    }

    const modelPath = editorModel.name.trim();
    if (!isPathOutsideGgufDir(modelPath) || skippedMovePromptPath === modelPath) {
      const success = await saveModel();
      if (success && shouldNavigateAfterSave) {
        editNavigate(returnTo!);
      }
      return;
    }

    // Save without navigating, then show the move prompt
    const success = await saveModel();
    if (success) {
      setMovePromptSource("save");
      setMovePromptPath(modelPath);
      setPendingReturnAfterMovePrompt(shouldNavigateAfterSave ? returnTo! : null);
      setSkippedMovePromptPath(null);
      setMoveError(null);
      setShowMovePrompt(true);
    }
  };

  const handleMoveToLibrary = async () => {
    if (!editorModel?.name?.trim()) return;
    setMovingModel(true);
    setMoveError(null);
    try {
      // Unload llama.cpp first so the file isn't locked
      try {
        await invoke("llamacpp_unload");
      } catch {
        // May not be loaded, that's fine
      }

      const newPath = await invoke<string>("hf_move_model_to_gguf_dir", {
        sourcePath: editorModel.name.trim(),
        modelName: editorModel.displayName?.trim() || null,
      });

      if (movePromptSource === "save") {
        await addOrUpdateModel({
          ...editorModel,
          name: newPath,
        });
      } else {
        updateEditorModel({ name: newPath });
      }

      setSkippedMovePromptPath(null);
      const nextReturnTo = pendingReturnAfterMovePrompt;
      setPendingReturnAfterMovePrompt(null);
      setShowMovePrompt(false);
      setMovePromptPath(null);
      if (nextReturnTo) {
        editNavigate(nextReturnTo);
      }
    } catch (err: any) {
      console.error("Failed to move model", err);
      setMoveError(
        typeof err === "string" ? err : err?.message || t("hfBrowser.moveToLibraryFailed"),
      );
    } finally {
      setMovingModel(false);
    }
  };

  const handleSkipMove = () => {
    if (movePromptPath) {
      setSkippedMovePromptPath(movePromptPath);
    }
    const nextReturnTo = pendingReturnAfterMovePrompt;
    setPendingReturnAfterMovePrompt(null);
    setShowMovePrompt(false);
    setMovePromptPath(null);
    if (nextReturnTo) {
      editNavigate(nextReturnTo);
    }
  };
  const selectedProviderCredential =
    editorModel &&
    (providers.find(
      (p) => p.providerId === editorModel.providerId && p.label === editorModel.providerLabel,
    ) ||
      providers.find((p) => p.providerId === editorModel.providerId));
  const modelFetchEnabledForSelectedProvider = (() => {
    if (!selectedProviderCredential) return false;
    if (
      selectedProviderCredential.providerId === "llamacpp" ||
      selectedProviderCredential.providerId === "localdiffusion" ||
      selectedProviderCredential.providerId === "intenserp" ||
      selectedProviderCredential.providerId === "stability"
    ) {
      return false;
    }
    if (
      selectedProviderCredential.providerId === "custom" ||
      selectedProviderCredential.providerId === "custom-anthropic"
    ) {
      return selectedProviderCredential.config?.fetchModelsEnabled === true;
    }
    return true;
  })();

  // Switch to select mode automatically if models are fetched
  useEffect(() => {
    if (fetchedModels.length > 0) {
      setIsManualInput(false);
    }
  }, [fetchedModels.length]);

  // Auto-fetch models when provider changes or initial load
  useEffect(() => {
    if (editorModel?.providerId && modelFetchEnabledForSelectedProvider) {
      fetchModels();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [editorModel?.providerId, editorModel?.providerLabel, modelFetchEnabledForSelectedProvider]);

  // Reset search when selector closes
  useEffect(() => {
    if (!showModelSelector) {
      setSearchQuery("");
      setDebouncedSearchQuery("");
      setShowOnlyFreeModels(false);
    }
  }, [showModelSelector]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setDebouncedSearchQuery(searchQuery);
    }, 120);
    return () => window.clearTimeout(timer);
  }, [searchQuery]);

  const isOpenRouterProvider = editorModel?.providerId === "openrouter";
  const formatOpenRouterPricePerMillion = (price?: number) => {
    if (typeof price !== "number" || !Number.isFinite(price)) return null;
    const perMillion = price * 1_000_000;
    if (perMillion <= 0) return t("editModel.pricing.free");
    if (perMillion >= 100) return `$${perMillion.toFixed(0)}/M`;
    if (perMillion >= 10) return `$${perMillion.toFixed(1)}/M`;
    if (perMillion >= 1) return `$${perMillion.toFixed(2)}/M`;
    return `$${perMillion.toFixed(3)}/M`;
  };
  const isFreeOpenRouterModel = (model: {
    id: string;
    inputPrice?: number;
    outputPrice?: number;
  }) => {
    const inputPrice = typeof model.inputPrice === "number" ? model.inputPrice : Number.NaN;
    const outputPrice = typeof model.outputPrice === "number" ? model.outputPrice : Number.NaN;
    const hasZeroPricing =
      Number.isFinite(inputPrice) &&
      Number.isFinite(outputPrice) &&
      inputPrice <= 0 &&
      outputPrice <= 0;
    return hasZeroPricing || model.id.toLowerCase().includes(":free");
  };

  const filteredModels = useMemo(() => {
    const query = normalizeSearchText(debouncedSearchQuery);
    const tokens = query.length > 0 ? query.split(" ").filter(Boolean) : [];
    const hasQuery = tokens.length > 0;
    const selectedModelId = editorModel?.name ?? "";

    const ranked = fetchedModels
      .map((model, index) => {
        if (isOpenRouterProvider && showOnlyFreeModels && !isFreeOpenRouterModel(model)) {
          return null;
        }

        if (!hasQuery) {
          return { model, index, score: 0 };
        }

        const id = normalizeSearchText(model.id);
        const name = normalizeSearchText(model.displayName);
        const description = normalizeSearchText(model.description);
        const idWords = id.split(" ").filter(Boolean);
        const nameWords = name.split(" ").filter(Boolean);
        const descWords = description.split(" ").filter(Boolean);
        const combined = `${id} ${name} ${description}`;

        if (!tokens.every((token) => combined.includes(token))) {
          return null;
        }

        let score = 0;

        if (id === query) score += 2000;
        if (name === query) score += 1800;
        if (id.startsWith(query)) score += 1300;
        if (name.startsWith(query)) score += 1100;
        if (id.includes(query)) score += 700;
        if (name.includes(query)) score += 550;
        if (description.includes(query)) score += 120;

        for (const token of tokens) {
          if (idWords.some((word) => word === token)) score += 140;
          else if (idWords.some((word) => word.startsWith(token))) score += 95;
          else if (id.includes(token)) score += 60;

          if (nameWords.some((word) => word === token)) score += 120;
          else if (nameWords.some((word) => word.startsWith(token))) score += 85;
          else if (name.includes(token)) score += 50;

          if (descWords.some((word) => word === token)) score += 30;
          else if (descWords.some((word) => word.startsWith(token))) score += 20;
          else if (description.includes(token)) score += 10;
        }

        if (model.id === selectedModelId) {
          score += 35;
        }

        return { model, index, score };
      })
      .filter(
        (entry): entry is { model: (typeof fetchedModels)[number]; index: number; score: number } =>
          !!entry,
      );

    if (hasQuery) {
      ranked.sort((a, b) => b.score - a.score || a.index - b.index);
    }

    return ranked.map((entry) => entry.model);
  }, [
    fetchedModels,
    debouncedSearchQuery,
    isOpenRouterProvider,
    showOnlyFreeModels,
    editorModel?.name,
  ]);
  const didYouMeanSuggestions = useMemo(() => {
    if (filteredModels.length > 0) return [];
    const query = normalizeSearchText(debouncedSearchQuery);
    if (!query) return [];

    const threshold = query.length <= 4 ? 1 : 2;
    const queryWords = query.split(" ").filter(Boolean);

    const ranked = fetchedModels
      .map((model, index) => {
        if (isOpenRouterProvider && showOnlyFreeModels && !isFreeOpenRouterModel(model)) {
          return null;
        }

        const id = normalizeSearchText(model.id);
        const name = normalizeSearchText(model.displayName);
        const idWords = id.split(" ").filter(Boolean);
        const nameWords = name.split(" ").filter(Boolean);
        const bestDistance = Math.min(
          getEditDistance(query, id),
          name ? getEditDistance(query, name) : Number.MAX_SAFE_INTEGER,
        );
        const sharedPrefix = (a: string, b: string) => {
          const max = Math.min(a.length, b.length);
          let i = 0;
          while (i < max && a[i] === b[i]) i++;
          return i;
        };
        const hasNearPrefix = [...idWords, ...nameWords].some((word) =>
          queryWords.some((qWord) => {
            if (!word || !qWord) return false;
            return (
              word.startsWith(qWord) || qWord.startsWith(word) || sharedPrefix(word, qWord) >= 3
            );
          }),
        );
        const softMatch =
          id.includes(query) ||
          name.includes(query) ||
          id.startsWith(query) ||
          name.startsWith(query) ||
          idWords.some((word) => word.startsWith(query) || query.startsWith(word)) ||
          nameWords.some((word) => word.startsWith(query) || query.startsWith(word)) ||
          hasNearPrefix;

        if (bestDistance > threshold && !softMatch) {
          return null;
        }

        const score = bestDistance * 100 + (softMatch ? -20 : 0);
        return {
          model,
          index,
          score,
        };
      })
      .filter(
        (entry): entry is { model: (typeof fetchedModels)[number]; index: number; score: number } =>
          !!entry,
      )
      .sort((a, b) => a.score - b.score || a.index - b.index)
      .slice(0, 3)
      .map((entry) => entry.model);

    return ranked;
  }, [
    filteredModels.length,
    debouncedSearchQuery,
    fetchedModels,
    isOpenRouterProvider,
    showOnlyFreeModels,
  ]);
  const modelIdLabel =
    isLocalModel ? t("editModel.fields.modelPath") : t("editModel.fields.modelId");
  const modelIdPlaceholder = isLocalModel
    ? t("editModel.placeholders.modelPath")
    : t("editModel.placeholders.modelId");
  const mmprojLibraryModels = useMemo(
    () =>
      downloadedModels.filter(
        (model) => model.isMmproj ?? model.filename.toLowerCase().includes("mmproj"),
      ),
    [downloadedModels],
  );
  const mtpLibraryModels = useMemo(
    () =>
      downloadedModels.filter((model) => {
        if (model.isMtp !== undefined) return model.isMtp;
        const base = model.filename.toLowerCase();
        return base.startsWith("mtp-") || base.includes("-mtp.") || base.includes("_mtp.");
      }),
    [downloadedModels],
  );
  const localLibraryModels =
    localLibraryPickerMode === "mmproj"
      ? mmprojLibraryModels
      : localLibraryPickerMode === "mtp"
        ? mtpLibraryModels
        : downloadedModels;
  const localLibraryTitle =
    localLibraryPickerMode === "mmproj"
      ? t("editModel.localLibrary.mmprojTitle")
      : localLibraryPickerMode === "mtp"
        ? "Select MTP Draft File"
        : t("hfBrowser.libraryTitle");
  const localLibraryEmptyLabel =
    localLibraryPickerMode === "mmproj"
      ? t("editModel.localLibrary.mmprojEmpty")
      : localLibraryPickerMode === "mtp"
        ? "No MTP files downloaded"
        : t("hfBrowser.libraryEmpty");
  const localLibraryEmptyHint =
    localLibraryPickerMode === "mmproj"
      ? t("editModel.localLibrary.mmprojEmptyHint")
      : localLibraryPickerMode === "mtp"
        ? "Download the mtp-*.gguf sidecar from the model's repository in the model browser."
        : t("hfBrowser.libraryEmptyHint");
  const isAutomatic1111Provider = editorModel?.providerId === "automatic1111";
  const isLocalDiffusionModel = editorModel?.providerId === "localdiffusion";
  const isFixedImageProvider = isAutomatic1111Provider || isLocalDiffusionModel;

  useEffect(() => {
    if (!isLocalDiffusionModel || sdEntries !== null) return;
    sdListModels()
      .then(setSdEntries)
      .catch(() => setSdEntries([]));
  }, [isLocalDiffusionModel, sdEntries]);

  const selectedSdEntry = sdEntries?.find((entry) => entry.id === editorModel?.name) ?? null;

  useEffect(() => {
    const files = selectedSdEntry?.files ?? {};
    setSdFilesDraft({
      checkpoint: files.checkpoint ?? "",
      diffusionModel: files.diffusionModel ?? "",
      clipL: files.clipL ?? "",
      clipG: files.clipG ?? "",
      t5xxl: files.t5xxl ?? "",
      llm: files.llm ?? "",
      llmVision: files.llmVision ?? "",
      vae: files.vae ?? "",
    });
    setSdMainPathDraft(files.checkpoint ?? files.diffusionModel ?? "");
  }, [selectedSdEntry]);

  useEffect(() => {
    if (sdLibraryRole === null || sdLibraryFiles !== null) return;
    sdListLocalFiles()
      .then(setSdLibraryFiles)
      .catch(() => setSdLibraryFiles([]));
  }, [sdLibraryRole, sdLibraryFiles]);

  const commitSdFile = async (role: SdModelRole, rawPath: string) => {
    const path = rawPath.trim() || null;
    try {
      if (selectedSdEntry) {
        const current = (selectedSdEntry.files as Record<string, string | null | undefined>)[role] ?? "";
        if ((path ?? "") === current) return;
        await sdSetModelFile(selectedSdEntry.id, role, path);
        setSdEntries(null);
      } else if (path) {
        const fallbackName =
          editorModel?.displayName?.trim() ||
          path.split(/[\\/]/).pop()?.replace(/\.(safetensors|gguf|ckpt|sft)$/i, "") ||
          "Local model";
        const entry = await sdImportModel(fallbackName, { [role]: path });
        handleModelNameChange(entry.id);
        if (!editorModel?.displayName?.trim()) {
          handleDisplayNameChange(entry.name);
        }
        setSdEntries(null);
      }
    } catch (err: any) {
      toast.error(
        t("imageGeneration.local.updateFailed"),
        typeof err === "string" ? err : err?.message || String(err),
      );
    }
  };

  const browseSdFile = async (role: SdModelRole) => {
    const selection = await open({
      multiple: false,
      filters: [
        { name: t("editModel.localDiffusion.modelFilesFilter"), extensions: ["safetensors", "gguf", "ckpt", "sft"] },
      ],
    });
    if (typeof selection !== "string") return;
    setSdFilesDraft((draft) => ({ ...draft, [role]: selection }));
    await commitSdFile(role, selection);
  };

  const commitSdMainModel = async (rawPath: string) => {
    const path = rawPath.trim();
    if (!path) return;
    const role: SdModelRole = path.toLowerCase().endsWith(".gguf")
      ? "diffusionModel"
      : "checkpoint";
    const sibling: SdModelRole = role === "checkpoint" ? "diffusionModel" : "checkpoint";
    try {
      if (selectedSdEntry) {
        const currentMain =
          selectedSdEntry.files.checkpoint ?? selectedSdEntry.files.diffusionModel ?? "";
        if (path === currentMain) return;
        await sdSetModelFile(selectedSdEntry.id, role, path);
        const siblingValue = (selectedSdEntry.files as Record<string, string | null | undefined>)[
          sibling
        ];
        if (siblingValue) {
          await sdSetModelFile(selectedSdEntry.id, sibling, null);
        }
        setSdEntries(null);
        return;
      }
      const fallbackName =
        editorModel?.displayName?.trim() ||
        path.split(/[\\/]/).pop()?.replace(/\.(safetensors|gguf|ckpt|sft)$/i, "") ||
        "Local model";
      const entry = await sdImportModel(fallbackName, { [role]: path });
      handleModelNameChange(entry.id);
      if (!editorModel?.displayName?.trim()) {
        handleDisplayNameChange(entry.name);
      }
      setSdEntries(null);
    } catch (err: any) {
      toast.error(
        t("imageGeneration.local.updateFailed"),
        typeof err === "string" ? err : err?.message || String(err),
      );
    }
  };

  const browseSdMainModel = async () => {
    const selection = await open({
      multiple: false,
      filters: [
        { name: t("editModel.localDiffusion.modelFilesFilter"), extensions: ["safetensors", "gguf", "ckpt", "sft"] },
      ],
    });
    if (typeof selection !== "string") return;
    setSdMainPathDraft(selection);
    await commitSdMainModel(selection);
  };

  // Get reasoning support for the current provider
  const reasoningSupport: ReasoningSupport = editorModel?.providerId
    ? getProviderReasoningSupport(editorModel.providerId)
    : "none";
  const showReasoningSection = reasoningSupport !== "none";
  const isAutoReasoning = reasoningSupport === "auto";
  const showEffortOptions = reasoningSupport === "effort" || reasoningSupport === "dynamic";
  // Get caching support for the current provider
  const cachingSupport = editorModel?.providerId
    ? getProviderCachingSupport(editorModel.providerId)
    : "none";
  const showCachingSection = cachingSupport !== "none";
  const hasAutomaticCaching = cachingSupport === "automatic";
  const promptCachingTtlOptions =
    editorModel?.providerId === "openai"
      ? [
        { value: "in_memory", label: t("editModel.promptCaching.ttl.inMemory") },
        { value: "24h", label: t("editModel.promptCaching.ttl.24h") },
      ]
      : [
        { value: "5min", label: t("editModel.promptCaching.ttl.5min") },
        { value: "1h", label: t("editModel.promptCaching.ttl.1h") },
      ];
  const selectedPromptCachingTtl =
    modelAdvancedDraft.promptCachingTtl ??
    (editorModel?.providerId === "openai" ? "in_memory" : "5min");
  const numberInputClassName =
    "w-full rounded-lg border border-fg/10 bg-surface-el/20 px-4 py-3.5 text-[13px] text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none";
  const selectInputClassName =
    "w-full rounded-lg border border-fg/10 bg-surface-el/20 px-4 py-3.5 text-[13px] text-fg transition focus:border-fg/30 focus:outline-none";
  const textAreaInputClassName =
    "w-full rounded-lg border border-fg/10 bg-surface-el/20 px-4 py-3.5 text-[13px] text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none";
  const contextLimit = llamaContextInfo?.maxContextLength ?? ADVANCED_CONTEXT_LENGTH_RANGE.max;
  const recommendedContextLength = llamaContextInfo?.recommendedContextLength ?? null;
  const llamaLayerPlacementSummary = useMemo(() => {
    const totalLayers = llamaContextInfo?.layerCount;
    if (!totalLayers || totalLayers <= 0) {
      return null;
    }

    const requestedGpuLayers = modelAdvancedDraft.llamaGpuLayers;
    if (requestedGpuLayers === null || requestedGpuLayers === undefined) {
      return {
        totalLayers,
        detail: `Auto placement • ${totalLayers.toLocaleString()} total layers`,
      };
    }

    const gpuLayers = Math.max(0, Math.min(totalLayers, Math.trunc(requestedGpuLayers)));
    const cpuLayers = Math.max(totalLayers - gpuLayers, 0);
    return {
      totalLayers,
      detail: `${gpuLayers.toLocaleString()} to GPU • ${cpuLayers.toLocaleString()} on CPU • ${totalLayers.toLocaleString()} total`,
    };
  }, [llamaContextInfo?.layerCount, modelAdvancedDraft.llamaGpuLayers]);
  const selectedContextLength = modelAdvancedDraft.contextLength ?? null;
  const showContextWarning =
    isLocalModel &&
    selectedContextLength &&
    recommendedContextLength !== null &&
    recommendedContextLength > 0 &&
    selectedContextLength > recommendedContextLength;
  const showContextCritical =
    isLocalModel &&
    selectedContextLength &&
    recommendedContextLength !== null &&
    recommendedContextLength === 0;
  const formatGiB = (bytes?: number | null) => {
    if (!bytes || bytes <= 0) return null;
    return (bytes / 1024 ** 3).toFixed(1);
  };
  const availableRamGiB = formatGiB(llamaContextInfo?.availableMemoryBytes ?? null);
  const availableVramGiB = formatGiB(llamaContextInfo?.availableVramBytes ?? null);
  const modelSizeGiB = formatGiB(llamaContextInfo?.modelSizeBytes ?? null);
  const supportsLlamaGpuOffload =
    llamaContextInfo?.supportsGpuOffload ?? llamaRuntimeReport?.supportsGpuOffload ?? null;
  const isCpuOnlyLlamaBackend = isLocalModel && supportsLlamaGpuOffload === false;
  const contextCacheLocationLabel =
    isCpuOnlyLlamaBackend
      ? t("editModel.runtimeSummary.ram")
      : modelAdvancedDraft.llamaOffloadKqv === true
        ? t("editModel.runtimeSummary.vram")
        : modelAdvancedDraft.llamaOffloadKqv === false
          ? t("editModel.runtimeSummary.ram")
          : t("common.labels.auto");
  const selectedSamplerProfile = modelAdvancedDraft.llamaSamplerProfile ?? "balanced";
  const ollamaStopText = (modelAdvancedDraft.ollamaStop ?? []).join("\n");
  const selectedFetchedModel = fetchedModels.find((model) => model.id === editorModel?.name);
  const selectedProviderLabel =
    selectedProviderCredential?.label ||
    editorModel?.providerLabel ||
    editorModel?.providerId ||
    t("editModel.setup.selectPlatform");
  const hasRuntimePanel = isLocalModel || isOllamaModel;
  const runtimePanelTitle = isLocalModel
    ? "llama.cpp"
    : isOllamaModel
      ? "Ollama"
      : t("editModel.sections.runtime");
  const activeDetailPanel = activePanel;
  const editorPanels: { key: EditorSectionKey; label: string }[] = [
    { key: "generation", label: t("editModel.sections.generation") },
    ...(isLocalDiffusionModel
      ? [{ key: "configuration" as const, label: t("editModel.sections.configuration") }]
      : []),
    ...(hasRuntimePanel
      ? [{ key: "runtime" as const, label: t("editModel.sections.runtime") }]
      : []),
    ...(showReasoningSection
      ? [{ key: "reasoning" as const, label: t("editModel.sections.reasoning") }]
      : []),
    ...(showCachingSection
      ? [{ key: "caching" as const, label: t("editModel.sections.promptCaching") }]
      : []),
    { key: "capabilities", label: t("editModel.sections.capabilities") },
  ];
  function updateSdSetting<K extends keyof typeof modelAdvancedDraft>(
    key: K,
    value: (typeof modelAdvancedDraft)[K],
  ) {
    setModelAdvancedDraft({
      ...modelAdvancedDraft,
      [key]: value,
    });
  }
  const generationSummary = isFixedImageProvider
    ? [
      modelAdvancedDraft.sdSteps != null ? `Steps ${modelAdvancedDraft.sdSteps}` : null,
      modelAdvancedDraft.sdCfgScale != null
        ? `CFG ${modelAdvancedDraft.sdCfgScale.toFixed(1)}`
        : null,
      modelAdvancedDraft.sdSampler ? modelAdvancedDraft.sdSampler : null,
      modelAdvancedDraft.sdSize ? modelAdvancedDraft.sdSize : null,
    ]
      .filter(Boolean)
      .join(" • ") || t("editModel.summaries.generationAutomatic1111")
    : [
      modelAdvancedDraft.temperature != null
        ? `Temp ${modelAdvancedDraft.temperature.toFixed(2)}`
        : null,
      modelAdvancedDraft.topP != null ? `Top P ${modelAdvancedDraft.topP.toFixed(2)}` : null,
      modelAdvancedDraft.maxOutputTokens != null
        ? `Max ${modelAdvancedDraft.maxOutputTokens.toLocaleString()}`
        : null,
    ]
      .filter(Boolean)
      .join(" • ") || t("editModel.summaries.generationDefault");
  const runtimeSummary = isLocalModel
    ? [
      modelAdvancedDraft.llamaBatchSize != null
        ? `Batch ${modelAdvancedDraft.llamaBatchSize}`
        : null,
      modelAdvancedDraft.llamaKvType ? `KV ${modelAdvancedDraft.llamaKvType}` : null,
      modelAdvancedDraft.llamaOffloadKqv === true
        ? t("editModel.runtimeSummary.kvCacheInVram")
        : modelAdvancedDraft.llamaOffloadKqv === false
          ? t("editModel.runtimeSummary.kvCacheInRam")
          : null,
    ]
      .filter(Boolean)
      .join(" • ") || t("editModel.summaries.runtimeLlama")
    : isOllamaModel
      ? [
        modelAdvancedDraft.ollamaNumCtx != null
          ? `Ctx ${modelAdvancedDraft.ollamaNumCtx.toLocaleString()}`
          : null,
        modelAdvancedDraft.ollamaNumPredict != null
          ? `Predict ${modelAdvancedDraft.ollamaNumPredict.toLocaleString()}`
          : null,
        modelAdvancedDraft.ollamaNumThread != null
          ? `Threads ${modelAdvancedDraft.ollamaNumThread}`
          : null,
      ]
        .filter(Boolean)
        .join(" • ") || t("editModel.summaries.runtimeOllama")
      : "";
  const reasoningSummary = isAutoReasoning
    ? t("editModel.summaries.reasoningAlwaysEnabled")
    : modelAdvancedDraft.reasoningEnabled === false
      ? t("editModel.summaries.reasoningDisabled")
      : [
        modelAdvancedDraft.reasoningEnabled
          ? t("editModel.reasoning.enabled")
          : t("editModel.reasoning.providerDefault"),
        modelAdvancedDraft.reasoningEffort
          ? `Effort ${modelAdvancedDraft.reasoningEffort}`
          : null,
        modelAdvancedDraft.reasoningBudgetTokens != null
          ? `Budget ${modelAdvancedDraft.reasoningBudgetTokens.toLocaleString()}`
          : null,
      ]
        .filter(Boolean)
        .join(" • ") || t("editModel.summaries.reasoningDefault");
  const inputCapabilitySummary = (editorModel?.inputScopes ?? [])
    .filter((scope) => scope !== "text")
    .map((scope) => scope[0].toUpperCase() + scope.slice(1))
    .join(", ");
  const outputCapabilitySummary = (editorModel?.outputScopes ?? [])
    .filter((scope) => scope !== "text")
    .map((scope) => scope[0].toUpperCase() + scope.slice(1))
    .join(", ");
  const capabilitiesSummary = t("editModel.summaries.capabilities", {
    input: inputCapabilitySummary || t("editModel.summaries.textOnly"),
    output: outputCapabilitySummary || t("editModel.summaries.textOnly"),
  });
  const applyLlamaPreset = (preset: "balanced" | "throughput" | "vram" | "cpu_ram") => {
    setSelectedLlamaQuickPreset(preset);
    if (preset === "balanced") {
      handleLlamaBatchSizeChange(512);
      handleLlamaKvTypeChange("q8_0");
      handleLlamaOffloadKqvChange(isCpuOnlyLlamaBackend ? false : true);
      handleLlamaFlashAttentionChange("auto");
      return;
    }
    if (preset === "throughput") {
      handleLlamaBatchSizeChange(1024);
      handleLlamaKvTypeChange("f16");
      handleLlamaOffloadKqvChange(isCpuOnlyLlamaBackend ? false : true);
      handleLlamaFlashAttentionChange("enabled");
      return;
    }
    if (preset === "vram") {
      handleLlamaBatchSizeChange(512);
      handleLlamaKvTypeChange("q4_k");
      handleLlamaOffloadKqvChange(isCpuOnlyLlamaBackend ? false : true);
      handleLlamaFlashAttentionChange("enabled");
      return;
    }
    handleLlamaBatchSizeChange(256);
    handleLlamaKvTypeChange("q8_0");
    handleLlamaOffloadKqvChange(false);
    handleLlamaFlashAttentionChange("auto");
  };

  // Register window globals for header save button
  useEffect(() => {
    const globalWindow = window as any;
    globalWindow.__saveModel = () => void handleSaveWithMoveCheck();
    globalWindow.__saveModelCanSave = canSave;
    globalWindow.__saveModelSaving = saving || verifying;
    return () => {
      delete globalWindow.__saveModel;
      delete globalWindow.__saveModelCanSave;
      delete globalWindow.__saveModelSaving;
    };
  }, [handleSaveWithMoveCheck, canSave, saving, verifying]);

  useEffect(() => {
    const handleDiscard = () => resetToInitial();
    window.addEventListener("unsaved:discard", handleDiscard);
    return () => window.removeEventListener("unsaved:discard", handleDiscard);
  }, [resetToInitial]);

  useEffect(() => {
    if (activePanel === "runtime" && !hasRuntimePanel) {
      setActivePanel(showReasoningSection ? "reasoning" : "generation");
    } else if (activePanel === "configuration" && !isLocalDiffusionModel) {
      setActivePanel("generation");
    } else if (activePanel === "reasoning" && !showReasoningSection) {
      setActivePanel(hasRuntimePanel ? "runtime" : "generation");
    } else if (activePanel === "caching" && !showCachingSection) {
      setActivePanel("generation");
    }
  }, [activePanel, hasRuntimePanel, isLocalDiffusionModel, showReasoningSection, showCachingSection]);

  useEffect(() => {
    if (!showLlamaRuntimeReport) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") setShowLlamaRuntimeReport(false);
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [showLlamaRuntimeReport]);

  useEffect(() => {
    if (!isLocalModel) {
      setLlamaContextInfo(null);
      setLlamaContextError(null);
      setLlamaContextLoading(false);
      return;
    }

    const modelPath = editorModel?.name?.trim();
    if (!modelPath) {
      setLlamaContextInfo(null);
      setLlamaContextError(null);
      setLlamaContextLoading(false);
      return;
    }

    let cancelled = false;
    setLlamaContextLoading(true);
    setLlamaContextError(null);

    const timer = setTimeout(async () => {
      try {
        const info = await invoke<LlamaCppContextInfo>("llamacpp_context_info", {
          modelPath,
          llamaOffloadKqv: modelAdvancedDraft.llamaOffloadKqv ?? null,
          llamaKvType: modelAdvancedDraft.llamaKvType ?? null,
          llamaGpuLayers: modelAdvancedDraft.llamaGpuLayers ?? null,
          llamaMmprojPath: modelAdvancedDraft.llamaMmprojPath ?? null,
          llamaMtpEnabled: modelAdvancedDraft.llamaMtpEnabled ?? null,
          llamaMtpModelPath: modelAdvancedDraft.llamaMtpModelPath ?? null,
        });
        if (!cancelled) {
          setLlamaContextInfo(info);
          setLlamaContextError(null);
        }
      } catch (err: any) {
        if (!cancelled) {
          setLlamaContextInfo(null);
          const errorMessage =
            err?.message ??
            (typeof err === "string" ? err : err?.toString?.()) ??
            "Failed to load context limits";
          setLlamaContextError(errorMessage);
        }
      } finally {
        if (!cancelled) {
          setLlamaContextLoading(false);
        }
      }
    }, 350);

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [
    editorModel?.name,
    isLocalModel,
    modelAdvancedDraft.llamaOffloadKqv,
    modelAdvancedDraft.llamaKvType,
    modelAdvancedDraft.llamaGpuLayers,
    modelAdvancedDraft.llamaMmprojPath,
    modelAdvancedDraft.llamaMtpEnabled,
    modelAdvancedDraft.llamaMtpModelPath,
  ]);

  useEffect(() => {
    if (!isCpuOnlyLlamaBackend) {
      return;
    }
    if (
      modelAdvancedDraft.llamaGpuLayers === 0 &&
      modelAdvancedDraft.llamaOffloadKqv === false
    ) {
      return;
    }
    setModelAdvancedDraft({
      ...modelAdvancedDraft,
      llamaGpuLayers: 0,
      llamaOffloadKqv: false,
    });
  }, [isCpuOnlyLlamaBackend, modelAdvancedDraft, setModelAdvancedDraft]);

  // Fetch runability score for local models
  useEffect(() => {
    if (!isLocalModel) {
      setRunabilityScore(null);
      setRunabilityLoading(false);
      return;
    }

    const modelPath = editorModel?.name?.trim();
    if (!modelPath) {
      setRunabilityScore(null);
      setRunabilityLoading(false);
      return;
    }

    let cancelled = false;
    setRunabilityLoading(true);

    const timer = setTimeout(async () => {
      try {
        const result = await invoke<NonNullable<typeof runabilityScore>>(
          "hf_compute_local_runability",
          {
            filePath: modelPath,
            llamaMmprojPath: modelAdvancedDraft.llamaMmprojPath ?? null,
            llamaMtpEnabled: modelAdvancedDraft.llamaMtpEnabled ?? null,
            llamaMtpModelPath: modelAdvancedDraft.llamaMtpModelPath ?? null,
          },
        );
        if (!cancelled) {
          setRunabilityScore(result);
        }
      } catch {
        if (!cancelled) {
          setRunabilityScore(null);
        }
      } finally {
        if (!cancelled) {
          setRunabilityLoading(false);
        }
      }
    }, 400);

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [
    editorModel?.name,
    isLocalModel,
    modelAdvancedDraft.llamaMmprojPath,
    modelAdvancedDraft.llamaMtpEnabled,
    modelAdvancedDraft.llamaMtpModelPath,
  ]);

  const scopeOrder = ["text", "image", "audio"] as const;
  const toggleScope = (
    key: "inputScopes" | "outputScopes",
    scope: "text" | "image" | "audio",
    enabled: boolean,
  ) => {
    if (!editorModel) return;
    if (isFixedImageProvider) return;
    const current = new Set((editorModel as any)[key] ?? ["text"]);
    if (enabled) current.add(scope);
    else current.delete(scope);
    let next = scopeOrder.filter((s) => current.has(s));
    if (next.length === 0) next = ["text"];
    updateEditorModel({ [key]: next } as any);
  };

  const inferScopesFromFetchedModel = (model: (typeof fetchedModels)[number]) => {
    const inputModalities = new Set(model.inputModalities ?? []);
    const outputModalities = new Set(model.outputModalities ?? []);
    const supportedEndpoints = model.supportedEndpoints ?? [];

    const supportsImageInput =
      inputModalities.has("image") ||
      supportedEndpoints.some((endpoint) => endpoint.includes("/images/edits"));
    const supportsImageOutput =
      outputModalities.has("image") ||
      supportedEndpoints.some((endpoint) => endpoint.includes("/images/"));
    const supportsAudioInput = inputModalities.has("audio");
    const supportsAudioOutput = outputModalities.has("audio");

    const supportsTextInput = inputModalities.size === 0 || inputModalities.has("text");
    const supportsTextOutput = outputModalities.size === 0 || outputModalities.has("text");

    const inputScopes = scopeOrder.filter(
      (scope) =>
        (scope === "text" && supportsTextInput) ||
        (scope === "image" && supportsImageInput) ||
        (scope === "audio" && supportsAudioInput),
    );
    const outputScopes = scopeOrder.filter(
      (scope) =>
        (scope === "text" && supportsTextOutput) ||
        (scope === "image" && supportsImageOutput) ||
        (scope === "audio" && supportsAudioOutput),
    );

    const fallback: Array<"text" | "image" | "audio"> = ["text"];
    return {
      inputScopes: inputScopes.length > 0 ? inputScopes : fallback,
      outputScopes: outputScopes.length > 0 ? outputScopes : fallback,
    };
  };

  const handleSelectModel = (modelId: string, displayName?: string) => {
    const fetchedModel = fetchedModels.find((model) => model.id === modelId);
    handleModelNameChange(modelId);
    if (displayName) {
      handleDisplayNameChange(displayName);
    } else {
      handleDisplayNameChange(modelId);
    }
    if (fetchedModel) {
      updateEditorModel(inferScopesFromFetchedModel(fetchedModel));
    }
    setShowModelSelector(false);
  };

  if (loading || !editorModel) {
    return (
      <div className="flex h-full flex-col text-fg/90">
        <div className="flex flex-1 items-center justify-center">
          <div className="h-8 w-8 animate-spin rounded-full border-2 border-fg/10 border-t-fg/60" />
        </div>
      </div>
    );
  }

  return (
    <div className="flex min-h-dvh flex-col text-fg/90">
      <main className="flex-1 overflow-y-auto px-4 pt-6 pb-32 sm:px-6 lg:px-10">
        <motion.div
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          className="mx-auto w-full max-w-7xl space-y-6"
        >
          {error && (
            <div className="rounded-xl border border-danger/30 bg-danger/10 px-4 py-3">
              <p className="text-[13px] text-danger/80">{error}</p>
            </div>
          )}

          <div className="relative">
            <div className="w-full space-y-6">
              <section className="space-y-6">
                <div>
                  <h2 className="text-[15px] font-semibold text-fg">
                    {t("editModel.setup.title")}
                  </h2>
                  <p className="mt-1 text-[13px] leading-relaxed text-fg/45">
                    {t("editModel.setup.description")}
                  </p>
                </div>
                <div className="space-y-6">
                  {isLocalModel && llamaRuntimeReport && (
                    <button
                      type="button"
                      onClick={() => setShowLlamaRuntimeReport(true)}
                      className="flex w-full items-center justify-between gap-4 rounded-lg border border-fg/10 bg-surface-el/20 px-4 py-3 text-left transition hover:bg-surface-el/30"
                    >
                      <div className="flex min-w-0 items-center gap-3">
                        <div
                          className={cn(
                            "flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border",
                            llamaRuntimeReport.status === "succeeded"
                              ? "border-emerald-400/30 bg-emerald-400/10 text-emerald-400"
                              : llamaRuntimeReport.status === "cpuFallbackSucceeded"
                                ? "border-warning/30 bg-warning/10 text-warning"
                                : "border-danger/30 bg-danger/10 text-danger",
                          )}
                        >
                          {llamaRuntimeReport.status === "succeeded" ||
                          llamaRuntimeReport.status === "cpuFallbackSucceeded" ? (
                            <Check className="h-4 w-4" />
                          ) : (
                            <AlertTriangle className="h-4 w-4" />
                          )}
                        </div>
                        <div className="min-w-0">
                          <div className="flex items-center gap-2">
                            <span className="text-[13px] font-medium text-fg">
                              {t("editModel.runtime.lastReport")}
                            </span>
                            <span
                              className={cn(
                                "rounded-md px-2 py-0.5 text-[11px] font-medium",
                                llamaRuntimeReport.status === "succeeded"
                                  ? "bg-emerald-400/12 text-emerald-400"
                                  : llamaRuntimeReport.status === "cpuFallbackSucceeded"
                                    ? "bg-warning/12 text-warning"
                                    : "bg-danger/12 text-danger",
                              )}
                            >
                              {llamaRuntimeReport.status === "succeeded"
                                ? t("editModel.runtime.badges.succeeded")
                                : llamaRuntimeReport.status === "cpuFallbackSucceeded"
                                  ? t("editModel.runtime.badges.cpuFallbackSucceeded")
                                  : llamaRuntimeReport.status === "cpuFallbackFailed"
                                    ? t("editModel.runtime.badges.cpuFallbackFailed")
                                    : t("editModel.runtime.badges.failed")}
                            </span>
                          </div>
                          <p className="truncate text-[12px] text-fg/50">
                            {t(getLlamaRuntimeHeadlineKey(llamaRuntimeReport))}
                          </p>
                        </div>
                      </div>
                      <ChevronRight className="h-4 w-4 shrink-0 text-fg/40" />
                    </button>
                  )}

                  <FieldBlock label={t("editModel.fields.platform")}>
                    {providers.length === 0 ? (
                      <div className="rounded-lg border border-warning/40 bg-warning/10 px-3 py-2.5 text-[13px] text-warning">
                        {t("settings.items.providers.subtitle")}
                      </div>
                    ) : (
                      <>
                        <button
                          type="button"
                          onClick={() => setShowPlatformSelector(true)}
                          className="flex w-full items-center justify-between rounded-lg border border-fg/10 bg-surface-el/20 px-4 py-3 text-fg transition hover:bg-surface-el/30"
                        >
                          <div className="flex min-w-0 items-center gap-3">
                            <div className="flex h-8 w-8 items-center justify-center rounded-lg border border-fg/10 bg-fg/5 text-fg/60">
                              {getProviderIcon(editorModel.providerId)}
                            </div>
                            <span className="truncate text-[13px] text-fg/85">
                              {selectedProviderLabel}
                            </span>
                          </div>
                          <ChevronDown className="h-4 w-4 text-fg/40" />
                        </button>

                        <BottomMenu
                          isOpen={showPlatformSelector}
                          onClose={() => setShowPlatformSelector(false)}
                          title={t("editModel.setup.selectPlatform")}
                        >
                          <MenuSection>
                            {providers.map((prov) => {
                              const isSelected =
                                prov.providerId === editorModel.providerId &&
                                prov.label === editorModel.providerLabel;
                              return (
                                <MenuButton
                                  key={prov.id}
                                  icon={getProviderIcon(prov.providerId)}
                                  title={prov.label || prov.providerId}
                                  description={prov.providerId}
                                  color={
                                    isSelected
                                      ? "from-accent to-accent/80"
                                      : "from-white/10 to-white/5"
                                  }
                                  rightElement={
                                    isSelected ? (
                                      <Check className="h-4 w-4 text-accent" />
                                    ) : (
                                      <ChevronRight className="h-4 w-4 text-fg/20" />
                                    )
                                  }
                                  onClick={() => {
                                    handleProviderSelection(
                                      prov.providerId,
                                      prov.label || prov.providerId,
                                    );
                                    setShowPlatformSelector(false);
                                  }}
                                />
                              );
                            })}
                          </MenuSection>
                        </BottomMenu>
                      </>
                    )}
                  </FieldBlock>

                  {isLocalModel ? (
                    <div className="grid items-start grid-cols-1 gap-6 lg:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
                      <FieldBlock label={t("editModel.fields.displayName")}>
                        <input
                          type="text"
                          value={editorModel.displayName}
                          onChange={(e) => handleDisplayNameChange(e.target.value)}
                          placeholder={t("editModel.placeholders.displayName")}
                          className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-4 py-3 text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none"
                        />
                      </FieldBlock>

                      <FieldBlock
                        label={modelIdLabel}
                        action={
                          <div className="flex items-center gap-2">
                            <button
                              type="button"
                              onClick={openLocalModelPicker}
                              className="inline-flex items-center gap-1.5 rounded-md border border-fg/10 bg-fg/5 px-2.5 py-1.5 text-[12px] font-medium text-fg/68 transition hover:border-fg/20 hover:bg-fg/10 hover:text-fg"
                            >
                              <FolderOpen className="h-3.5 w-3.5 text-accent/70" />
                              {t("hfBrowser.selectFromLibrary")}
                            </button>
                            <button
                              type="button"
                              onClick={() => void handleBrowseLocalModel()}
                              className="rounded-md border border-fg/10 px-2.5 py-1.5 text-[12px] font-medium text-fg/65 transition hover:border-fg/20 hover:bg-fg/5 hover:text-fg/90"
                            >
                              {t("common.buttons.browseFiles")}
                            </button>
                          </div>
                        }
                      >
                        <div className="space-y-3">
                          <input
                            type="text"
                            value={editorModel.name}
                            onChange={(e) => handleModelNameChange(e.target.value)}
                            placeholder={modelIdPlaceholder}
                            className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-4 py-3 font-mono text-[13px] text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none"
                          />
                          <p className="text-[13px] leading-relaxed text-fg/45">
                            {t("editModel.localLibrary.localPathHelp")}
                          </p>

                          <BottomMenu
                            isOpen={showLocalModelPicker}
                            onClose={() => setShowLocalModelPicker(false)}
                            title={localLibraryTitle}
                          >
                            <MenuSection>
                              {loadingDownloaded ? (
                                <div className="flex items-center justify-center gap-2 py-12 text-fg/50">
                                  <Loader size={18} className="animate-spin" />
                                  <span className="text-[13px]">{t("hfBrowser.searching")}</span>
                                </div>
                              ) : localLibraryModels.length === 0 ? (
                                <div className="flex flex-col items-center gap-2 py-16 text-center">
                                  <HardDrive size={32} className="text-fg/20" />
                                  <p className="text-[13px] font-medium text-fg/60">
                                    {localLibraryEmptyLabel}
                                  </p>
                                  <p className="px-6 text-[13px] text-fg/40">
                                    {localLibraryEmptyHint}
                                  </p>
                                </div>
                              ) : (
                                localLibraryModels.map((model) => (
                                  <MenuButton
                                    key={model.path}
                                    icon={<HardDrive className="h-5 w-5 text-accent/60" />}
                                    title={model.filename.replace(/\.gguf$/i, "")}
                                    description={`${model.quantization} · ${formatBytes(model.size)}`}
                                    color="from-accent/20 to-accent/10"
                                    rightElement={
                                      (
                                        localLibraryPickerMode === "mmproj"
                                          ? modelAdvancedDraft.llamaMmprojPath === model.path
                                          : localLibraryPickerMode === "mtp"
                                            ? modelAdvancedDraft.llamaMtpModelPath === model.path
                                            : editorModel.name === model.path
                                      ) ? (
                                        <Check className="h-4 w-4 text-accent" />
                                      ) : (
                                        <ArrowRight className="h-4 w-4 text-fg/20" />
                                      )
                                    }
                                    onClick={() => handleSelectLocalLibraryFile(model)}
                                  />
                                ))
                              )}
                            </MenuSection>
                          </BottomMenu>
                        </div>
                      </FieldBlock>
                    </div>
                  ) : isLocalDiffusionModel ? (
                    <div className="grid items-start grid-cols-1 gap-6 lg:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
                      <FieldBlock label={t("editModel.fields.displayName")}>
                        <input
                          type="text"
                          value={editorModel.displayName}
                          onChange={(e) => handleDisplayNameChange(e.target.value)}
                          placeholder={t("editModel.placeholders.displayName")}
                          className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-4 py-3 text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none"
                        />
                      </FieldBlock>

                      <FieldBlock
                        label={t("editModel.fields.localDiffusionModel")}
                        action={
                          <div className="flex items-center gap-2">
                            <button
                              type="button"
                              onClick={() => setShowSdModelPicker(true)}
                              className="inline-flex items-center gap-1.5 rounded-md border border-fg/10 bg-fg/5 px-2.5 py-1.5 text-[12px] font-medium text-fg/68 transition hover:border-fg/20 hover:bg-fg/10 hover:text-fg"
                            >
                              <FolderOpen className="h-3.5 w-3.5 text-accent/70" />
                              {t("hfBrowser.selectFromLibrary")}
                            </button>
                            <button
                              type="button"
                              onClick={() => void browseSdMainModel()}
                              className="rounded-md border border-fg/10 px-2.5 py-1.5 text-[12px] font-medium text-fg/65 transition hover:border-fg/20 hover:bg-fg/5 hover:text-fg/90"
                            >
                              {t("common.buttons.browseFiles")}
                            </button>
                          </div>
                        }
                      >
                        <div className="space-y-3">
                          <input
                            type="text"
                            value={sdMainPathDraft}
                            onChange={(e) => setSdMainPathDraft(e.target.value)}
                            onBlur={(e) => void commitSdMainModel(e.target.value)}
                            placeholder={t("editModel.localDiffusion.pathPlaceholder")}
                            className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-4 py-3 font-mono text-[13px] text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none"
                          />
                          <p className="text-[13px] leading-relaxed text-fg/45">
                            {selectedSdEntry
                              ? `${selectedSdEntry.name} · ${selectedSdEntry.family.toUpperCase()}${selectedSdEntry.totalBytes > 0 ? ` · ${formatBytes(selectedSdEntry.totalBytes)}` : ""}`
                              : t("editModel.localDiffusion.idHelp")}
                          </p>
                          <BottomMenu
                            isOpen={showSdModelPicker}
                            onClose={() => setShowSdModelPicker(false)}
                            title={t("editModel.localDiffusion.selectModel")}
                          >
                            <MenuSection>
                              {!sdEntries || sdEntries.length === 0 ? (
                                <div className="flex flex-col items-center gap-2 py-16 text-center">
                                  <HardDrive size={32} className="text-fg/20" />
                                  <p className="px-6 text-[13px] text-fg/40">
                                    {t("imageGeneration.local.noModels")}
                                  </p>
                                </div>
                              ) : (
                                sdEntries
                                  .filter((entry) => entry.complete)
                                  .map((entry) => (
                                    <MenuButton
                                      key={entry.id}
                                      icon={<HardDrive className="h-5 w-5 text-accent/60" />}
                                      title={entry.name}
                                      description={`${entry.family.toUpperCase()} · ${formatBytes(entry.totalBytes)}`}
                                      color="from-accent/20 to-accent/10"
                                      rightElement={
                                        editorModel.name === entry.id ? (
                                          <Check className="h-4 w-4 text-accent" />
                                        ) : (
                                          <ArrowRight className="h-4 w-4 text-fg/20" />
                                        )
                                      }
                                      onClick={() => {
                                        handleModelNameChange(entry.id);
                                        if (!editorModel.displayName?.trim()) {
                                          handleDisplayNameChange(entry.name);
                                        }
                                        setShowSdModelPicker(false);
                                      }}
                                    />
                                  ))
                              )}
                            </MenuSection>
                          </BottomMenu>
                        </div>
                      </FieldBlock>
                    </div>
                  ) : (
                    <div className="grid items-start grid-cols-1 gap-6 lg:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
                      <FieldBlock label={t("editModel.fields.displayName")}>
                        <input
                          type="text"
                          value={editorModel.displayName}
                          onChange={(e) => handleDisplayNameChange(e.target.value)}
                          placeholder={t("editModel.placeholders.displayName")}
                          className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-4 py-3 text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none"
                        />
                      </FieldBlock>

                      <FieldBlock
                        label={modelIdLabel}
                        action={
                          <div className="flex flex-wrap items-center justify-end gap-2">
                            {fetchedModels.length > 0 && modelFetchEnabledForSelectedProvider && (
                              <button
                                type="button"
                                onClick={() => setIsManualInput(!isManualInput)}
                                className="rounded-md border border-fg/10 px-2.5 py-1 text-[13px] text-fg/65 transition hover:border-fg/20 hover:bg-fg/5 hover:text-fg/90"
                              >
                                {isManualInput
                                  ? t("editModel.modelSource.useCatalog")
                                  : t("editModel.modelSource.enterManually")}
                              </button>
                            )}
                            {modelFetchEnabledForSelectedProvider && (
                              <button
                                type="button"
                                onClick={fetchModels}
                                disabled={fetchingModels || !editorModel?.providerId}
                                className="rounded-md border border-fg/10 p-1.5 text-fg/45 transition hover:border-fg/20 hover:bg-fg/5 hover:text-fg/85 disabled:opacity-30"
                                title={t("editModel.modelSource.refreshModelList")}
                              >
                                <RefreshCw
                                  className={cn("h-3.5 w-3.5", fetchingModels && "animate-spin")}
                                />
                              </button>
                            )}
                          </div>
                        }
                      >
                        {modelFetchEnabledForSelectedProvider &&
                          !isManualInput &&
                          fetchedModels.length > 0 ? (
                          <>
                            <button
                              type="button"
                              onClick={() => setShowModelSelector(true)}
                              className="flex w-full items-center justify-between rounded-lg border border-fg/10 bg-surface-el/20 px-4 py-3 text-fg transition hover:bg-surface-el/30"
                            >
                              <span
                                className={cn(
                                  "block truncate text-left",
                                  !editorModel.name && "text-fg/40",
                                )}
                              >
                                {selectedFetchedModel?.displayName ||
                                  editorModel.name ||
                                  t("components.modelSelector.placeholder")}
                              </span>
                              <ChevronDown className="h-4 w-4 text-fg/40" />
                            </button>

                            <ModelSelectionBottomMenu
                              isOpen={showModelSelector}
                              onClose={() => setShowModelSelector(false)}
                              title={t("components.modelSelector.title")}
                              models={filteredModels as any}
                              selectedModelIds={editorModel.name ? [editorModel.name] : []}
                              searchQuery={searchQuery}
                              onSearchChange={setSearchQuery}
                              searchPlaceholder={t("components.modelSelector.searchPlaceholder")}
                              filterModels={false}
                              rightAction={
                                isOpenRouterProvider ? (
                                  <span className="flex items-center gap-2">
                                    <span className="text-[13px] text-fg/70 whitespace-nowrap">
                                      {t("editModel.modelSource.onlyFreeModels")}
                                    </span>
                                    <Switch
                                      checked={showOnlyFreeModels}
                                      onChange={setShowOnlyFreeModels}
                                    />
                                  </span>
                                ) : null
                              }
                              renderModelIcon={() => getProviderIcon(editorModel.providerId)}
                              renderModelTitle={(model: any) => model.displayName || model.id}
                              renderModelDescription={(model: any) => model.description || model.id}
                              renderModelMeta={
                                isOpenRouterProvider
                                  ? (model: any) => {
                                    const inputPrice = formatOpenRouterPricePerMillion(
                                      model.inputPrice,
                                    );
                                    const outputPrice = formatOpenRouterPricePerMillion(
                                      model.outputPrice,
                                    );
                                    if (!inputPrice && !outputPrice) return null;
                                    return (
                                      <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-fg/35">
                                        {inputPrice && (
                                          <span>
                                            {t("editModel.pricing.input", { price: inputPrice })}
                                          </span>
                                        )}
                                        {outputPrice && (
                                          <span>
                                            {t("editModel.pricing.output", { price: outputPrice })}
                                          </span>
                                        )}
                                      </div>
                                    );
                                  }
                                  : undefined
                              }
                              renderEmptyState={() => (
                                <div className="py-10 text-center text-[13px] text-fg/40">
                                  <p>
                                    {t("common.buttons.search")}: "{searchQuery}"
                                  </p>
                                  {didYouMeanSuggestions.length > 0 && (
                                    <div className="mt-4">
                                      <p className="mb-2 text-[13px] text-fg/50">
                                        {t("editModel.search.didYouMean")}
                                      </p>
                                      <div className="flex flex-wrap justify-center gap-2">
                                        {didYouMeanSuggestions.map((model) => (
                                          <button
                                            key={model.id}
                                            type="button"
                                            onClick={() => setSearchQuery(model.id)}
                                            className="rounded-full border border-fg/15 bg-fg/5 px-3 py-1.5 text-[13px] text-fg/80 transition hover:border-fg/30 hover:bg-fg/10"
                                          >
                                            {model.displayName || model.id}
                                          </button>
                                        ))}
                                      </div>
                                    </div>
                                  )}
                                </div>
                              )}
                              onSelectModel={(modelId) => {
                                const model = filteredModels.find((item) => item.id === modelId);
                                handleSelectModel(modelId, model?.displayName);
                              }}
                            />
                          </>
                        ) : (
                          <>
                            <input
                              type="text"
                              value={editorModel.name}
                              onChange={(e) => handleModelNameChange(e.target.value)}
                              placeholder={modelIdPlaceholder}
                              className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-4 py-3 font-mono text-[13px] text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none"
                            />
                            {!modelFetchEnabledForSelectedProvider &&
                              (selectedProviderCredential?.providerId === "custom" ||
                                selectedProviderCredential?.providerId === "custom-anthropic") && (
                                <p className="text-[13px] leading-relaxed text-fg/45">
                                  {t("editModel.modelSource.customEndpointFetchDisabled")}
                                </p>
                              )}
                          </>
                        )}
                      </FieldBlock>
                    </div>
                  )}
                </div>

                <BottomMenu
                  isOpen={showMovePrompt}
                  onClose={handleSkipMove}
                  title={t("editModel.moveModel.title")}
                >
                  <div className="px-4 pb-2">
                    <p className="text-[13px] leading-relaxed text-fg/70">
                      {t("hfBrowser.moveToLibrary")}
                    </p>
                    {moveError && (
                      <div className="mt-3 rounded-xl border border-danger/30 bg-danger/10 px-3 py-2">
                        <p className="text-[13px] text-danger/80">{moveError}</p>
                      </div>
                    )}
                  </div>
                  <MenuSection>
                    <MenuButton
                      icon={<FolderOpen className="h-5 w-5 text-accent" />}
                      title={t("hfBrowser.moveToLibraryYes")}
                      description={movingModel ? t("hfBrowser.moveToLibraryMoving") : undefined}
                      color="from-accent to-accent/80"
                      onClick={handleMoveToLibrary}
                      loading={movingModel}
                      disabled={movingModel}
                    />
                    <MenuButton
                      icon={<ArrowRight className="h-5 w-5 text-fg/40" />}
                      title={t("hfBrowser.moveToLibraryNo")}
                      color="from-white/10 to-white/5"
                      onClick={handleSkipMove}
                      disabled={movingModel}
                    />
                  </MenuSection>
                </BottomMenu>
              </section>

              <div className="border-t border-fg/10 pt-6">
                <div className="-mb-px flex gap-6 overflow-x-auto border-b border-fg/10 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
                  {editorPanels.map((panel) => {
                    const isActive = activePanel === panel.key;
                    return (
                      <button
                        key={panel.key}
                        type="button"
                        onClick={() => setActivePanel(panel.key)}
                        className={cn(
                          "shrink-0 border-b-2 px-1 pb-3 text-[13px] font-medium transition-colors",
                          isActive
                            ? "border-fg text-fg"
                            : "border-transparent text-fg/45 hover:text-fg/75",
                        )}
                      >
                        {panel.label}
                      </button>
                    );
                  })}
                </div>
                <AnimatePresence mode="wait" initial={false}>
                  <motion.div
                    key={activePanel}
                    initial={{ opacity: 0, y: 6 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0, y: -6 }}
                    transition={{ duration: EDITOR_FADE_DURATION, ease: "easeInOut" }}
                    className="pt-6"
                  >
                      <div className="space-y-8">
                        {/* Generation Parameters */}
                        {activeDetailPanel === "generation" && (
                          <div className="space-y-4">
                            <div className="flex items-start justify-between gap-3">
                              <p className="text-[12px] text-fg/45">{generationSummary}</p>
                              <button
                                type="button"
                                onClick={() => setShowParameterSupport(true)}
                                className="shrink-0 text-fg/40 hover:text-fg/60 transition"
                              >
                                <Info size={14} />
                              </button>
                            </div>

                            {isFixedImageProvider ? (
                              <div className="space-y-5">
                                <div className="text-[13px] leading-relaxed text-fg/55">
                                  {isLocalDiffusionModel
                                    ? t("editModel.generation.localDiffusionHelp")
                                    : t("editModel.generation.automatic1111Help")}
                                </div>

                                <div className="grid grid-cols-1 gap-x-6 gap-y-8 md:grid-cols-2 xl:grid-cols-3 xl:gap-x-8">
                                  <div className="space-y-4">
                                    <div className="flex items-center justify-between">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.genLabels.steps")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.generationDescriptions.sdSteps")}
                                        </span>
                                      </div>
                                      <span className="font-mono text-[13px] text-fg/55">
                                        {modelAdvancedDraft.sdSteps ?? "28"}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_SD_STEPS_RANGE.min}
                                      max={ADVANCED_SD_STEPS_RANGE.max}
                                      step={1}
                                      value={modelAdvancedDraft.sdSteps ?? null}
                                      onChange={(next) =>
                                        updateSdSetting(
                                          "sdSteps",
                                          next === null ? null : Math.trunc(next),
                                        )
                                      }
                                      placeholder={t("editModel.placeholders.sdSteps")}
                                      className={numberInputClassName}
                                    />
                                    <div className="flex justify-between text-[13px] text-fg/30 px-0.5 mt-1">
                                      <span>{ADVANCED_SD_STEPS_RANGE.min}</span>
                                      <span>{ADVANCED_SD_STEPS_RANGE.max}</span>
                                    </div>
                                  </div>

                                  <div className="space-y-4">
                                    <div className="flex items-center justify-between">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.genLabels.cfgScale")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.generationDescriptions.sdCfgScale")}
                                        </span>
                                      </div>
                                      <span className="font-mono text-[13px] text-fg/55">
                                        {modelAdvancedDraft.sdCfgScale?.toFixed(1) ?? "6.5"}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_SD_CFG_SCALE_RANGE.min}
                                      max={ADVANCED_SD_CFG_SCALE_RANGE.max}
                                      step={0.1}
                                      decimals={2}
                                      value={modelAdvancedDraft.sdCfgScale ?? null}
                                      onChange={(next) => updateSdSetting("sdCfgScale", next)}
                                      placeholder={t("editModel.placeholders.sdCfgScale")}
                                      className={numberInputClassName}
                                    />
                                    <div className="flex justify-between text-[13px] text-fg/30 px-0.5 mt-1">
                                      <span>{ADVANCED_SD_CFG_SCALE_RANGE.min}</span>
                                      <span>{ADVANCED_SD_CFG_SCALE_RANGE.max}</span>
                                    </div>
                                  </div>

                                  <div className="space-y-4">
                                    <div className="flex items-center justify-between">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.genLabels.defaultSize")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.generationDescriptions.sdSize")}
                                        </span>
                                      </div>
                                      <span className="font-mono text-[13px] text-fg/55">
                                        {modelAdvancedDraft.sdSize ?? "1024x1024"}
                                      </span>
                                    </div>
                                    <input
                                      type="text"
                                      value={modelAdvancedDraft.sdSize ?? ""}
                                      onChange={(e) => updateSdSetting("sdSize", e.target.value)}
                                      placeholder={t("editModel.placeholders.sdSize")}
                                      className={numberInputClassName}
                                    />
                                    <div className="text-[13px] text-fg/30 px-0.5 mt-1">
                                      {t("editModel.generation.formatWidthHeight")}
                                    </div>
                                  </div>

                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.genLabels.sampler")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.generationDescriptions.sdSampler")}
                                      </span>
                                    </div>
                                    <input
                                      type="text"
                                      value={modelAdvancedDraft.sdSampler ?? ""}
                                      onChange={(e) => updateSdSetting("sdSampler", e.target.value)}
                                      placeholder={t("editModel.placeholders.sdSampler")}
                                      className={selectInputClassName}
                                    />
                                  </div>

                                  <div className="space-y-4">
                                    <div className="flex items-center justify-between">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.genLabels.seed")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.generationDescriptions.sdSeed")}
                                        </span>
                                      </div>
                                      <span className="font-mono text-[13px] text-fg/55">
                                        {modelAdvancedDraft.sdSeed ?? t("editModel.placeholders.random")}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_SD_SEED_RANGE.min}
                                      max={ADVANCED_SD_SEED_RANGE.max}
                                      step={1}
                                      value={modelAdvancedDraft.sdSeed ?? null}
                                      onChange={(next) =>
                                        updateSdSetting(
                                          "sdSeed",
                                          next === null ? null : Math.trunc(next),
                                        )
                                      }
                                      placeholder={t("editModel.placeholders.random")}
                                      className={numberInputClassName}
                                    />
                                    <div className="flex justify-between text-[13px] text-fg/30 px-0.5 mt-1">
                                      <span>{t("editModel.placeholders.random")}</span>
                                      <span>{ADVANCED_SD_SEED_RANGE.max.toLocaleString()}</span>
                                    </div>
                                  </div>

                                  <div className="space-y-4">
                                    <div className="flex items-center justify-between">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.genLabels.img2imgDenoise")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.generationDescriptions.sdDenoise")}
                                        </span>
                                      </div>
                                      <span className="font-mono text-[13px] text-fg/55">
                                        {modelAdvancedDraft.sdDenoisingStrength?.toFixed(2) ??
                                          "0.75"}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_SD_DENOISING_STRENGTH_RANGE.min}
                                      max={ADVANCED_SD_DENOISING_STRENGTH_RANGE.max}
                                      step={0.01}
                                      decimals={2}
                                      value={modelAdvancedDraft.sdDenoisingStrength ?? null}
                                      onChange={(next) =>
                                        updateSdSetting("sdDenoisingStrength", next)
                                      }
                                      placeholder={t("editModel.placeholders.sdDenoise")}
                                      className={numberInputClassName}
                                    />
                                    <div className="flex justify-between text-[13px] text-fg/30 px-0.5 mt-1">
                                      <span>{ADVANCED_SD_DENOISING_STRENGTH_RANGE.min}</span>
                                      <span>{ADVANCED_SD_DENOISING_STRENGTH_RANGE.max}</span>
                                    </div>
                                  </div>
                                </div>

                                <div className="space-y-4">
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      {t("editModel.genLabels.negativePrompt")}
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      {t("editModel.generationDescriptions.sdNegativePrompt")}
                                    </span>
                                  </div>
                                  <textarea
                                    value={modelAdvancedDraft.sdNegativePrompt ?? ""}
                                    onChange={(e) =>
                                      updateSdSetting("sdNegativePrompt", e.target.value)
                                    }
                                    placeholder={t("editModel.placeholders.sdNegativePrompt")}
                                    rows={4}
                                    className={textAreaInputClassName}
                                  />
                                </div>

                                <div className="space-y-4">
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      {t("editModel.generationDescriptions.sdExtraPromptTitle")}
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      {t("editModel.generationDescriptions.sdExtraPrompt")}
                                    </span>
                                  </div>
                                  <textarea
                                    value={modelAdvancedDraft.sdExtraPrompt ?? ""}
                                    onChange={(e) =>
                                      updateSdSetting("sdExtraPrompt", e.target.value)
                                    }
                                    placeholder={t("editModel.placeholders.sdExtraPrompt")}
                                    rows={4}
                                    className={textAreaInputClassName}
                                  />
                                </div>

                                <div className="space-y-4">
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      {t(
                                        "editModel.generationDescriptions.sdWriterInstructionsTitle",
                                      )}
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      {t("editModel.generationDescriptions.sdWriterInstructions")}
                                    </span>
                                  </div>
                                  <textarea
                                    value={modelAdvancedDraft.sdPromptWriterInstructions ?? ""}
                                    onChange={(e) =>
                                      updateSdSetting("sdPromptWriterInstructions", e.target.value)
                                    }
                                    placeholder={t("editModel.placeholders.sdWriterInstructions")}
                                    rows={4}
                                    className={textAreaInputClassName}
                                  />
                                </div>

                                {isLocalDiffusionModel ? (
                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.generationDescriptions.sdOffloadTitle")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.generationDescriptions.sdOffloadMode")}
                                      </span>
                                    </div>
                                    <div className="grid grid-cols-3 gap-2">
                                      {(
                                        [
                                          ["auto", t("editModel.sdOffload.auto"), t("editModel.sdOffload.autoHint")],
                                          ["gpu", t("editModel.sdOffload.gpu"), t("editModel.sdOffload.gpuHint")],
                                          ["mixed", t("editModel.sdOffload.mixed"), t("editModel.sdOffload.mixedHint")],
                                        ] as const
                                      ).map(([value, label, hint]) => {
                                        const active =
                                          (modelAdvancedDraft.sdOffloadMode ?? "auto") === value;
                                        return (
                                          <button
                                            key={value}
                                            type="button"
                                            onClick={() =>
                                              updateSdSetting(
                                                "sdOffloadMode",
                                                value === "auto" ? null : value,
                                              )
                                            }
                                            className={cn(
                                              "rounded-lg border px-3 py-2.5 text-left transition",
                                              active
                                                ? "border-accent/30 bg-accent/10"
                                                : "border-fg/10 bg-fg/5 hover:border-fg/20 hover:bg-fg/8",
                                            )}
                                          >
                                            <span
                                              className={cn(
                                                "block text-[13px] font-medium",
                                                active ? "text-accent" : "text-fg/70",
                                              )}
                                            >
                                              {label}
                                            </span>
                                            <span className="mt-0.5 block text-[12px] leading-snug text-fg/40">
                                              {hint}
                                            </span>
                                          </button>
                                        );
                                      })}
                                    </div>
                                  </div>
                                ) : null}
                              </div>
                            ) : (
                              <div className="grid grid-cols-1 gap-x-6 gap-y-8 md:grid-cols-2 xl:grid-cols-3 xl:gap-x-8">
                                {/* Temperature */}
                                <div className="space-y-4">
                                  <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-2">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.genLabels.temperature")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.generationDescriptions.temperature")}
                                        </span>
                                      </div>
                                      <button
                                        type="button"
                                        onClick={() => openDocs("models", "temperature")}
                                        className="text-fg/30 hover:text-fg/60 transition"
                                        aria-label={t("editModel.help.temperature")}
                                      >
                                        <HelpCircle size={12} />
                                      </button>
                                    </div>
                                    <span className="font-mono text-[13px] text-fg/55">
                                      {modelAdvancedDraft.temperature?.toFixed(2) ?? "0.70"}
                                    </span>
                                  </div>
                                  <NumberInput
                                    min={ADVANCED_TEMPERATURE_RANGE.min}
                                    max={ADVANCED_TEMPERATURE_RANGE.max}
                                    step={0.01}
                                    decimals={2}
                                    value={modelAdvancedDraft.temperature ?? null}
                                    onChange={(next) => handleTemperatureChange(next)}
                                    placeholder={t("editModel.placeholders.temperature")}
                                    className={numberInputClassName}
                                  />
                                  <div className="flex justify-between text-[13px] text-fg/30 px-0.5 mt-1">
                                    <span>{ADVANCED_TEMPERATURE_RANGE.min}</span>
                                    <span>{ADVANCED_TEMPERATURE_RANGE.max}</span>
                                  </div>
                                </div>

                                {/* Top P */}
                                <div className="space-y-4">
                                  <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-2">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.genLabels.topP")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.generationDescriptions.topP")}
                                        </span>
                                      </div>
                                      <button
                                        type="button"
                                        onClick={() => openDocs("models", "top-p")}
                                        className="text-fg/30 hover:text-fg/60 transition"
                                        aria-label={t("editModel.help.topP")}
                                      >
                                        <HelpCircle size={12} />
                                      </button>
                                    </div>
                                    <span className="font-mono text-[13px] text-fg/55">
                                      {modelAdvancedDraft.topP?.toFixed(2) ?? "1.00"}
                                    </span>
                                  </div>
                                  <NumberInput
                                    min={ADVANCED_TOP_P_RANGE.min}
                                    max={ADVANCED_TOP_P_RANGE.max}
                                    step={0.01}
                                    decimals={2}
                                    value={modelAdvancedDraft.topP ?? null}
                                    onChange={(next) => handleTopPChange(next)}
                                    placeholder={t("editModel.placeholders.topP")}
                                    className={numberInputClassName}
                                  />
                                  <div className="flex justify-between text-[13px] text-fg/30 px-0.5 mt-1">
                                    <span>{ADVANCED_TOP_P_RANGE.min}</span>
                                    <span>{ADVANCED_TOP_P_RANGE.max}</span>
                                  </div>
                                </div>

                                {/* Max Tokens */}
                                <div className="space-y-4">
                                  <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-2">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.genLabels.maxOutputTokens")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.generationDescriptions.maxOutputTokens")}
                                        </span>
                                      </div>
                                      <button
                                        type="button"
                                        onClick={() => openDocs("models", "max-output-tokens")}
                                        className="text-fg/30 hover:text-fg/60 transition"
                                        aria-label={t("editModel.help.maxOutputTokens")}
                                      >
                                        <HelpCircle size={12} />
                                      </button>
                                    </div>
                                    <span className="font-mono text-[13px] text-fg/55">
                                      {modelAdvancedDraft.maxOutputTokens
                                        ? modelAdvancedDraft.maxOutputTokens.toLocaleString()
                                        : t("common.labels.auto")}
                                    </span>
                                  </div>
                                  <NumberInput
                                    min={ADVANCED_MAX_TOKENS_RANGE.min}
                                    max={ADVANCED_MAX_TOKENS_RANGE.max}
                                    step={1}
                                    value={modelAdvancedDraft.maxOutputTokens || null}
                                    onChange={(next) =>
                                      handleMaxTokensChange(
                                        next === null || next === 0 ? null : Math.trunc(next),
                                      )
                                    }
                                    placeholder={t("common.labels.auto")}
                                    className={numberInputClassName}
                                  />
                                  <div className="flex justify-between text-[13px] text-fg/30 px-0.5 mt-1">
                                    <span>{t("common.labels.auto")}</span>
                                    <span>{ADVANCED_MAX_TOKENS_RANGE.max.toLocaleString()}</span>
                                  </div>
                                </div>

                                {/* Top K */}
                                <div className="space-y-4">
                                  <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-2">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.genLabels.topK")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.generationDescriptions.topK")}
                                        </span>
                                      </div>
                                      <button
                                        type="button"
                                        onClick={() => openDocs("models", "top-k-if-supported")}
                                        className="text-fg/30 hover:text-fg/60 transition"
                                        aria-label={t("editModel.help.topK")}
                                      >
                                        <HelpCircle size={12} />
                                      </button>
                                    </div>
                                    <span className="font-mono text-[13px] text-fg/55">
                                      {modelAdvancedDraft.topK
                                        ? modelAdvancedDraft.topK
                                        : t("common.labels.auto")}
                                    </span>
                                  </div>
                                  <NumberInput
                                    min={ADVANCED_TOP_K_RANGE.min}
                                    max={ADVANCED_TOP_K_RANGE.max}
                                    step={1}
                                    value={modelAdvancedDraft.topK || null}
                                    onChange={(next) =>
                                      handleTopKChange(
                                        next === null || next === 0 ? null : Math.trunc(next),
                                      )
                                    }
                                    placeholder={t("common.labels.auto")}
                                    className={numberInputClassName}
                                  />
                                  <div className="flex justify-between text-[13px] text-fg/30 px-0.5 mt-1">
                                    <span>{t("common.labels.auto")}</span>
                                    <span>{ADVANCED_TOP_K_RANGE.max}</span>
                                  </div>
                                </div>

                                {/* Penalties - Frequency */}
                                <div className="space-y-4">
                                  <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-2">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.genLabels.frequencyPenalty")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.generationDescriptions.frequencyPenalty")}
                                        </span>
                                      </div>
                                      <button
                                        type="button"
                                        onClick={() => openDocs("models", "frequency-penalty")}
                                        className="text-fg/30 hover:text-fg/60 transition"
                                        aria-label={t("editModel.help.frequencyPenalty")}
                                      >
                                        <HelpCircle size={12} />
                                      </button>
                                    </div>
                                    <span className="font-mono text-[13px] text-fg/55">
                                      {modelAdvancedDraft.frequencyPenalty?.toFixed(2) ?? "0.00"}
                                    </span>
                                  </div>
                                  <NumberInput
                                    min={ADVANCED_FREQUENCY_PENALTY_RANGE.min}
                                    max={ADVANCED_FREQUENCY_PENALTY_RANGE.max}
                                    step={0.01}
                                    decimals={2}
                                    value={modelAdvancedDraft.frequencyPenalty ?? null}
                                    onChange={(next) => handleFrequencyPenaltyChange(next)}
                                    placeholder={t("editModel.placeholders.zero")}
                                    className={numberInputClassName}
                                  />
                                  <div className="flex justify-between text-[13px] text-fg/30 px-0.5 mt-1">
                                    <span>{ADVANCED_FREQUENCY_PENALTY_RANGE.min}</span>
                                    <span>{ADVANCED_FREQUENCY_PENALTY_RANGE.max}</span>
                                  </div>
                                </div>

                                {/* Penalties - Presence */}
                                <div className="space-y-4">
                                  <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-2">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.genLabels.presencePenalty")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.generationDescriptions.presencePenalty")}
                                        </span>
                                      </div>
                                      <button
                                        type="button"
                                        onClick={() => openDocs("models", "presence-penalty")}
                                        className="text-fg/30 hover:text-fg/60 transition"
                                        aria-label={t("editModel.help.presencePenalty")}
                                      >
                                        <HelpCircle size={12} />
                                      </button>
                                    </div>
                                    <span className="font-mono text-[13px] text-fg/55">
                                      {modelAdvancedDraft.presencePenalty?.toFixed(2) ?? "0.00"}
                                    </span>
                                  </div>
                                  <NumberInput
                                    min={ADVANCED_PRESENCE_PENALTY_RANGE.min}
                                    max={ADVANCED_PRESENCE_PENALTY_RANGE.max}
                                    step={0.01}
                                    decimals={2}
                                    value={modelAdvancedDraft.presencePenalty ?? null}
                                    onChange={(next) => handlePresencePenaltyChange(next)}
                                    placeholder={t("editModel.placeholders.zero")}
                                    className={numberInputClassName}
                                  />
                                  <div className="flex justify-between text-[13px] text-fg/30 px-0.5 mt-1">
                                    <span>{ADVANCED_PRESENCE_PENALTY_RANGE.min}</span>
                                    <span>{ADVANCED_PRESENCE_PENALTY_RANGE.max}</span>
                                  </div>
                                </div>
                              </div>
                            )}
                          </div>
                        )}

                        {/* Local llama.cpp Settings */}
                        {activeDetailPanel === "configuration" && isLocalDiffusionModel && (
                          <div className="space-y-6">
                            <p className="text-[13px] leading-relaxed text-fg/55">
                              {t("editModel.localDiffusion.configurationHelp")}
                            </p>
                            <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
                              {(
                                [
                                  ["checkpoint", t("imageGeneration.local.roles.checkpoint")],
                                  ["diffusionModel", t("imageGeneration.local.roles.diffusionModel")],
                                  ["clipL", t("imageGeneration.local.roles.clipL")],
                                  ["clipG", t("imageGeneration.local.roles.clipG")],
                                  ["t5xxl", t("imageGeneration.local.roles.t5xxl")],
                                  ["llm", t("imageGeneration.local.roles.llm")],
                                  ["llmVision", t("imageGeneration.local.roles.llmVision")],
                                  ["vae", t("imageGeneration.local.roles.vae")],
                                ] as Array<[SdModelRole, string]>
                              ).map(([role, label]) => (
                                <FieldBlock
                                  key={role}
                                  label={label}
                                  action={
                                    <div className="flex items-center gap-2">
                                      <button
                                        type="button"
                                        onClick={() => setSdLibraryRole(role)}
                                        className="inline-flex items-center gap-1.5 rounded-md border border-fg/10 bg-fg/5 px-2.5 py-1.5 text-[12px] font-medium text-fg/68 transition hover:border-fg/20 hover:bg-fg/10 hover:text-fg"
                                      >
                                        <FolderOpen className="h-3.5 w-3.5 text-accent/70" />
                                        {t("hfBrowser.selectFromLibrary")}
                                      </button>
                                      <button
                                        type="button"
                                        onClick={() => void browseSdFile(role)}
                                        className="rounded-md border border-fg/10 px-2.5 py-1.5 text-[12px] font-medium text-fg/65 transition hover:border-fg/20 hover:bg-fg/5 hover:text-fg/90"
                                      >
                                        {t("common.buttons.browseFiles")}
                                      </button>
                                    </div>
                                  }
                                >
                                  <input
                                    type="text"
                                    value={sdFilesDraft[role] ?? ""}
                                    onChange={(e) =>
                                      setSdFilesDraft((draft) => ({
                                        ...draft,
                                        [role]: e.target.value,
                                      }))
                                    }
                                    onBlur={(e) => void commitSdFile(role, e.target.value)}
                                    placeholder={t("editModel.localDiffusion.pathPlaceholder")}
                                    className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-4 py-3 font-mono text-[13px] text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none"
                                  />
                                </FieldBlock>
                              ))}
                            </div>
                            {selectedSdEntry && !selectedSdEntry.complete ? (
                              <p className="text-[12px] leading-relaxed text-warning/80">
                                {t("imageGeneration.local.missingFiles")}
                              </p>
                            ) : null}

                            <BottomMenu
                              isOpen={sdLibraryRole !== null}
                              onClose={() => setSdLibraryRole(null)}
                              title={t("hfBrowser.selectFromLibrary")}
                            >
                              <MenuSection>
                                {!sdLibraryFiles ? (
                                  <div className="flex items-center justify-center gap-2 py-12 text-fg/50">
                                    <Loader size={18} className="animate-spin" />
                                    <span className="text-[13px]">{t("hfBrowser.searching")}</span>
                                  </div>
                                ) : sdLibraryFiles.length === 0 ? (
                                  <div className="flex flex-col items-center gap-2 py-16 text-center">
                                    <HardDrive size={32} className="text-fg/20" />
                                    <p className="text-[13px] font-medium text-fg/60">
                                      {t("hfBrowser.libraryEmpty")}
                                    </p>
                                  </div>
                                ) : (
                                  sdLibraryFiles.map((file) => (
                                    <MenuButton
                                      key={file.path}
                                      icon={<HardDrive className="h-5 w-5 text-accent/60" />}
                                      title={file.filename}
                                      description={formatBytes(file.size)}
                                      color="from-accent/20 to-accent/10"
                                      rightElement={
                                        sdLibraryRole !== null &&
                                        sdFilesDraft[sdLibraryRole] === file.path ? (
                                          <Check className="h-4 w-4 text-accent" />
                                        ) : (
                                          <ArrowRight className="h-4 w-4 text-fg/20" />
                                        )
                                      }
                                      onClick={() => {
                                        const role = sdLibraryRole;
                                        setSdLibraryRole(null);
                                        if (!role) return;
                                        setSdFilesDraft((draft) => ({
                                          ...draft,
                                          [role]: file.path,
                                        }));
                                        void commitSdFile(role, file.path);
                                      }}
                                    />
                                  ))
                                )}
                              </MenuSection>
                            </BottomMenu>
                          </div>
                        )}

                        {activeDetailPanel === "runtime" && isLocalModel && (
                          <div className="space-y-4">
                            <p className="text-[12px] text-fg/45">
                              {runtimePanelTitle} · {runtimeSummary}
                            </p>

                            <div className="grid grid-cols-1 gap-6 xl:grid-cols-2 xl:items-start">
                              {/* 1. Memory & Context */}
                              <div className="space-y-6">
                                <div className="flex items-center gap-2 border-l-2 border-fg/20 pl-3">
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-bold text-fg/80 uppercase tracking-tight">
                                      {t("editModel.runtimeSections.memoryContextTitle")}
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      {t("editModel.runtimeSections.memoryContextDescription")}
                                    </span>
                                  </div>
                                </div>

                                {/* Context Length */}
                                <div className="space-y-4">
                                  <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-2">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.layerPlacement.contextLength")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.layerPlacement.contextOverride")}
                                        </span>
                                      </div>
                                      <button
                                        type="button"
                                        onClick={() => openDocs("models", "context-length")}
                                        className="text-fg/30 hover:text-fg/60 transition"
                                        aria-label={t("editModel.help.contextLength")}
                                      >
                                        <HelpCircle size={12} />
                                      </button>
                                    </div>
                                    <span className="font-mono text-[13px] text-fg/55">
                                      {modelAdvancedDraft.contextLength
                                        ? modelAdvancedDraft.contextLength.toLocaleString()
                                        : t("common.labels.auto")}
                                    </span>
                                  </div>
                                  <div className="space-y-3">
                                    <NumberInput
                                      min={ADVANCED_CONTEXT_LENGTH_RANGE.min}
                                      max={contextLimit}
                                      step={1}
                                      value={modelAdvancedDraft.contextLength || null}
                                      onChange={(next) =>
                                        handleContextLengthChange(
                                          next === null || next === 0 ? null : Math.trunc(next),
                                        )
                                      }
                                      placeholder={t("common.labels.auto")}
                                      className={numberInputClassName}
                                    />
                                    <div className="mt-1 flex justify-between px-0.5 text-[13px] text-fg/30">
                                      <span>{t("common.labels.auto")}</span>
                                      <span>{contextLimit.toLocaleString()}</span>
                                    </div>
                                    {llamaContextLoading && (
                                      <p className="text-[13px] text-fg/40">
                                        {t("editModel.layerPlacement.calculatingMemory")}
                                      </p>
                                    )}
                                    {llamaContextError && (
                                      <p className="text-[13px] text-warning/80">
                                        {llamaContextError}
                                      </p>
                                    )}
                                    {showContextWarning && (
                                      <div className="flex items-start gap-2 text-[13px] text-warning/80">
                                        <AlertTriangle size={14} className="mt-0.5 shrink-0" />
                                        <span>
                                          {t("editModel.layerPlacement.contextWarning", {
                                            recommended:
                                              recommendedContextLength?.toLocaleString() ?? "",
                                          })}
                                        </span>
                                      </div>
                                    )}
                                    {showContextCritical && (
                                      <div className="flex items-start gap-2 text-[13px] text-danger/80">
                                        <AlertTriangle size={14} className="mt-0.5 shrink-0" />
                                        <span>{t("editModel.layerPlacement.contextCritical")}</span>
                                      </div>
                                    )}
                                  </div>
                                </div>

                                {/* Runability & System Info */}
                                {editorModel?.name?.trim() && (
                                  <div className="text-[13px] text-fg/55">
                                    {runabilityLoading ? (
                                      /* Skeleton */
                                      <div className="space-y-3 animate-pulse">
                                        <div className="flex items-center justify-between">
                                          <div className="flex items-center gap-3">
                                            <div className="h-4 w-20 rounded bg-fg/8" />
                                            <div className="h-5 w-14 rounded-md bg-fg/8" />
                                          </div>
                                          <div className="flex gap-2">
                                            <div className="h-5 w-18 rounded-md bg-fg/8" />
                                            <div className="h-5 w-20 rounded-md bg-fg/8" />
                                          </div>
                                        </div>
                                        <div className="grid grid-cols-2 gap-x-6 gap-y-2 sm:grid-cols-3 lg:grid-cols-6">
                                          {Array.from({ length: 6 }).map((_, i) => (
                                            <div key={i} className="space-y-1">
                                              <div className="h-3 w-16 rounded bg-fg/6" />
                                              <div className="h-4 w-12 rounded bg-fg/8" />
                                            </div>
                                          ))}
                                        </div>
                                        <div className="space-y-2">
                                          {Array.from({ length: 4 }).map((_, i) => (
                                            <div key={i} className="space-y-1">
                                              <div className="flex justify-between">
                                                <div className="h-3 w-28 rounded bg-fg/6" />
                                                <div className="h-3 w-8 rounded bg-fg/8" />
                                              </div>
                                              <div className="h-1.5 w-full rounded-full bg-fg/8" />
                                            </div>
                                          ))}
                                        </div>
                                      </div>
                                    ) : (
                                      <div className="space-y-3">
                                        {/* Header row */}
                                        {runabilityScore && (
                                          <div className="flex items-center justify-between">
                                            <div className="flex items-center gap-3">
                                              <span className="font-medium text-fg/70">
                                                {t("editModel.runability.title")}
                                              </span>
                                              <span
                                                className={cn(
                                                  "rounded-md border px-2 py-0.5 text-[12px] font-semibold",
                                                  runabilityScore.label === "excellent"
                                                    ? "border-emerald-400/30 bg-emerald-400/15 text-emerald-400"
                                                    : runabilityScore.label === "good"
                                                      ? "border-blue-400/30 bg-blue-400/15 text-blue-400"
                                                      : runabilityScore.label === "marginal"
                                                        ? "border-amber-400/30 bg-amber-400/15 text-amber-400"
                                                        : runabilityScore.label === "poor"
                                                          ? "border-orange-400/30 bg-orange-400/15 text-orange-400"
                                                          : "border-red-400/30 bg-red-400/15 text-red-400",
                                                )}
                                              >
                                                {runabilityScore.score}/100
                                              </span>
                                            </div>
                                            <div className="flex items-center gap-2 text-[11px]">
                                              {runabilityScore.fitsInRam && (
                                                <span className="rounded-md bg-emerald-400/10 px-1.5 py-0.5 text-emerald-400/80">
                                                  {t("editModel.runability.fitsInRam")}
                                                </span>
                                              )}
                                              {runabilityScore.fitsInVram && (
                                                <span className="rounded-md bg-emerald-400/10 px-1.5 py-0.5 text-emerald-400/80">
                                                  {t("editModel.runability.fitsInVram")}
                                                </span>
                                              )}
                                            </div>
                                          </div>
                                        )}

                                        {/* System info stats row */}
                                        {(llamaContextInfo ||
                                          availableRamGiB ||
                                          availableVramGiB ||
                                          modelSizeGiB) && (
                                            <div className="grid grid-cols-2 gap-x-6 gap-y-2 text-[13px] leading-5 text-fg/52 sm:grid-cols-3 lg:grid-cols-6">
                                              <div>
                                                <div className="text-fg/38">
                                                  {t("editModel.contextInfo.maxSupported")}
                                                </div>
                                                <div className="font-mono text-fg/78">
                                                  {llamaContextInfo
                                                    ? llamaContextInfo.maxContextLength.toLocaleString()
                                                    : contextLimit.toLocaleString()}
                                                </div>
                                              </div>
                                              {recommendedContextLength !== null && (
                                                <div>
                                                  <div className="text-fg/38">
                                                    {t("editModel.contextInfo.recommended")}
                                                  </div>
                                                  <div className="font-mono text-fg/78">
                                                    {recommendedContextLength.toLocaleString()}
                                                  </div>
                                                </div>
                                              )}
                                              {availableRamGiB && (
                                                <div>
                                                  <div className="text-fg/38">
                                                    {t("editModel.contextInfo.availableRam")}
                                                  </div>
                                                  <div className="font-mono text-fg/78">
                                                    {availableRamGiB} GB
                                                  </div>
                                                </div>
                                              )}
                                              {availableVramGiB && (
                                                <div>
                                                  <div className="text-fg/38">
                                                    {t("editModel.contextInfo.availableVram")}
                                                  </div>
                                                  <div className="font-mono text-fg/78">
                                                    {availableVramGiB} GB
                                                  </div>
                                                </div>
                                              )}
                                              {modelSizeGiB && (
                                                <div>
                                                  <div className="text-fg/38">
                                                    {t("editModel.contextInfo.modelSize")}
                                                  </div>
                                                  <div className="font-mono text-fg/78">
                                                    {modelSizeGiB} GB
                                                  </div>
                                                </div>
                                              )}
                                              <div>
                                                <div className="text-fg/38">
                                                  {t("editModel.contextInfo.contextCache")}
                                                </div>
                                                <div className="font-mono text-fg/78">
                                                  {contextCacheLocationLabel}
                                                </div>
                                              </div>
                                            </div>
                                          )}
                                        {!selectedContextLength &&
                                          recommendedContextLength &&
                                          recommendedContextLength > 0 &&
                                          !runabilityScore && (
                                            <p className="text-fg/52">
                                              {t("editModel.runability.autoRecommended")}
                                            </p>
                                          )}

                                        {/* Score breakdown bars */}
                                        {runabilityScore && (
                                          <>
                                            <div className="space-y-2">
                                              {(
                                                [
                                                  {
                                                    label: t("editModel.contextInfo.memoryFitness"),
                                                    value: runabilityScore.memoryScore,
                                                    weight: 0.25,
                                                  },
                                                  {
                                                    label: t("editModel.contextInfo.gpuAcceleration"),
                                                    value: runabilityScore.gpuScore,
                                                    weight: 0.35,
                                                  },
                                                  {
                                                    label: t("editModel.contextInfo.kvHeadroom"),
                                                    value: runabilityScore.kvScore,
                                                    weight: 0.15,
                                                  },
                                                  {
                                                    label: t("editModel.contextInfo.quantQuality"),
                                                    value: runabilityScore.quantScore,
                                                    weight: 0.25,
                                                  },
                                                ] as const
                                              ).map((item) => (
                                                <div key={item.label} className="space-y-1">
                                                  <div className="flex items-center justify-between">
                                                    <span className="text-[12px] text-fg/50">
                                                      {item.label}{" "}
                                                      <span className="text-fg/30">
                                                        ({Math.round(item.weight * 100)}%)
                                                      </span>
                                                    </span>
                                                    <span
                                                      className={cn(
                                                        "text-[12px] font-mono font-medium",
                                                        item.value >= 70
                                                          ? "text-emerald-400"
                                                          : item.value >= 40
                                                            ? "text-amber-400"
                                                            : "text-red-400",
                                                      )}
                                                    >
                                                      {item.value}
                                                    </span>
                                                  </div>
                                                  <div className="h-1.5 w-full overflow-hidden rounded-full bg-fg/8">
                                                    <div
                                                      className={cn(
                                                        "h-full rounded-full transition-all duration-300",
                                                        item.value >= 70
                                                          ? "bg-emerald-400/60"
                                                          : item.value >= 40
                                                            ? "bg-amber-400/60"
                                                            : "bg-red-400/60",
                                                      )}
                                                      style={{ width: `${item.value}%` }}
                                                    />
                                                  </div>
                                                </div>
                                              ))}
                                            </div>
                                            {runabilityScore.quantization && (
                                              <div className="flex items-center gap-3 border-t border-fg/8 pt-2 text-[12px] text-fg/45">
                                                <span>
                                                  {t("editModel.runability.quantization")}{" "}
                                                  <span className="font-mono text-fg/65">
                                                    {runabilityScore.quantization}
                                                  </span>
                                                </span>
                                                <span>
                                                  {t("editModel.runability.size")}{" "}
                                                  <span className="font-mono text-fg/65">
                                                    {formatBytes(runabilityScore.modelSize)}
                                                  </span>
                                                </span>
                                              </div>
                                            )}
                                          </>
                                        )}
                                      </div>
                                    )}
                                  </div>
                                )}

                                <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.layerPlacement.kvCacheType")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.layerPlacement.kvCacheTypeDescription")}
                                      </span>
                                    </div>
                                    <select
                                      value={modelAdvancedDraft.llamaKvType ?? "auto"}
                                      onChange={(e) =>
                                        handleLlamaKvTypeChange(
                                          e.target.value === "auto"
                                            ? null
                                            : (e.target.value as NonNullable<
                                              typeof modelAdvancedDraft.llamaKvType
                                            >),
                                        )
                                      }
                                      className={selectInputClassName}
                                    >
                                      {LLAMA_KV_TYPE_OPTIONS.map((option) => (
                                        <option
                                          key={option.value}
                                          value={option.value}
                                          className="bg-[#16171d]"
                                        >
                                          {t(option.labelKey)}
                                        </option>
                                      ))}
                                    </select>
                                  </div>

                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.layerPlacement.offloadKqv")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {isCpuOnlyLlamaBackend
                                          ? t("editModel.layerPlacement.offloadKqvCpuOnly")
                                          : t("editModel.layerPlacement.offloadKqvDescription")}
                                      </span>
                                    </div>
                                    <select
                                      value={
                                        modelAdvancedDraft.llamaOffloadKqv === null ||
                                          modelAdvancedDraft.llamaOffloadKqv === undefined
                                          ? "auto"
                                          : modelAdvancedDraft.llamaOffloadKqv
                                            ? "on"
                                            : "off"
                                      }
                                      onChange={(e) => {
                                        const val = e.target.value;
                                        handleLlamaOffloadKqvChange(
                                          val === "auto" ? null : val === "on",
                                        );
                                      }}
                                      disabled={isCpuOnlyLlamaBackend}
                                      className={cn(
                                        selectInputClassName,
                                        isCpuOnlyLlamaBackend && "cursor-not-allowed opacity-60",
                                      )}
                                    >
                                      <option value="auto" className="bg-[#16171d]">
                                        {t("common.labels.auto")}
                                      </option>
                                      <option value="on" className="bg-[#16171d]">
                                        {t("common.labels.on")}
                                      </option>
                                      <option value="off" className="bg-[#16171d]">
                                        {t("common.labels.off")}
                                      </option>
                                    </select>
                                  </div>
                                </div>

                                <div className="space-y-4 border-t border-fg/8 pt-4">
                                  <div className="flex items-center justify-between gap-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.layerPlacement.fullSwaCache")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.layerPlacement.fullSwaCacheDescription")}
                                      </span>
                                    </div>
                                    <div className="flex shrink-0 items-center gap-3">
                                      <span
                                        className={cn(
                                          "text-[12px] font-medium transition",
                                          modelAdvancedDraft.llamaSwaFull === true
                                            ? "text-accent/80"
                                            : "text-fg/42",
                                        )}
                                      >
                                        {modelAdvancedDraft.llamaSwaFull === true
                                          ? t("common.labels.on")
                                          : t("common.labels.off")}
                                      </span>
                                      <Switch
                                        id="llama-swa-full"
                                        checked={modelAdvancedDraft.llamaSwaFull === true}
                                        onChange={(next) =>
                                          handleLlamaSwaFullChange(next ? true : null)
                                        }
                                        aria-label={t("editModel.layerPlacement.toggleFullSwaCache")}
                                      />
                                    </div>
                                  </div>
                                </div>

                                <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.layerPlacement.ropeBase")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.layerPlacement.ropeBaseDescription")}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_LLAMA_ROPE_FREQ_BASE_RANGE.min}
                                      max={ADVANCED_LLAMA_ROPE_FREQ_BASE_RANGE.max}
                                      step={0.1}
                                      value={modelAdvancedDraft.llamaRopeFreqBase ?? null}
                                      onChange={(next) => handleLlamaRopeFreqBaseChange(next)}
                                      placeholder={t("common.labels.auto")}
                                      className={numberInputClassName}
                                    />
                                  </div>

                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.layerPlacement.ropeScale")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.layerPlacement.ropeScaleDescription")}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_LLAMA_ROPE_FREQ_SCALE_RANGE.min}
                                      max={ADVANCED_LLAMA_ROPE_FREQ_SCALE_RANGE.max}
                                      step={0.01}
                                      value={modelAdvancedDraft.llamaRopeFreqScale ?? null}
                                      onChange={(next) => handleLlamaRopeFreqScaleChange(next)}
                                      placeholder={t("common.labels.auto")}
                                      className={numberInputClassName}
                                    />
                                  </div>
                                </div>

                                {/* Performance */}
                                <div className="space-y-6 border-t border-fg/8 pt-6">
                                  <div className="flex items-center gap-2 border-l-2 border-fg/20 pl-3">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-bold text-fg/80 uppercase tracking-tight">
                                        {t("editModel.runtimeSections.performanceTitle")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.runtimeSections.performanceDescription")}
                                      </span>
                                    </div>
                                  </div>

                                  <div className="space-y-3">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      {t("editModel.layerPlacement.quickPresets")}
                                    </span>
                                    <div className="grid grid-cols-2 gap-2 md:grid-cols-4">
                                      <button
                                        type="button"
                                        onClick={() => applyLlamaPreset("balanced")}
                                        className="rounded-lg border border-fg/10 bg-surface-el/20 px-2.5 py-2 text-[13px] text-fg/80 transition hover:border-fg/20 hover:bg-surface-el/30"
                                      >
                                        {t("editModel.layerPlacement.presetBalanced")}
                                      </button>
                                      <button
                                        type="button"
                                        onClick={() => applyLlamaPreset("throughput")}
                                        className="rounded-lg border border-fg/10 bg-surface-el/20 px-2.5 py-2 text-[13px] text-fg/80 transition hover:border-fg/20 hover:bg-surface-el/30"
                                      >
                                        {t("editModel.layerPlacement.presetThroughput")}
                                      </button>
                                      <button
                                        type="button"
                                        onClick={() => applyLlamaPreset("vram")}
                                        className="rounded-lg border border-fg/10 bg-surface-el/20 px-2.5 py-2 text-[13px] text-fg/80 transition hover:border-fg/20 hover:bg-surface-el/30"
                                      >
                                        {t("editModel.layerPlacement.presetVramSaver")}
                                      </button>
                                      <button
                                        type="button"
                                        onClick={() => applyLlamaPreset("cpu_ram")}
                                        className="rounded-lg border border-fg/10 bg-surface-el/20 px-2.5 py-2 text-[13px] text-fg/80 transition hover:border-fg/20 hover:bg-surface-el/30"
                                      >
                                        {t("editModel.layerPlacement.presetCpuRam")}
                                      </button>
                                    </div>
                                    {selectedLlamaQuickPreset && (
                                      <div className="flex flex-wrap gap-2 border-t border-fg/8 pt-3">
                                        {LLAMA_QUICK_PRESET_DETAILS[selectedLlamaQuickPreset].map(
                                          (detail) => (
                                            <span
                                              key={detail}
                                              className="font-mono text-[13px] text-fg/55"
                                            >
                                              {t(detail)}
                                            </span>
                                          ),
                                        )}
                                      </div>
                                    )}
                                  </div>

                                  <div className="space-y-4">
                                    <div className="flex items-center justify-between">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.layerPlacement.gpuLayers")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {isCpuOnlyLlamaBackend
                                            ? t("editModel.layerPlacement.gpuLayersCpuOnly")
                                            : t("editModel.layerPlacement.gpuLayersDescription")}
                                        </span>
                                        {llamaLayerPlacementSummary ? (
                                          <span className="block text-[12px] text-fg/34">
                                            {llamaLayerPlacementSummary.detail}
                                          </span>
                                        ) : null}
                                      </div>
                                      <span className="font-mono text-[13px] text-fg/55">
                                        {modelAdvancedDraft.llamaGpuLayers !== null &&
                                          modelAdvancedDraft.llamaGpuLayers !== undefined
                                          ? modelAdvancedDraft.llamaGpuLayers
                                          : t("common.labels.auto")}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_LLAMA_GPU_LAYERS_RANGE.min}
                                      max={ADVANCED_LLAMA_GPU_LAYERS_RANGE.max}
                                      step={1}
                                      value={modelAdvancedDraft.llamaGpuLayers ?? null}
                                      onChange={(next) =>
                                        handleLlamaGpuLayersChange(
                                          next === null || next < 0 ? null : Math.trunc(next),
                                        )
                                      }
                                      disabled={isCpuOnlyLlamaBackend}
                                      placeholder={t("common.labels.auto")}
                                      className={cn(
                                        numberInputClassName,
                                        isCpuOnlyLlamaBackend && "cursor-not-allowed opacity-60",
                                      )}
                                    />
                                  </div>

                                  <div className="grid grid-cols-2 gap-6">
                                    <div className="space-y-4">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.layerPlacement.threads")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.layerPlacement.threadsDescription")}
                                        </span>
                                      </div>
                                      <NumberInput
                                        min={ADVANCED_LLAMA_THREADS_RANGE.min}
                                        max={ADVANCED_LLAMA_THREADS_RANGE.max}
                                        step={1}
                                        value={modelAdvancedDraft.llamaThreads ?? null}
                                        onChange={(next) =>
                                          handleLlamaThreadsChange(
                                            next === null || next <= 0 ? null : Math.trunc(next),
                                          )
                                        }
                                        placeholder={t("common.labels.auto")}
                                        className={numberInputClassName}
                                      />
                                    </div>

                                    <div className="space-y-4">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.layerPlacement.batchThreads")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.layerPlacement.batchThreadsDescription")}
                                        </span>
                                      </div>
                                      <NumberInput
                                        min={ADVANCED_LLAMA_THREADS_BATCH_RANGE.min}
                                        max={ADVANCED_LLAMA_THREADS_BATCH_RANGE.max}
                                        step={1}
                                        value={modelAdvancedDraft.llamaThreadsBatch ?? null}
                                        onChange={(next) =>
                                          handleLlamaThreadsBatchChange(
                                            next === null || next <= 0 ? null : Math.trunc(next),
                                          )
                                        }
                                        placeholder={t("common.labels.auto")}
                                        className={numberInputClassName}
                                      />
                                    </div>
                                  </div>

                                  <div className="grid grid-cols-2 gap-6">
                                    <div className="space-y-4">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.layerPlacement.batchSize")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.layerPlacement.batchSizeDescription")}
                                        </span>
                                      </div>
                                      <NumberInput
                                        min={ADVANCED_LLAMA_BATCH_SIZE_RANGE.min}
                                        max={ADVANCED_LLAMA_BATCH_SIZE_RANGE.max}
                                        step={1}
                                        value={modelAdvancedDraft.llamaBatchSize ?? null}
                                        onChange={(next) =>
                                          handleLlamaBatchSizeChange(
                                            next === null || next <= 0 ? null : Math.trunc(next),
                                          )
                                        }
                                        placeholder={t("editModel.placeholders.batch512")}
                                        className={numberInputClassName}
                                      />
                                    </div>

                                    <div className="space-y-4">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.layerPlacement.flashAttention")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.layerPlacement.flashAttentionDescription")}
                                        </span>
                                      </div>
                                      <select
                                        value={modelAdvancedDraft.llamaFlashAttention ?? "auto"}
                                        onChange={(e) => {
                                          const val = e.target.value;
                                          handleLlamaFlashAttentionChange(
                                            val === "auto" ? null : (val as "enabled" | "disabled"),
                                          );
                                        }}
                                        className={selectInputClassName}
                                      >
                                        <option value="auto" className="bg-[#16171d]">
                                          {t("common.labels.auto")}
                                        </option>
                                        <option value="enabled" className="bg-[#16171d]">
                                          {t("common.labels.enabled")}
                                        </option>
                                        <option value="disabled" className="bg-[#16171d]">
                                          {t("common.labels.disabled")}
                                        </option>
                                      </select>
                                    </div>
                                  </div>

                                  <div className="space-y-4 border-t border-fg/8 pt-4">
                                    <div className="flex items-center justify-between gap-4">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.mtp.title")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.mtp.description")}
                                        </span>
                                      </div>
                                      <div className="flex shrink-0 items-center gap-3">
                                        <span
                                          className={cn(
                                            "text-[12px] font-medium transition",
                                            modelAdvancedDraft.llamaMtpEnabled === true
                                              ? "text-accent/80"
                                              : "text-fg/42",
                                          )}
                                        >
                                          {modelAdvancedDraft.llamaMtpEnabled === true
                                            ? t("common.labels.on")
                                            : t("common.labels.off")}
                                        </span>
                                        <Switch
                                          id="llama-mtp-enabled"
                                          checked={modelAdvancedDraft.llamaMtpEnabled === true}
                                          onChange={(next) =>
                                            handleLlamaMtpEnabledChange(next ? true : null)
                                          }
                                          aria-label={t("editModel.mtp.toggle")}
                                        />
                                      </div>
                                    </div>

                                    {modelAdvancedDraft.llamaMtpEnabled === true && (
                                      <div className="space-y-4">
                                        <div className="flex items-center justify-between">
                                          <div className="space-y-0.5">
                                            <span className="block text-[13px] font-medium text-fg/70">
                                              {t("editModel.mtp.draftTokens")}
                                            </span>
                                            <span className="block text-[13px] text-fg/40">
                                              {t("editModel.mtp.draftTokensDescription")}
                                            </span>
                                          </div>
                                          <span className="font-mono text-[13px] text-fg/55">
                                            {modelAdvancedDraft.llamaMtpDraftTokens ??
                                              t("common.labels.auto")}
                                          </span>
                                        </div>
                                        <NumberInput
                                          min={1}
                                          max={8}
                                          step={1}
                                          value={modelAdvancedDraft.llamaMtpDraftTokens ?? null}
                                          onChange={(next) =>
                                            handleLlamaMtpDraftTokensChange(
                                              next === null || next <= 0
                                                ? null
                                                : Math.min(8, Math.trunc(next)),
                                            )
                                          }
                                          placeholder={t("editModel.placeholders.four")}
                                          className={numberInputClassName}
                                        />
                                      </div>
                                    )}

                                    {modelAdvancedDraft.llamaMtpEnabled === true && (
                                      <div className="space-y-4">
                                        <div className="flex items-start justify-between gap-3">
                                          <div className="space-y-0.5">
                                            <span className="block text-[13px] font-medium text-fg/70">
                                              {t("editModel.mtp.draftFile")}
                                            </span>
                                            <span className="block text-[13px] text-fg/40">
                                              {t("editModel.mtp.draftFileDescription")}
                                            </span>
                                          </div>
                                          <button
                                            type="button"
                                            onClick={openLocalMtpPicker}
                                            className="inline-flex shrink-0 items-center gap-1.5 rounded-md border border-fg/10 bg-fg/5 px-2.5 py-1.5 text-[12px] font-medium text-fg/68 transition hover:border-fg/20 hover:bg-fg/10 hover:text-fg"
                                          >
                                            <FolderOpen className="h-3.5 w-3.5 text-accent/70" />
                                            {t("hfBrowser.selectFromLibrary")}
                                          </button>
                                        </div>
                                        <input
                                          type="text"
                                          value={modelAdvancedDraft.llamaMtpModelPath ?? ""}
                                          onChange={(e) =>
                                            handleLlamaMtpModelPathChange(
                                              e.target.value === "" ? null : e.target.value,
                                            )
                                          }
                                          placeholder={t("editModel.mtp.draftFilePlaceholder")}
                                          className={selectInputClassName}
                                          spellCheck={false}
                                        />
                                      </div>
                                    )}
                                  </div>
                                </div>
                              </div>

                              {/* 3. Sampling & Quality + 4. Prompting & Templates */}
                              <div className="space-y-6">
                                <div className="flex items-center gap-2 border-l-2 border-fg/20 pl-3">
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-bold text-fg/80 uppercase tracking-tight">
                                      {t("editModel.runtimeSections.samplingQualityTitle")}
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      {t("editModel.runtimeSections.samplingQualityDescription")}
                                    </span>
                                  </div>
                                </div>

                                <div className="space-y-4">
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      {t("editModel.llamaSampler.samplerProfile")}
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      {t("editModel.llamaSampler.samplerProfileDescription")}
                                    </span>
                                  </div>
                                  <select
                                    value={selectedSamplerProfile}
                                    onChange={(e) =>
                                      handleLlamaSamplerProfileChange(
                                        e.target.value as
                                        | "balanced"
                                        | "creative"
                                        | "stable"
                                        | "reasoning",
                                      )
                                    }
                                    className={selectInputClassName}
                                  >
                                    {LLAMA_SAMPLER_PROFILE_OPTIONS.map((option) => (
                                      <option
                                        key={option.value}
                                        value={option.value}
                                        className="bg-[#16171d]"
                                      >
                                        {t(option.labelKey)}
                                      </option>
                                    ))}
                                  </select>
                                  <div className="flex flex-wrap gap-2 pt-1">
                                    {LLAMA_SAMPLER_PROFILE_DETAILS[selectedSamplerProfile].map(
                                      (detail) => (
                                        <span
                                          key={detail}
                                          className="font-mono text-[13px] text-fg/55"
                                        >
                                          {t(detail)}
                                        </span>
                                      ),
                                    )}
                                  </div>
                                </div>

                                <LlamaSamplerOrderEditor
                                  value={modelAdvancedDraft.llamaSamplerOrder}
                                  onChange={handleLlamaSamplerOrderChange}
                                />

                                <div className="grid grid-cols-2 gap-6">
                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.llamaSampler.minP")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.llamaSampler.localOverride")}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={0}
                                      max={1}
                                      step={0.01}
                                      value={modelAdvancedDraft.llamaMinP ?? null}
                                      onChange={(next) => handleLlamaMinPChange(next)}
                                      placeholder={t("editModel.placeholders.default")}
                                      className={numberInputClassName}
                                    />
                                  </div>

                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.llamaSampler.typicalP")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.llamaSampler.localOverride")}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={0}
                                      max={1}
                                      step={0.01}
                                      value={modelAdvancedDraft.llamaTypicalP ?? null}
                                      onChange={(next) => handleLlamaTypicalPChange(next)}
                                      placeholder={t("editModel.placeholders.default")}
                                      className={numberInputClassName}
                                    />
                                  </div>
                                </div>

                                <div className="grid grid-cols-2 gap-6">
                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.llamaSampler.dryMultiplier")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.llamaSampler.dryMultiplierDescription")}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_LLAMA_DRY_MULTIPLIER_RANGE.min}
                                      max={ADVANCED_LLAMA_DRY_MULTIPLIER_RANGE.max}
                                      step={0.05}
                                      value={modelAdvancedDraft.llamaDryMultiplier ?? null}
                                      onChange={(next) => handleLlamaDryMultiplierChange(next)}
                                      placeholder={t("editModel.placeholders.dryMultiplier")}
                                      className={numberInputClassName}
                                    />
                                  </div>

                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.llamaSampler.dryBase")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.llamaSampler.dryBaseDescription")}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_LLAMA_DRY_BASE_RANGE.min}
                                      max={ADVANCED_LLAMA_DRY_BASE_RANGE.max}
                                      step={0.05}
                                      value={modelAdvancedDraft.llamaDryBase ?? null}
                                      onChange={(next) => handleLlamaDryBaseChange(next)}
                                      placeholder={t("editModel.placeholders.dryBase")}
                                      className={numberInputClassName}
                                    />
                                  </div>
                                </div>

                                <div className="grid grid-cols-2 gap-6">
                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.llamaSampler.dryAllowedLength")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.llamaSampler.dryAllowedLengthDescription")}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_LLAMA_DRY_ALLOWED_LENGTH_RANGE.min}
                                      max={ADVANCED_LLAMA_DRY_ALLOWED_LENGTH_RANGE.max}
                                      step={1}
                                      value={modelAdvancedDraft.llamaDryAllowedLength ?? null}
                                      onChange={(next) => handleLlamaDryAllowedLengthChange(next)}
                                      placeholder={t("editModel.placeholders.dryAllowedLength")}
                                      className={numberInputClassName}
                                    />
                                  </div>

                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.llamaSampler.dryPenaltyLastN")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.llamaSampler.dryPenaltyLastNDescription")}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_LLAMA_DRY_PENALTY_LAST_N_RANGE.min}
                                      max={ADVANCED_LLAMA_DRY_PENALTY_LAST_N_RANGE.max}
                                      step={1}
                                      value={modelAdvancedDraft.llamaDryPenaltyLastN ?? null}
                                      onChange={(next) => handleLlamaDryPenaltyLastNChange(next)}
                                      placeholder={t("editModel.placeholders.dryPenaltyLastN")}
                                      className={numberInputClassName}
                                    />
                                  </div>
                                </div>

                                <div className="space-y-4">
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      {t("editModel.llamaSampler.drySequenceBreakers")}
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      {t("editModel.llamaSampler.drySequenceBreakersDescription")}
                                    </span>
                                  </div>
                                  <input
                                    type="text"
                                    value={joinStringList(modelAdvancedDraft.llamaDrySequenceBreakers)}
                                    onChange={(e) => {
                                      const next = e.target.value
                                        .split(",")
                                        .map((item) => item.trim())
                                        .filter((item) => item.length > 0);
                                      handleLlamaDrySequenceBreakersChange(
                                        next.length ? next : null,
                                      );
                                    }}
                                    placeholder={'\\n, :, ", *'}
                                    className={textAreaInputClassName}
                                  />
                                </div>

                                <div className="space-y-4">
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      {t("editModel.llamaSampler.seed")}
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      {t("editModel.llamaSampler.seedDescription")}
                                    </span>
                                  </div>
                                  <NumberInput
                                    min={ADVANCED_LLAMA_SEED_RANGE.min}
                                    max={ADVANCED_LLAMA_SEED_RANGE.max}
                                    step={1}
                                    value={modelAdvancedDraft.llamaSeed ?? null}
                                    onChange={(next) =>
                                      handleLlamaSeedChange(
                                        next === null || next < 0 ? null : Math.trunc(next),
                                      )
                                    }
                                    placeholder={t("editModel.placeholders.random")}
                                    className={numberInputClassName}
                                  />
                                </div>

                                {/* Prompting & Templates */}
                                <div className="space-y-6 border-t border-fg/8 pt-6">
                                  <div className="flex items-center gap-2 border-l-2 border-fg/20 pl-3">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-bold text-fg/80 uppercase tracking-tight">
                                        {t("editModel.runtimeSections.promptingTemplatesTitle")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.runtimeSections.promptingTemplatesDescription")}
                                      </span>
                                    </div>
                                  </div>

                                  <div className="space-y-4">
                                    <div className="flex items-start justify-between gap-3">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.templates.templateOverride")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.templates.templateOverrideDescription")}
                                        </span>
                                      </div>
                                      <button
                                        type="button"
                                        onClick={openTemplateOverlay}
                                        className="inline-flex shrink-0 items-center gap-1.5 rounded-md border border-fg/10 bg-fg/5 px-2.5 py-1.5 text-[12px] font-medium text-fg/68 transition hover:border-fg/20 hover:bg-fg/10 hover:text-fg"
                                      >
                                        <Maximize2 className="h-3.5 w-3.5 text-accent/70" />
                                        {t("common.buttons.edit")}
                                      </button>
                                    </div>
                                    <button
                                      type="button"
                                      onClick={openTemplateOverlay}
                                      className={cn(
                                        selectInputClassName,
                                        "block w-full cursor-pointer truncate text-left",
                                        modelAdvancedDraft.llamaChatTemplateOverride
                                          ? "text-fg/78"
                                          : "text-fg/35",
                                      )}
                                    >
                                      {modelAdvancedDraft.llamaChatTemplateOverride
                                        ? modelAdvancedDraft.llamaChatTemplateOverride.length > 80
                                          ? `${modelAdvancedDraft.llamaChatTemplateOverride.slice(0, 80)}...`
                                          : modelAdvancedDraft.llamaChatTemplateOverride
                                        : t("editModel.templates.preferEmbedded")}
                                    </button>
                                  </div>

                                  <div className="space-y-4">
                                    <div className="flex items-start justify-between gap-3">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.templates.mmprojPath")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.templates.mmprojPathDescription")}
                                        </span>
                                      </div>
                                      <button
                                        type="button"
                                        onClick={openLocalMmprojPicker}
                                        className="inline-flex shrink-0 items-center gap-1.5 rounded-md border border-fg/10 bg-fg/5 px-2.5 py-1.5 text-[12px] font-medium text-fg/68 transition hover:border-fg/20 hover:bg-fg/10 hover:text-fg"
                                      >
                                        <FolderOpen className="h-3.5 w-3.5 text-accent/70" />
                                        {t("hfBrowser.selectFromLibrary")}
                                      </button>
                                    </div>
                                    <input
                                      type="text"
                                      value={modelAdvancedDraft.llamaMmprojPath ?? ""}
                                      onChange={(e) => {
                                        const nextValue =
                                          e.target.value === "" ? null : e.target.value;
                                        handleLlamaMmprojPathChange(nextValue);
                                        syncImageInputScope(nextValue);
                                      }}
                                      placeholder={t("editModel.placeholders.mmprojPath")}
                                      className={selectInputClassName}
                                      spellCheck={false}
                                    />
                                  </div>

                                  <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
                                    <div className="space-y-4">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.templates.templatePreset")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.templates.templatePresetDescription")}
                                        </span>
                                      </div>
                                      <select
                                        value={modelAdvancedDraft.llamaChatTemplatePreset ?? "auto"}
                                        onChange={(e) =>
                                          handleLlamaChatTemplatePresetChange(
                                            e.target.value === "auto" ? null : e.target.value,
                                          )
                                        }
                                        className={selectInputClassName}
                                      >
                                        {LLAMA_CHAT_TEMPLATE_PRESET_OPTIONS.map((option) => (
                                          <option
                                            key={option.value}
                                            value={option.value}
                                            className="bg-[#16171d]"
                                          >
                                            {t(option.labelKey)}
                                          </option>
                                        ))}
                                      </select>
                                    </div>

                                    <div className="space-y-4">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.templates.rawCompletionFallback")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.templates.rawCompletionFallbackDescription")}
                                        </span>
                                      </div>
                                      <select
                                        value={
                                          modelAdvancedDraft.llamaRawCompletionFallback === true
                                            ? "enabled"
                                            : modelAdvancedDraft.llamaRawCompletionFallback ===
                                              false
                                              ? "disabled"
                                              : "default"
                                        }
                                        onChange={(e) => {
                                          const val = e.target.value;
                                          handleLlamaRawCompletionFallbackChange(
                                            val === "default" ? null : val === "enabled",
                                          );
                                        }}
                                        className={selectInputClassName}
                                      >
                                        <option value="default" className="bg-[#16171d]">
                                          {t("editModel.templates.rawCompletionDefault")}
                                        </option>
                                        <option value="enabled" className="bg-[#16171d]">
                                          {t("common.labels.enabled")}
                                        </option>
                                        <option value="disabled" className="bg-[#16171d]">
                                          {t("common.labels.disabled")}
                                        </option>
                                      </select>
                                    </div>
                                  </div>

                                  <div className="text-danger/80">
                                    <div className="flex items-start justify-between gap-4">
                                      <div className="min-w-0 space-y-1.5">
                                        <div className="flex items-start gap-3">
                                          <div className="mt-0.5 shrink-0 text-danger/80">
                                            <AlertTriangle className="h-4 w-4" />
                                          </div>
                                          <div className="min-w-0 space-y-1">
                                            <span className="block text-[13px] font-medium text-fg/82">
                                              {t("editModel.templates.strictMode")}
                                            </span>
                                            <span className="block text-[13px] leading-relaxed text-fg/48">
                                              {t("editModel.templates.strictModeDescription")}
                                            </span>
                                          </div>
                                        </div>
                                        <span className="block text-[12px] text-danger/75">
                                          {t("editModel.templates.strictModeWarning")}
                                        </span>
                                      </div>
                                      <div className="flex shrink-0 items-center gap-3">
                                        <span
                                          className={cn(
                                            "text-[12px] font-medium transition",
                                            modelAdvancedDraft.llamaStrictMode === true
                                              ? "text-danger/85"
                                              : "text-fg/42",
                                          )}
                                        >
                                          {modelAdvancedDraft.llamaStrictMode === true
                                            ? t("common.labels.on")
                                            : t("common.labels.off")}
                                        </span>
                                        <Switch
                                          id="llama-strict-mode"
                                          checked={modelAdvancedDraft.llamaStrictMode === true}
                                          onChange={(next) =>
                                            handleLlamaStrictModeChange(next ? true : null)
                                          }
                                          aria-label={t("editModel.llama.toggleStrictMode")}
                                        />
                                      </div>
                                    </div>
                                  </div>
                                </div>
                              </div>
                            </div>
                          </div>
                        )}

                        {/* Ollama Settings */}
                        {activeDetailPanel === "runtime" && isOllamaModel && (
                          <div className="space-y-4">
                            <p className="text-[12px] text-fg/45">
                              {runtimePanelTitle} · {runtimeSummary}
                            </p>

                            <div className="space-y-6">
                              {/* 1. Memory & Tokens */}
                              <div className="space-y-6">
                                <div className="flex items-center gap-2 border-l-2 border-fg/20 pl-3">
                                  <span className="text-[13px] font-bold text-fg/80 uppercase tracking-tight">
                                    {t("editModel.runtimeSections.memoryTokensTitle")}
                                  </span>
                                </div>

                                <div className="grid grid-cols-2 gap-6">
                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.ollamaParams.contextLength")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.ollama.numCtxShort")}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_OLLAMA_NUM_CTX_RANGE.min}
                                      max={ADVANCED_OLLAMA_NUM_CTX_RANGE.max}
                                      step={1}
                                      value={modelAdvancedDraft.ollamaNumCtx ?? null}
                                      onChange={(next) =>
                                        handleOllamaNumCtxChange(
                                          next === null || next < 0 ? null : Math.trunc(next),
                                        )
                                      }
                                      placeholder={t("common.labels.auto")}
                                      className={numberInputClassName}
                                    />
                                  </div>

                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.ollamaParams.maxPredict")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.ollamaParams.numPredict")}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_OLLAMA_NUM_PREDICT_RANGE.min}
                                      max={ADVANCED_OLLAMA_NUM_PREDICT_RANGE.max}
                                      step={1}
                                      value={modelAdvancedDraft.ollamaNumPredict ?? null}
                                      onChange={(next) =>
                                        handleOllamaNumPredictChange(
                                          next === null || next < 0 ? null : Math.trunc(next),
                                        )
                                      }
                                      placeholder={t("common.labels.auto")}
                                      className={numberInputClassName}
                                    />
                                  </div>
                                </div>

                                <div className="space-y-4">
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      {t("editModel.ollamaParams.numKeep")}
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      {t("editModel.ollamaParams.numKeepDescription")}
                                    </span>
                                  </div>
                                  <NumberInput
                                    min={ADVANCED_OLLAMA_NUM_KEEP_RANGE.min}
                                    max={ADVANCED_OLLAMA_NUM_KEEP_RANGE.max}
                                    step={1}
                                    value={modelAdvancedDraft.ollamaNumKeep ?? null}
                                    onChange={(next) =>
                                      handleOllamaNumKeepChange(
                                        next === null || next < 0 ? null : Math.trunc(next),
                                      )
                                    }
                                    placeholder={t("common.labels.auto")}
                                    className={numberInputClassName}
                                  />
                                </div>
                              </div>

                              {/* 2. Performance */}
                              <div className="space-y-6">
                                <div className="flex items-center gap-2 border-l-2 border-fg/20 pl-3">
                                  <span className="text-[13px] font-bold text-fg/80 uppercase tracking-tight">
                                    {t("editModel.runtimeSections.performanceTitle")}
                                  </span>
                                </div>

                                <div className="grid grid-cols-2 gap-6">
                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.ollamaParams.numGpu")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.ollamaParams.numGpuDescription")}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_OLLAMA_NUM_GPU_RANGE.min}
                                      max={ADVANCED_OLLAMA_NUM_GPU_RANGE.max}
                                      step={1}
                                      value={modelAdvancedDraft.ollamaNumGpu ?? null}
                                      onChange={(next) =>
                                        handleOllamaNumGpuChange(
                                          next === null || next < 0 ? null : Math.trunc(next),
                                        )
                                      }
                                      placeholder={t("common.labels.auto")}
                                      className={numberInputClassName}
                                    />
                                  </div>

                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.ollamaParams.numThread")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.ollamaParams.numThreadDescription")}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_OLLAMA_NUM_THREAD_RANGE.min}
                                      max={ADVANCED_OLLAMA_NUM_THREAD_RANGE.max}
                                      step={1}
                                      value={modelAdvancedDraft.ollamaNumThread ?? null}
                                      onChange={(next) =>
                                        handleOllamaNumThreadChange(
                                          next === null || next < 1 ? null : Math.trunc(next),
                                        )
                                      }
                                      placeholder={t("common.labels.auto")}
                                      className={numberInputClassName}
                                    />
                                  </div>
                                </div>

                                <div className="space-y-4">
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      {t("editModel.ollamaParams.numBatch")}
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      {t("editModel.ollamaParams.numBatchDescription")}
                                    </span>
                                  </div>
                                  <NumberInput
                                    min={ADVANCED_OLLAMA_NUM_BATCH_RANGE.min}
                                    max={ADVANCED_OLLAMA_NUM_BATCH_RANGE.max}
                                    step={1}
                                    value={modelAdvancedDraft.ollamaNumBatch ?? null}
                                    onChange={(next) =>
                                      handleOllamaNumBatchChange(
                                        next === null || next < 1 ? null : Math.trunc(next),
                                      )
                                    }
                                    placeholder={t("common.labels.auto")}
                                    className={numberInputClassName}
                                  />
                                </div>
                              </div>

                              {/* 3. Sampling */}
                              <div className="space-y-6">
                                <div className="flex items-center gap-2 border-l-2 border-fg/20 pl-3">
                                  <span className="text-[13px] font-bold text-fg/80 uppercase tracking-tight">
                                    {t("editModel.runtimeSections.samplingPenaltiesTitle")}
                                  </span>
                                </div>

                                <div className="grid grid-cols-2 gap-6">
                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.ollamaParams.tfsZ")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.ollamaParams.tfsZDescription")}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_OLLAMA_TFS_Z_RANGE.min}
                                      max={ADVANCED_OLLAMA_TFS_Z_RANGE.max}
                                      step={0.01}
                                      value={modelAdvancedDraft.ollamaTfsZ ?? null}
                                      onChange={(next) => handleOllamaTfsZChange(next)}
                                      placeholder={t("common.labels.auto")}
                                      className={numberInputClassName}
                                    />
                                  </div>

                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.ollamaParams.repeatPenalty")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.ollamaParams.repeatPenaltyDescription")}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_OLLAMA_REPEAT_PENALTY_RANGE.min}
                                      max={ADVANCED_OLLAMA_REPEAT_PENALTY_RANGE.max}
                                      step={0.01}
                                      value={modelAdvancedDraft.ollamaRepeatPenalty ?? null}
                                      onChange={(next) => handleOllamaRepeatPenaltyChange(next)}
                                      placeholder={t("common.labels.auto")}
                                      className={numberInputClassName}
                                    />
                                  </div>
                                </div>

                                <div className="grid grid-cols-2 gap-6">
                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.ollamaParams.minP")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.ollamaParams.minPDescription")}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_OLLAMA_MIN_P_RANGE.min}
                                      max={ADVANCED_OLLAMA_MIN_P_RANGE.max}
                                      step={0.01}
                                      value={modelAdvancedDraft.ollamaMinP ?? null}
                                      onChange={(next) => handleOllamaMinPChange(next)}
                                      placeholder={t("common.labels.auto")}
                                      className={numberInputClassName}
                                    />
                                  </div>

                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.ollamaParams.typicalP")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.ollamaParams.typicalPDescription")}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_OLLAMA_TYPICAL_P_RANGE.min}
                                      max={ADVANCED_OLLAMA_TYPICAL_P_RANGE.max}
                                      step={0.01}
                                      value={modelAdvancedDraft.ollamaTypicalP ?? null}
                                      onChange={(next) => handleOllamaTypicalPChange(next)}
                                      placeholder={t("common.labels.auto")}
                                      className={numberInputClassName}
                                    />
                                  </div>
                                </div>

                                <div className="space-y-4">
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      {t("editModel.ollamaParams.mirostat")}
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      {t("editModel.ollamaParams.mirostatDescription")}
                                    </span>
                                  </div>
                                  <select
                                    value={
                                      modelAdvancedDraft.ollamaMirostat === null ||
                                        modelAdvancedDraft.ollamaMirostat === undefined
                                        ? "auto"
                                        : modelAdvancedDraft.ollamaMirostat.toString()
                                    }
                                    onChange={(e) => {
                                      const val = e.target.value;
                                      handleOllamaMirostatChange(
                                        val === "auto" ? null : Number(val),
                                      );
                                    }}
                                    className={selectInputClassName}
                                  >
                                    <option value="auto" className="bg-[#16171d]">
                                      {t("common.labels.auto")}
                                    </option>
                                    <option value="0" className="bg-[#16171d]">
                                      {t("editModel.ollamaParams.mirostatOff")}
                                    </option>
                                    <option value="1" className="bg-[#16171d]">
                                      1
                                    </option>
                                    <option value="2" className="bg-[#16171d]">
                                      2
                                    </option>
                                  </select>
                                </div>

                                <div className="grid grid-cols-2 gap-6">
                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.ollamaParams.tau")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.ollamaParams.tauDescription")}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_OLLAMA_MIROSTAT_TAU_RANGE.min}
                                      max={ADVANCED_OLLAMA_MIROSTAT_TAU_RANGE.max}
                                      step={0.1}
                                      value={modelAdvancedDraft.ollamaMirostatTau ?? null}
                                      onChange={(next) => handleOllamaMirostatTauChange(next)}
                                      placeholder={t("common.labels.auto")}
                                      className={numberInputClassName}
                                    />
                                  </div>

                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        {t("editModel.ollamaParams.eta")}
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {t("editModel.ollamaParams.etaDescription")}
                                      </span>
                                    </div>
                                    <NumberInput
                                      min={ADVANCED_OLLAMA_MIROSTAT_ETA_RANGE.min}
                                      max={ADVANCED_OLLAMA_MIROSTAT_ETA_RANGE.max}
                                      step={0.01}
                                      value={modelAdvancedDraft.ollamaMirostatEta ?? null}
                                      onChange={(next) => handleOllamaMirostatEtaChange(next)}
                                      placeholder={t("common.labels.auto")}
                                      className={numberInputClassName}
                                    />
                                  </div>
                                </div>

                                <div className="space-y-4">
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      {t("editModel.ollamaParams.seed")}
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      {t("editModel.ollamaParams.seedDescription")}
                                    </span>
                                  </div>
                                  <NumberInput
                                    min={ADVANCED_OLLAMA_SEED_RANGE.min}
                                    max={ADVANCED_OLLAMA_SEED_RANGE.max}
                                    step={1}
                                    value={modelAdvancedDraft.ollamaSeed ?? null}
                                    onChange={(next) =>
                                      handleOllamaSeedChange(
                                        next === null || next < 0 ? null : Math.trunc(next),
                                      )
                                    }
                                    placeholder={t("editModel.placeholders.random")}
                                    className={numberInputClassName}
                                  />
                                </div>
                              </div>

                              {/* 4. Stop Sequences */}
                              <div className="space-y-4">
                                <div className="flex items-center gap-2 border-l-2 border-fg/20 pl-3">
                                  <span className="text-[13px] font-bold text-fg/80 uppercase tracking-tight">
                                    {t("editModel.runtimeSections.stopSequencesTitle")}
                                  </span>
                                </div>
                                <textarea
                                  value={ollamaStopText}
                                  onChange={(e) => {
                                    const raw = e.target.value;
                                    const next = raw
                                      .split(/[\n,]+/)
                                      .map((s) => s.trim())
                                      .filter((s) => s.length > 0);
                                    handleOllamaStopChange(next.length > 0 ? next : null);
                                  }}
                                  placeholder={t("editModel.placeholders.stopSequences")}
                                  rows={2}
                                  className={textAreaInputClassName}
                                />
                              </div>
                            </div>
                          </div>
                        )}

                        {/* Reasoning Section (Thinking) */}
                        {activeDetailPanel === "reasoning" && showReasoningSection && (
                          <div className="space-y-4">
                            <div className="flex items-start justify-between gap-3">
                              <p className="text-[12px] text-fg/45">{reasoningSummary}</p>
                              <button
                                type="button"
                                onClick={() => openDocs("models", "reasoning-mode")}
                                className="shrink-0 text-fg/40 hover:text-fg/60 transition"
                                aria-label={t("editModel.reasoning.helpLabel")}
                              >
                                <HelpCircle size={14} />
                              </button>
                            </div>

                            <div className="space-y-6">
                              <div className="flex items-center justify-between">
                                <div className="flex items-center gap-3 border-l-2 border-warning/40 pl-3">
                                  <Brain size={16} className="text-warning/80" />
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      {t("editModel.reasoning.enabled")}
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      {t("editModel.reasoning.enabledDescription")}
                                    </span>
                                  </div>
                                  <button
                                    type="button"
                                    onClick={() => openDocs("models", "reasoning-mode")}
                                    className="text-fg/30 hover:text-fg/60 transition"
                                    aria-label={t("editModel.reasoning.helpLabel")}
                                  >
                                    <HelpCircle size={12} />
                                  </button>
                                </div>
                                {!isAutoReasoning && (
                                  <Switch
                                    checked={modelAdvancedDraft.reasoningEnabled || false}
                                    onChange={handleReasoningEnabledChange}
                                  />
                                )}
                              </div>

                              {(modelAdvancedDraft.reasoningEnabled || isAutoReasoning) && (
                                <div className="space-y-8 pl-4 border-l border-fg/10 mt-4">
                                  {showEffortOptions && (
                                    <div className="space-y-3">
                                      <span className="text-[13px] font-bold text-fg/30 uppercase tracking-wider">
                                        {t("editModel.reasoning.effort")}
                                      </span>
                                      <div className="grid grid-cols-4 gap-2">
                                        {([null, "low", "medium", "high"] as const).map((level) => (
                                          <button
                                            key={level || "auto"}
                                            type="button"
                                            onClick={() => handleReasoningEffortChange(level)}
                                            className={cn(
                                              "rounded-lg py-1.5 text-[13px] font-bold uppercase transition",
                                              modelAdvancedDraft.reasoningEffort === level
                                                ? "bg-warning/20 text-warning border border-warning/30"
                                                : "bg-fg/5 text-fg/30 border border-transparent hover:text-fg/50",
                                            )}
                                          >
                                            {level || t("common.labels.auto")}
                                          </button>
                                        ))}
                                      </div>
                                    </div>
                                  )}

                                  {(reasoningSupport === "budget-only" ||
                                    reasoningSupport === "dynamic") && (
                                      <div className="space-y-4">
                                        <div className="flex items-center justify-between">
                                          <span className="text-[13px] font-bold text-fg/30 uppercase tracking-wider">
                                            {t("editModel.reasoning.budgetTokens")}
                                          </span>
                                          <span className="font-mono text-[13px] text-warning">
                                            {modelAdvancedDraft.reasoningBudgetTokens
                                              ? modelAdvancedDraft.reasoningBudgetTokens.toLocaleString()
                                              : t("common.labels.auto")}
                                          </span>
                                        </div>
                                        <NumberInput
                                          min={ADVANCED_REASONING_BUDGET_RANGE.min}
                                          max={ADVANCED_REASONING_BUDGET_RANGE.max}
                                          step={1024}
                                          value={modelAdvancedDraft.reasoningBudgetTokens || null}
                                          onChange={(next) =>
                                            handleReasoningBudgetChange(
                                              next === null || next === 0 ? null : Math.trunc(next),
                                            )
                                          }
                                          placeholder={t("common.labels.auto")}
                                          className={numberInputClassName}
                                        />
                                      </div>
                                    )}
                                </div>
                              )}

                              <div className="flex items-center justify-between">
                                <div className="flex items-center gap-3 border-l-2 border-warning/40 pl-3">
                                  <SendHorizontal size={16} className="text-warning/80" />
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      {t("editModel.reasoning.forceSend")}
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      {t("editModel.reasoning.forceSendDescription")}
                                    </span>
                                  </div>
                                </div>
                                <Switch
                                  checked={modelAdvancedDraft.forceSendThinkingState || false}
                                  onChange={handleForceSendThinkingStateChange}
                                />
                              </div>
                            </div>
                          </div>
                        )}

                        {/* Prompt Caching Section */}
                        {activeDetailPanel === "caching" && showCachingSection && (
                          <div className="space-y-4">
                            <div className="flex items-start justify-between gap-3">
                              <p className="text-[12px] text-fg/45">
                                {t("editModel.sectionDescriptions.promptCaching")}
                              </p>
                              <button
                                type="button"
                                onClick={() => setShowParameterSupport(true)}
                                className="shrink-0 text-fg/40 hover:text-fg/60 transition"
                                title={t("editModel.parameterSupport.title")}
                              >
                                <Info size={14} />
                              </button>
                            </div>

                            <div className="space-y-6">
                              {hasAutomaticCaching ? (
                                <div className="space-y-4">
                                  <div className="flex items-start justify-between">
                                    <div className="flex items-start gap-3 border-l-2 border-fg/20 pl-3">
                                      <HardDrive size={16} className="mt-0.5 text-accent/80" />
                                      <div className="space-y-1">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.promptCaching.automatic.title")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.promptCaching.automatic.description")}
                                        </span>
                                      </div>
                                    </div>
                                  </div>

                                  <div className="text-[13px] leading-relaxed text-fg/55">
                                    {editorModel?.providerId === "groq" && (
                                      <>
                                        <strong className="text-fg/80">
                                          {t("editModel.promptCaching.groqLabel")}
                                        </strong>{" "}
                                        {t("editModel.promptCaching.groqDescription")}
                                      </>
                                    )}
                                    {isGeminiFamilyProvider(
                                      editorModel?.providerId,
                                    ) && (
                                        <>
                                          <strong className="text-fg/80">
                                            {t("editModel.promptCaching.geminiLabel")}
                                          </strong>{" "}
                                          {t("editModel.promptCaching.geminiDescription")}
                                        </>
                                      )}
                                  </div>
                                </div>
                              ) : (
                                <>
                                  {/* ── Enable toggle ── */}
                                  <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-3 border-l-2 border-fg/20 pl-3">
                                      <HardDrive size={16} className="text-accent/80" />
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          {t("editModel.promptCaching.enableTitle")}
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {t("editModel.promptCaching.enableDescription")}
                                        </span>
                                      </div>
                                    </div>

                                    <Switch
                                      checked={modelAdvancedDraft.promptCachingEnabled || false}
                                      onChange={handlePromptCachingEnabledChange}
                                    />
                                  </div>

                                  {modelAdvancedDraft.promptCachingEnabled && (
                                    <>
                                      {/* ── TTL toggle ── */}
                                      <div className="flex items-center justify-between">
                                        <div className="flex items-center gap-3 border-l-2 border-fg/10 pl-3">
                                          <div className="space-y-0.5">
                                            <span className="block text-[13px] font-medium text-fg/70">
                                              {t("editModel.promptCaching.ttlTitle")}
                                            </span>
                                            <span className="block text-[13px] text-fg/40">
                                              {t("editModel.promptCaching.ttlDescription")}
                                            </span>
                                          </div>
                                        </div>

                                        <div className="inline-flex shrink-0 rounded-lg border border-fg/10 bg-fg/4 p-0.5">
                                          <button
                                            type="button"
                                            onClick={(e) => {
                                              e.preventDefault();
                                              handlePromptCachingTtlChange(
                                                promptCachingTtlOptions[0].value,
                                              );
                                            }}
                                            className={cn(
                                              "rounded-md px-3 py-1 text-[12px] font-medium transition",
                                              selectedPromptCachingTtl ===
                                                promptCachingTtlOptions[0].value
                                                ? "bg-accent/15 text-accent"
                                                : "text-fg/45 hover:text-fg/70",
                                            )}
                                          >
                                            {promptCachingTtlOptions[0].label}
                                          </button>
                                          <button
                                            type="button"
                                            onClick={(e) => {
                                              e.preventDefault();
                                              handlePromptCachingTtlChange(
                                                promptCachingTtlOptions[1].value,
                                              );
                                            }}
                                            className={cn(
                                              "rounded-md px-3 py-1 text-[12px] font-medium transition",
                                              selectedPromptCachingTtl ===
                                                promptCachingTtlOptions[1].value
                                                ? "bg-accent/15 text-accent"
                                                : "text-fg/45 hover:text-fg/70",
                                            )}
                                          >
                                            {promptCachingTtlOptions[1].label}
                                          </button>
                                        </div>
                                      </div>

                                      {/* ── Pricing / TTL notes ── */}
                                      <div className="text-[13px] leading-relaxed text-fg/55">
                                        <strong className="text-fg/80">
                                          {t("editModel.promptCaching.pricingTitle")}
                                        </strong>{" "}
                                        {t("editModel.promptCaching.pricingDescription")}
                                        {editorModel?.providerId !== "openai" &&
                                          modelAdvancedDraft.promptCachingTtl === "1h" && (
                                            <span className="mt-1.5 block text-fg/50">
                                              {t("editModel.promptCaching.oneHourNote")}
                                            </span>
                                          )}
                                        {editorModel?.providerId === "openai" &&
                                          selectedPromptCachingTtl === "24h" && (
                                            <span className="mt-1.5 block text-fg/50">
                                              {t("editModel.promptCaching.openai24hNote")}
                                            </span>
                                          )}
                                      </div>
                                    </>
                                  )}
                                </>
                              )}
                            </div>
                          </div>
                        )}

                        {activeDetailPanel === "capabilities" && (
                          <div className="space-y-4">
                            <div className="flex items-start justify-between gap-3">
                              <p className="text-[12px] text-fg/45">{capabilitiesSummary}</p>
                              <button
                                type="button"
                                onClick={() => openDocs("imagegen", "model-capabilities")}
                                className="shrink-0 text-fg/40 transition hover:text-fg/60"
                                aria-label={t("editModel.capabilities.helpLabel")}
                              >
                                <HelpCircle size={14} />
                              </button>
                            </div>

                            {editorModel?.providerId === "llamacpp" && (
                              <div className="rounded-xl border border-fg/10 bg-fg/5 p-4">
                                <div className="flex items-start justify-between gap-4">
                                  <div className="min-w-0 space-y-1.5">
                                    <div className="flex items-start gap-3">
                                      <div className="mt-0.5 shrink-0 text-accent/80">
                                        <Info className="h-4 w-4" />
                                      </div>
                                      <div className="min-w-0 space-y-1">
                                        <span className="block text-[13px] font-medium text-fg/82">
                                          {t("editModel.streaming.title")}
                                        </span>
                                        <span className="block text-[13px] leading-relaxed text-fg/48">
                                          {t("editModel.streaming.description")}
                                        </span>
                                      </div>
                                    </div>
                                    <span className="block text-[12px] text-fg/42">
                                      {t("editModel.streaming.offNote")}
                                    </span>
                                  </div>
                                  <div className="flex shrink-0 items-center gap-3">
                                    <span
                                      className={cn(
                                        "text-[12px] font-medium transition",
                                        modelAdvancedDraft.llamaStreamingEnabled !== false
                                          ? "text-accent/80"
                                          : "text-fg/42",
                                      )}
                                    >
                                      {modelAdvancedDraft.llamaStreamingEnabled !== false
                                        ? t("common.labels.on")
                                        : t("common.labels.off")}
                                    </span>
                                    <Switch
                                      id="llama-streaming-enabled"
                                      checked={modelAdvancedDraft.llamaStreamingEnabled !== false}
                                      onChange={(next) =>
                                        handleLlamaStreamingEnabledChange(next ? true : false)
                                      }
                                      aria-label={t("editModel.streaming.toggle")}
                                    />
                                  </div>
                                </div>
                              </div>
                            )}

                            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                              <div className="space-y-3">
                                <p className="text-[13px] font-medium text-fg/72">
                                  {t("editModel.capabilities.input")}
                                </p>
                                {["text", "image", "audio"].map((scope) => (
                                  <button
                                    key={scope}
                                    type="button"
                                    disabled={isFixedImageProvider}
                                    onClick={() =>
                                      toggleScope(
                                        "inputScopes",
                                        scope as any,
                                        !editorModel.inputScopes?.includes(scope as any),
                                      )
                                    }
                                    className={cn(
                                      "flex w-full items-center justify-between rounded-lg border px-3 py-2.5 text-[13px] transition",
                                      isFixedImageProvider && "cursor-not-allowed opacity-60",
                                      editorModel.inputScopes?.includes(scope as any)
                                        ? "border-accent/25 bg-accent/10 text-accent"
                                        : "border-fg/10 bg-fg/5 text-fg/55 hover:border-fg/20 hover:bg-fg/8 hover:text-fg/85",
                                    )}
                                  >
                                    <span className="capitalize">{scope}</span>
                                    {editorModel.inputScopes?.includes(scope as any) ? (
                                      <Check size={14} />
                                    ) : null}
                                  </button>
                                ))}
                              </div>

                              <div className="space-y-3">
                                <p className="text-[13px] font-medium text-fg/72">
                                  {t("editModel.capabilities.output")}
                                </p>
                                {["text", "image", "audio"].map((scope) => (
                                  <button
                                    key={scope}
                                    type="button"
                                    disabled={isFixedImageProvider}
                                    onClick={() =>
                                      toggleScope(
                                        "outputScopes",
                                        scope as any,
                                        !editorModel.outputScopes?.includes(scope as any),
                                      )
                                    }
                                    className={cn(
                                      "flex w-full items-center justify-between rounded-lg border px-3 py-2.5 text-[13px] transition",
                                      isFixedImageProvider && "cursor-not-allowed opacity-60",
                                      editorModel.outputScopes?.includes(scope as any)
                                        ? "border-accent/25 bg-accent/10 text-accent"
                                        : "border-fg/10 bg-fg/5 text-fg/55 hover:border-fg/20 hover:bg-fg/8 hover:text-fg/85",
                                    )}
                                  >
                                    <span className="capitalize">{scope}</span>
                                    {editorModel.outputScopes?.includes(scope as any) ? (
                                      <Check size={14} />
                                    ) : null}
                                  </button>
                                ))}
                              </div>
                            </div>
                            {isFixedImageProvider && (
                              <p className="text-[12px] leading-relaxed text-fg/45">
                                {isLocalDiffusionModel
                                  ? t("editModel.capabilities.localDiffusionFixed")
                                  : t("editModel.capabilities.automatic1111Fixed")}
                              </p>
                            )}
                          </div>
                        )}
                      </div>
                    </motion.div>
                  </AnimatePresence>
              </div>
            </div>
          </div>
        </motion.div>
      </main>

      {/* LAST RUNTIME REPORT DRAWER */}
      <AnimatePresence>
        {isLocalModel && llamaRuntimeReport && showLlamaRuntimeReport && (
          <>
            <motion.div
              className="fixed inset-0 z-40 bg-black/50"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.2 }}
              onClick={() => setShowLlamaRuntimeReport(false)}
            />
            <motion.aside
              className="fixed bottom-0 right-0 top-[var(--titlebar-h,0px)] z-50 flex w-120 max-w-[90vw] flex-col border-l border-fg/10 bg-surface shadow-2xl"
              initial={{ x: "100%" }}
              animate={{ x: 0 }}
              exit={{ x: "100%" }}
              transition={{ type: "spring", damping: 28, stiffness: 320 }}
            >
              <div className="flex shrink-0 items-center justify-between border-b border-fg/10 px-5 py-4">
                <div>
                  <p className="text-base font-semibold text-fg">
                    {t("editModel.runtime.lastReport")}
                  </p>
                  <p className="text-[12px] text-fg/50">{runtimePanelTitle}</p>
                </div>
                <button
                  type="button"
                  onClick={() => setShowLlamaRuntimeReport(false)}
                  className="flex h-8 w-8 items-center justify-center rounded-lg text-fg/50 transition hover:bg-fg/8 hover:text-fg"
                  aria-label={t("editModel.templateOverride.close")}
                >
                  <X className="h-4 w-4" />
                </button>
              </div>

              <div className="flex-1 overflow-y-auto px-5 py-5">
                <div className="space-y-6">
                  <div className="flex items-start gap-3">
                    <div
                      className={cn(
                        "flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border",
                        llamaRuntimeReport.status === "succeeded"
                          ? "border-emerald-400/30 bg-emerald-400/10 text-emerald-400"
                          : llamaRuntimeReport.status === "cpuFallbackSucceeded"
                            ? "border-warning/30 bg-warning/10 text-warning"
                            : "border-danger/30 bg-danger/10 text-danger",
                      )}
                    >
                      {llamaRuntimeReport.status === "succeeded" ||
                      llamaRuntimeReport.status === "cpuFallbackSucceeded" ? (
                        <Check className="h-4 w-4" />
                      ) : (
                        <AlertTriangle className="h-4 w-4" />
                      )}
                    </div>
                    <div className="min-w-0 space-y-1">
                      <p className="text-[13px] font-medium text-fg">
                        {t(getLlamaRuntimeHeadlineKey(llamaRuntimeReport))}
                      </p>
                      <p className="text-[12px] leading-relaxed text-fg/55">
                        {t(getLlamaRuntimeDetailKey(llamaRuntimeReport))}
                      </p>
                    </div>
                  </div>

                  {llamaRuntimeReport.gpuFallbackReason && (
                    <div className="space-y-1">
                      <div className="text-[11px] font-medium uppercase tracking-wide text-warning/90">
                        {t("editModel.runtime.gpuFallbackReason")}
                      </div>
                      <p className="text-[13px] leading-relaxed text-fg/72">
                        {llamaRuntimeReport.gpuFallbackReason}
                      </p>
                    </div>
                  )}

                  {llamaRuntimeReport.errorMessage && (
                    <div className="space-y-1">
                      <div className="text-[11px] font-medium uppercase tracking-wide text-danger/90">
                        {t("editModel.runtime.finalError")}
                      </div>
                      <p className="text-[13px] leading-relaxed text-fg/72">
                        {llamaRuntimeReport.errorMessage}
                      </p>
                    </div>
                  )}

                  {llamaRuntimeFacts.length > 0 && (
                    <div className="divide-y divide-fg/10 border-t border-fg/10">
                      {llamaRuntimeFacts.map((item) => (
                        <div
                          key={item.label}
                          className="flex items-start justify-between gap-4 py-2.5"
                        >
                          <div className="text-[12px] text-fg/50">{item.label}</div>
                          <div className="wrap-break-word text-right text-[13px] text-fg/85">
                            {item.value}
                          </div>
                        </div>
                      ))}
                    </div>
                  )}

                  {llamaRuntimeReport.status === "cpuFallbackSucceeded" &&
                    llamaRuntimeReport.suggestedSettings && (
                      <div className="space-y-3 border-t border-fg/10 pt-5">
                        <div className="space-y-1">
                          <div className="text-[13px] font-medium text-fg">
                            {t("editModel.runtime.workingRecoveryConfig")}
                          </div>
                          <p className="text-[12px] text-fg/52">
                            {t("editModel.runtime.context")}{" "}
                            {formatRuntimeNumber(
                              llamaRuntimeReport.suggestedSettings.contextLength,
                            ) ?? t("editModel.runtime.na")}
                            {" • "}
                            {t("editModel.runtime.batch")}{" "}
                            {formatRuntimeNumber(
                              llamaRuntimeReport.suggestedSettings.llamaBatchSize,
                            ) ?? t("editModel.runtime.na")}
                          </p>
                        </div>
                        <button
                          type="button"
                          onClick={() => void handleApplyLlamaRuntimeSuggestion()}
                          disabled={saving}
                          className="w-full rounded-lg border border-warning/30 bg-warning/12 px-3 py-2 text-[13px] font-medium text-warning transition hover:bg-warning/18 disabled:cursor-not-allowed disabled:opacity-60"
                        >
                          {t("editModel.runtime.applyWorkingConfig")}
                        </button>
                      </div>
                    )}
                </div>
              </div>
            </motion.aside>
          </>
        )}
      </AnimatePresence>

      {/* PARAMETER SUPPORT MODAL */}
      <BottomMenu
        isOpen={showParameterSupport}
        onClose={() => setShowParameterSupport(false)}
        title={t("editModel.parameterSupport.title")}
      >
        <div className="px-4 pb-8">
          <ProviderParameterSupportInfo providerId={editorModel?.providerId || "openai"} />
        </div>
      </BottomMenu>

      <AnimatePresence>
        {showTemplateOverlay && (
          <motion.div
            className="fixed inset-x-0 bottom-0 top-[var(--titlebar-h,0px)] z-50 flex flex-col bg-surface"
            style={{ paddingTop: "env(safe-area-inset-top)" }}
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 20 }}
            transition={{ duration: 0.2 }}
          >
            <div className="flex items-center justify-between border-b border-fg/10 px-4 py-3">
              <div className="text-base font-semibold text-fg">
                {t("editModel.templateOverride.title")}
              </div>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={toggleEmbeddedTemplate}
                  className={cn(
                    "inline-flex items-center gap-1.5 rounded-full border px-3 py-1.5 text-xs font-medium transition",
                    showEmbeddedTemplateViewer
                      ? "border-accent/30 bg-accent/10 text-accent"
                      : "border-fg/10 text-fg/70 hover:bg-fg/10 hover:text-fg",
                  )}
                >
                  <CopyCheck className="h-3 w-3" />
                  {showEmbeddedTemplateViewer
                    ? t("editModel.templateOverride.hideEmbedded")
                    : t("editModel.templateOverride.showEmbedded")}
                </button>
                <button
                  type="button"
                  onClick={cancelTemplateOverlay}
                  className="rounded-full border border-fg/10 px-3 py-1.5 text-xs font-medium text-fg/70 transition hover:bg-fg/10 hover:text-fg"
                >
                  {t("editModel.templateOverride.close")}
                </button>
                <button
                  type="button"
                  onClick={saveTemplateOverlay}
                  className={cn(
                    "rounded-full px-3 py-1.5 text-xs font-semibold text-fg transition",
                    "bg-linear-to-r from-accent to-accent/80",
                    "hover:from-accent/80 hover:to-accent/60",
                  )}
                >
                  {t("common.buttons.save")}
                </button>
              </div>
            </div>

            <div className="flex-1 overflow-y-auto px-4 pb-6 pt-4">
              <div className="mx-auto max-w-4xl space-y-4">
                {showEmbeddedTemplateViewer && (
                  <div className="overflow-hidden rounded-xl border border-fg/8 bg-[#0b0c10]">
                    {embeddedTemplateLoading ? (
                      <div className="flex h-40 items-center justify-center text-[12px] text-fg/50">
                        <Loader className="mr-2 h-3.5 w-3.5 animate-spin" />
                        {t("editModel.templateOverride.readingEmbedded")}
                      </div>
                    ) : embeddedTemplateError ? (
                      <div className="space-y-1 p-3">
                        <div className="text-[12px] font-medium text-danger">
                          {t("editModel.templateOverride.readEmbeddedFailed")}
                        </div>
                        <div className="whitespace-pre-wrap wrap-break-word text-[12px] text-fg/50">
                          {embeddedTemplateError}
                        </div>
                      </div>
                    ) : (
                      <>
                        <pre
                          className="max-h-64 overflow-auto px-4 py-3 font-mono text-[11px] leading-4.5"
                          dangerouslySetInnerHTML={{
                            __html: highlightedTemplate ?? "",
                          }}
                        />
                        <div className="flex items-center justify-end gap-2 border-t border-fg/6 px-3 py-2">
                          <button
                            type="button"
                            onClick={() => {
                              navigator.clipboard.writeText(embeddedTemplateText);
                              toast.success(t("editModel.templateOverride.copiedToClipboard"));
                            }}
                            className="inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-[11px] font-medium text-fg/50 transition hover:bg-fg/8 hover:text-fg/70"
                          >
                            <Copy className="h-3 w-3" />
                            {t("common.buttons.copy")}
                          </button>
                          <button
                            type="button"
                            onClick={handleUseEmbeddedTemplate}
                            disabled={!embeddedTemplateText.trim()}
                            className="inline-flex items-center gap-1.5 rounded-md bg-accent/12 px-2.5 py-1 text-[11px] font-medium text-accent transition hover:bg-accent/20 disabled:cursor-not-allowed disabled:opacity-40"
                          >
                            <CopyCheck className="h-3 w-3" />
                            {t("editModel.templateOverride.pasteIntoEditor")}
                          </button>
                        </div>
                      </>
                    )}
                  </div>
                )}

                <div className="space-y-2">
                  <div className="text-sm font-medium text-fg/80">
                    {t("editModel.templateOverride.jinjaTemplate")}
                  </div>
                  <textarea
                    value={templateOverlayDraft}
                    onChange={(e) => setTemplateOverlayDraft(e.target.value)}
                    className="min-h-[50vh] w-full resize-none rounded-2xl border border-fg/10 bg-surface-el/40 px-4 py-4 font-mono text-[12px] leading-relaxed text-fg placeholder-fg/40 transition focus:border-fg/20 focus:outline-none"
                    placeholder={t("editModel.templateOverride.placeholder")}
                    spellCheck={false}
                  />
                </div>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Continue Setup button when coming from onboarding */}
      {returnTo && (() => {
        const canContinueWithCurrentModel = !isNew && !hasUnsavedChanges;
        const canContinueSetup =
          !(saving || verifying) && (canSave || canContinueWithCurrentModel);
        return (
          <div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-50">
            <button
              onClick={() => {
                if (canSave) {
                  void handleSaveWithMoveCheck({ navigateAfterSave: true });
                  return;
                }
                if (canContinueWithCurrentModel) {
                  editNavigate(returnTo);
                }
              }}
              disabled={!canContinueSetup}
              className={cn(
                "flex items-center gap-2 rounded-full px-6 py-3 text-sm font-bold transition active:scale-[0.98]",
                canContinueSetup
                  ? "border border-emerald-500/40 bg-emerald-500 text-black shadow-[0_4px_20px_rgba(16,185,129,0.35)] hover:bg-emerald-400"
                  : "border border-white/10 bg-white/10 text-white/40 cursor-not-allowed",
              )}
            >
              {isOnboardingReturnFlow || canContinueWithCurrentModel
                ? t("editModel.continueSetup.continue")
                : t("editModel.continueSetup.saveToContinue")}
              <ArrowRight size={16} />
            </button>
          </div>
        );
      })()}
    </div>
  );
}
