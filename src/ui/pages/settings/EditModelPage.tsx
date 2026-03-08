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
import {
  Info,
  Brain,
  RefreshCw,
  Check,
  Search,
  ChevronDown,
  ChevronRight,
  HelpCircle,
  AlertTriangle,
  FolderOpen,
  Loader,
  HardDrive,
  ArrowRight,
} from "lucide-react";
import { ProviderParameterSupportInfo } from "../../components/ProviderParameterSupportInfo";
import { useModelEditorController } from "./hooks/useModelEditorController";
import { Routes, useNavigationManager } from "../../navigation";
import { addOrUpdateModel } from "../../../core/storage/repo";
import type { ReasoningSupport } from "../../../core/storage/schemas";
import { getProviderReasoningSupport } from "../../../core/storage/schemas";
import { getProviderIcon } from "../../../core/utils/providerIcons";
import { cn } from "../../design-tokens";
import { openDocs } from "../../../core/utils/docs";
import { useI18n } from "../../../core/i18n/context";

type DownloadedGgufModel = {
  modelId: string;
  filename: string;
  path: string;
  size: number;
  quantization: string;
};

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
};

type EditorViewMode = "simple" | "advanced";
type EditorSectionKey = "generation" | "runtime" | "reasoning";
type SimpleEditorSectionKey = EditorSectionKey | "capabilities";

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

