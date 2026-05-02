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
} from "../../../core/storage/schemas";
import { getProviderIcon } from "../../../core/utils/providerIcons";
import { cn } from "../../design-tokens";
import { openDocs } from "../../../core/utils/docs";
import { useI18n } from "../../../core/i18n/context";
import { Switch } from "../../components/Switch";
import { getPlatform } from "../../../core/utils/platform";

type DownloadedGgufModel = {
  modelId: string;
  filename: string;
  path: string;
  size: number;
  quantization: string;
  isMmproj?: boolean;
};

type LocalLibraryPickerMode = "model" | "mmproj";

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

function getLlamaRuntimeHeadline(report: LlamaLastRuntimeReport): string {
  if (report.status === "succeeded") {
    return "The last local run completed successfully.";
  }
  if (report.status === "cpuFallbackSucceeded") {
    return "GPU load failed, then the model recovered on CPU.";
  }
  if (report.status === "cpuFallbackFailed") {
    return "GPU load failed and the CPU fallback still did not complete.";
  }
  return "The last local run failed before llama.cpp could complete.";
}

function getLlamaRuntimeDetail(report: LlamaLastRuntimeReport): string {
  if (report.status === "succeeded") {
    return "This report also seeds the smart offload cache so future runs can reuse the last stable GPU setup.";
  }
  if (report.status === "cpuFallbackSucceeded") {
    return "We stored the CPU-safe context and batch that did run so you can reuse them.";
  }
  if (report.status === "cpuFallbackFailed") {
    return "The model was retried on CPU, but the recovered configuration still failed.";
  }
  return "This report keeps the last known runtime state so you can inspect what happened.";
}

type EditorViewMode = "simple" | "advanced";
type EditorSectionKey = "generation" | "runtime" | "reasoning" | "caching" | "capabilities";
type SimpleEditorSectionKey = "generation" | "runtime" | "reasoning" | "capabilities";

const EDITOR_FADE_DURATION = 0.16;
const MODEL_EDITOR_VIEW_MODE_STORAGE_KEY = "lettuce.settings.models.editorViewMode";

function getStoredEditorViewMode(): EditorViewMode {
  if (typeof window === "undefined") {
    return "simple";
  }
  const stored = window.localStorage.getItem(MODEL_EDITOR_VIEW_MODE_STORAGE_KEY);
  return stored === "advanced" ? "advanced" : "simple";
}

const LLAMA_KV_TYPE_OPTIONS = [
  { value: "auto", label: "Auto (model default)" },
  { value: "f16", label: "F16 (best quality, highest VRAM)" },
  { value: "q8_0", label: "Q8_0 (recommended)" },
  { value: "q8_1", label: "Q8_1" },
  { value: "q6_k", label: "Q6_K" },
  { value: "q5_k", label: "Q5_K" },
  { value: "q5_1", label: "Q5_1" },
  { value: "q5_0", label: "Q5_0" },
  { value: "q4_k", label: "Q4_K" },
  { value: "q4_1", label: "Q4_1" },
  { value: "q4_0", label: "Q4_0" },
  { value: "q3_k", label: "Q3_K" },
  { value: "q2_k", label: "Q2_K (max VRAM saving)" },
] as const;

const LLAMA_CHAT_TEMPLATE_PRESET_OPTIONS = [
  { value: "auto", label: "Auto (prefer embedded GGUF template)" },
  { value: "chatml", label: "ChatML" },
  { value: "llama2", label: "Llama 2" },
  { value: "llama3", label: "Llama 3" },
  { value: "mistral-v1", label: "Mistral Instruct v1" },
  { value: "vicuna", label: "Vicuna" },
  { value: "gemma", label: "Gemma" },
] as const;

const LLAMA_SAMPLER_PROFILE_OPTIONS = [
  { value: "balanced", label: "Balanced" },
  { value: "creative", label: "Creative" },
  { value: "stable", label: "Stable" },
  { value: "reasoning", label: "Reasoning" },
] as const;

const LLAMA_QUICK_PRESET_DETAILS = {
  balanced: ["Batch Size 512", "KV Cache q8_0", "Offload KQV On", "Flash Attention Auto"],
  throughput: ["Batch Size 1024", "KV Cache f16", "Offload KQV On", "Flash Attention Enabled"],
  vram: ["Batch Size 512", "KV Cache q4_k", "Offload KQV On", "Flash Attention Enabled"],
  cpu_ram: ["Batch Size 256", "KV Cache q8_0", "Offload KQV Off", "Flash Attention Auto"],
} as const;

const LLAMA_SAMPLER_PROFILE_DETAILS = {
  balanced: ["Temp 0.80", "Top P 0.95", "Top K 40", "Min P 0.05", "Freq Pen. 0.15"],
  creative: ["Temp 0.95", "Top P 0.98", "Top K 80", "Min P 0.02", "Presence Pen. 0.25"],
  stable: ["Temp 0.55", "Top P 0.90", "Top K 32", "Min P 0.08", "Typical P 0.97"],
  reasoning: ["Temp 0.35", "Top P 0.90", "Top K 24", "Typical P 0.95", "Freq Pen. 0.10"],
} as const;

const normalizeSearchText = (value?: string) =>
  (value ?? "")
    .toLowerCase()
    .replace(/[_:/.-]+/g, " ")
    .replace(/[^a-z0-9\s]/g, "")
    .replace(/\s+/g, " ")
    .trim();

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

function EditorPanel({
  title,
  description,
  action,
  children,
  className,
}: {
  title: string;
  description?: string;
  action?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={cn("rounded-xl border border-fg/10 bg-surface-el/35", className)}>
      <div className="flex items-start justify-between gap-4 border-b border-fg/8 px-5 py-4">
        <div className="min-w-0">
          <h2 className="text-[13px] font-semibold text-fg">{title}</h2>
          {description ? (
            <p className="mt-1 text-[13px] leading-relaxed text-fg/45">{description}</p>
          ) : null}
        </div>
        {action ? <div className="shrink-0">{action}</div> : null}
      </div>
      <div className="p-5">{children}</div>
    </section>
  );
}

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
      <div className="flex items-center justify-between gap-3">
        <label className="text-[13px] font-medium text-fg/72">{label}</label>
        {action ? <div className="shrink-0">{action}</div> : null}
      </div>
      {children}
    </div>
  );
}

function CollapsedEditorSectionButton({
  title,
  summary,
  description,
  isOpen,
  onClick,
}: {
  title: string;
  summary: string;
  description: string;
  isOpen: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "flex w-full items-start justify-between gap-4 rounded-lg border px-4 py-3 text-left transition",
        isOpen
          ? "border-fg/20 bg-fg/6 text-fg"
          : "border-fg/10 bg-transparent text-fg/70 hover:border-fg/18 hover:bg-fg/4 hover:text-fg/92",
      )}
    >
      <div className="min-w-0">
        <div className="text-[13px] font-medium text-current">{title}</div>
        <div className="mt-1 text-[13px] text-fg/48">{description}</div>
        <div className="mt-2 text-[12px] text-fg/36">{summary}</div>
      </div>
      <div className="mt-0.5 shrink-0 text-fg/40">
        {isOpen ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
      </div>
    </button>
  );
}