function SummaryField({
  label,
  value,
  mono = false,
}: {
  label: string;
  value: ReactNode;
  mono?: boolean;
}) {
  return (
    <div className="grid grid-cols-[92px_minmax(0,1fr)] gap-3 text-[13px]">
      <dt className="text-fg/45">{label}</dt>
      <dd className={cn("min-w-0 text-fg/82", mono && "font-mono text-[12px]")}>{value}</dd>
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
  const [showParameterSupport, setShowParameterSupport] = useState(false);
  const [isManualInput, setIsManualInput] = useState(false);
  const [showModelSelector, setShowModelSelector] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [debouncedSearchQuery, setDebouncedSearchQuery] = useState("");
  const [showOnlyFreeModels, setShowOnlyFreeModels] = useState(false);
  const [editorViewMode, setEditorViewMode] = useState<EditorViewMode>(() => getStoredEditorViewMode());
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
  const [downloadedModels, setDownloadedModels] = useState<DownloadedGgufModel[]>([]);
  const [loadingDownloaded, setLoadingDownloaded] = useState(false);
  const [ggufModelsDir, setGgufModelsDir] = useState<string | null>(null);
  const [showMovePrompt, setShowMovePrompt] = useState(false);
  const [movePromptSource, setMovePromptSource] = useState<"save" | "browse">("save");
  const [movingModel, setMovingModel] = useState(false);
  const [moveError, setMoveError] = useState<string | null>(null);

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
    canSave,
    updateEditorModel,
    handleDisplayNameChange,
    handleModelNameChange,
    handleProviderSelection,
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
    handleLlamaMinPChange,
    handleLlamaTypicalPChange,
    handleLlamaChatTemplateOverrideChange,
    handleLlamaChatTemplatePresetChange,
    handleLlamaRawCompletionFallbackChange,
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
    handleSave,
    saveModel,
    resetToInitial,
    fetchModels,
  } = useModelEditorController();
  const { backOrReplace } = useNavigationManager();
  const isLocalModel = editorModel?.providerId === "llamacpp";
  const isOllamaModel = editorModel?.providerId === "ollama";

  // Fetch GGUF models directory path on mount
  useEffect(() => {
    invoke<string>("hf_get_gguf_models_dir")
      .then((dir) => setGgufModelsDir(dir))
      .catch(() => setGgufModelsDir(null));
  }, []);

  // Fetch downloaded models when the local picker is opened
  const openLocalModelPicker = async () => {
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

  const handleSelectLocalModel = (model: DownloadedGgufModel) => {
    handleModelNameChange(model.path);
    if (!editorModel?.displayName?.trim()) {
      const cleanName = deriveDisplayNameFromPath(model.filename);
      handleDisplayNameChange(cleanName);
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
      if (isPathOutsideGgufDir(selected)) {
        setMovePromptSource("browse");
        setMoveError(null);
        setShowMovePrompt(true);
      }
    } catch (error) {
      console.error("Failed to browse for local model", error);
    }
  };

  // Check if a path is outside the GGUF models dir
  const isPathOutsideGgufDir = (path: string): boolean => {
    if (!ggufModelsDir || !path.trim()) return false;
    return !path.startsWith(ggufModelsDir);
  };

  // Intercept save for llamacpp models to check if move prompt is needed
  const handleSaveWithMoveCheck = async () => {
    if (!isLocalModel || !editorModel?.name?.trim()) {
      handleSave();
      return;
    }

    const modelPath = editorModel.name.trim();
    if (!isPathOutsideGgufDir(modelPath)) {
      handleSave();
      return;
    }

    // Save without navigating, then show the move prompt
    const success = await saveModel();
    if (success) {
      setMovePromptSource("save");
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

      setShowMovePrompt(false);
      if (movePromptSource === "save") {
        backOrReplace(Routes.settingsModels);
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
    setShowMovePrompt(false);
    if (movePromptSource === "save") {
      backOrReplace(Routes.settingsModels);
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
    if (selectedProviderCredential.providerId === "llamacpp") return false;
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

  // Get reasoning support for the current provider
  const reasoningSupport: ReasoningSupport = editorModel?.providerId
    ? getProviderReasoningSupport(editorModel.providerId)
    : "none";
  const showReasoningSection = reasoningSupport !== "none";
  const isAutoReasoning = reasoningSupport === "auto";
  const showEffortOptions = reasoningSupport === "effort" || reasoningSupport === "dynamic";
  const numberInputClassName =
    "w-full rounded-lg border border-fg/10 bg-surface-el/20 px-4 py-3.5 text-[13px] text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none";
  const selectInputClassName =
    "w-full rounded-lg border border-fg/10 bg-surface-el/20 px-4 py-3.5 text-[13px] text-fg transition focus:border-fg/30 focus:outline-none";
  const textAreaInputClassName =
    "w-full rounded-lg border border-fg/10 bg-surface-el/20 px-4 py-3.5 text-[13px] text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none";
  const contextLimit = llamaContextInfo?.maxContextLength ?? ADVANCED_CONTEXT_LENGTH_RANGE.max;
  const recommendedContextLength = llamaContextInfo?.recommendedContextLength ?? null;
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
  const contextCacheLocationLabel =
    modelAdvancedDraft.llamaOffloadKqv === true
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
  const localModelFilename = editorModel?.name?.split(/[/\\]/).filter(Boolean).pop() || "";
  const summaryModelLabel = isLocalModel
    ? editorModel?.displayName || localModelFilename || "Not selected"
    : selectedFetchedModel?.displayName || editorModel?.name || "Not selected";
  const modelSourceLabel = isLocalModel
    ? "Local file"
    : !modelFetchEnabledForSelectedProvider
      ? "Manual entry"
      : isManualInput || fetchedModels.length === 0
        ? "Manual entry"
        : "Provider catalog";
  const shouldShowMoveReminder =
    isLocalModel && isPathOutsideGgufDir(editorModel.name?.trim() ?? "");
  const hasRuntimePanel = isLocalModel || isOllamaModel;
  const runtimePanelTitle = isLocalModel ? "llama.cpp" : isOllamaModel ? "Ollama" : "Runtime";
  const activeDetailPanel = editorViewMode === "advanced" ? activeAdvancedPanel : activeSimplePanel;
  const toggleSimplePanel = (panel: SimpleEditorSectionKey) => {
    setActiveSimplePanel((current) => (current === panel ? null : panel));
  };
  const generationSummary =
    [
      modelAdvancedDraft.temperature != null ? `Temp ${modelAdvancedDraft.temperature.toFixed(2)}` : null,
      modelAdvancedDraft.topP != null ? `Top P ${modelAdvancedDraft.topP.toFixed(2)}` : null,
      modelAdvancedDraft.maxOutputTokens != null
        ? `Max ${modelAdvancedDraft.maxOutputTokens.toLocaleString()}`
        : null,
    ]
      .filter(Boolean)
      .join(" • ") || "Default sampling and output limits";
  const runtimeSummary = isLocalModel
    ? [
        modelAdvancedDraft.llamaBatchSize != null ? `Batch ${modelAdvancedDraft.llamaBatchSize}` : null,
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
          modelAdvancedDraft.reasoningEffort ? `Effort ${modelAdvancedDraft.reasoningEffort}` : null,
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
      handleLlamaOffloadKqvChange(true);
      handleLlamaFlashAttentionChange("auto");
      return;
    }
    if (preset === "throughput") {
      handleLlamaBatchSizeChange(1024);
      handleLlamaKvTypeChange("f16");
      handleLlamaOffloadKqvChange(true);
      handleLlamaFlashAttentionChange("enabled");
      return;
    }
    if (preset === "vram") {
      handleLlamaBatchSizeChange(512);
      handleLlamaKvTypeChange("q4_k");
      handleLlamaOffloadKqvChange(true);
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
    globalWindow.__saveModel = handleSaveWithMoveCheck;
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
    window.localStorage.setItem(MODEL_EDITOR_VIEW_MODE_STORAGE_KEY, editorViewMode);
  }, [editorViewMode]);

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
  ]);

  const scopeOrder = ["text", "image", "audio"] as const;
  const toggleScope = (
    key: "inputScopes" | "outputScopes",
    scope: "image" | "audio",
    enabled: boolean,
  ) => {
    if (!editorModel) return;
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

          <div
            className="relative xl:pr-[384px]"
          >
            <div
              className={cn(
                "w-full space-y-6 transition-transform duration-200 ease-in-out xl:max-w-[760px]",
                editorViewMode === "advanced"
                  ? "xl:translate-x-0"
                  : "xl:translate-x-[192px]",
              )}
            >
              <EditorPanel
                title="Model setup"
                description="Choose the platform, give this entry a readable name, and connect it to the model identifier or file you want to use."
              >
                <div className="space-y-6">
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
                          <button
                            type="button"
                            onClick={() => void handleBrowseLocalModel()}
                            className="rounded-md border border-fg/10 px-2.5 py-1 text-[13px] text-fg/65 transition hover:border-fg/20 hover:bg-fg/5 hover:text-fg/90"
                          >
                            Browse files
                          </button>
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
                          <button
                            type="button"
                            onClick={openLocalModelPicker}
                            className="flex w-full items-center gap-3 rounded-lg border border-fg/10 bg-fg/5 px-4 py-3 text-left transition hover:bg-fg/10"
                          >
                            <FolderOpen className="h-4 w-4 shrink-0 text-accent/70" />
                            <div className="min-w-0 flex-1">
                              <span className="block text-[13px] font-medium text-fg/70">
                                {t("hfBrowser.selectFromLibrary")}
                              </span>
                              <span className="block text-[13px] text-fg/40">
                                {t("hfBrowser.browseOnHuggingFace")}
                              </span>
                            </div>
                            <ChevronRight className="h-4 w-4 shrink-0 text-fg/20" />
                          </button>

                          <BottomMenu
                            isOpen={showLocalModelPicker}
                            onClose={() => setShowLocalModelPicker(false)}
                            title={t("hfBrowser.libraryTitle")}
                          >
                            <MenuSection>
                              {loadingDownloaded ? (
                                <div className="flex items-center justify-center gap-2 py-12 text-fg/50">
                                  <Loader size={18} className="animate-spin" />
                                  <span className="text-[13px]">{t("hfBrowser.searching")}</span>
                                </div>
                              ) : downloadedModels.length === 0 ? (
                                <div className="flex flex-col items-center gap-2 py-16 text-center">
                                  <HardDrive size={32} className="text-fg/20" />
                                  <p className="text-[13px] font-medium text-fg/60">
                                    {t("hfBrowser.libraryEmpty")}
                                  </p>
                                  <p className="px-6 text-[13px] text-fg/40">
                                    {t("hfBrowser.libraryEmptyHint")}
                                  </p>
                                </div>
                              ) : (
                                downloadedModels.map((model) => (
                                  <MenuButton
                                    key={model.path}
                                    icon={<HardDrive className="h-5 w-5 text-accent/60" />}
                                    title={model.filename.replace(/\.gguf$/i, "")}
                                    description={`${model.quantization} · ${formatBytes(model.size)}`}
                                    color="from-accent/20 to-accent/10"
                                    rightElement={
                                      editorModel.name === model.path ? (
                                        <Check className="h-4 w-4 text-accent" />
                                      ) : (
                                        <ArrowRight className="h-4 w-4 text-fg/20" />
                                      )
                                    }
                                    onClick={() => handleSelectLocalModel(model)}
                                  />
                                ))
                              )}
                            </MenuSection>
                          </BottomMenu>
                        </div>
                      </FieldBlock>
                    </div>
                  ) : (
                    <div className="grid items-start grid-cols-1 gap-x-6 gap-y-2 lg:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
                      <label className="text-[13px] font-medium text-fg/72">Display name</label>
                      <div className="flex items-center justify-between gap-3">
                        <label className="text-[13px] font-medium text-fg/72">{modelIdLabel}</label>
                        <div className="flex items-center gap-2">
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
                      </div>

                      <input
                        type="text"
                        value={editorModel.displayName}
                        onChange={(e) => handleDisplayNameChange(e.target.value)}
                        placeholder="e.g. My Favorite ChatGPT"
                        className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-4 py-3 text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none"
                      />

                      <div className="space-y-3">
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

                            <BottomMenu
                              isOpen={showModelSelector}
                              onClose={() => setShowModelSelector(false)}
                              title="Select Model"
                              rightAction={
                                isOpenRouterProvider ? (
                                  <label className="flex items-center gap-2">
                                    <span className="text-[13px] text-fg/70 whitespace-nowrap">
                                      only free models
                                    </span>
                                    <span className="relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full transition-colors duration-200">
                                      <input
                                        type="checkbox"
                                        checked={showOnlyFreeModels}
                                        onChange={(e) => setShowOnlyFreeModels(e.target.checked)}
                                        className="sr-only"
                                      />
                                      <span
                                        className={cn(
                                          "inline-block h-full w-full rounded-full transition-colors duration-200",
                                          showOnlyFreeModels ? "bg-accent" : "bg-fg/10",
                                        )}
                                      />
                                      <span
                                        className={cn(
                                          "absolute h-3.5 w-3.5 transform rounded-full bg-fg transition-transform duration-200",
                                          showOnlyFreeModels ? "translate-x-4.5" : "translate-x-1",
                                        )}
                                      />
                                    </span>
                                  </label>
                                ) : null
                              }
                            >
                              <div className="sticky top-0 z-10 bg-[#0f1014] px-4 pb-2">
                                <div className="relative">
                                  <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-fg/40" />
                                  <input
                                    value={searchQuery}
                                    onChange={(e) => setSearchQuery(e.target.value)}
                                    placeholder="Search models..."
                                    className="w-full rounded-xl border border-fg/10 bg-fg/5 py-2.5 pl-9 pr-4 text-[13px] text-fg placeholder-fg/40 focus:border-fg/20 focus:outline-none"
                                    autoFocus
                                  />
                                </div>
                              </div>
                              <MenuSection>
                                {filteredModels.length > 0 ? (
                                  filteredModels.map((m) => {
                                    const isSelected = m.id === editorModel.name;
                                    return (
                                      <MenuButton
                                        key={m.id}
                                        icon={getProviderIcon(editorModel.providerId)}
                                        title={m.displayName || m.id}
                                        description={m.description || m.id}
                                        color="from-accent to-accent/80"
                                        rightElement={
                                          isSelected ? (
                                            <Check className="h-4 w-4 text-accent" />
                                          ) : undefined
                                        }
                                        onClick={() => handleSelectModel(m.id, m.displayName)}
                                      />
                                    );
                                  })
                                ) : (
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
                              </MenuSection>
                            </BottomMenu>
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
                      </div>
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
                  key={editorViewMode}
                  initial={{ opacity: 0, y: 6 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -6 }}
                  transition={{
                    duration: EDITOR_FADE_DURATION,
                    ease: "easeInOut",
                  }}
                  className={cn(
                    "space-y-4",
                    editorViewMode === "simple" && "flex flex-col gap-2 space-y-0",
                  )}
                >
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

                  {editorViewMode === "advanced" ? (
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
                        Temperature, sampling, penalties, and output limits.
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
                    </div>
                  ) : (
                    <>
                      <div style={{ order: 10 }}>
                        <CollapsedEditorSectionButton
                          title="Generation Parameters"
                          description="Temperature, sampling, penalties, and output limits."
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
                      key={`${editorViewMode}-${activeDetailPanel}`}
                      initial={{ opacity: 0, y: 6 }}
                      animate={{ opacity: 1, y: 0 }}
                      className="pt-1"
                      style={editorViewMode === "simple" ? { order: simpleDetailOrder } : undefined}
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
                      </div>
                    )}

                    {/* Local llama.cpp Settings */}
                    {activeDetailPanel === "runtime" && isLocalModel && (
                      <div className="space-y-4">
                        <label className="text-[13px] font-bold tracking-wider text-fg/50 uppercase">
                          Local Inference (llama.cpp)
                        </label>

                        <div className="space-y-6">
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
                              <div className="grid grid-cols-1 gap-4 xl:grid-cols-[minmax(0,420px)_minmax(260px,1fr)] xl:items-start">
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
                                        Are you sure? This may not run on your device. We recommend{" "}
                                        {recommendedContextLength?.toLocaleString()} tokens.
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

                                {(llamaContextInfo ||
                                  availableRamGiB ||
                                  availableVramGiB ||
                                  modelSizeGiB) && (
                                  <div className="rounded-lg border border-fg/8 bg-fg/4 px-4 py-3 text-[13px] leading-5 text-fg/52">
                                    <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
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
                                    {!selectedContextLength &&
                                      recommendedContextLength &&
                                      recommendedContextLength > 0 && (
                                        <p className="mt-3 border-t border-fg/8 pt-3">
                                          Auto will use the recommended context length.
                                        </p>
                                      )}
                                  </div>
                                )}
                              </div>
                            </div>

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
                                    KV cache &amp; KQV ops on GPU
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
                                  className={selectInputClassName}
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
                                    handleLlamaRopeFreqBaseChange(raw === "" ? null : Number(raw));
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
                                    handleLlamaRopeFreqScaleChange(raw === "" ? null : Number(raw));
                                  }}
                                  placeholder="Auto"
                                  className={numberInputClassName}
                                />
                              </div>
                            </div>
                          </div>

                          {/* 2. Performance */}
                          <div className="space-y-6 rounded-xl border border-fg/8 bg-surface-el/10 p-4">
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
                                    Offload layers to GPU (0 = CPU only)
                                  </span>
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
                                placeholder="Auto"
                                className={numberInputClassName}
                              />
                            </div>

                            <div className="grid grid-cols-2 gap-6">
                              <div className="space-y-4">
                                <div className="space-y-0.5">
                                  <span className="block text-[13px] font-medium text-fg/70">
                                    Threads
                                  </span>
                                  <span className="block text-[13px] text-fg/40">Inference</span>
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
                                  <span className="block text-[13px] text-fg/40">Processing</span>
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
                                  <span className="block text-[13px] text-fg/40">Prompt chunk</span>
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
                                  <span className="block text-[13px] text-fg/40">Optimization</span>
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

                          {/* 3. Sampling & Quality */}
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
                          </div>

                          {/* 4. Chat Templates */}
                          <div className="space-y-6 rounded-xl border border-fg/8 bg-surface-el/10 p-4">
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
                              <div className="space-y-0.5">
                                <span className="block text-[13px] font-medium text-fg/70">
                                  Template Override
                                </span>
                                <span className="block text-[13px] text-fg/40">
                                  Jinja template or internal name
                                </span>
                              </div>
                              <textarea
                                value={modelAdvancedDraft.llamaChatTemplateOverride ?? ""}
                                onChange={(e) =>
                                  handleLlamaChatTemplateOverrideChange(
                                    e.target.value === "" ? null : e.target.value,
                                  )
                                }
                                rows={2}
                                placeholder="Prefer embedded GGUF template"
                                className={selectInputClassName}
                              />
                            </div>

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
                                    : modelAdvancedDraft.llamaRawCompletionFallback === false
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
                                  <span className="block text-[13px] text-fg/40">Num Predict</span>
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
                                  <span className="block text-[13px] text-fg/40">CPU threads</span>
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
                                  <span className="block text-[13px] text-fg/40">Tail-free</span>
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
                                <span className="block text-[13px] text-fg/40">0=off, 1, 2</span>
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
                                  handleOllamaMirostatChange(val === "auto" ? null : Number(val));
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
                                    handleOllamaMirostatTauChange(raw === "" ? null : Number(raw));
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
                                    handleOllamaMirostatEtaChange(raw === "" ? null : Number(raw));
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
                              <label className="relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full transition-colors duration-200">
                                <input
                                  type="checkbox"
                                  checked={modelAdvancedDraft.reasoningEnabled || false}
                                  onChange={(e) => handleReasoningEnabledChange(e.target.checked)}
                                  className="sr-only"
                                />
                                <span
                                  className={cn(
                                    "inline-block h-full w-full rounded-full transition-colors duration-200",
                                    modelAdvancedDraft.reasoningEnabled ? "bg-warning" : "bg-fg/10",
                                  )}
                                />
                                <span
                                  className={cn(
                                    "absolute h-3.5 w-3.5 transform rounded-full bg-fg transition-transform duration-200",
                                    modelAdvancedDraft.reasoningEnabled
                                      ? "translate-x-4.5"
                                      : "translate-x-1",
                                  )}
                                />
                              </label>
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
                                onClick={() =>
                                  toggleScope(
                                    "inputScopes",
                                    scope as any,
                                    !editorModel.inputScopes?.includes(scope as any),
                                  )
                                }
                                className={cn(
                                  "flex w-full items-center justify-between rounded-lg border px-3 py-2.5 text-[13px] transition",
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
                                onClick={() =>
                                  toggleScope(
                                    "outputScopes",
                                    scope as any,
                                    !editorModel.outputScopes?.includes(scope as any),
                                  )
                                }
                                className={cn(
                                  "flex w-full items-center justify-between rounded-lg border px-3 py-2.5 text-[13px] transition",
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
                      </div>
                    )}
                  </div>
                    </motion.div>
                  ) : editorViewMode === "simple" ? (
                    <div
                      className="rounded-lg border border-dashed border-fg/10 px-4 py-6 text-[13px] text-fg/45"
                      style={{ order: 50 }}
                    >
                      Open a section to adjust its settings. The advanced editor stays available when
                      you need the full form.
                    </div>
                  ) : null}
                </motion.div>
              </AnimatePresence>
            </div>

            <AnimatePresence initial={false}>
              {editorViewMode === "advanced" && (
                <motion.aside
                  key="advanced-sidebar"
                  initial={{ opacity: 0, x: 18 }}
                  animate={{ opacity: 1, x: 0 }}
                  exit={{ opacity: 0, x: 12 }}
                  transition={{
                    duration: EDITOR_FADE_DURATION,
                    ease: "easeInOut",
                  }}
                  className="space-y-6 xl:absolute xl:right-0 xl:top-0 xl:w-[360px]"
                >
                  <EditorPanel
                    title="Current configuration"
                    description="This stays visible on desktop so you can confirm what you are editing without scrolling back through the form."
                  >
                    <dl className="space-y-3.5">
                      <SummaryField label="Platform" value={selectedProviderLabel} />
                      <SummaryField label="Source" value={modelSourceLabel} />
                      <SummaryField label="Model" value={summaryModelLabel} mono={isLocalModel} />
                    </dl>
                    {isLocalModel && editorModel.name && (
                      <div className="mt-4 rounded-lg border border-fg/10 bg-fg/4 px-3 py-2.5">
                        <div className="text-[13px] text-fg/45">Path</div>
                        <div
                          className="mt-1 break-all font-mono text-[12px] leading-5 text-fg/62"
                          title={editorModel.name}
                        >
                          {editorModel.name}
                        </div>
                      </div>
                    )}
                  </EditorPanel>

                  <EditorPanel
                    title="Capabilities"
                    description="Mark which modalities this model accepts and what it can produce."
                    action={
                      <button
                        type="button"
                        onClick={() => openDocs("imagegen", "model-capabilities")}
                        className="rounded-md border border-fg/10 p-1.5 text-fg/45 transition hover:border-fg/20 hover:bg-fg/5 hover:text-fg/80"
                        aria-label="Help with capabilities"
                      >
                        <HelpCircle size={14} />
                      </button>
                    }
                  >
                    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-1">
                      <div className="space-y-3">
                        <p className="text-[13px] font-medium text-fg/72">Input</p>
                        {["image", "audio"].map((scope) => (
                          <button
                            key={scope}
                            type="button"
                            onClick={() =>
                              toggleScope(
                                "inputScopes",
                                scope as any,
                                !editorModel.inputScopes?.includes(scope as any),
                              )
                            }
                            className={cn(
                              "flex w-full items-center justify-between rounded-lg border px-3 py-2.5 text-[13px] transition",
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
                            onClick={() =>
                              toggleScope(
                                "outputScopes",
                                scope as any,
                                !editorModel.outputScopes?.includes(scope as any),
                              )
                            }
                            className={cn(
                              "flex w-full items-center justify-between rounded-lg border px-3 py-2.5 text-[13px] transition",
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
                  </EditorPanel>

                  {(shouldShowMoveReminder ||
                    !modelFetchEnabledForSelectedProvider ||
                    isLocalModel) && (
                    <EditorPanel
                      title="Notes"
                      description="A few page-level details that affect how this model behaves after save."
                    >
                      <div className="space-y-3 text-[13px] leading-relaxed text-fg/65">
                        {isLocalModel && (
                          <p>
                            Local llama.cpp models use the file path above. The runtime settings in the
                            advanced section only apply to this provider.
                          </p>
                        )}
                        {!isLocalModel && !modelFetchEnabledForSelectedProvider && (
                          <p>
                            This provider does not expose model discovery here, so the model identifier
                            must be entered manually.
                          </p>
                        )}
                        {shouldShowMoveReminder && (
                          <div className="rounded-lg border border-warning/30 bg-warning/10 px-3 py-2.5 text-warning/85">
                            Saving will ask whether this GGUF file should be moved into your local model
                            library.
                          </div>
                        )}
                      </div>
                    </EditorPanel>
                  )}
                </motion.aside>
              )}
            </AnimatePresence>
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
    </div>
  );
}