export function EditModelPage() {
  const { t } = useI18n();
  const isMobile = useMemo(() => getPlatform().type === "mobile", []);
  const [showParameterSupport, setShowParameterSupport] = useState(false);
  const [isManualInput, setIsManualInput] = useState(false);
  const [showModelSelector, setShowModelSelector] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [debouncedSearchQuery, setDebouncedSearchQuery] = useState("");
  const [showOnlyFreeModels, setShowOnlyFreeModels] = useState(false);
  const [editorViewMode, setEditorViewMode] = useState<EditorViewMode>(() =>
    getStoredEditorViewMode(),
  );
  const [activeAdvancedPanel, setActiveAdvancedPanel] = useState<EditorSectionKey>("generation");
  const [activeSimplePanel, setActiveSimplePanel] = useState<SimpleEditorSectionKey | null>(null);
  const [selectedLlamaQuickPreset, setSelectedLlamaQuickPreset] = useState<
    "balanced" | "throughput" | "vram" | "cpu_ram" | null
  >(null);
  const [showPlatformSelector, setShowPlatformSelector] = useState(false);
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
    handleLlamaSamplerProfileChange,
    handleLlamaSamplerOrderChange,
    handleLlamaMinPChange,
    handleLlamaTypicalPChange,
    handleLlamaChatTemplateOverrideChange,
    handleLlamaMmprojPathChange,
    handleLlamaChatTemplatePresetChange,
    handleLlamaRawCompletionFallbackChange,
    handleLlamaStrictModeChange,
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
      ["Updated", formatRuntimeDate(llamaRuntimeReport.updatedAt)],
      ["Model path", llamaRuntimeReport.modelPath],
      ["Backend used", llamaRuntimeReport.backendPathUsed ?? null],
      ["Failure stage", llamaRuntimeReport.failureStage ?? null],
      ["Requested context", formatRuntimeNumber(llamaRuntimeReport.requestedContext)],
      ["Recommended context", formatRuntimeNumber(llamaRuntimeReport.recommendedContext)],
      ["Initial context", formatRuntimeNumber(llamaRuntimeReport.initialContextCandidate)],
      ["Actual context", formatRuntimeNumber(llamaRuntimeReport.actualContextUsed)],
      ["Requested GPU layers", formatRuntimeNumber(llamaRuntimeReport.requestedGpuLayers)],
      ["Actual GPU layers", formatRuntimeNumber(llamaRuntimeReport.actualGpuLayersUsed)],
      ["Requested batch", formatRuntimeNumber(llamaRuntimeReport.requestedBatchLimit)],
      ["Initial batch", formatRuntimeNumber(llamaRuntimeReport.initialBatchCandidate)],
      ["Actual batch", formatRuntimeNumber(llamaRuntimeReport.actualBatchUsed)],
      [
        "Smart offload fallback",
        llamaRuntimeReport.smartGpuLayerFallbackActivated == null
          ? null
          : llamaRuntimeReport.smartGpuLayerFallbackActivated
            ? "Active"
            : "Not needed",
      ],
      [
        "KQV fallback",
        llamaRuntimeReport.kqvFallbackActivated == null
          ? null
          : llamaRuntimeReport.kqvFallbackActivated
            ? "Moved to RAM"
            : "Not needed",
      ],
      [
        "Smart offload estimate",
        formatRuntimeNumber(llamaRuntimeReport.smartOffloadEstimatedGpuLayers),
      ],
      [
        "Smart offload candidates",
        llamaRuntimeReport.smartOffloadCandidateLayers?.length
          ? llamaRuntimeReport.smartOffloadCandidateLayers.join(", ")
          : null,
      ],
      ["KV cache", llamaRuntimeReport.actualKvTypeUsed ?? null],
      ["KQV offload", llamaRuntimeReport.actualOffloadKqvMode ?? null],
      ["Flash attention", llamaRuntimeReport.flashAttentionPolicy ?? null],
      [
        "GPU backends",
        llamaRuntimeReport.compiledGpuBackends?.length
          ? llamaRuntimeReport.compiledGpuBackends.join(", ")
          : null,
      ],
      [
        "Available RAM",
        llamaRuntimeReport.availableMemoryBytes
          ? formatBytes(llamaRuntimeReport.availableMemoryBytes)
          : null,
      ],
      [
        "Available VRAM",
        llamaRuntimeReport.availableVramBytes
          ? formatBytes(llamaRuntimeReport.availableVramBytes)
          : null,
      ],
      [
        "Model size",
        llamaRuntimeReport.modelSizeBytes ? formatBytes(llamaRuntimeReport.modelSizeBytes) : null,
      ],
      ["Prompt tokens", formatRuntimeNumber(llamaRuntimeReport.promptTokens)],
      ["Prompt positions", formatRuntimeNumber(llamaRuntimeReport.promptPositions)],
      ["Target new tokens", formatRuntimeNumber(llamaRuntimeReport.targetNewTokens)],
      ["Completion tokens", formatRuntimeNumber(llamaRuntimeReport.completionTokens)],
      ["Finish reason", llamaRuntimeReport.finishReason ?? null],
      [
        "First token",
        llamaRuntimeReport.firstTokenMs
          ? `${formatRuntimeNumber(llamaRuntimeReport.firstTokenMs)} ms`
          : null,
      ],
      ["Throughput", formatRuntimeRate(llamaRuntimeReport.tokensPerSecond)],
      ["Prompt template", llamaRuntimeReport.promptTemplateSource ?? null],
    ] as const;
    return fields.filter(([, value]) => value).map(([label, value]) => ({ label, value: value! }));
  }, [llamaRuntimeReport]);
  const handleApplyLlamaRuntimeSuggestion = async () => {
    const applied = await applyLlamaRuntimeSuggestion();
    if (applied) {
      toast.success(
        "Runtime config applied",
        "Future local runs will reuse the last CPU-safe context and batch.",
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
        "Model path required",
        "Select a GGUF model path before reading the embedded template.",
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
    toast.success("Embedded template pasted into editor");
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
  const modelIdLabel = isLocalModel ? "Model Path (GGUF)" : "Model ID";
  const modelIdPlaceholder = isLocalModel ? "/path/to/model.gguf" : "e.g. gpt-4o";
  const mmprojLibraryModels = useMemo(
    () =>
      downloadedModels.filter(
        (model) => model.isMmproj ?? model.filename.toLowerCase().includes("mmproj"),
      ),
    [downloadedModels],
  );
  const localLibraryModels =
    localLibraryPickerMode === "mmproj" ? mmprojLibraryModels : downloadedModels;
  const localLibraryTitle =
    localLibraryPickerMode === "mmproj" ? "Downloaded MMProj Files" : t("hfBrowser.libraryTitle");
  const localLibraryEmptyLabel =
    localLibraryPickerMode === "mmproj"
      ? "No downloaded mmproj files yet"
      : t("hfBrowser.libraryEmpty");
  const localLibraryEmptyHint =
    localLibraryPickerMode === "mmproj"
      ? "Download a multimodal projector from the Model Browser, or enter a path manually."
      : t("hfBrowser.libraryEmptyHint");
  const isAutomatic1111Provider = editorModel?.providerId === "automatic1111";

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
          { value: "in_memory", label: "In-memory" },
          { value: "24h", label: "24 hr" },
        ]
      : [
          { value: "5min", label: "5 min" },
          { value: "1h", label: "1 hr" },
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
      ? "RAM"
      : modelAdvancedDraft.llamaOffloadKqv === true
      ? "VRAM"
      : modelAdvancedDraft.llamaOffloadKqv === false
        ? "RAM"
        : "Auto";
  const selectedSamplerProfile = modelAdvancedDraft.llamaSamplerProfile ?? "balanced";
  const ollamaStopText = (modelAdvancedDraft.ollamaStop ?? []).join("\n");
  const selectedFetchedModel = fetchedModels.find((model) => model.id === editorModel?.name);
  const selectedProviderLabel =
    selectedProviderCredential?.label ||
    editorModel?.providerLabel ||
    editorModel?.providerId ||
    "Select platform";
  const hasRuntimePanel = isLocalModel || isOllamaModel;
  const runtimePanelTitle = isLocalModel ? "llama.cpp" : isOllamaModel ? "Ollama" : "Runtime";
  const effectiveEditorViewMode: EditorViewMode = isMobile ? "simple" : editorViewMode;
  const activeDetailPanel =
    effectiveEditorViewMode === "advanced" ? activeAdvancedPanel : activeSimplePanel;
  const toggleSimplePanel = (panel: SimpleEditorSectionKey) => {
    setActiveSimplePanel((current) => (current === panel ? null : panel));
  };
  function updateSdSetting<K extends keyof typeof modelAdvancedDraft>(
    key: K,
    value: (typeof modelAdvancedDraft)[K],
  ) {
    setModelAdvancedDraft({
      ...modelAdvancedDraft,
      [key]: value,
    });
  }
  const generationSummary = isAutomatic1111Provider
    ? [
        modelAdvancedDraft.sdSteps != null ? `Steps ${modelAdvancedDraft.sdSteps}` : null,
        modelAdvancedDraft.sdCfgScale != null
          ? `CFG ${modelAdvancedDraft.sdCfgScale.toFixed(1)}`
          : null,
        modelAdvancedDraft.sdSampler ? modelAdvancedDraft.sdSampler : null,
        modelAdvancedDraft.sdSize ? modelAdvancedDraft.sdSize : null,
      ]
        .filter(Boolean)
        .join(" • ") || "Stable Diffusion sampler, CFG, seed, and size defaults"
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
        .join(" • ") || "Default sampling and output limits";
  const runtimeSummary = isLocalModel
    ? [
        modelAdvancedDraft.llamaBatchSize != null
          ? `Batch ${modelAdvancedDraft.llamaBatchSize}`
          : null,
        modelAdvancedDraft.llamaKvType ? `KV ${modelAdvancedDraft.llamaKvType}` : null,
        modelAdvancedDraft.llamaOffloadKqv === true
          ? "KV cache in VRAM"
          : modelAdvancedDraft.llamaOffloadKqv === false
            ? "KV cache in RAM"
            : null,
      ]
        .filter(Boolean)
        .join(" • ") || "Execution, memory, and hardware defaults"
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
          .join(" • ") || "Ollama runtime defaults"
      : "";
  const reasoningSummary = isAutoReasoning
    ? "Always enabled for this provider"
    : modelAdvancedDraft.reasoningEnabled === false
      ? "Reasoning disabled"
      : [
          modelAdvancedDraft.reasoningEnabled ? "Enabled" : "Provider default",
          modelAdvancedDraft.reasoningEffort
            ? `Effort ${modelAdvancedDraft.reasoningEffort}`
            : null,
          modelAdvancedDraft.reasoningBudgetTokens != null
            ? `Budget ${modelAdvancedDraft.reasoningBudgetTokens.toLocaleString()}`
            : null,
        ]
          .filter(Boolean)
          .join(" • ") || "Thinking controls stay on provider defaults";
  const inputCapabilitySummary = (editorModel?.inputScopes ?? [])
    .filter((scope) => scope !== "text")
    .map((scope) => scope[0].toUpperCase() + scope.slice(1))
    .join(", ");
  const outputCapabilitySummary = (editorModel?.outputScopes ?? [])
    .filter((scope) => scope !== "text")
    .map((scope) => scope[0].toUpperCase() + scope.slice(1))
    .join(", ");
  const capabilitiesSummary = `Input: ${inputCapabilitySummary || "Text only"} • Output: ${outputCapabilitySummary || "Text only"}`;
  const simpleDetailOrder =
    activeSimplePanel === "generation"
      ? 15
      : activeSimplePanel === "runtime"
        ? 25
        : activeSimplePanel === "reasoning"
          ? 35
          : 45;
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
    if (activeAdvancedPanel === "runtime" && !hasRuntimePanel) {
      setActiveAdvancedPanel(showReasoningSection ? "reasoning" : "generation");
      return;
    }
    if (activeAdvancedPanel === "reasoning" && !showReasoningSection) {
      setActiveAdvancedPanel(hasRuntimePanel ? "runtime" : "generation");
    }
  }, [activeAdvancedPanel, hasRuntimePanel, showReasoningSection]);

  useEffect(() => {
    if (activeSimplePanel === "runtime" && !hasRuntimePanel) {
      setActiveSimplePanel(null);
      return;
    }
    if (activeSimplePanel === "reasoning" && !showReasoningSection) {
      setActiveSimplePanel(null);
    }
  }, [activeSimplePanel, hasRuntimePanel, showReasoningSection]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    if (isMobile) {
      return;
    }
    window.localStorage.setItem(MODEL_EDITOR_VIEW_MODE_STORAGE_KEY, editorViewMode);
  }, [editorViewMode, isMobile]);

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
          { filePath: modelPath },
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
  }, [editorModel?.name, isLocalModel]);

  const scopeOrder = ["text", "image", "audio"] as const;
  const toggleScope = (
    key: "inputScopes" | "outputScopes",
    scope: "image" | "audio",
    enabled: boolean,
  ) => {
    if (!editorModel) return;
    if (isAutomatic1111Provider) return;
    const current = new Set((editorModel as any)[key] ?? ["text"]);
    if (enabled) current.add(scope);
    else current.delete(scope);
    current.add("text");
    const next = scopeOrder.filter((s) => current.has(s));
    updateEditorModel({ [key]: next } as any);
  };

  const handleSelectModel = (modelId: string, displayName?: string) => {
    handleModelNameChange(modelId);
    if (displayName) {
      handleDisplayNameChange(displayName);
    } else {
      handleDisplayNameChange(modelId);
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
      <main className="flex-1 overflow-y-auto px-4 pt-4 pb-32">
        <motion.div
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          className="mx-auto w-full max-w-6xl space-y-6"
        >
          {error && (
            <div className="rounded-xl border border-danger/30 bg-danger/10 px-4 py-3">
              <p className="text-[13px] text-danger/80">{error}</p>
            </div>
          )}

          <div className="relative">
            <div className="w-full space-y-6">
              <EditorPanel
                title="Model setup"
                description="Choose the platform, give this entry a readable name, and connect it to the model identifier or file you want to use."
              >
                <div className="space-y-6">
                  {isLocalModel && llamaRuntimeReport && (
                    <div className="rounded-lg border border-fg/10 bg-fg/4">
                      <button
                        type="button"
                        onClick={() => setShowLlamaRuntimeReport((value) => !value)}
                        className="flex w-full items-start justify-between gap-4 px-4 py-3 text-left"
                      >
                        <div className="flex min-w-0 items-start gap-3">
                          <div
                            className={cn(
                              "mt-0.5 flex h-7 w-7 items-center justify-center rounded-md border",
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
                            <div className="flex flex-wrap items-center gap-2">
                              <span className="text-[13px] font-medium text-fg">
                                Last runtime report
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
                                  ? "Run succeeded"
                                  : llamaRuntimeReport.status === "cpuFallbackSucceeded"
                                    ? "CPU fallback recovered"
                                    : llamaRuntimeReport.status === "cpuFallbackFailed"
                                      ? "CPU fallback failed"
                                      : "Run failed"}
                              </span>
                            </div>
                            <p className="text-[13px] leading-relaxed text-fg/72">
                              {getLlamaRuntimeHeadline(llamaRuntimeReport)}
                            </p>
                            <p className="text-[12px] leading-relaxed text-fg/48">
                              {getLlamaRuntimeDetail(llamaRuntimeReport)}
                            </p>
                          </div>
                        </div>
                        {showLlamaRuntimeReport ? (
                          <ChevronDown className="mt-1 h-4 w-4 text-fg/45" />
                        ) : (
                          <ChevronRight className="mt-1 h-4 w-4 text-fg/45" />
                        )}
                      </button>

                      <AnimatePresence initial={false}>
                        {showLlamaRuntimeReport && (
                          <motion.div
                            initial={{ height: 0, opacity: 0 }}
                            animate={{ height: "auto", opacity: 1 }}
                            exit={{ height: 0, opacity: 0 }}
                            transition={{ duration: 0.16, ease: "easeOut" }}
                            className="overflow-hidden border-t border-fg/8"
                          >
                            <div className="space-y-4 px-4 py-4">
                              {(llamaRuntimeReport.gpuFallbackReason ||
                                llamaRuntimeReport.errorMessage) && (
                                <div className="grid gap-3 lg:grid-cols-2">
                                  {llamaRuntimeReport.gpuFallbackReason && (
                                    <div className="rounded-lg border border-warning/25 bg-warning/8 px-3 py-2.5">
                                      <div className="text-[11px] font-medium uppercase tracking-wide text-warning/90">
                                        GPU fallback reason
                                      </div>
                                      <p className="mt-1 text-[13px] leading-relaxed text-fg/78">
                                        {llamaRuntimeReport.gpuFallbackReason}
                                      </p>
                                    </div>
                                  )}
                                  {llamaRuntimeReport.errorMessage && (
                                    <div className="rounded-lg border border-danger/25 bg-danger/8 px-3 py-2.5">
                                      <div className="text-[11px] font-medium uppercase tracking-wide text-danger/90">
                                        Final error
                                      </div>
                                      <p className="mt-1 text-[13px] leading-relaxed text-fg/78">
                                        {llamaRuntimeReport.errorMessage}
                                      </p>
                                    </div>
                                  )}
                                </div>
                              )}

                              <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
                                {llamaRuntimeFacts.map((item) => (
                                  <div
                                    key={item.label}
                                    className="rounded-lg border border-fg/8 bg-surface-el/14 px-3 py-2.5"
                                  >
                                    <div className="text-[11px] text-fg/42">{item.label}</div>
                                    <div className="mt-1 wrap-break-word text-[13px] text-fg/86">
                                      {item.value}
                                    </div>
                                  </div>
                                ))}
                              </div>

                              {llamaRuntimeReport.status === "cpuFallbackSucceeded" &&
                                llamaRuntimeReport.suggestedSettings && (
                                  <div className="flex flex-col gap-3 rounded-lg border border-fg/10 bg-surface-el/18 px-3 py-3 sm:flex-row sm:items-center sm:justify-between">
                                    <div className="space-y-1">
                                      <div className="text-[13px] font-medium text-fg">
                                        Working recovery config
                                      </div>
                                      <p className="text-[12px] text-fg/52">
                                        Context{" "}
                                        {formatRuntimeNumber(
                                          llamaRuntimeReport.suggestedSettings.contextLength,
                                        ) ?? "n/a"}
                                        {" • "}Batch{" "}
                                        {formatRuntimeNumber(
                                          llamaRuntimeReport.suggestedSettings.llamaBatchSize,
                                        ) ?? "n/a"}
                                      </p>
                                    </div>
                                    <button
                                      type="button"
                                      onClick={() => void handleApplyLlamaRuntimeSuggestion()}
                                      disabled={saving}
                                      className="rounded-lg border border-warning/30 bg-warning/12 px-3 py-2 text-[13px] font-medium text-warning transition hover:bg-warning/18 disabled:cursor-not-allowed disabled:opacity-60"
                                    >
                                      Apply working config
                                    </button>
                                  </div>
                                )}
                            </div>
                          </motion.div>
                        )}
                      </AnimatePresence>
                    </div>
                  )}

                  <FieldBlock label="Platform">
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
                          title="Select Platform"
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
                      <FieldBlock label="Display name">
                        <input
                          type="text"
                          value={editorModel.displayName}
                          onChange={(e) => handleDisplayNameChange(e.target.value)}
                          placeholder="e.g. My Favorite ChatGPT"
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
                              Browse files
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
                            Use the full file path to a local GGUF model.
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
                  ) : (
                    <div className="grid items-start grid-cols-1 gap-6 lg:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
                      <FieldBlock label="Display name">
                        <input
                          type="text"
                          value={editorModel.displayName}
                          onChange={(e) => handleDisplayNameChange(e.target.value)}
                          placeholder="e.g. My Favorite ChatGPT"
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
                                {isManualInput ? "Use catalog" : "Enter manually"}
                              </button>
                            )}
                            {modelFetchEnabledForSelectedProvider && (
                              <button
                                type="button"
                                onClick={fetchModels}
                                disabled={fetchingModels || !editorModel?.providerId}
                                className="rounded-md border border-fg/10 p-1.5 text-fg/45 transition hover:border-fg/20 hover:bg-fg/5 hover:text-fg/85 disabled:opacity-30"
                                title="Refresh model list"
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
                                  "Select a model"}
                              </span>
                              <ChevronDown className="h-4 w-4 text-fg/40" />
                            </button>

                            <ModelSelectionBottomMenu
                              isOpen={showModelSelector}
                              onClose={() => setShowModelSelector(false)}
                              title="Select Model"
                              models={filteredModels as any}
                              selectedModelIds={editorModel.name ? [editorModel.name] : []}
                              searchQuery={searchQuery}
                              onSearchChange={setSearchQuery}
                              searchPlaceholder="Search models..."
                              filterModels={false}
                              rightAction={
                                isOpenRouterProvider ? (
                                  <span className="flex items-center gap-2">
                                    <span className="text-[13px] text-fg/70 whitespace-nowrap">
                                      only free models
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
                              renderEmptyState={() => (
                                <div className="py-10 text-center text-[13px] text-fg/40">
                                  <p>
                                    {t("common.buttons.search")}: "{searchQuery}"
                                  </p>
                                  {didYouMeanSuggestions.length > 0 && (
                                    <div className="mt-4">
                                      <p className="mb-2 text-[13px] text-fg/50">Did you mean:</p>
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
                                  Model fetching is disabled for this custom endpoint. Enable it in
                                  Provider settings and set a Models Endpoint if you want model list
                                  discovery.
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
                  title="Move Model File"
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
              </EditorPanel>

              <AnimatePresence mode="wait" initial={false}>
                <motion.div
                  key={effectiveEditorViewMode}
                  initial={{ opacity: 0, y: 6 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -6 }}
                  transition={{
                    duration: EDITOR_FADE_DURATION,
                    ease: "easeInOut",
                  }}
                  className={cn(
                    "space-y-4",
                    effectiveEditorViewMode === "simple" && "flex flex-col gap-2 space-y-0",
                  )}
                >
                  {!isMobile && (
                    <div className="flex flex-wrap items-center justify-between gap-3">
                      <div>
                        <div className="text-[13px] font-medium text-fg/78">Editor mode</div>
                        <div className="mt-1 text-[13px] text-fg/45">
                          Simple starts collapsed. Advanced keeps the current full editor.
                        </div>
                      </div>
                      <div className="inline-flex rounded-lg border border-fg/10 bg-fg/4 p-1">
                        <button
                          type="button"
                          onClick={() => setEditorViewMode("simple")}
                          className={cn(
                            "rounded-md px-3 py-1.5 text-[13px] transition",
                            editorViewMode === "simple"
                              ? "bg-fg/10 text-fg"
                              : "text-fg/55 hover:text-fg/82",
                          )}
                        >
                          Simple
                        </button>
                        <button
                          type="button"
                          onClick={() => setEditorViewMode("advanced")}
                          className={cn(
                            "rounded-md px-3 py-1.5 text-[13px] transition",
                            editorViewMode === "advanced"
                              ? "bg-fg/10 text-fg"
                              : "text-fg/55 hover:text-fg/82",
                          )}
                        >
                          Advanced
                        </button>
                      </div>
                    </div>
                  )}

                  {effectiveEditorViewMode === "advanced" ? (
                    <div className="grid grid-cols-1 gap-2 sm:grid-cols-2 xl:grid-cols-3">
                      <button
                        type="button"
                        onClick={() => setActiveAdvancedPanel("generation")}
                        className={cn(
                          "rounded-lg border px-4 py-3 text-left transition",
                          activeAdvancedPanel === "generation"
                            ? "border-fg/22 bg-fg/8 text-fg"
                            : "border-fg/10 bg-transparent text-fg/65 hover:border-fg/18 hover:bg-fg/4 hover:text-fg/90",
                        )}
                      >
                        <div className="text-[13px] font-medium">Generation Parameters</div>
                        <div className="mt-1 text-[13px] text-fg/45">
                          {isAutomatic1111Provider
                            ? "Sampler, CFG, seed, negative prompt, denoise, and size defaults."
                            : "Temperature, sampling, penalties, and output limits."}
                        </div>
                      </button>

                      {hasRuntimePanel && (
                        <button
                          type="button"
                          onClick={() => setActiveAdvancedPanel("runtime")}
                          className={cn(
                            "rounded-lg border px-4 py-3 text-left transition",
                            activeAdvancedPanel === "runtime"
                              ? "border-fg/22 bg-fg/8 text-fg"
                              : "border-fg/10 bg-transparent text-fg/65 hover:border-fg/18 hover:bg-fg/4 hover:text-fg/90",
                          )}
                        >
                          <div className="text-[13px] font-medium">Runtime</div>
                          <div className="mt-1 text-[13px] text-fg/45">
                            {runtimePanelTitle} execution, memory, and hardware controls.
                          </div>
                        </button>
                      )}

                      {showReasoningSection && (
                        <button
                          type="button"
                          onClick={() => setActiveAdvancedPanel("reasoning")}
                          className={cn(
                            "rounded-lg border px-4 py-3 text-left transition",
                            activeAdvancedPanel === "reasoning"
                              ? "border-fg/22 bg-fg/8 text-fg"
                              : "border-fg/10 bg-transparent text-fg/65 hover:border-fg/18 hover:bg-fg/4 hover:text-fg/90",
                          )}
                        >
                          <div className="text-[13px] font-medium">Reasoning</div>
                          <div className="mt-1 text-[13px] text-fg/45">
                            Thinking mode, effort, and token budget controls.
                          </div>
                        </button>
                      )}

                      {showCachingSection && (
                        <button
                          type="button"
                          onClick={() => setActiveAdvancedPanel("caching")}
                          className={cn(
                            "rounded-lg border px-4 py-3 text-left transition",
                            activeAdvancedPanel === "caching"
                              ? "border-fg/22 bg-fg/8 text-fg"
                              : "border-fg/10 bg-transparent text-fg/65 hover:border-fg/18 hover:bg-fg/4 hover:text-fg/90",
                          )}
                        >
                          <div className="text-[13px] font-medium">Prompt Caching</div>
                          <div className="mt-1 text-[13px] text-fg/45">
                            Context reuse and prefix caching behavior.
                          </div>
                        </button>
                      )}

                      <button
                        type="button"
                        onClick={() => setActiveAdvancedPanel("capabilities")}
                        className={cn(
                          "rounded-lg border px-4 py-3 text-left transition",
                          activeAdvancedPanel === "capabilities"
                            ? "border-fg/22 bg-fg/8 text-fg"
                            : "border-fg/10 bg-transparent text-fg/65 hover:border-fg/18 hover:bg-fg/4 hover:text-fg/90",
                        )}
                      >
                        <div className="text-[13px] font-medium">Capabilities</div>
                        <div className="mt-1 text-[13px] text-fg/45">
                          Input and output modalities this model supports.
                        </div>
                      </button>
                    </div>
                  ) : (
                    <>
                      <div style={{ order: 10 }}>
                        <CollapsedEditorSectionButton
                          title="Generation Parameters"
                          description={
                            isAutomatic1111Provider
                              ? "Sampler, CFG, seed, negative prompt, denoise, and size defaults."
                              : "Temperature, sampling, penalties, and output limits."
                          }
                          summary={generationSummary}
                          isOpen={activeSimplePanel === "generation"}
                          onClick={() => toggleSimplePanel("generation")}
                        />
                      </div>
                      {hasRuntimePanel && (
                        <div style={{ order: 20 }}>
                          <CollapsedEditorSectionButton
                            title="Runtime"
                            description={`${runtimePanelTitle} execution, memory, and hardware controls.`}
                            summary={runtimeSummary}
                            isOpen={activeSimplePanel === "runtime"}
                            onClick={() => toggleSimplePanel("runtime")}
                          />
                        </div>
                      )}
                      {showReasoningSection && (
                        <div style={{ order: 30 }}>
                          <CollapsedEditorSectionButton
                            title="Reasoning"
                            description="Thinking mode, effort, and token budget controls."
                            summary={reasoningSummary}
                            isOpen={activeSimplePanel === "reasoning"}
                            onClick={() => toggleSimplePanel("reasoning")}
                          />
                        </div>
                      )}
                      <div style={{ order: 40 }}>
                        <CollapsedEditorSectionButton
                          title="Capabilities"
                          description="Mark which modalities this model accepts and what it can produce."
                          summary={capabilitiesSummary}
                          isOpen={activeSimplePanel === "capabilities"}
                          onClick={() => toggleSimplePanel("capabilities")}
                        />
                      </div>
                    </>
                  )}

                  {activeDetailPanel ? (
                    <motion.div
                      key={`${effectiveEditorViewMode}-${activeDetailPanel}`}
                      initial={{ opacity: 0, y: 6 }}
                      animate={{ opacity: 1, y: 0 }}
                      className="pt-1"
                      style={
                        effectiveEditorViewMode === "simple"
                          ? { order: simpleDetailOrder }
                          : undefined
                      }
                    >
                      <div className="space-y-8">
                        {/* Generation Parameters */}
                        {activeDetailPanel === "generation" && (
                          <div className="space-y-4">
                            <div className="flex items-center justify-between">
                              <label className="text-[13px] font-bold tracking-wider text-fg/50 uppercase">
                                Generation Parameters
                              </label>
                              <button
                                type="button"
                                onClick={() => setShowParameterSupport(true)}
                                className="text-fg/40 hover:text-fg/60 transition"
                              >
                                <Info size={14} />
                              </button>
                            </div>

                            {isAutomatic1111Provider ? (
                              <div className="space-y-5">
                                <div className="rounded-xl border border-accent/20 bg-accent/8 px-4 py-3 text-[13px] leading-relaxed text-fg/72">
                                  AUTOMATIC1111 uses Stable Diffusion controls here. These values
                                  become the default sampler settings for avatars, scene images, and
                                  other local diffusion requests.
                                </div>

                                <div className="grid grid-cols-1 gap-x-6 gap-y-8 md:grid-cols-2 xl:grid-cols-3 xl:gap-x-8">
                                  <div className="space-y-4">
                                    <div className="flex items-center justify-between">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          Steps
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Diffusion sampling steps
                                        </span>
                                      </div>
                                      <span className="rounded-lg bg-surface-el/30 px-2 py-1 font-mono text-[13px] text-accent">
                                        {modelAdvancedDraft.sdSteps ?? "28"}
                                      </span>
                                    </div>
                                    <input
                                      type="number"
                                      inputMode="numeric"
                                      min={ADVANCED_SD_STEPS_RANGE.min}
                                      max={ADVANCED_SD_STEPS_RANGE.max}
                                      step={1}
                                      value={modelAdvancedDraft.sdSteps ?? ""}
                                      onChange={(e) => {
                                        const raw = e.target.value;
                                        const next = raw === "" ? null : Number(raw);
                                        updateSdSetting(
                                          "sdSteps",
                                          next === null || !Number.isFinite(next)
                                            ? null
                                            : Math.trunc(next),
                                        );
                                      }}
                                      placeholder="28"
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
                                          CFG Scale
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Prompt guidance strength
                                        </span>
                                      </div>
                                      <span className="rounded-lg bg-surface-el/30 px-2 py-1 font-mono text-[13px] text-accent">
                                        {modelAdvancedDraft.sdCfgScale?.toFixed(1) ?? "6.5"}
                                      </span>
                                    </div>
                                    <input
                                      type="number"
                                      inputMode="decimal"
                                      min={ADVANCED_SD_CFG_SCALE_RANGE.min}
                                      max={ADVANCED_SD_CFG_SCALE_RANGE.max}
                                      step={0.1}
                                      value={modelAdvancedDraft.sdCfgScale ?? ""}
                                      onChange={(e) => {
                                        const raw = e.target.value;
                                        updateSdSetting(
                                          "sdCfgScale",
                                          raw === "" ? null : Number(raw),
                                        );
                                      }}
                                      placeholder="6.5"
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
                                          Default Size
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Used when the request does not override size
                                        </span>
                                      </div>
                                      <span className="rounded-lg bg-surface-el/30 px-2 py-1 font-mono text-[13px] text-accent">
                                        {modelAdvancedDraft.sdSize ?? "1024x1024"}
                                      </span>
                                    </div>
                                    <input
                                      type="text"
                                      value={modelAdvancedDraft.sdSize ?? ""}
                                      onChange={(e) => updateSdSetting("sdSize", e.target.value)}
                                      placeholder="1024x1024"
                                      className={numberInputClassName}
                                    />
                                    <div className="text-[13px] text-fg/30 px-0.5 mt-1">
                                      Format: width x height
                                    </div>
                                  </div>

                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        Sampler
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        Sampler name sent to A1111
                                      </span>
                                    </div>
                                    <input
                                      type="text"
                                      value={modelAdvancedDraft.sdSampler ?? ""}
                                      onChange={(e) => updateSdSetting("sdSampler", e.target.value)}
                                      placeholder="DPM++ 2M Karras"
                                      className={selectInputClassName}
                                    />
                                  </div>

                                  <div className="space-y-4">
                                    <div className="flex items-center justify-between">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          Seed
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Leave blank for random generations
                                        </span>
                                      </div>
                                      <span className="rounded-lg bg-surface-el/30 px-2 py-1 font-mono text-[13px] text-accent">
                                        {modelAdvancedDraft.sdSeed ?? "Random"}
                                      </span>
                                    </div>
                                    <input
                                      type="number"
                                      inputMode="numeric"
                                      min={ADVANCED_SD_SEED_RANGE.min}
                                      max={ADVANCED_SD_SEED_RANGE.max}
                                      step={1}
                                      value={modelAdvancedDraft.sdSeed ?? ""}
                                      onChange={(e) => {
                                        const raw = e.target.value;
                                        const next = raw === "" ? null : Number(raw);
                                        updateSdSetting(
                                          "sdSeed",
                                          next === null || !Number.isFinite(next)
                                            ? null
                                            : Math.trunc(next),
                                        );
                                      }}
                                      placeholder="Random"
                                      className={numberInputClassName}
                                    />
                                    <div className="flex justify-between text-[13px] text-fg/30 px-0.5 mt-1">
                                      <span>Random</span>
                                      <span>{ADVANCED_SD_SEED_RANGE.max.toLocaleString()}</span>
                                    </div>
                                  </div>

                                  <div className="space-y-4">
                                    <div className="flex items-center justify-between">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          Img2img Denoise
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Edit strength for reference-based generations
                                        </span>
                                      </div>
                                      <span className="rounded-lg bg-surface-el/30 px-2 py-1 font-mono text-[13px] text-accent">
                                        {modelAdvancedDraft.sdDenoisingStrength?.toFixed(2) ??
                                          "0.75"}
                                      </span>
                                    </div>
                                    <input
                                      type="number"
                                      inputMode="decimal"
                                      min={ADVANCED_SD_DENOISING_STRENGTH_RANGE.min}
                                      max={ADVANCED_SD_DENOISING_STRENGTH_RANGE.max}
                                      step={0.01}
                                      value={modelAdvancedDraft.sdDenoisingStrength ?? ""}
                                      onChange={(e) => {
                                        const raw = e.target.value;
                                        updateSdSetting(
                                          "sdDenoisingStrength",
                                          raw === "" ? null : Number(raw),
                                        );
                                      }}
                                      placeholder="0.75"
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
                                      Negative Prompt
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      Applied to every AUTOMATIC1111 request for this model
                                    </span>
                                  </div>
                                  <textarea
                                    value={modelAdvancedDraft.sdNegativePrompt ?? ""}
                                    onChange={(e) =>
                                      updateSdSetting("sdNegativePrompt", e.target.value)
                                    }
                                    placeholder="blurry, low quality, bad anatomy, extra fingers"
                                    rows={4}
                                    className={textAreaInputClassName}
                                  />
                                </div>
                              </div>
                            ) : (
                              <div className="grid grid-cols-1 gap-x-6 gap-y-8 md:grid-cols-2 xl:grid-cols-3 xl:gap-x-8">
                                {/* Temperature */}
                                <div className="space-y-4">
                                  <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-2">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          Temperature
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Higher = more creative
                                        </span>
                                      </div>
                                      <button
                                        type="button"
                                        onClick={() => openDocs("models", "temperature")}
                                        className="text-fg/30 hover:text-fg/60 transition"
                                        aria-label="Help with temperature"
                                      >
                                        <HelpCircle size={12} />
                                      </button>
                                    </div>
                                    <span className="rounded-lg bg-surface-el/30 px-2 py-1 font-mono text-[13px] text-accent">
                                      {modelAdvancedDraft.temperature?.toFixed(2) ?? "0.70"}
                                    </span>
                                  </div>
                                  <input
                                    type="number"
                                    inputMode="decimal"
                                    min={ADVANCED_TEMPERATURE_RANGE.min}
                                    max={ADVANCED_TEMPERATURE_RANGE.max}
                                    step={0.01}
                                    value={modelAdvancedDraft.temperature ?? ""}
                                    onChange={(e) => {
                                      const raw = e.target.value;
                                      handleTemperatureChange(raw === "" ? null : Number(raw));
                                    }}
                                    placeholder="0.70"
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
                                          Top P
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Lower = more focused
                                        </span>
                                      </div>
                                      <button
                                        type="button"
                                        onClick={() => openDocs("models", "top-p")}
                                        className="text-fg/30 hover:text-fg/60 transition"
                                        aria-label="Help with top p"
                                      >
                                        <HelpCircle size={12} />
                                      </button>
                                    </div>
                                    <span className="rounded-lg bg-surface-el/30 px-2 py-1 font-mono text-[13px] text-accent">
                                      {modelAdvancedDraft.topP?.toFixed(2) ?? "1.00"}
                                    </span>
                                  </div>
                                  <input
                                    type="number"
                                    inputMode="decimal"
                                    min={ADVANCED_TOP_P_RANGE.min}
                                    max={ADVANCED_TOP_P_RANGE.max}
                                    step={0.01}
                                    value={modelAdvancedDraft.topP ?? ""}
                                    onChange={(e) => {
                                      const raw = e.target.value;
                                      handleTopPChange(raw === "" ? null : Number(raw));
                                    }}
                                    placeholder="1.00"
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
                                          Max Output Tokens
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Limit response length
                                        </span>
                                      </div>
                                      <button
                                        type="button"
                                        onClick={() => openDocs("models", "max-output-tokens")}
                                        className="text-fg/30 hover:text-fg/60 transition"
                                        aria-label="Help with max output tokens"
                                      >
                                        <HelpCircle size={12} />
                                      </button>
                                    </div>
                                    <span className="rounded-lg bg-surface-el/30 px-2 py-1 font-mono text-[13px] text-accent">
                                      {modelAdvancedDraft.maxOutputTokens
                                        ? modelAdvancedDraft.maxOutputTokens.toLocaleString()
                                        : "Auto"}
                                    </span>
                                  </div>
                                  <input
                                    type="number"
                                    inputMode="numeric"
                                    min={ADVANCED_MAX_TOKENS_RANGE.min}
                                    max={ADVANCED_MAX_TOKENS_RANGE.max}
                                    step={1}
                                    value={modelAdvancedDraft.maxOutputTokens || ""}
                                    onChange={(e) => {
                                      const raw = e.target.value;
                                      const next = raw === "" ? null : Number(raw);
                                      handleMaxTokensChange(
                                        next === null || !Number.isFinite(next) || next === 0
                                          ? null
                                          : Math.trunc(next),
                                      );
                                    }}
                                    placeholder="Auto"
                                    className={numberInputClassName}
                                  />
                                  <div className="flex justify-between text-[13px] text-fg/30 px-0.5 mt-1">
                                    <span>Auto</span>
                                    <span>{ADVANCED_MAX_TOKENS_RANGE.max.toLocaleString()}</span>
                                  </div>
                                </div>

                                {/* Top K */}
                                <div className="space-y-4">
                                  <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-2">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          Top K
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Sample from top K tokens
                                        </span>
                                      </div>
                                      <button
                                        type="button"
                                        onClick={() => openDocs("models", "top-k-if-supported")}
                                        className="text-fg/30 hover:text-fg/60 transition"
                                        aria-label="Help with top k"
                                      >
                                        <HelpCircle size={12} />
                                      </button>
                                    </div>
                                    <span className="rounded-lg bg-surface-el/30 px-2 py-1 font-mono text-[13px] text-accent">
                                      {modelAdvancedDraft.topK ? modelAdvancedDraft.topK : "Auto"}
                                    </span>
                                  </div>
                                  <input
                                    type="number"
                                    inputMode="numeric"
                                    min={ADVANCED_TOP_K_RANGE.min}
                                    max={ADVANCED_TOP_K_RANGE.max}
                                    step={1}
                                    value={modelAdvancedDraft.topK || ""}
                                    onChange={(e) => {
                                      const raw = e.target.value;
                                      const next = raw === "" ? null : Number(raw);
                                      handleTopKChange(
                                        next === null || !Number.isFinite(next) || next === 0
                                          ? null
                                          : Math.trunc(next),
                                      );
                                    }}
                                    placeholder="Auto"
                                    className={numberInputClassName}
                                  />
                                  <div className="flex justify-between text-[13px] text-fg/30 px-0.5 mt-1">
                                    <span>Auto</span>
                                    <span>{ADVANCED_TOP_K_RANGE.max}</span>
                                  </div>
                                </div>

                                {/* Penalties - Frequency */}
                                <div className="space-y-4">
                                  <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-2">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          Frequency Penalty
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Reduce word repetition
                                        </span>
                                      </div>
                                      <button
                                        type="button"
                                        onClick={() => openDocs("models", "frequency-penalty")}
                                        className="text-fg/30 hover:text-fg/60 transition"
                                        aria-label="Help with frequency penalty"
                                      >
                                        <HelpCircle size={12} />
                                      </button>
                                    </div>
                                    <span className="rounded-lg bg-surface-el/30 px-2 py-1 font-mono text-[13px] text-accent">
                                      {modelAdvancedDraft.frequencyPenalty?.toFixed(2) ?? "0.00"}
                                    </span>
                                  </div>
                                  <input
                                    type="number"
                                    inputMode="decimal"
                                    min={ADVANCED_FREQUENCY_PENALTY_RANGE.min}
                                    max={ADVANCED_FREQUENCY_PENALTY_RANGE.max}
                                    step={0.01}
                                    value={modelAdvancedDraft.frequencyPenalty ?? ""}
                                    onChange={(e) => {
                                      const raw = e.target.value;
                                      handleFrequencyPenaltyChange(raw === "" ? null : Number(raw));
                                    }}
                                    placeholder="0.00"
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
                                          Presence Penalty
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Encourage new topics
                                        </span>
                                      </div>
                                      <button
                                        type="button"
                                        onClick={() => openDocs("models", "presence-penalty")}
                                        className="text-fg/30 hover:text-fg/60 transition"
                                        aria-label="Help with presence penalty"
                                      >
                                        <HelpCircle size={12} />
                                      </button>
                                    </div>
                                    <span className="rounded-lg bg-surface-el/30 px-2 py-1 font-mono text-[13px] text-accent">
                                      {modelAdvancedDraft.presencePenalty?.toFixed(2) ?? "0.00"}
                                    </span>
                                  </div>
                                  <input
                                    type="number"
                                    inputMode="decimal"
                                    min={ADVANCED_PRESENCE_PENALTY_RANGE.min}
                                    max={ADVANCED_PRESENCE_PENALTY_RANGE.max}
                                    step={0.01}
                                    value={modelAdvancedDraft.presencePenalty ?? ""}
                                    onChange={(e) => {
                                      const raw = e.target.value;
                                      handlePresencePenaltyChange(raw === "" ? null : Number(raw));
                                    }}
                                    placeholder="0.00"
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
                        {activeDetailPanel === "runtime" && isLocalModel && (
                          <div className="space-y-4">
                            <label className="text-[13px] font-bold tracking-wider text-fg/50 uppercase">
                              Local Inference (llama.cpp)
                            </label>

                            <div className="grid grid-cols-1 gap-6 xl:grid-cols-2 xl:items-start">
                              {/* 1. Memory & Context */}
                              <div className="space-y-6 rounded-xl border border-fg/8 bg-surface-el/10 p-4">
                                <div className="flex items-center gap-2 border-l-2 border-accent/30 pl-3">
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-bold text-fg/80 uppercase tracking-tight">
                                      Memory & Context
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      Context window and VRAM optimization
                                    </span>
                                  </div>
                                </div>

                                {/* Context Length */}
                                <div className="space-y-4">
                                  <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-2">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          Context Length
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Override llama.cpp context window
                                        </span>
                                      </div>
                                      <button
                                        type="button"
                                        onClick={() => openDocs("models", "context-length")}
                                        className="text-fg/30 hover:text-fg/60 transition"
                                        aria-label="Help with context length"
                                      >
                                        <HelpCircle size={12} />
                                      </button>
                                    </div>
                                    <span className="rounded-lg bg-surface-el/30 px-2 py-1 font-mono text-[13px] text-accent">
                                      {modelAdvancedDraft.contextLength
                                        ? modelAdvancedDraft.contextLength.toLocaleString()
                                        : "Auto"}
                                    </span>
                                  </div>
                                  <div className="space-y-3">
                                    <input
                                      type="number"
                                      inputMode="numeric"
                                      min={ADVANCED_CONTEXT_LENGTH_RANGE.min}
                                      max={contextLimit}
                                      step={1}
                                      value={modelAdvancedDraft.contextLength || ""}
                                      onChange={(e) => {
                                        const raw = e.target.value;
                                        const next = raw === "" ? null : Number(raw);
                                        handleContextLengthChange(
                                          next === null || !Number.isFinite(next) || next === 0
                                            ? null
                                            : Math.trunc(next),
                                        );
                                      }}
                                      placeholder="Auto"
                                      className={numberInputClassName}
                                    />
                                    <div className="mt-1 flex justify-between px-0.5 text-[13px] text-fg/30">
                                      <span>Auto</span>
                                      <span>{contextLimit.toLocaleString()}</span>
                                    </div>
                                    {llamaContextLoading && (
                                      <p className="text-[13px] text-fg/40">
                                        Calculating memory limits for this model...
                                      </p>
                                    )}
                                    {llamaContextError && (
                                      <p className="text-[13px] text-warning/80">
                                        {llamaContextError}
                                      </p>
                                    )}
                                    {showContextWarning && (
                                      <div className="flex items-start gap-2 rounded-xl border border-warning/30 bg-warning/10 px-3 py-2 text-[13px] text-warning/80">
                                        <AlertTriangle size={14} className="mt-0.5 shrink-0" />
                                        <span>
                                          Are you sure? This may not run on your device. We
                                          recommend {recommendedContextLength?.toLocaleString()}{" "}
                                          tokens.
                                        </span>
                                      </div>
                                    )}
                                    {showContextCritical && (
                                      <div className="flex items-start gap-2 rounded-xl border border-danger/30 bg-danger/10 px-3 py-2 text-[13px] text-danger/80">
                                        <AlertTriangle size={14} className="mt-0.5 shrink-0" />
                                        <span>
                                          This model likely won&apos;t fit in memory on your device.
                                          Try a smaller model or a much shorter context.
                                        </span>
                                      </div>
                                    )}
                                  </div>
                                </div>

                                {/* Runability & System Info */}
                                {editorModel?.name?.trim() && (
                                  <div className="rounded-lg border border-fg/8 bg-fg/4 px-4 py-3 text-[13px]">
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
                                                Runability
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
                                                  Fits in RAM
                                                </span>
                                              )}
                                              {runabilityScore.fitsInVram && (
                                                <span className="rounded-md bg-emerald-400/10 px-1.5 py-0.5 text-emerald-400/80">
                                                  Fits in VRAM
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
                                              <div className="text-fg/38">Max supported</div>
                                              <div className="font-mono text-fg/78">
                                                {llamaContextInfo
                                                  ? llamaContextInfo.maxContextLength.toLocaleString()
                                                  : contextLimit.toLocaleString()}
                                              </div>
                                            </div>
                                            {recommendedContextLength !== null && (
                                              <div>
                                                <div className="text-fg/38">Recommended</div>
                                                <div className="font-mono text-fg/78">
                                                  {recommendedContextLength.toLocaleString()}
                                                </div>
                                              </div>
                                            )}
                                            {availableRamGiB && (
                                              <div>
                                                <div className="text-fg/38">Available RAM</div>
                                                <div className="font-mono text-fg/78">
                                                  {availableRamGiB} GB
                                                </div>
                                              </div>
                                            )}
                                            {availableVramGiB && (
                                              <div>
                                                <div className="text-fg/38">Available VRAM</div>
                                                <div className="font-mono text-fg/78">
                                                  {availableVramGiB} GB
                                                </div>
                                              </div>
                                            )}
                                            {modelSizeGiB && (
                                              <div>
                                                <div className="text-fg/38">Model size</div>
                                                <div className="font-mono text-fg/78">
                                                  {modelSizeGiB} GB
                                                </div>
                                              </div>
                                            )}
                                            <div>
                                              <div className="text-fg/38">Context cache</div>
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
                                              Auto will use the recommended context length.
                                            </p>
                                          )}

                                        {/* Score breakdown bars */}
                                        {runabilityScore && (
                                          <>
                                            <div className="space-y-2">
                                              {(
                                                [
                                                  {
                                                    label: "Memory Fitness",
                                                    value: runabilityScore.memoryScore,
                                                    weight: 0.25,
                                                  },
                                                  {
                                                    label: "GPU Acceleration",
                                                    value: runabilityScore.gpuScore,
                                                    weight: 0.35,
                                                  },
                                                  {
                                                    label: "KV Headroom",
                                                    value: runabilityScore.kvScore,
                                                    weight: 0.15,
                                                  },
                                                  {
                                                    label: "Quant Quality",
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
                                                  Quantization:{" "}
                                                  <span className="font-mono text-fg/65">
                                                    {runabilityScore.quantization}
                                                  </span>
                                                </span>
                                                <span>
                                                  Size:{" "}
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
                                        KV Cache Type
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        Quantize KV cache to save VRAM
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
                                          {option.label}
                                        </option>
                                      ))}
                                    </select>
                                  </div>

                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        Offload KQV
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        {isCpuOnlyLlamaBackend
                                          ? "Disabled on CPU-only backends"
                                          : "KV cache & KQV ops on GPU"}
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
                                        Auto
                                      </option>
                                      <option value="on" className="bg-[#16171d]">
                                        On
                                      </option>
                                      <option value="off" className="bg-[#16171d]">
                                        Off
                                      </option>
                                    </select>
                                  </div>
                                </div>

                                <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        RoPE Base
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        Frequency base override
                                      </span>
                                    </div>
                                    <input
                                      type="number"
                                      inputMode="decimal"
                                      min={ADVANCED_LLAMA_ROPE_FREQ_BASE_RANGE.min}
                                      max={ADVANCED_LLAMA_ROPE_FREQ_BASE_RANGE.max}
                                      step={0.1}
                                      value={modelAdvancedDraft.llamaRopeFreqBase ?? ""}
                                      onChange={(e) => {
                                        const raw = e.target.value;
                                        handleLlamaRopeFreqBaseChange(
                                          raw === "" ? null : Number(raw),
                                        );
                                      }}
                                      placeholder="Auto"
                                      className={numberInputClassName}
                                    />
                                  </div>

                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        RoPE Scale
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        Frequency scale override
                                      </span>
                                    </div>
                                    <input
                                      type="number"
                                      inputMode="decimal"
                                      min={ADVANCED_LLAMA_ROPE_FREQ_SCALE_RANGE.min}
                                      max={ADVANCED_LLAMA_ROPE_FREQ_SCALE_RANGE.max}
                                      step={0.01}
                                      value={modelAdvancedDraft.llamaRopeFreqScale ?? ""}
                                      onChange={(e) => {
                                        const raw = e.target.value;
                                        handleLlamaRopeFreqScaleChange(
                                          raw === "" ? null : Number(raw),
                                        );
                                      }}
                                      placeholder="Auto"
                                      className={numberInputClassName}
                                    />
                                  </div>
                                </div>

                                {/* Performance */}
                                <div className="space-y-6 border-t border-fg/8 pt-6">
                                  <div className="flex items-center gap-2 border-l-2 border-accent/30 pl-3">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-bold text-fg/80 uppercase tracking-tight">
                                        Performance
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        Hardware acceleration and threading
                                      </span>
                                    </div>
                                  </div>

                                  <div className="space-y-3 rounded-xl border border-fg/10 bg-surface-el/10 p-3">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      Quick Presets
                                    </span>
                                    <div className="grid grid-cols-2 gap-2 md:grid-cols-4">
                                      <button
                                        type="button"
                                        onClick={() => applyLlamaPreset("balanced")}
                                        className="rounded-lg border border-fg/10 bg-surface-el/20 px-2.5 py-2 text-[13px] text-fg/80 transition hover:border-fg/20 hover:bg-surface-el/30"
                                      >
                                        Balanced
                                      </button>
                                      <button
                                        type="button"
                                        onClick={() => applyLlamaPreset("throughput")}
                                        className="rounded-lg border border-fg/10 bg-surface-el/20 px-2.5 py-2 text-[13px] text-fg/80 transition hover:border-fg/20 hover:bg-surface-el/30"
                                      >
                                        Throughput
                                      </button>
                                      <button
                                        type="button"
                                        onClick={() => applyLlamaPreset("vram")}
                                        className="rounded-lg border border-fg/10 bg-surface-el/20 px-2.5 py-2 text-[13px] text-fg/80 transition hover:border-fg/20 hover:bg-surface-el/30"
                                      >
                                        VRAM Saver
                                      </button>
                                      <button
                                        type="button"
                                        onClick={() => applyLlamaPreset("cpu_ram")}
                                        className="rounded-lg border border-fg/10 bg-surface-el/20 px-2.5 py-2 text-[13px] text-fg/80 transition hover:border-fg/20 hover:bg-surface-el/30"
                                      >
                                        CPU + RAM
                                      </button>
                                    </div>
                                    {selectedLlamaQuickPreset && (
                                      <div className="flex flex-wrap gap-2 border-t border-fg/8 pt-3">
                                        {LLAMA_QUICK_PRESET_DETAILS[selectedLlamaQuickPreset].map(
                                          (detail) => (
                                            <span
                                              key={detail}
                                              className="rounded-md border border-fg/10 bg-fg/4 px-2.5 py-1 text-[13px] text-fg/62"
                                            >
                                              {detail}
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
                                          GPU Layers
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          {isCpuOnlyLlamaBackend
                                            ? "Disabled on CPU-only backends"
                                            : "Offload layers to GPU (0 = CPU only)"}
                                        </span>
                                        {llamaLayerPlacementSummary ? (
                                          <span className="block text-[12px] text-fg/34">
                                            {llamaLayerPlacementSummary.detail}
                                          </span>
                                        ) : null}
                                      </div>
                                      <span className="rounded-lg bg-surface-el/30 px-2 py-1 font-mono text-[13px] text-accent">
                                        {modelAdvancedDraft.llamaGpuLayers !== null &&
                                        modelAdvancedDraft.llamaGpuLayers !== undefined
                                          ? modelAdvancedDraft.llamaGpuLayers
                                          : "Auto"}
                                      </span>
                                    </div>
                                    <input
                                      type="number"
                                      inputMode="numeric"
                                      min={ADVANCED_LLAMA_GPU_LAYERS_RANGE.min}
                                      max={ADVANCED_LLAMA_GPU_LAYERS_RANGE.max}
                                      step={1}
                                      value={modelAdvancedDraft.llamaGpuLayers ?? ""}
                                      onChange={(e) => {
                                        const raw = e.target.value;
                                        const next = raw === "" ? null : Number(raw);
                                        handleLlamaGpuLayersChange(
                                          next === null || !Number.isFinite(next) || next < 0
                                            ? null
                                            : Math.trunc(next),
                                        );
                                      }}
                                      disabled={isCpuOnlyLlamaBackend}
                                      placeholder="Auto"
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
                                          Threads
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Inference
                                        </span>
                                      </div>
                                      <input
                                        type="number"
                                        inputMode="numeric"
                                        min={ADVANCED_LLAMA_THREADS_RANGE.min}
                                        max={ADVANCED_LLAMA_THREADS_RANGE.max}
                                        step={1}
                                        value={modelAdvancedDraft.llamaThreads ?? ""}
                                        onChange={(e) => {
                                          const raw = e.target.value;
                                          const next = raw === "" ? null : Number(raw);
                                          handleLlamaThreadsChange(
                                            next === null || !Number.isFinite(next) || next <= 0
                                              ? null
                                              : Math.trunc(next),
                                          );
                                        }}
                                        placeholder="Auto"
                                        className={numberInputClassName}
                                      />
                                    </div>

                                    <div className="space-y-4">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          Batch Threads
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Processing
                                        </span>
                                      </div>
                                      <input
                                        type="number"
                                        inputMode="numeric"
                                        min={ADVANCED_LLAMA_THREADS_BATCH_RANGE.min}
                                        max={ADVANCED_LLAMA_THREADS_BATCH_RANGE.max}
                                        step={1}
                                        value={modelAdvancedDraft.llamaThreadsBatch ?? ""}
                                        onChange={(e) => {
                                          const raw = e.target.value;
                                          const next = raw === "" ? null : Number(raw);
                                          handleLlamaThreadsBatchChange(
                                            next === null || !Number.isFinite(next) || next <= 0
                                              ? null
                                              : Math.trunc(next),
                                          );
                                        }}
                                        placeholder="Auto"
                                        className={numberInputClassName}
                                      />
                                    </div>
                                  </div>

                                  <div className="grid grid-cols-2 gap-6">
                                    <div className="space-y-4">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          Batch Size
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Prompt chunk
                                        </span>
                                      </div>
                                      <input
                                        type="number"
                                        inputMode="numeric"
                                        min={ADVANCED_LLAMA_BATCH_SIZE_RANGE.min}
                                        max={ADVANCED_LLAMA_BATCH_SIZE_RANGE.max}
                                        step={1}
                                        value={modelAdvancedDraft.llamaBatchSize ?? ""}
                                        onChange={(e) => {
                                          const raw = e.target.value;
                                          const next = raw === "" ? null : Number(raw);
                                          handleLlamaBatchSizeChange(
                                            next === null || !Number.isFinite(next) || next <= 0
                                              ? null
                                              : Math.trunc(next),
                                          );
                                        }}
                                        placeholder="512"
                                        className={numberInputClassName}
                                      />
                                    </div>

                                    <div className="space-y-4">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          Flash Attention
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Optimization
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
                                          Auto
                                        </option>
                                        <option value="enabled" className="bg-[#16171d]">
                                          Enabled
                                        </option>
                                        <option value="disabled" className="bg-[#16171d]">
                                          Disabled
                                        </option>
                                      </select>
                                    </div>
                                  </div>
                                </div>
                              </div>

                              {/* 3. Sampling & Quality + 4. Prompting & Templates */}
                              <div className="space-y-6 rounded-xl border border-fg/8 bg-surface-el/10 p-4">
                                <div className="flex items-center gap-2 border-l-2 border-accent/30 pl-3">
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-bold text-fg/80 uppercase tracking-tight">
                                      Sampling & Quality
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      Local-only sampler overrides
                                    </span>
                                  </div>
                                </div>

                                <div className="space-y-4">
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      Sampler Profile
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      Tuned local defaults for stability or reasoning
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
                                        {option.label}
                                      </option>
                                    ))}
                                  </select>
                                  <div className="flex flex-wrap gap-2 pt-1">
                                    {LLAMA_SAMPLER_PROFILE_DETAILS[selectedSamplerProfile].map(
                                      (detail) => (
                                        <span
                                          key={detail}
                                          className="rounded-md border border-fg/10 bg-fg/4 px-2.5 py-1 text-[13px] text-fg/62"
                                        >
                                          {detail}
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
                                        Min P
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        Local override
                                      </span>
                                    </div>
                                    <input
                                      type="number"
                                      inputMode="decimal"
                                      min={0}
                                      max={1}
                                      step="0.01"
                                      value={modelAdvancedDraft.llamaMinP ?? ""}
                                      onChange={(e) => {
                                        const raw = e.target.value.trim();
                                        handleLlamaMinPChange(raw === "" ? null : Number(raw));
                                      }}
                                      placeholder="Default"
                                      className={numberInputClassName}
                                    />
                                  </div>

                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        Typical P
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        Local override
                                      </span>
                                    </div>
                                    <input
                                      type="number"
                                      inputMode="decimal"
                                      min={0}
                                      max={1}
                                      step="0.01"
                                      value={modelAdvancedDraft.llamaTypicalP ?? ""}
                                      onChange={(e) => {
                                        const raw = e.target.value.trim();
                                        handleLlamaTypicalPChange(raw === "" ? null : Number(raw));
                                      }}
                                      placeholder="Default"
                                      className={numberInputClassName}
                                    />
                                  </div>
                                </div>

                                <div className="space-y-4">
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      Seed
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      Leave blank for random
                                    </span>
                                  </div>
                                  <input
                                    type="number"
                                    inputMode="numeric"
                                    min={ADVANCED_LLAMA_SEED_RANGE.min}
                                    max={ADVANCED_LLAMA_SEED_RANGE.max}
                                    step={1}
                                    value={modelAdvancedDraft.llamaSeed ?? ""}
                                    onChange={(e) => {
                                      const raw = e.target.value;
                                      const next = raw === "" ? null : Number(raw);
                                      handleLlamaSeedChange(
                                        next === null || !Number.isFinite(next) || next < 0
                                          ? null
                                          : Math.trunc(next),
                                      );
                                    }}
                                    placeholder="Random"
                                    className={numberInputClassName}
                                  />
                                </div>

                                {/* Prompting & Templates */}
                                <div className="space-y-6 border-t border-fg/8 pt-6">
                                  <div className="flex items-center gap-2 border-l-2 border-accent/30 pl-3">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-bold text-fg/80 uppercase tracking-tight">
                                        Prompting & Templates
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        Format controls and fallbacks
                                      </span>
                                    </div>
                                  </div>

                                  <div className="space-y-4">
                                    <div className="flex items-start justify-between gap-3">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          Template Override
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Jinja template or internal name
                                        </span>
                                      </div>
                                      <button
                                        type="button"
                                        onClick={openTemplateOverlay}
                                        className="inline-flex shrink-0 items-center gap-1.5 rounded-md border border-fg/10 bg-fg/5 px-2.5 py-1.5 text-[12px] font-medium text-fg/68 transition hover:border-fg/20 hover:bg-fg/10 hover:text-fg"
                                      >
                                        <Maximize2 className="h-3.5 w-3.5 text-accent/70" />
                                        Edit
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
                                        : "Prefer embedded GGUF template"}
                                    </button>
                                  </div>

                                  <div className="space-y-4">
                                    <div className="flex items-start justify-between gap-3">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          MMProj Path
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Multimodal projector GGUF required for vision models
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
                                      placeholder="/path/to/mmproj.gguf"
                                      className={selectInputClassName}
                                      spellCheck={false}
                                    />
                                  </div>

                                  <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
                                    <div className="space-y-4">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          Template Preset
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Fallback if GGUF has no template
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
                                            {option.label}
                                          </option>
                                        ))}
                                      </select>
                                    </div>

                                    <div className="space-y-4">
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          Raw Completion Fallback
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Only for raw-tuned models
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
                                          Default (disabled)
                                        </option>
                                        <option value="enabled" className="bg-[#16171d]">
                                          Enabled
                                        </option>
                                        <option value="disabled" className="bg-[#16171d]">
                                          Disabled
                                        </option>
                                      </select>
                                    </div>
                                  </div>

                                  <div className="rounded-xl border border-danger/20 bg-danger/6 p-4">
                                    <div className="flex items-start justify-between gap-4">
                                      <div className="min-w-0 space-y-1.5">
                                        <div className="flex items-start gap-3">
                                          <div className="mt-0.5 shrink-0 text-danger/80">
                                            <AlertTriangle className="h-4 w-4" />
                                          </div>
                                          <div className="min-w-0 space-y-1">
                                            <span className="block text-[13px] font-medium text-fg/82">
                                              Strict Mode
                                            </span>
                                            <span className="block text-[13px] leading-relaxed text-fg/48">
                                              Don&apos;t use this if you don&apos;t know what you
                                              are doing. This bypasses llama.cpp safety fallbacks
                                              that lower GPU layers, clamp context or batch, or
                                              switch a failed GPU load to CPU.
                                            </span>
                                          </div>
                                        </div>
                                        <span className="block text-[12px] text-danger/75">
                                          Use only when you want manual layer offload, context, and
                                          batch settings enforced as-is.
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
                                            ? "On"
                                            : "Off"}
                                        </span>
                                        <Switch
                                          id="llama-strict-mode"
                                          checked={modelAdvancedDraft.llamaStrictMode === true}
                                          onChange={(next) =>
                                            handleLlamaStrictModeChange(next ? true : null)
                                          }
                                          aria-label="Toggle llama strict mode"
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
                            <label className="text-[13px] font-bold tracking-wider text-fg/50 uppercase">
                              Local Inference (Ollama)
                            </label>

                            <div className="space-y-6">
                              {/* 1. Memory & Tokens */}
                              <div className="space-y-6 rounded-xl border border-fg/8 bg-surface-el/10 p-4">
                                <div className="flex items-center gap-2 border-l-2 border-accent/30 pl-3">
                                  <span className="text-[13px] font-bold text-fg/80 uppercase tracking-tight">
                                    Memory & Tokens
                                  </span>
                                </div>

                                <div className="grid grid-cols-2 gap-6">
                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        Context Length
                                      </span>
                                      <span className="block text-[13px] text-fg/40">Num Ctx</span>
                                    </div>
                                    <input
                                      type="number"
                                      inputMode="numeric"
                                      min={ADVANCED_OLLAMA_NUM_CTX_RANGE.min}
                                      max={ADVANCED_OLLAMA_NUM_CTX_RANGE.max}
                                      step={1}
                                      value={modelAdvancedDraft.ollamaNumCtx ?? ""}
                                      onChange={(e) => {
                                        const raw = e.target.value;
                                        const next = raw === "" ? null : Number(raw);
                                        handleOllamaNumCtxChange(
                                          next === null || !Number.isFinite(next) || next < 0
                                            ? null
                                            : Math.trunc(next),
                                        );
                                      }}
                                      placeholder="Auto"
                                      className={numberInputClassName}
                                    />
                                  </div>

                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        Max Predict
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        Num Predict
                                      </span>
                                    </div>
                                    <input
                                      type="number"
                                      inputMode="numeric"
                                      min={ADVANCED_OLLAMA_NUM_PREDICT_RANGE.min}
                                      max={ADVANCED_OLLAMA_NUM_PREDICT_RANGE.max}
                                      step={1}
                                      value={modelAdvancedDraft.ollamaNumPredict ?? ""}
                                      onChange={(e) => {
                                        const raw = e.target.value;
                                        const next = raw === "" ? null : Number(raw);
                                        handleOllamaNumPredictChange(
                                          next === null || !Number.isFinite(next) || next < 0
                                            ? null
                                            : Math.trunc(next),
                                        );
                                      }}
                                      placeholder="Auto"
                                      className={numberInputClassName}
                                    />
                                  </div>
                                </div>

                                <div className="space-y-4">
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      Num Keep
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      Tokens to keep from prompt
                                    </span>
                                  </div>
                                  <input
                                    type="number"
                                    inputMode="numeric"
                                    min={ADVANCED_OLLAMA_NUM_KEEP_RANGE.min}
                                    max={ADVANCED_OLLAMA_NUM_KEEP_RANGE.max}
                                    step={1}
                                    value={modelAdvancedDraft.ollamaNumKeep ?? ""}
                                    onChange={(e) => {
                                      const raw = e.target.value;
                                      const next = raw === "" ? null : Number(raw);
                                      handleOllamaNumKeepChange(
                                        next === null || !Number.isFinite(next) || next < 0
                                          ? null
                                          : Math.trunc(next),
                                      );
                                    }}
                                    placeholder="Auto"
                                    className={numberInputClassName}
                                  />
                                </div>
                              </div>

                              {/* 2. Performance */}
                              <div className="space-y-6 rounded-xl border border-fg/8 bg-surface-el/10 p-4">
                                <div className="flex items-center gap-2 border-l-2 border-accent/30 pl-3">
                                  <span className="text-[13px] font-bold text-fg/80 uppercase tracking-tight">
                                    Performance
                                  </span>
                                </div>

                                <div className="grid grid-cols-2 gap-6">
                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        Num GPU
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        Layers offload
                                      </span>
                                    </div>
                                    <input
                                      type="number"
                                      inputMode="numeric"
                                      min={ADVANCED_OLLAMA_NUM_GPU_RANGE.min}
                                      max={ADVANCED_OLLAMA_NUM_GPU_RANGE.max}
                                      step={1}
                                      value={modelAdvancedDraft.ollamaNumGpu ?? ""}
                                      onChange={(e) => {
                                        const raw = e.target.value;
                                        const next = raw === "" ? null : Number(raw);
                                        handleOllamaNumGpuChange(
                                          next === null || !Number.isFinite(next) || next < 0
                                            ? null
                                            : Math.trunc(next),
                                        );
                                      }}
                                      placeholder="Auto"
                                      className={numberInputClassName}
                                    />
                                  </div>

                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        Num Thread
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        CPU threads
                                      </span>
                                    </div>
                                    <input
                                      type="number"
                                      inputMode="numeric"
                                      min={ADVANCED_OLLAMA_NUM_THREAD_RANGE.min}
                                      max={ADVANCED_OLLAMA_NUM_THREAD_RANGE.max}
                                      step={1}
                                      value={modelAdvancedDraft.ollamaNumThread ?? ""}
                                      onChange={(e) => {
                                        const raw = e.target.value;
                                        const next = raw === "" ? null : Number(raw);
                                        handleOllamaNumThreadChange(
                                          next === null || !Number.isFinite(next) || next < 1
                                            ? null
                                            : Math.trunc(next),
                                        );
                                      }}
                                      placeholder="Auto"
                                      className={numberInputClassName}
                                    />
                                  </div>
                                </div>

                                <div className="space-y-4">
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      Num Batch
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      Processing batch
                                    </span>
                                  </div>
                                  <input
                                    type="number"
                                    inputMode="numeric"
                                    min={ADVANCED_OLLAMA_NUM_BATCH_RANGE.min}
                                    max={ADVANCED_OLLAMA_NUM_BATCH_RANGE.max}
                                    step={1}
                                    value={modelAdvancedDraft.ollamaNumBatch ?? ""}
                                    onChange={(e) => {
                                      const raw = e.target.value;
                                      const next = raw === "" ? null : Number(raw);
                                      handleOllamaNumBatchChange(
                                        next === null || !Number.isFinite(next) || next < 1
                                          ? null
                                          : Math.trunc(next),
                                      );
                                    }}
                                    placeholder="Auto"
                                    className={numberInputClassName}
                                  />
                                </div>
                              </div>

                              {/* 3. Sampling */}
                              <div className="space-y-6 rounded-xl border border-fg/8 bg-surface-el/10 p-4">
                                <div className="flex items-center gap-2 border-l-2 border-accent/30 pl-3">
                                  <span className="text-[13px] font-bold text-fg/80 uppercase tracking-tight">
                                    Sampling & Penalties
                                  </span>
                                </div>

                                <div className="grid grid-cols-2 gap-6">
                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        TFS Z
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        Tail-free
                                      </span>
                                    </div>
                                    <input
                                      type="number"
                                      inputMode="decimal"
                                      min={ADVANCED_OLLAMA_TFS_Z_RANGE.min}
                                      max={ADVANCED_OLLAMA_TFS_Z_RANGE.max}
                                      step={0.01}
                                      value={modelAdvancedDraft.ollamaTfsZ ?? ""}
                                      onChange={(e) => {
                                        const raw = e.target.value;
                                        handleOllamaTfsZChange(raw === "" ? null : Number(raw));
                                      }}
                                      placeholder="Auto"
                                      className={numberInputClassName}
                                    />
                                  </div>

                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        Repeat Penalty
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        Punish repetition
                                      </span>
                                    </div>
                                    <input
                                      type="number"
                                      inputMode="decimal"
                                      min={ADVANCED_OLLAMA_REPEAT_PENALTY_RANGE.min}
                                      max={ADVANCED_OLLAMA_REPEAT_PENALTY_RANGE.max}
                                      step={0.01}
                                      value={modelAdvancedDraft.ollamaRepeatPenalty ?? ""}
                                      onChange={(e) => {
                                        const raw = e.target.value;
                                        handleOllamaRepeatPenaltyChange(
                                          raw === "" ? null : Number(raw),
                                        );
                                      }}
                                      placeholder="Auto"
                                      className={numberInputClassName}
                                    />
                                  </div>
                                </div>

                                <div className="grid grid-cols-2 gap-6">
                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        Min P
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        Min-p sampling
                                      </span>
                                    </div>
                                    <input
                                      type="number"
                                      inputMode="decimal"
                                      min={ADVANCED_OLLAMA_MIN_P_RANGE.min}
                                      max={ADVANCED_OLLAMA_MIN_P_RANGE.max}
                                      step={0.01}
                                      value={modelAdvancedDraft.ollamaMinP ?? ""}
                                      onChange={(e) => {
                                        const raw = e.target.value;
                                        handleOllamaMinPChange(raw === "" ? null : Number(raw));
                                      }}
                                      placeholder="Auto"
                                      className={numberInputClassName}
                                    />
                                  </div>

                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        Typical P
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        Typical sampling
                                      </span>
                                    </div>
                                    <input
                                      type="number"
                                      inputMode="decimal"
                                      min={ADVANCED_OLLAMA_TYPICAL_P_RANGE.min}
                                      max={ADVANCED_OLLAMA_TYPICAL_P_RANGE.max}
                                      step={0.01}
                                      value={modelAdvancedDraft.ollamaTypicalP ?? ""}
                                      onChange={(e) => {
                                        const raw = e.target.value;
                                        handleOllamaTypicalPChange(raw === "" ? null : Number(raw));
                                      }}
                                      placeholder="Auto"
                                      className={numberInputClassName}
                                    />
                                  </div>
                                </div>

                                <div className="space-y-4">
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      Mirostat
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      0=off, 1, 2
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
                                      Auto
                                    </option>
                                    <option value="0" className="bg-[#16171d]">
                                      0 (Off)
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
                                        Tau
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        Target entropy
                                      </span>
                                    </div>
                                    <input
                                      type="number"
                                      inputMode="decimal"
                                      min={ADVANCED_OLLAMA_MIROSTAT_TAU_RANGE.min}
                                      max={ADVANCED_OLLAMA_MIROSTAT_TAU_RANGE.max}
                                      step={0.1}
                                      value={modelAdvancedDraft.ollamaMirostatTau ?? ""}
                                      onChange={(e) => {
                                        const raw = e.target.value;
                                        handleOllamaMirostatTauChange(
                                          raw === "" ? null : Number(raw),
                                        );
                                      }}
                                      placeholder="Auto"
                                      className={numberInputClassName}
                                    />
                                  </div>

                                  <div className="space-y-4">
                                    <div className="space-y-0.5">
                                      <span className="block text-[13px] font-medium text-fg/70">
                                        Eta
                                      </span>
                                      <span className="block text-[13px] text-fg/40">
                                        Learning rate
                                      </span>
                                    </div>
                                    <input
                                      type="number"
                                      inputMode="decimal"
                                      min={ADVANCED_OLLAMA_MIROSTAT_ETA_RANGE.min}
                                      max={ADVANCED_OLLAMA_MIROSTAT_ETA_RANGE.max}
                                      step={0.01}
                                      value={modelAdvancedDraft.ollamaMirostatEta ?? ""}
                                      onChange={(e) => {
                                        const raw = e.target.value;
                                        handleOllamaMirostatEtaChange(
                                          raw === "" ? null : Number(raw),
                                        );
                                      }}
                                      placeholder="Auto"
                                      className={numberInputClassName}
                                    />
                                  </div>
                                </div>

                                <div className="space-y-4">
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      Seed
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      Random if blank
                                    </span>
                                  </div>
                                  <input
                                    type="number"
                                    inputMode="numeric"
                                    min={ADVANCED_OLLAMA_SEED_RANGE.min}
                                    max={ADVANCED_OLLAMA_SEED_RANGE.max}
                                    step={1}
                                    value={modelAdvancedDraft.ollamaSeed ?? ""}
                                    onChange={(e) => {
                                      const raw = e.target.value;
                                      const next = raw === "" ? null : Number(raw);
                                      handleOllamaSeedChange(
                                        next === null || !Number.isFinite(next) || next < 0
                                          ? null
                                          : Math.trunc(next),
                                      );
                                    }}
                                    placeholder="Random"
                                    className={numberInputClassName}
                                  />
                                </div>
                              </div>

                              {/* 4. Stop Sequences */}
                              <div className="space-y-4 rounded-xl border border-fg/8 bg-surface-el/10 p-4">
                                <div className="flex items-center gap-2 border-l-2 border-accent/30 pl-3">
                                  <span className="text-[13px] font-bold text-fg/80 uppercase tracking-tight">
                                    Stop Sequences
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
                                  placeholder="e.g. \n\n###\nUser:\n"
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
                            <label className="text-[13px] font-bold tracking-wider text-fg/50 uppercase">
                              Reasoning (Thinking)
                            </label>

                            <div className="space-y-6">
                              <div className="flex items-center justify-between">
                                <div className="flex items-center gap-2">
                                  <Brain size={14} className="text-warning" />
                                  <div className="space-y-0.5">
                                    <span className="block text-[13px] font-medium text-fg/70">
                                      Enabled
                                    </span>
                                    <span className="block text-[13px] text-fg/40">
                                      Show thinking process
                                    </span>
                                  </div>
                                  <button
                                    type="button"
                                    onClick={() => openDocs("models", "reasoning-mode")}
                                    className="text-fg/30 hover:text-fg/60 transition"
                                    aria-label="Help with reasoning mode"
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
                                        Reasoning Effort
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
                                            {level || "auto"}
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
                                          Budget Tokens
                                        </span>
                                        <span className="font-mono text-[13px] text-warning">
                                          {modelAdvancedDraft.reasoningBudgetTokens
                                            ? modelAdvancedDraft.reasoningBudgetTokens.toLocaleString()
                                            : "Auto"}
                                        </span>
                                      </div>
                                      <input
                                        type="number"
                                        inputMode="numeric"
                                        min={ADVANCED_REASONING_BUDGET_RANGE.min}
                                        max={ADVANCED_REASONING_BUDGET_RANGE.max}
                                        step={1024}
                                        value={modelAdvancedDraft.reasoningBudgetTokens || ""}
                                        onChange={(e) => {
                                          const raw = e.target.value;
                                          const next = raw === "" ? null : Number(raw);
                                          handleReasoningBudgetChange(
                                            next === null || !Number.isFinite(next) || next === 0
                                              ? null
                                              : Math.trunc(next),
                                          );
                                        }}
                                        placeholder="Auto"
                                        className={numberInputClassName}
                                      />
                                    </div>
                                  )}
                                </div>
                              )}
                            </div>
                          </div>
                        )}

                        {/* Prompt Caching Section */}
                        {activeDetailPanel === "caching" && showCachingSection && (
                          <div className="space-y-4">
                            <div className="flex items-center justify-between">
                              <label className="text-[13px] font-bold tracking-wider text-fg/50 uppercase">
                                Prompt Caching
                              </label>
                              <button
                                type="button"
                                onClick={() => setShowParameterSupport(true)}
                                className="text-fg/40 hover:text-fg/60 transition"
                                title="View Parameter Support"
                              >
                                <Info size={14} />
                              </button>
                            </div>

                            <div className="space-y-6">
                              {hasAutomaticCaching ? (
                                <div className="space-y-4">
                                  <div className="flex items-start justify-between rounded-xl border border-fg/8 bg-surface-el/10 p-4">
                                    <div className="flex items-start gap-3 border-l-2 border-accent/30 pl-3">
                                      <HardDrive size={16} className="mt-0.5 text-accent/80" />
                                      <div className="space-y-1">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          Automatic Provider Caching
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          This provider handles prompt caching automatically on the
                                          upstream API. There is no app-side toggle to send.
                                        </span>
                                      </div>
                                    </div>
                                  </div>

                                  <div className="rounded-lg border border-fg/10 bg-fg/4 px-4 py-3 text-[13px] leading-relaxed text-fg/65">
                                    {editorModel?.providerId === "groq" && (
                                      <>
                                        <strong className="text-fg/80">Groq:</strong> caching is
                                        automatic on supported models only. This app does not force
                                        or tune it per request.
                                      </>
                                    )}
                                    {(editorModel?.providerId === "gemini" ||
                                      editorModel?.providerId === "google") && (
                                      <>
                                        <strong className="text-fg/80">Gemini:</strong> implicit
                                        caching is automatic for supported models. Explicit cached
                                        content resources are not created by this app yet.
                                      </>
                                    )}
                                  </div>
                                </div>
                              ) : (
                                <>
                                  {/* ── Enable toggle ── */}
                                  <div className="flex items-center justify-between rounded-xl border border-fg/8 bg-surface-el/10 p-4">
                                    <div className="flex items-center gap-3 border-l-2 border-accent/30 pl-3">
                                      <HardDrive size={16} className="text-accent/80" />
                                      <div className="space-y-0.5">
                                        <span className="block text-[13px] font-medium text-fg/70">
                                          Enable Context Caching
                                        </span>
                                        <span className="block text-[13px] text-fg/40">
                                          Preserve static system prompts and document context across
                                          interactions.
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
                                      <div className="flex items-center justify-between rounded-xl border border-fg/8 bg-surface-el/10 p-4">
                                        <div className="flex items-center gap-3 border-l-2 border-fg/10 pl-3">
                                          <div className="space-y-0.5">
                                            <span className="block text-[13px] font-medium text-fg/70">
                                              Cache TTL
                                            </span>
                                            <span className="block text-[13px] text-fg/40">
                                              How long cached prefixes remain valid between
                                              requests.
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
                                      <div className="rounded-lg border border-fg/10 bg-fg/4 px-4 py-3 text-[13px] leading-relaxed text-fg/65">
                                        <strong className="text-fg/80">Note on pricing:</strong>{" "}
                                        While caching reduces the cost of repeated input tokens, the
                                        initial write to the cache may incur a slight premium
                                        depending on the selected provider.
                                        {editorModel?.providerId !== "openai" &&
                                          modelAdvancedDraft.promptCachingTtl === "1h" && (
                                            <span className="mt-1.5 block text-fg/50">
                                              Extended 1-hour TTL may not be honoured by all
                                              providers. Falls back to the provider's default cache
                                              lifetime when unsupported.
                                            </span>
                                          )}
                                        {editorModel?.providerId === "openai" &&
                                          selectedPromptCachingTtl === "24h" && (
                                            <span className="mt-1.5 block text-fg/50">
                                              OpenAI uses `in_memory` and `24h` retention policies
                                              rather than a 1-hour TTL.
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
                            <div className="flex items-center justify-between">
                              <label className="text-[13px] font-bold tracking-wider text-fg/50 uppercase">
                                Capabilities
                              </label>
                              <button
                                type="button"
                                onClick={() => openDocs("imagegen", "model-capabilities")}
                                className="text-fg/40 transition hover:text-fg/60"
                                aria-label="Help with capabilities"
                              >
                                <HelpCircle size={14} />
                              </button>
                            </div>

                            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                              <div className="space-y-3">
                                <p className="text-[13px] font-medium text-fg/72">Input</p>
                                {["image", "audio"].map((scope) => (
                                  <button
                                    key={scope}
                                    type="button"
                                    disabled={isAutomatic1111Provider}
                                    onClick={() =>
                                      toggleScope(
                                        "inputScopes",
                                        scope as any,
                                        !editorModel.inputScopes?.includes(scope as any),
                                      )
                                    }
                                    className={cn(
                                      "flex w-full items-center justify-between rounded-lg border px-3 py-2.5 text-[13px] transition",
                                      isAutomatic1111Provider && "cursor-not-allowed opacity-60",
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
                                <p className="text-[13px] font-medium text-fg/72">Output</p>
                                {["image", "audio"].map((scope) => (
                                  <button
                                    key={scope}
                                    type="button"
                                    disabled={isAutomatic1111Provider}
                                    onClick={() =>
                                      toggleScope(
                                        "outputScopes",
                                        scope as any,
                                        !editorModel.outputScopes?.includes(scope as any),
                                      )
                                    }
                                    className={cn(
                                      "flex w-full items-center justify-between rounded-lg border px-3 py-2.5 text-[13px] transition",
                                      isAutomatic1111Provider && "cursor-not-allowed opacity-60",
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
                            {isAutomatic1111Provider && (
                              <p className="text-[12px] leading-relaxed text-fg/45">
                                AUTOMATIC1111 models are fixed to text + image input and image
                                output.
                              </p>
                            )}
                          </div>
                        )}
                      </div>
                    </motion.div>
                  ) : effectiveEditorViewMode === "simple" ? (
                    <div
                      className="rounded-lg border border-dashed border-fg/10 px-4 py-6 text-[13px] text-fg/45"
                      style={{ order: 50 }}
                    >
                      Open a section to adjust its settings. The advanced editor stays available
                      when you need the full form.
                    </div>
                  ) : null}
                </motion.div>
              </AnimatePresence>
            </div>
          </div>
        </motion.div>
      </main>

      {/* PARAMETER SUPPORT MODAL */}
      <BottomMenu
        isOpen={showParameterSupport}
        onClose={() => setShowParameterSupport(false)}
        title="Parameter Support"
      >
        <div className="px-4 pb-8">
          <ProviderParameterSupportInfo providerId={editorModel?.providerId || "openai"} />
        </div>
      </BottomMenu>

      <AnimatePresence>
        {showTemplateOverlay && (
          <motion.div
            className="fixed inset-0 z-50 flex h-full flex-col bg-surface"
            style={{ paddingTop: "env(safe-area-inset-top)" }}
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 20 }}
            transition={{ duration: 0.2 }}
          >
            <div className="flex items-center justify-between border-b border-fg/10 px-4 py-3">
              <div className="text-base font-semibold text-fg">Template Override</div>
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
                  {showEmbeddedTemplateViewer ? "Hide Embedded" : "Show Embedded"}
                </button>
                <button
                  type="button"
                  onClick={cancelTemplateOverlay}
                  className="rounded-full border border-fg/10 px-3 py-1.5 text-xs font-medium text-fg/70 transition hover:bg-fg/10 hover:text-fg"
                >
                  Close
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
                  Save
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
                        Reading embedded template...
                      </div>
                    ) : embeddedTemplateError ? (
                      <div className="space-y-1 p-3">
                        <div className="text-[12px] font-medium text-danger">
                          Could not read embedded template
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
                              toast.success("Copied to clipboard");
                            }}
                            className="inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-[11px] font-medium text-fg/50 transition hover:bg-fg/8 hover:text-fg/70"
                          >
                            <Copy className="h-3 w-3" />
                            Copy
                          </button>
                          <button
                            type="button"
                            onClick={handleUseEmbeddedTemplate}
                            disabled={!embeddedTemplateText.trim()}
                            className="inline-flex items-center gap-1.5 rounded-md bg-accent/12 px-2.5 py-1 text-[11px] font-medium text-accent transition hover:bg-accent/20 disabled:cursor-not-allowed disabled:opacity-40"
                          >
                            <CopyCheck className="h-3 w-3" />
                            Paste into Editor
                          </button>
                        </div>
                      </>
                    )}
                  </div>
                )}

                <div className="space-y-2">
                  <div className="text-sm font-medium text-fg/80">Jinja Template</div>
                  <textarea
                    value={templateOverlayDraft}
                    onChange={(e) => setTemplateOverlayDraft(e.target.value)}
                    className="min-h-[50vh] w-full resize-none rounded-2xl border border-fg/10 bg-surface-el/40 px-4 py-4 font-mono text-[12px] leading-relaxed text-fg placeholder-fg/40 transition focus:border-fg/20 focus:outline-none"
                    placeholder="Enter a Jinja chat template or an internal template name..."
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
                ? "Continue Setup"
                : "Save a model to continue"}
              <ArrowRight size={16} />
            </button>
          </div>
        );
      })()}
    </div>
  );
}
