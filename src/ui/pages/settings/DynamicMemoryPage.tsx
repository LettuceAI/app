import { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  Info,
  RefreshCw,
  Trash2,
  ChevronDown,
  Sparkles,
  Users,
  Cpu,
  BookOpen,
  Check,
  Zap,
  Scale,
  Brain,
  Rocket,
  Code2,
} from "lucide-react";
import {
  readSettings,
  saveAdvancedSettings,
  getEmbeddingModelInfo,
} from "../../../core/storage/repo";
import { listPromptTemplates } from "../../../core/prompts/service";
import { storageBridge } from "../../../core/storage/files";
import type {
  DynamicMemorySettings,
  DynamicMemoryStructuredFallbackFormat,
  Model,
  Settings,
  SystemPromptTemplate,
} from "../../../core/storage/schemas";
import {
  APP_DYNAMIC_MEMORY_LOCAL_TEMPLATE_ID,
  APP_DYNAMIC_MEMORY_TEMPLATE_ID,
  APP_DYNAMIC_SUMMARY_TEMPLATE_ID,
} from "../../../core/prompts/constants";
import { cn, interactive } from "../../design-tokens";
import { useNavigate } from "react-router-dom";
import {
  EmbeddingUpgradePrompt,
  isEmbeddingUpgradeDismissed,
  isEmbeddingUpgradePromptForced,
} from "../../components/EmbeddingUpgradePrompt";
import { BottomMenu } from "../../components/BottomMenu";
import { confirmBottomMenu } from "../../components/ConfirmBottomMenu";
import { ModelSelectionBottomMenu } from "../../components/ModelSelectionBottomMenu";
import { getProviderIcon } from "../../../core/utils/providerIcons";
import { useI18n, type TranslationKey } from "../../../core/i18n/context";
import { Switch } from "../../components/Switch";
import { NumberInput } from "../../components/NumberInput";
import { getPlatform } from "../../../core/utils/platform";

const DYNAMIC_MEMORY_LLAMA_OVERWRITE_ORDER = [
  "penalties",
  "grammar",
  "top_k",
  "top_p",
  "temp",
  "dry",
  "min_p",
  "typical",
] as const;

const DYNAMIC_MEMORY_OVERWRITE_SAMPLING = [
  ["dynamicMemory.page.samplerTemperature", "0.4"],
  ["dynamicMemory.page.samplerTopP", "1.0"],
  ["dynamicMemory.page.samplerTopK", "40"],
  ["dynamicMemory.page.samplerMinP", "off"],
  ["dynamicMemory.page.samplerTypicalP", "off"],
  ["dynamicMemory.page.samplerFrequencyPenalty", "0.0"],
  ["dynamicMemory.page.samplerPresencePenalty", "0.0"],
] satisfies ReadonlyArray<[TranslationKey, string]>;

const DYNAMIC_MEMORY_OVERWRITE_DRY = [
  ["dynamicMemory.page.samplerDryMultiplier", "0.8"],
  ["dynamicMemory.page.samplerDryBase", "1.75"],
  ["dynamicMemory.page.samplerDryAllowedLength", "2"],
  ["dynamicMemory.page.samplerDryRange", "full context"],
] satisfies ReadonlyArray<[TranslationKey, string]>;

const DEFAULT_DYNAMIC_MEMORY_SETTINGS: DynamicMemorySettings = {
  enabled: false,
  summaryMessageInterval: 20,
  maxEntries: 50,
  minSimilarityThreshold: 0.35,
  retrievalLimit: 5,
  retrievalStrategy: "smart",
  hotMemoryTokenBudget: 2000,
  decayRate: 0.08,
  coldThreshold: 0.3,
  deleteConfidenceDefault: 0.5,
  maxHardDeleteRatioPerCycle: 0.5,
  contextEnrichmentEnabled: true,
  recursiveMemoryLoops: false,
  recursiveMemoryLoopHardCap: 20,
  runMode: "auto",
};

function ModeOption({
  title,
  description,
  active,
  onClick,
}: {
  title: string;
  description: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "w-full rounded-[10px] border px-4 py-3 text-left transition-colors duration-150",
        active ? "border-accent/40 bg-accent/10" : "border-fg/10 bg-fg/5 hover:bg-fg/8",
      )}
    >
      <div className="flex items-start justify-between gap-3">
        <div>
          <div className="text-sm font-semibold text-fg">{title}</div>
          <p className="mt-1 text-xs leading-5 text-fg/52">{description}</p>
        </div>
        <div
          className={cn(
            "mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full border transition-colors",
            active ? "border-accent/70 bg-accent/15 text-accent" : "border-fg/18 text-transparent",
          )}
        >
          <Check className="h-3.5 w-3.5" />
        </div>
      </div>
    </button>
  );
}

type MemoryPreset = "minimal" | "balanced" | "comprehensive" | "custom";
type SelectableEmbeddingVersion = "v3" | "v4";

const V4_DIMENSION_OPTIONS = [64, 128, 256, 512, 768] as const;

const PRESETS: Record<
  Exclude<MemoryPreset, "custom">,
  Omit<
    DynamicMemorySettings,
    | "enabled"
    | "contextEnrichmentEnabled"
    | "recursiveMemoryLoops"
    | "recursiveMemoryLoopHardCap"
    | "deleteConfidenceDefault"
    | "maxHardDeleteRatioPerCycle"
  >
> = {
  minimal: {
    summaryMessageInterval: 30,
    maxEntries: 100,
    minSimilarityThreshold: 0.5,
    retrievalLimit: 3,
    retrievalStrategy: "smart",
    hotMemoryTokenBudget: 3072,
    decayRate: 0.15,
    coldThreshold: 0.4,
  },
  balanced: {
    summaryMessageInterval: 20,
    maxEntries: 200,
    minSimilarityThreshold: 0.35,
    retrievalLimit: 5,
    retrievalStrategy: "smart",
    hotMemoryTokenBudget: 6144,
    decayRate: 0.08,
    coldThreshold: 0.3,
  },
  comprehensive: {
    summaryMessageInterval: 15,
    maxEntries: 400,
    minSimilarityThreshold: 0.25,
    retrievalLimit: 8,
    retrievalStrategy: "smart",
    hotMemoryTokenBudget: 10240,
    decayRate: 0.05,
    coldThreshold: 0.2,
  },
};

const PRESET_INFO = {
  minimal: {
    icon: Zap,
    title: "Minimal",
    description: "Fast & efficient. Keeps only essential memories.",
    color: "emerald",
  },
  balanced: {
    icon: Scale,
    title: "Balanced",
    description: "Good mix of context retention and performance.",
    color: "blue",
  },
  comprehensive: {
    icon: Brain,
    title: "Comprehensive",
    description: "Maximum context. Best for long, detailed conversations.",
    color: "amber",
  },
};

const hydrateDynamicMemorySettings = (settings?: DynamicMemorySettings): DynamicMemorySettings => ({
  ...DEFAULT_DYNAMIC_MEMORY_SETTINGS,
  ...settings,
  contextEnrichmentEnabled:
    settings?.contextEnrichmentEnabled ?? DEFAULT_DYNAMIC_MEMORY_SETTINGS.contextEnrichmentEnabled,
  recursiveMemoryLoops:
    settings?.recursiveMemoryLoops ?? DEFAULT_DYNAMIC_MEMORY_SETTINGS.recursiveMemoryLoops,
  recursiveMemoryLoopHardCap:
    settings?.recursiveMemoryLoopHardCap ??
    DEFAULT_DYNAMIC_MEMORY_SETTINGS.recursiveMemoryLoopHardCap,
});

const ensureAdvancedSettings = (settings: Settings): NonNullable<Settings["advancedSettings"]> => {
  const advanced = settings.advancedSettings ?? {
    creationHelperEnabled: false,
    helpMeReplyEnabled: true,
    dynamicMemoryStructuredFallbackFormat: "xml",
    dynamicMemoryLlamaSamplerOverwriteEnabled: true,
    dynamicMemory: { ...DEFAULT_DYNAMIC_MEMORY_SETTINGS },
  };
  if (advanced.helpMeReplyEnabled === undefined) {
    advanced.helpMeReplyEnabled = true;
  }
  if (advanced.dynamicMemoryStructuredFallbackFormat === undefined) {
    advanced.dynamicMemoryStructuredFallbackFormat = "xml";
  }
  if (!advanced.dynamicMemory) {
    advanced.dynamicMemory = { ...DEFAULT_DYNAMIC_MEMORY_SETTINGS };
  }
  if (advanced.groupDynamicMemory) {
    advanced.groupDynamicMemory = hydrateDynamicMemorySettings(advanced.groupDynamicMemory);
  }
  if (advanced.dynamicMemoryLlamaSamplerOverwriteEnabled === undefined) {
    advanced.dynamicMemoryLlamaSamplerOverwriteEnabled = true;
  }
  advanced.dynamicMemory = hydrateDynamicMemorySettings(advanced.dynamicMemory);
  settings.advancedSettings = advanced;
  return advanced;
};

const normalizeModelId = (value?: string | null) => (value && value.trim() ? value : null);

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <h4 className="px-1 text-[10px] font-semibold uppercase tracking-[0.22em] text-fg/40">
      {children}
    </h4>
  );
}

function FieldRow({
  label,
  helper,
  children,
}: {
  label: string;
  helper?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-2">
      <p className="px-1 text-xs font-medium text-fg/65">{label}</p>
      {children}
      {helper ? (
        <p className="px-1 text-[11px] leading-relaxed text-fg/45">{helper}</p>
      ) : null}
    </div>
  );
}

function SegmentedRow<T extends string | number>({
  options,
  value,
  onChange,
  formatLabel,
}: {
  options: ReadonlyArray<T>;
  value: T;
  onChange: (next: T) => void;
  formatLabel?: (option: T) => string;
}) {
  return (
    <div className="grid w-full rounded-lg border border-fg/10 bg-fg/5 p-1" style={{ gridTemplateColumns: `repeat(${options.length}, minmax(0, 1fr))` }}>
      {options.map((option) => {
        const active = option === value;
        return (
          <button
            key={String(option)}
            type="button"
            onClick={() => onChange(option)}
            className={cn(
              "rounded-md px-3 py-2 text-sm font-medium transition",
              active
                ? "bg-info text-fg shadow-sm"
                : "text-fg/55 hover:text-fg",
            )}
          >
            {formatLabel ? formatLabel(option) : String(option)}
          </button>
        );
      })}
    </div>
  );
}

function detectPreset(settings: DynamicMemorySettings): MemoryPreset {
  for (const [key, preset] of Object.entries(PRESETS) as [
    Exclude<MemoryPreset, "custom">,
    (typeof PRESETS)["balanced"],
  ][]) {
    if (
      settings.summaryMessageInterval === preset.summaryMessageInterval &&
      settings.maxEntries === preset.maxEntries &&
      settings.minSimilarityThreshold === preset.minSimilarityThreshold &&
      settings.retrievalLimit === preset.retrievalLimit &&
      settings.retrievalStrategy === preset.retrievalStrategy &&
      settings.hotMemoryTokenBudget === preset.hotMemoryTokenBudget &&
      settings.decayRate === preset.decayRate &&
      settings.coldThreshold === preset.coldThreshold
    ) {
      return key;
    }
  }
  return "custom";
}

type TabType = "direct" | "group";

export function DynamicMemoryPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [isLoading, setIsLoading] = useState(true);
  const [activeTab, setActiveTab] = useState<TabType>("direct");

  // Direct chat settings
  const [enabled, setEnabled] = useState(false);
  const [directSettings, setDirectSettings] = useState<DynamicMemorySettings>(
    DEFAULT_DYNAMIC_MEMORY_SETTINGS,
  );
  const [directPreset, setDirectPreset] = useState<MemoryPreset>("balanced");
  const [directAdvancedOpen, setDirectAdvancedOpen] = useState(false);

  // Group chat settings
  const [groupSettings, setGroupSettings] = useState<DynamicMemorySettings>(
    DEFAULT_DYNAMIC_MEMORY_SETTINGS,
  );
  const [groupPreset, setGroupPreset] = useState<MemoryPreset>("balanced");
  const [groupAdvancedOpen, setGroupAdvancedOpen] = useState(false);

  // Shared settings
  const [summarisationModelId, setSummarisationModelId] = useState<string | null>(null);
  const [models, setModels] = useState<Model[]>([]);
  const [templates, setTemplates] = useState<SystemPromptTemplate[]>([]);
  const [selectedSummaryPromptTemplateId, setSelectedSummaryPromptTemplateId] = useState<
    string | null
  >(null);
  const [selectedMemoryManagerPromptTemplateId, setSelectedMemoryManagerPromptTemplateId] =
    useState<string | null>(null);
  const [embeddingMaxTokens, setEmbeddingMaxTokens] = useState<number>(2048);
  const [embeddingDimensions, setEmbeddingDimensions] = useState<number>(768);
  const [embeddingKeepModelLoaded, setEmbeddingKeepModelLoaded] = useState(false);
  const [modelVersion, setModelVersion] = useState<string | null>(null);
  const [modelSourceVersion, setModelSourceVersion] = useState<string | null>(null);
  const [availableEmbeddingVersions, setAvailableEmbeddingVersions] = useState<string[]>([]);
  const [installBundleComplete, setInstallBundleComplete] = useState(false);
  const [selectedEmbeddingVersion, setSelectedEmbeddingVersion] = useState<string | null>(null);
  const [showDownloadModelMenu, setShowDownloadModelMenu] = useState(false);
  const [showUpgradePrompt, setShowUpgradePrompt] = useState(false);
  const [defaultModelId, setDefaultModelId] = useState<string | null>(null);
  const [showModelMenu, setShowModelMenu] = useState(false);
  const [structuredFallbackFormat, setStructuredFallbackFormat] =
    useState<DynamicMemoryStructuredFallbackFormat>("xml");
  const [dynamicMemoryLlamaSamplerOverwriteEnabled, setDynamicMemoryLlamaSamplerOverwriteEnabled] =
    useState(true);

  useEffect(() => {
    const loadData = async () => {
      try {
        const [settings, modelInfo, promptTemplates] = await Promise.all([
          readSettings(),
          getEmbeddingModelInfo(),
          listPromptTemplates(),
        ]);

        const dynamicSettings = hydrateDynamicMemorySettings(
          settings.advancedSettings?.dynamicMemory,
        );
        const groupDynamicSettings = hydrateDynamicMemorySettings(
          settings.advancedSettings?.groupDynamicMemory ?? settings.advancedSettings?.dynamicMemory,
        );

        setEnabled(dynamicSettings.enabled);
        setDirectSettings(dynamicSettings);
        setDirectPreset(detectPreset(dynamicSettings));

        setGroupSettings(groupDynamicSettings);
        setGroupPreset(detectPreset(groupDynamicSettings));

        const defaultModelIdValue = normalizeModelId(settings.defaultModelId);
        const summarisationModelValue = normalizeModelId(
          settings.advancedSettings?.summarisationModelId,
        );
        setDefaultModelId(defaultModelIdValue);
        setSummarisationModelId(
          defaultModelIdValue && summarisationModelValue === defaultModelIdValue
            ? null
            : summarisationModelValue,
        );
        setSelectedSummaryPromptTemplateId(
          settings.advancedSettings?.dynamicMemorySummarizerPromptTemplateId ?? null,
        );
        setSelectedMemoryManagerPromptTemplateId(
          settings.advancedSettings?.dynamicMemoryManagerPromptTemplateId ?? null,
        );
        setTemplates(promptTemplates);
        setStructuredFallbackFormat(
          settings.advancedSettings?.dynamicMemoryStructuredFallbackFormat ?? "xml",
        );
        setDynamicMemoryLlamaSamplerOverwriteEnabled(
          settings.advancedSettings?.dynamicMemoryLlamaSamplerOverwriteEnabled ?? true,
        );
        setEmbeddingMaxTokens(settings.advancedSettings?.embeddingMaxTokens ?? 2048);
        setEmbeddingDimensions(settings.advancedSettings?.embeddingDimensions ?? 768);
        setEmbeddingKeepModelLoaded(settings.advancedSettings?.embeddingKeepModelLoaded ?? false);
        setModels(settings.models);
        setInstallBundleComplete(modelInfo.installBundleComplete ?? modelInfo.installed);

        if (modelInfo.installed) {
          setModelVersion(modelInfo.version);
          const sourceVersion =
            modelInfo.selectedSourceVersion ?? modelInfo.sourceVersion ?? modelInfo.version;
          setModelSourceVersion(sourceVersion);
          setSelectedEmbeddingVersion(sourceVersion);
          const available = modelInfo.availableVersions ?? [];
          setAvailableEmbeddingVersions(available);
          // Show the upgrade-to-v4 banner for v1/v2 installs only. v3 has its
          // own app-level toast (`V3UpgradeToast`) that fires anywhere, so
          // we don't double-prompt v3 users when they visit this page.
          //
          // If v4 is genuinely installed (whether as the active source or
          // alongside the legacy one), the prompt is suppressed regardless of
          // any debug flag. This guards against the force-prompt flag being
          // accidentally left on after testing.
          const v4Installed = available.includes("v4") || sourceVersion === "v4";
          const isLegacy = sourceVersion === "v1" || sourceVersion === "v2";
          const naturallyEligible =
            isLegacy && !v4Installed && !isEmbeddingUpgradeDismissed("v4");
          const forced = isEmbeddingUpgradePromptForced() && !v4Installed;
          if (naturallyEligible || forced) {
            setShowUpgradePrompt(true);
          }
        }

        setIsLoading(false);
      } catch (err) {
        console.error("Failed to load settings:", err);
        setIsLoading(false);
      }
    };

    loadData();
  }, []);

  const updateAdvancedSettings = async (
    updater: (advanced: NonNullable<Settings["advancedSettings"]>) => void,
    errorMessage: string,
  ) => {
    try {
      const settings = await readSettings();
      const advanced = ensureAdvancedSettings(settings);
      updater(advanced);
      await saveAdvancedSettings(advanced);
    } catch (err) {
      console.error(errorMessage, err);
    }
  };

  const handleDirectPresetChange = async (preset: Exclude<MemoryPreset, "custom">) => {
    const presetValues = PRESETS[preset];
    const newSettings: DynamicMemorySettings = { ...directSettings, ...presetValues };
    setDirectSettings(newSettings);
    setDirectPreset(preset);

    await updateAdvancedSettings((advanced) => {
      advanced.dynamicMemory = {
        ...DEFAULT_DYNAMIC_MEMORY_SETTINGS,
        ...(advanced.dynamicMemory ?? {}),
        ...presetValues,
      } as DynamicMemorySettings;
    }, "Failed to save direct memory preset:");
  };

  const handleGroupPresetChange = async (preset: Exclude<MemoryPreset, "custom">) => {
    const presetValues = PRESETS[preset];
    const newSettings: DynamicMemorySettings = { ...groupSettings, ...presetValues };
    setGroupSettings(newSettings);
    setGroupPreset(preset);

    await updateAdvancedSettings((advanced) => {
      advanced.groupDynamicMemory = {
        ...DEFAULT_DYNAMIC_MEMORY_SETTINGS,
        ...(advanced.groupDynamicMemory ?? {}),
        ...presetValues,
      } as DynamicMemorySettings;
    }, "Failed to save group memory preset:");
  };

  const handleDirectSettingChange = async <K extends keyof DynamicMemorySettings>(
    key: K,
    value: DynamicMemorySettings[K],
  ) => {
    const newSettings: DynamicMemorySettings = { ...directSettings, [key]: value };
    setDirectSettings(newSettings);
    setDirectPreset(detectPreset(newSettings));

    await updateAdvancedSettings((advanced) => {
      advanced.dynamicMemory = {
        ...DEFAULT_DYNAMIC_MEMORY_SETTINGS,
        ...(advanced.dynamicMemory ?? {}),
        [key]: value,
      } as DynamicMemorySettings;
    }, `Failed to save direct memory ${key}:`);
  };

  const handleGroupSettingChange = async <K extends keyof DynamicMemorySettings>(
    key: K,
    value: DynamicMemorySettings[K],
  ) => {
    const newSettings: DynamicMemorySettings = { ...groupSettings, [key]: value };
    setGroupSettings(newSettings);
    setGroupPreset(detectPreset(newSettings));

    await updateAdvancedSettings((advanced) => {
      advanced.groupDynamicMemory = {
        ...DEFAULT_DYNAMIC_MEMORY_SETTINGS,
        ...(advanced.groupDynamicMemory ?? {}),
        [key]: value,
      } as DynamicMemorySettings;
    }, `Failed to save group memory ${key}:`);
  };

  const handleSummarisationModelChange = async (modelId: string | null) => {
    setSummarisationModelId(modelId);
    await updateAdvancedSettings((advanced) => {
      if (modelId) {
        advanced.summarisationModelId = modelId;
      } else {
        advanced.summarisationModelId = defaultModelId ?? undefined;
      }
    }, "Failed to save summarisation model:");
  };

  const handleSummaryPromptTemplateChange = async (templateId: string | null) => {
    setSelectedSummaryPromptTemplateId(templateId);
    await updateAdvancedSettings((advanced) => {
      advanced.dynamicMemorySummarizerPromptTemplateId = templateId ?? undefined;
    }, "Failed to save dynamic memory summary prompt:");
  };

  const handleMemoryManagerPromptTemplateChange = async (templateId: string | null) => {
    setSelectedMemoryManagerPromptTemplateId(templateId);
    await updateAdvancedSettings((advanced) => {
      advanced.dynamicMemoryManagerPromptTemplateId = templateId ?? undefined;
    }, "Failed to save dynamic memory manager prompt:");
  };

  const handleDynamicMemoryLlamaSamplerOverwriteChange = async (enabled: boolean) => {
    setDynamicMemoryLlamaSamplerOverwriteEnabled(enabled);
    await updateAdvancedSettings((advanced) => {
      advanced.dynamicMemoryLlamaSamplerOverwriteEnabled = enabled;
    }, "Failed to save dynamic memory llama sampler overwrite setting:");
  };

  const handleStructuredFallbackFormatChange = async (
    format: DynamicMemoryStructuredFallbackFormat,
  ) => {
    setStructuredFallbackFormat(format);
    await updateAdvancedSettings((advanced) => {
      advanced.dynamicMemoryStructuredFallbackFormat = format;
    }, "Failed to save dynamic memory structured fallback format:");
  };

  const handleEmbeddingMaxTokensChange = async (val: number) => {
    setEmbeddingMaxTokens(val);
    await updateAdvancedSettings((advanced) => {
      advanced.embeddingMaxTokens = val;
    }, "Failed to save embedding max tokens:");
  };

  const handleEmbeddingModelVersionChange = async (version: SelectableEmbeddingVersion) => {
    setSelectedEmbeddingVersion(version);
    setModelSourceVersion(version);
    await updateAdvancedSettings((advanced) => {
      advanced.embeddingModelVersion = version;
    }, "Failed to save embedding model version:");
    try {
      await storageBridge.clearEmbeddingRuntimeCache();
      await storageBridge.initializeEmbeddingModel();
    } catch (err) {
      console.error("Failed to reinitialize embedding runtime after version switch:", err);
    }
  };

  const handleEmbeddingDimensionsChange = async (
    dimensions: (typeof V4_DIMENSION_OPTIONS)[number],
  ) => {
    setEmbeddingDimensions(dimensions);
    await updateAdvancedSettings((advanced) => {
      advanced.embeddingDimensions = dimensions;
    }, "Failed to save embedding dimensions:");
    try {
      await storageBridge.clearEmbeddingRuntimeCache();
      await storageBridge.initializeEmbeddingModel();
    } catch (err) {
      console.error("Failed to reinitialize embedding runtime after dimension switch:", err);
    }
  };

  const handleEmbeddingKeepModelLoadedChange = async (enabled: boolean) => {
    setEmbeddingKeepModelLoaded(enabled);
    await updateAdvancedSettings((advanced) => {
      advanced.embeddingKeepModelLoaded = enabled;
    }, "Failed to save keep-model-loaded setting:");
  };

  const navigateToBundleDownload = (version?: SelectableEmbeddingVersion) => {
    const targetVersion =
      version ?? (selectedEmbeddingVersion === "v3" || modelSourceVersion === "v3" ? "v3" : "v4");
    navigate(`/settings/embedding-download?version=${targetVersion}`);
  };

  const handleDeleteSelectedEmbeddingModel = async () => {
    const version =
      selectedEmbeddingVersion === "v3"
        ? "v3"
        : selectedEmbeddingVersion === "v4"
          ? "v4"
          : null;
    if (!version) return;
    const confirmed = await confirmBottomMenu({
      title: t("dynamicMemory.page.deleteEmbeddingTitle", {
        version: version.toUpperCase(),
      }),
      message: t("dynamicMemory.page.deleteEmbeddingMessage", {
        version: version.toUpperCase(),
      }),
      confirmLabel: t("dynamicMemory.page.delete"),
      destructive: true,
    });
    if (!confirmed) return;

    try {
      await storageBridge.deleteEmbeddingModelVersion(version);
      const modelInfo = await getEmbeddingModelInfo();
      const sourceVersion =
        modelInfo.selectedSourceVersion ?? modelInfo.sourceVersion ?? modelInfo.version;
      const available = modelInfo.availableVersions ?? [];
      setModelVersion(modelInfo.version);
      setModelSourceVersion(sourceVersion);
      setAvailableEmbeddingVersions(available);
      setSelectedEmbeddingVersion(sourceVersion);
      setInstallBundleComplete(modelInfo.installBundleComplete ?? modelInfo.installed);
    } catch (err) {
      console.error("Failed to delete model version:", err);
    }
  };

  if (isLoading) {
    return null;
  }

  const isAnyEnabled = enabled;
  const currentSettings = activeTab === "direct" ? directSettings : groupSettings;
  const currentEnabled = activeTab === "direct" ? enabled : true;
  const currentPreset = activeTab === "direct" ? directPreset : groupPreset;
  const advancedOpen = activeTab === "direct" ? directAdvancedOpen : groupAdvancedOpen;
  const setAdvancedOpen = activeTab === "direct" ? setDirectAdvancedOpen : setGroupAdvancedOpen;
  const selectedSummarisationModel = summarisationModelId
    ? models.find((model) => model.id === summarisationModelId)
    : null;
  const effectiveSummarisationModel =
    selectedSummarisationModel ?? models.find((model) => model.id === defaultModelId) ?? null;
  const isLocalLlamaSummaryModel =
    effectiveSummarisationModel?.providerId === "llamacpp"
    && getPlatform().type !== "mobile";
  const hasV3Installed = availableEmbeddingVersions.includes("v3");
  const hasV4Installed = availableEmbeddingVersions.includes("v4");
  const hasBothMajorEmbeddingVersionsInstalled = hasV3Installed && hasV4Installed;
  const effectiveEmbeddingVersion =
    selectedEmbeddingVersion ?? modelSourceVersion ?? modelVersion ?? null;
  const supportsExtendedTokenCapacity =
    effectiveEmbeddingVersion === "v3" || effectiveEmbeddingVersion === "v4";
  const supportsMatryoshkaDimensions = effectiveEmbeddingVersion === "v4";
  const effectiveEmbeddingDimensions = supportsMatryoshkaDimensions ? embeddingDimensions : 512;
  const selectedSummarisationModelLabel =
    selectedSummarisationModel?.displayName || t("dynamicMemory.page.selectedModel");
  const summaryPromptTemplates = templates.filter(
    (template) =>
      template.promptType === "dynamicMemorySummarizer" &&
      template.id !== APP_DYNAMIC_SUMMARY_TEMPLATE_ID,
  );
  const memoryManagerPromptTemplates = templates.filter(
    (template) =>
      template.promptType === "dynamicMemoryManager" &&
      template.id !== APP_DYNAMIC_MEMORY_TEMPLATE_ID &&
      template.id !== APP_DYNAMIC_MEMORY_LOCAL_TEMPLATE_ID,
  );

  return (
    <div className="flex min-h-screen flex-col">
      <main className="flex-1 px-4 pb-24 pt-4">
        <div className="mx-auto w-full max-w-5xl space-y-4">
          {/* Info Card */}
          <div className={cn("rounded-xl border border-info/20 bg-info/5 p-3")}>
            <div className="flex items-start gap-2">
              <Info className="h-4 w-4 text-info shrink-0 mt-0.5" />
              <p className="text-xs text-info/80 leading-relaxed">{t("dynamicMemory.page.info")}</p>
            </div>
          </div>

          <AnimatePresence>
            {showUpgradePrompt && (
              <EmbeddingUpgradePrompt
                onDismiss={() => setShowUpgradePrompt(false)}
                returnTo="/settings/advanced/dynamic-memory"
                currentVersion={
                  modelSourceVersion === "v1"
                    ? "v1"
                    : modelSourceVersion === "v3"
                      ? "v3"
                      : "v2"
                }
              />
            )}
          </AnimatePresence>

          {/* Status Banner */}
          {!enabled && (
            <div className={cn("rounded-xl border border-warning/20 bg-warning/5 p-3")}>
              <div className="flex items-start gap-2">
                <Info className="h-4 w-4 text-warning shrink-0 mt-0.5" />
                <div className="flex-1">
                  <p className="text-xs font-medium text-warning">
                    {t("dynamicMemory.page.disabledDirectTitle")}
                  </p>
                  <p className="text-xs text-warning/60 mt-0.5">
                    {t("dynamicMemory.page.disabledDirectDescription")}
                  </p>
                </div>
              </div>
            </div>
          )}

          {/* Tab Switcher */}
          <div className="flex gap-2 p-1 bg-fg/5 rounded-xl border border-fg/10">
            <button
              onClick={() => setActiveTab("direct")}
              className={cn(
                "flex-1 flex items-center justify-center gap-2 py-2.5 px-4 rounded-lg text-sm font-medium transition-all",
                activeTab === "direct"
                  ? "bg-fg/10 text-fg shadow-sm"
                  : "text-fg/50 hover:text-fg/70",
              )}
            >
              <Sparkles className="h-4 w-4" />
              {t("dynamicMemory.page.directChats")}
              {enabled && <span className="ml-1 w-1.5 h-1.5 rounded-full bg-accent" />}
            </button>
            <button
              onClick={() => setActiveTab("group")}
              className={cn(
                "flex-1 flex items-center justify-center gap-2 py-2.5 px-4 rounded-lg text-sm font-medium transition-all",
                activeTab === "group"
                  ? "bg-fg/10 text-fg shadow-sm"
                  : "text-fg/50 hover:text-fg/70",
              )}
            >
              <Users className="h-4 w-4" />
              {t("dynamicMemory.page.groupChats")}
            </button>
          </div>

          {/* Tab Content */}
          <AnimatePresence mode="wait">
            <motion.div
              key={activeTab}
              initial={{ opacity: 0, x: activeTab === "direct" ? -10 : 10 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: activeTab === "direct" ? 10 : -10 }}
              transition={{ duration: 0.15 }}
              className="space-y-4"
            >
              {/* Enable Toggle (direct chats only) */}
              {activeTab === "direct" && (
                <div className={cn("rounded-xl border border-fg/10 bg-fg/5 px-4 py-3")}>
                  <div className="flex items-center justify-between gap-3">
                    <div className="min-w-0 flex-1">
                      <span className="text-sm font-medium text-fg">
                        {t("dynamicMemory.page.enableDirectChats")}
                      </span>
                    </div>
                    <Switch
                      checked={enabled}
                      onChange={async (next) => {
                        setEnabled(next);
                        await handleDirectSettingChange("enabled", next);
                      }}
                    />
                  </div>
                </div>
              )}

              {activeTab === "group" && (
                <div className={cn("rounded-xl border border-fg/8 bg-fg/3 px-4 py-3")}>
                  <p className="text-xs text-fg/50">{t("dynamicMemory.page.groupChatsInfo")}</p>
                </div>
              )}

              <div className={cn(!currentEnabled && "opacity-50 pointer-events-none", "space-y-4")}>
                <div className="space-y-3">
                  <h3 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35 px-1">
                    {t("dynamicMemory.mode.title")}
                  </h3>
                  <p className="px-1 text-xs leading-5 text-fg/50">
                    {t("dynamicMemory.mode.description")}
                  </p>
                  <div className="grid grid-cols-1 gap-2">
                    {(["auto", "askFirst", "manual"] as const).map((mode) => (
                      <ModeOption
                        key={mode}
                        title={t(`dynamicMemory.mode.${mode}` as TranslationKey)}
                        description={t(`dynamicMemory.mode.${mode}Description` as TranslationKey)}
                        active={(currentSettings.runMode ?? "auto") === mode}
                        onClick={() => {
                          if (activeTab === "direct") {
                            void handleDirectSettingChange("runMode", mode);
                          } else {
                            void handleGroupSettingChange("runMode", mode);
                          }
                        }}
                      />
                    ))}
                  </div>
                </div>

                {/* Presets */}
                <div className="space-y-3">
                  <h3 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35 px-1">
                    {t("dynamicMemory.page.memoryProfile")}
                  </h3>
                  <div className="grid grid-cols-3 gap-2">
                    {(Object.keys(PRESET_INFO) as Exclude<MemoryPreset, "custom">[]).map((key) => {
                      const info = PRESET_INFO[key];
                      const Icon = info.icon;
                      const isSelected = currentPreset === key;
                      const colorClasses = {
                        emerald: isSelected
                          ? "border-accent/50 bg-accent/15 text-accent/90"
                          : "border-fg/10 bg-fg/5 text-fg/60 hover:border-fg/20",
                        blue: isSelected
                          ? "border-info/50 bg-info/15 text-info"
                          : "border-fg/10 bg-fg/5 text-fg/60 hover:border-fg/20",
                        amber: isSelected
                          ? "border-warning/50 bg-warning/15 text-warning/90"
                          : "border-fg/10 bg-fg/5 text-fg/60 hover:border-fg/20",
                      };

                      return (
                        <button
                          key={key}
                          onClick={() => {
                            if (activeTab === "direct") {
                              handleDirectPresetChange(key);
                            } else {
                              handleGroupPresetChange(key);
                            }
                          }}
                          disabled={!currentEnabled}
                          className={cn(
                            "flex flex-col items-center gap-2 rounded-xl border p-3 transition-all",
                            colorClasses[info.color as keyof typeof colorClasses],
                            !currentEnabled && "cursor-not-allowed",
                          )}
                        >
                          <div
                            className={cn(
                              "flex h-10 w-10 items-center justify-center rounded-full border",
                              isSelected
                                ? `border-${info.color}-400/40 bg-${info.color}-500/20`
                                : "border-fg/10 bg-fg/10",
                            )}
                          >
                            <Icon className="h-5 w-5" />
                          </div>
                          <span className="text-xs font-semibold">
                            {t(`dynamicMemory.presets.${key}` as const)}
                          </span>
                        </button>
                      );
                    })}
                  </div>

                  {/* Preset Description */}
                  {currentPreset !== "custom" && (
                    <div className="px-1">
                      <p className="text-[11px] text-fg/45">
                        {t(`dynamicMemory.presetInfo.${currentPreset}` as const)}
                      </p>
                    </div>
                  )}
                  {currentPreset === "custom" && (
                    <div className="px-1">
                      <p className="text-[11px] text-warning/70">
                        {t("dynamicMemory.page.customSettings")}
                      </p>
                    </div>
                  )}
                </div>

                {/* Context Enrichment (v2/v3) */}
                {currentEnabled && (
                  <div className="flex flex-col gap-3 lg:flex-row">
                    {supportsExtendedTokenCapacity && (
                      <div
                        className={cn(
                          "rounded-xl border border-fg/10 bg-fg/5 px-4 py-3",
                          "lg:min-w-0 lg:flex-1",
                        )}
                      >
                        <div className="flex items-start justify-between gap-3">
                          <div className="min-w-0 flex-1">
                            <div className="flex items-center gap-2 mb-1">
                              <span className="text-sm font-medium text-fg">
                                {t("dynamicMemory.page.contextEnrichment")}
                              </span>
                              <span className="rounded-md border border-info/30 bg-info/10 px-1.5 py-0.5 text-[10px] font-medium text-info/80">
                                {t("dynamicMemory.page.experimental")}
                              </span>
                            </div>
                            <div className="text-[11px] text-fg/45 leading-relaxed">
                              {t("dynamicMemory.page.contextEnrichmentDescription")}
                            </div>
                          </div>
                          <Switch
                            checked={currentSettings.contextEnrichmentEnabled}
                            onChange={(next) => {
                              if (activeTab === "direct") {
                                handleDirectSettingChange("contextEnrichmentEnabled", next);
                              } else {
                                handleGroupSettingChange("contextEnrichmentEnabled", next);
                              }
                            }}
                          />
                        </div>
                      </div>
                    )}

                    <div
                      className={cn(
                        "rounded-xl border border-fg/10 bg-fg/5 px-4 py-3",
                        "lg:min-w-0 lg:flex-1",
                      )}
                    >
                      <div className="flex items-start justify-between gap-3">
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-2 mb-1">
                            <span className="text-sm font-medium text-fg">
                              {t("dynamicMemory.page.recursiveMemoryLoops")}
                            </span>
                            <span className="rounded-md border border-info/30 bg-info/10 px-1.5 py-0.5 text-[10px] font-medium text-info/80">
                              {t("dynamicMemory.page.experimental")}
                            </span>
                          </div>
                          <div className="text-[11px] text-fg/45 leading-relaxed">
                            {t("dynamicMemory.page.recursiveMemoryLoopsDescription")}
                          </div>
                        </div>
                        <Switch
                          checked={currentSettings.recursiveMemoryLoops}
                          onChange={(next) => {
                            if (activeTab === "direct") {
                              handleDirectSettingChange("recursiveMemoryLoops", next);
                            } else {
                              handleGroupSettingChange("recursiveMemoryLoops", next);
                            }
                          }}
                        />
                      </div>
                    </div>
                  </div>
                )}

                {/* Advanced Options Collapsible */}
                <div className="rounded-xl border border-fg/10 bg-fg/5 overflow-hidden">
                  <button
                    onClick={() => setAdvancedOpen(!advancedOpen)}
                    className="w-full flex items-center justify-between px-4 py-3 text-left hover:bg-fg/5 transition-colors"
                  >
                    <div>
                      <span className="text-sm font-medium text-fg">
                        {t("dynamicMemory.page.advancedOptions")}
                      </span>
                      <p className="text-[11px] text-fg/45 mt-0.5">
                        {t("dynamicMemory.page.advancedOptionsDescription")}
                      </p>
                    </div>
                    <ChevronDown
                      className={cn(
                        "h-5 w-5 text-fg/40 transition-transform",
                        advancedOpen && "rotate-180",
                      )}
                    />
                  </button>

                  <AnimatePresence>
                    {advancedOpen && (
                      <motion.div
                        initial={{ height: 0, opacity: 0 }}
                        animate={{ height: "auto", opacity: 1 }}
                        exit={{ height: 0, opacity: 0 }}
                        transition={{ duration: 0.2 }}
                        className="overflow-hidden"
                      >
                        <div className="px-4 pb-4 space-y-4 border-t border-fg/10 pt-4">
                          {/* Summary Interval */}
                          <SettingRow
                            label={t("dynamicMemory.page.summaryInterval")}
                            description={t("dynamicMemory.page.summaryIntervalDescription")}
                            value={currentSettings.summaryMessageInterval}
                            unit={t("dynamicMemory.page.msgsUnit")}
                            min={10}
                            max={100}
                            step={5}
                            onChange={(val) => {
                              if (activeTab === "direct") {
                                handleDirectSettingChange("summaryMessageInterval", val);
                              } else {
                                handleGroupSettingChange("summaryMessageInterval", val);
                              }
                            }}
                          />

                          {/* Max Entries */}
                          <SettingRow
                            label={t("dynamicMemory.page.maxMemoryEntries")}
                            description={t("dynamicMemory.page.maxMemoryEntriesDescription")}
                            value={currentSettings.maxEntries}
                            unit={t("dynamicMemory.page.entriesUnit")}
                            min={10}
                            max={500}
                            step={10}
                            onChange={(val) => {
                              if (activeTab === "direct") {
                                handleDirectSettingChange("maxEntries", val);
                              } else {
                                handleGroupSettingChange("maxEntries", val);
                              }
                            }}
                          />

                          {/* Hot Memory Budget */}
                          <SettingRow
                            label={t("dynamicMemory.page.hotMemoryBudget")}
                            description={t("dynamicMemory.page.hotMemoryBudgetDescription")}
                            value={currentSettings.hotMemoryTokenBudget}
                            unit={t("dynamicMemory.page.tokensUnit")}
                            min={500}
                            max={16384}
                            step={500}
                            onChange={(val) => {
                              if (activeTab === "direct") {
                                handleDirectSettingChange("hotMemoryTokenBudget", val);
                              } else {
                                handleGroupSettingChange("hotMemoryTokenBudget", val);
                              }
                            }}
                          />

                          {/* Relevance Threshold */}
                          <SettingRow
                            label={t("dynamicMemory.page.relevanceThreshold")}
                            description={t("dynamicMemory.page.relevanceThresholdDescription")}
                            value={currentSettings.minSimilarityThreshold}
                            min={0.1}
                            max={0.8}
                            step={0.05}
                            decimals={2}
                            onChange={(val) => {
                              if (activeTab === "direct") {
                                handleDirectSettingChange("minSimilarityThreshold", val);
                              } else {
                                handleGroupSettingChange("minSimilarityThreshold", val);
                              }
                            }}
                          />

                          {/* Retrieval Limit */}
                          <div className="space-y-2">
                            <div className="text-[11px] font-medium text-fg/90">
                              {t("dynamicMemory.page.retrievalMode")}
                            </div>
                            <div className="grid grid-cols-2 gap-2">
                              <button
                                onClick={() => {
                                  if (activeTab === "direct") {
                                    handleDirectSettingChange("retrievalStrategy", "smart");
                                  } else {
                                    handleGroupSettingChange("retrievalStrategy", "smart");
                                  }
                                }}
                                className={cn(
                                  "rounded-lg border px-3 py-2 text-xs font-medium transition-colors",
                                  currentSettings.retrievalStrategy === "smart"
                                    ? "border-info/50 bg-info/20 text-info"
                                    : "border-fg/10 bg-fg/5 text-fg/60 hover:border-fg/20",
                                )}
                              >
                                {t("dynamicMemory.page.retrievalModeSmart")}
                              </button>
                              <button
                                onClick={() => {
                                  if (activeTab === "direct") {
                                    handleDirectSettingChange("retrievalStrategy", "cosine");
                                  } else {
                                    handleGroupSettingChange("retrievalStrategy", "cosine");
                                  }
                                }}
                                className={cn(
                                  "rounded-lg border px-3 py-2 text-xs font-medium transition-colors",
                                  currentSettings.retrievalStrategy === "cosine"
                                    ? "border-info/50 bg-info/20 text-info"
                                    : "border-fg/10 bg-fg/5 text-fg/60 hover:border-fg/20",
                                )}
                              >
                                {t("dynamicMemory.page.retrievalModeCosine")}
                              </button>
                            </div>
                            <p className="text-[11px] text-fg/45">
                              {t("dynamicMemory.page.retrievalModeDescription")}
                            </p>
                          </div>

                          {/* Retrieval Limit */}
                          <SettingRow
                            label={t("dynamicMemory.page.retrievalLimit")}
                            description={t("dynamicMemory.page.retrievalLimitDescription")}
                            value={currentSettings.retrievalLimit}
                            unit={t("dynamicMemory.page.itemsUnit")}
                            min={1}
                            max={20}
                            step={1}
                            onChange={(val) => {
                              if (activeTab === "direct") {
                                handleDirectSettingChange("retrievalLimit", val);
                              } else {
                                handleGroupSettingChange("retrievalLimit", val);
                              }
                            }}
                          />

                          {/* Decay Rate */}
                          <SettingRow
                            label={t("dynamicMemory.page.decayRate")}
                            description={t("dynamicMemory.page.decayRateDescription")}
                            value={currentSettings.decayRate}
                            unit={t("dynamicMemory.page.perCycleUnit")}
                            min={0.01}
                            max={0.3}
                            step={0.01}
                            decimals={2}
                            onChange={(val) => {
                              if (activeTab === "direct") {
                                handleDirectSettingChange("decayRate", val);
                              } else {
                                handleGroupSettingChange("decayRate", val);
                              }
                            }}
                          />

                          {/* Cold Threshold */}
                          <SettingRow
                            label={t("dynamicMemory.page.coldStorageThreshold")}
                            description={t("dynamicMemory.page.coldStorageThresholdDescription")}
                            value={currentSettings.coldThreshold}
                            min={0.1}
                            max={0.5}
                            step={0.05}
                            decimals={2}
                            onChange={(val) => {
                              if (activeTab === "direct") {
                                handleDirectSettingChange("coldThreshold", val);
                              } else {
                                handleGroupSettingChange("coldThreshold", val);
                              }
                            }}
                          />

                          <SettingRow
                            label={t("dynamicMemory.page.deleteConfidenceDefault")}
                            description={t("dynamicMemory.page.deleteConfidenceDefaultDescription")}
                            value={currentSettings.deleteConfidenceDefault}
                            min={0}
                            max={1}
                            step={0.05}
                            decimals={2}
                            onChange={(val) => {
                              if (activeTab === "direct") {
                                handleDirectSettingChange("deleteConfidenceDefault", val);
                              } else {
                                handleGroupSettingChange("deleteConfidenceDefault", val);
                              }
                            }}
                          />

                          <SettingRow
                            label={t("dynamicMemory.page.maxHardDeleteRatio")}
                            description={t("dynamicMemory.page.maxHardDeleteRatioDescription")}
                            value={currentSettings.maxHardDeleteRatioPerCycle}
                            min={0.1}
                            max={1}
                            step={0.05}
                            decimals={2}
                            onChange={(val) => {
                              if (activeTab === "direct") {
                                handleDirectSettingChange("maxHardDeleteRatioPerCycle", val);
                              } else {
                                handleGroupSettingChange("maxHardDeleteRatioPerCycle", val);
                              }
                            }}
                          />

                          {currentSettings.recursiveMemoryLoops && (
                            <SettingRow
                              label={t("dynamicMemory.page.recursiveLoopHardCap")}
                              description={t("dynamicMemory.page.recursiveLoopHardCapDescription")}
                              value={currentSettings.recursiveMemoryLoopHardCap}
                              unit={t("dynamicMemory.page.turnsUnit")}
                              min={1}
                              max={100}
                              step={1}
                              onChange={(val) => {
                                if (activeTab === "direct") {
                                  handleDirectSettingChange("recursiveMemoryLoopHardCap", val);
                                } else {
                                  handleGroupSettingChange("recursiveMemoryLoopHardCap", val);
                                }
                              }}
                            />
                          )}
                        </div>
                      </motion.div>
                    )}
                  </AnimatePresence>
                </div>
              </div>
            </motion.div>
          </AnimatePresence>

          {/* Shared Settings (always visible) */}
          {isAnyEnabled && (
            <div className="space-y-6 pt-2">
              <h3 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35 px-1">
                {t("dynamicMemory.page.sharedSettings")}
              </h3>

              {/* Summarisation */}
              <section className="space-y-4">
                <SectionLabel>{t("dynamicMemory.page.summarisationSection")}</SectionLabel>

                {/* Model selector */}
                <div className="space-y-3">
                  <div className="flex items-center gap-2">
                    <div className="rounded-lg border border-warning/30 bg-warning/10 p-1.5">
                      <Cpu className="h-4 w-4 text-warning" />
                    </div>
                    <h3 className="text-sm font-semibold text-fg">
                      {t("dynamicMemory.page.summarisationModel")}
                    </h3>
                  </div>

                  {models.length > 0 ? (
                    <button
                      type="button"
                      onClick={() => setShowModelMenu(true)}
                      className="flex w-full items-center justify-between rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-left transition hover:bg-surface-el/30 focus:border-fg/25 focus:outline-none"
                    >
                      <div className="flex items-center gap-2">
                        {summarisationModelId ? (
                          getProviderIcon(selectedSummarisationModel?.providerId || "")
                        ) : (
                          <Cpu className="h-5 w-5 text-fg/40" />
                        )}
                        <span
                          className={`text-sm ${summarisationModelId ? "text-fg" : "text-fg/50"}`}
                        >
                          {summarisationModelId
                            ? selectedSummarisationModelLabel
                            : t("dynamicMemory.page.useGlobalDefaultModel")}
                        </span>
                      </div>
                      <ChevronDown className="h-4 w-4 text-fg/40" />
                    </button>
                  ) : (
                    <div className="rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                      <p className="text-sm text-fg/50">
                        {t("dynamicMemory.page.noModelsAvailable")}
                      </p>
                    </div>
                  )}
                  <p className="px-1 text-xs leading-relaxed text-fg/50">
                    {t("dynamicMemory.page.summarisationModelDescription")}
                  </p>
                </div>

                {/* Prompts side-by-side on lg */}
                <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
                  <div className="space-y-3">
                    <div className="flex items-center gap-2">
                      <div className="rounded-lg border border-warning/30 bg-warning/10 p-1.5">
                        <BookOpen className="h-4 w-4 text-warning" />
                      </div>
                      <h3 className="text-sm font-semibold text-fg">
                        {t("dynamicMemory.page.summaryPrompt")}
                      </h3>
                    </div>
                    <select
                      value={selectedSummaryPromptTemplateId ?? ""}
                      onChange={(e) =>
                        void handleSummaryPromptTemplateChange(e.target.value || null)
                      }
                      className="w-full appearance-none rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-sm text-fg transition focus:border-fg/25 focus:outline-none"
                    >
                      <option value="">{t("dynamicMemory.page.useBuiltInDefault")}</option>
                      {summaryPromptTemplates.map((template) => (
                        <option key={template.id} value={template.id}>
                          {template.name}
                        </option>
                      ))}
                    </select>
                    <p className="px-1 text-xs leading-relaxed text-fg/50">
                      {t("dynamicMemory.page.summaryPromptDescription")}
                    </p>
                  </div>

                  <div className="space-y-3">
                    <div className="flex items-center gap-2">
                      <div className="rounded-lg border border-warning/30 bg-warning/10 p-1.5">
                        <BookOpen className="h-4 w-4 text-warning" />
                      </div>
                      <h3 className="text-sm font-semibold text-fg">
                        {t("dynamicMemory.page.memoryManagerPrompt")}
                      </h3>
                    </div>
                    <select
                      value={selectedMemoryManagerPromptTemplateId ?? ""}
                      onChange={(e) =>
                        void handleMemoryManagerPromptTemplateChange(e.target.value || null)
                      }
                      className="w-full appearance-none rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-sm text-fg transition focus:border-fg/25 focus:outline-none"
                    >
                      <option value="">{t("dynamicMemory.page.useBuiltInDefault")}</option>
                      {memoryManagerPromptTemplates.map((template) => (
                        <option key={template.id} value={template.id}>
                          {template.name}
                        </option>
                      ))}
                    </select>
                    <p className="px-1 text-xs leading-relaxed text-fg/50">
                      {t("dynamicMemory.page.memoryManagerPromptDescription")}
                    </p>
                  </div>
                </div>

                {/* Structured Fallback */}
                <div className="space-y-3">
                  <div className="flex items-center gap-2">
                    <div className="rounded-lg border border-info/30 bg-info/10 p-1.5">
                      <Code2 className="h-4 w-4 text-info" />
                    </div>
                    <h3 className="text-sm font-semibold text-fg">
                      {t("dynamicMemory.page.structuredFallback")}
                    </h3>
                  </div>

                  <div className="grid grid-cols-2 gap-3">
                    {(
                      [
                        {
                          value: "json" as const,
                          title: t("dynamicMemory.page.structuredFallbackJsonTitle"),
                          description: t("dynamicMemory.page.structuredFallbackJsonDescription"),
                        },
                        {
                          value: "xml" as const,
                          title: t("dynamicMemory.page.structuredFallbackXmlTitle"),
                          description: t("dynamicMemory.page.structuredFallbackXmlDescription"),
                        },
                      ]
                    ).map((option) => {
                      const active = structuredFallbackFormat === option.value;
                      return (
                        <button
                          key={option.value}
                          type="button"
                          onClick={() =>
                            void handleStructuredFallbackFormatChange(option.value)
                          }
                          className={cn(
                            "flex flex-col items-start gap-2 rounded-xl border p-4 text-left transition",
                            active
                              ? "border-info/40 bg-info/10"
                              : "border-fg/10 bg-fg/5 hover:border-fg/20",
                          )}
                        >
                          <div className="flex w-full items-center justify-between">
                            <span
                              className={cn(
                                "text-sm font-semibold",
                                active ? "text-info" : "text-fg/80",
                              )}
                            >
                              {option.title}
                            </span>
                            {active && (
                              <div className="flex h-5 w-5 items-center justify-center rounded-full bg-info">
                                <Check className="h-3 w-3 text-fg" />
                              </div>
                            )}
                          </div>
                          <span className="text-[11px] leading-relaxed text-fg/50">
                            {option.description}
                          </span>
                        </button>
                      );
                    })}
                  </div>
                  <p className="px-1 text-xs leading-relaxed text-fg/50">
                    {t("dynamicMemory.page.structuredFallbackHint")}
                  </p>
                </div>

                {/* Sampler overwrite (conditional) */}
                {isLocalLlamaSummaryModel && (
                  <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 space-y-3">
                    <div className="flex items-center justify-between gap-3">
                      <div className="min-w-0 flex-1">
                        <div className="text-sm font-medium text-fg">
                          {t("dynamicMemory.page.overwriteSampler")}
                        </div>
                        <div className="mt-1 text-[11px] leading-relaxed text-fg/45">
                          {t("dynamicMemory.page.overwriteSamplerDescription")}
                        </div>
                      </div>
                      <Switch
                        checked={dynamicMemoryLlamaSamplerOverwriteEnabled}
                        onChange={handleDynamicMemoryLlamaSamplerOverwriteChange}
                      />
                    </div>

                    {dynamicMemoryLlamaSamplerOverwriteEnabled && (
                      <details className="group border-t border-fg/8 pt-2.5">
                        <summary className="flex cursor-pointer list-none items-center gap-1.5 text-[11px] text-fg/40 transition-colors hover:text-fg/70">
                          <ChevronDown className="h-3 w-3 -rotate-90 text-fg/30 transition-transform group-open:rotate-0" />
                          {t("dynamicMemory.page.samplerValues")}
                        </summary>
                        <div className="mt-2.5 space-y-3 text-[11px]">
                          <div className="grid grid-cols-1 gap-x-8 gap-y-1.5 sm:grid-cols-2">
                            {[
                              ...DYNAMIC_MEMORY_OVERWRITE_SAMPLING.map(
                                ([key, v]) => [t(key), v] as [string, string],
                              ),
                              ...DYNAMIC_MEMORY_OVERWRITE_DRY.map(
                                ([key, v]) =>
                                  [
                                    t("dynamicMemory.page.samplerDryPrefix", {
                                      label: t(key).toLowerCase(),
                                    }),
                                    v,
                                  ] as [string, string],
                              ),
                            ].map(([label, value]) => (
                              <div
                                key={label}
                                className="flex items-baseline justify-between gap-3 border-b border-fg/5 pb-1"
                              >
                                <span className="text-fg/45">{label}</span>
                                <span className="font-mono text-fg/75">{value}</span>
                              </div>
                            ))}
                          </div>
                          <div className="flex items-baseline gap-2 text-fg/40">
                            <span className="shrink-0">{t("dynamicMemory.page.samplerOrder")}</span>
                            <span className="font-mono text-fg/60">
                              {DYNAMIC_MEMORY_LLAMA_OVERWRITE_ORDER.join(" → ")}
                            </span>
                          </div>
                        </div>
                      </details>
                    )}
                  </div>
                )}
              </section>

              {/* Embedding Model */}
              {(availableEmbeddingVersions.filter((v) => v === "v3" || v === "v4").length > 1 ||
                supportsExtendedTokenCapacity ||
                supportsMatryoshkaDimensions ||
                modelVersion) && (
                <section className="space-y-4">
                  <SectionLabel>{t("dynamicMemory.page.embeddingModelSection")}</SectionLabel>

                  {availableEmbeddingVersions.filter((v) => v === "v3" || v === "v4").length >
                    1 && (
                    <FieldRow
                      label={t("dynamicMemory.page.embeddingModel")}
                      helper={
                        selectedEmbeddingVersion === "v3"
                          ? t("dynamicMemory.page.embeddingV3Deprecated")
                          : t("dynamicMemory.page.embeddingV4Latest")
                      }
                    >
                      <SegmentedRow
                        options={(["v3", "v4"] as const).filter((version) =>
                          availableEmbeddingVersions.includes(version),
                        )}
                        value={selectedEmbeddingVersion ?? "v4"}
                        onChange={(next) =>
                          handleEmbeddingModelVersionChange(next as SelectableEmbeddingVersion)
                        }
                        formatLabel={(v) => v.toUpperCase()}
                      />
                    </FieldRow>
                  )}

                  {supportsExtendedTokenCapacity && (
                    <FieldRow
                      label={t("dynamicMemory.page.tokenCapacity")}
                      helper={t("dynamicMemory.page.tokenCapacityDescription")}
                    >
                      <SegmentedRow
                        options={[1024, 2048, 4096] as const}
                        value={embeddingMaxTokens}
                        onChange={(next) => handleEmbeddingMaxTokensChange(next)}
                        formatLabel={(v) => `${v / 1024}K`}
                      />
                    </FieldRow>
                  )}

                  {supportsMatryoshkaDimensions && (
                    <FieldRow
                      label={t("dynamicMemory.page.embeddingDimensions")}
                      helper={t("dynamicMemory.page.embeddingDimensionsHelper")}
                    >
                      <SegmentedRow
                        options={V4_DIMENSION_OPTIONS}
                        value={embeddingDimensions}
                        onChange={(next) =>
                          handleEmbeddingDimensionsChange(
                            next as (typeof V4_DIMENSION_OPTIONS)[number],
                          )
                        }
                        formatLabel={(v) => `${v}d`}
                      />
                    </FieldRow>
                  )}

                  {supportsExtendedTokenCapacity && (
                    <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
                      <div className="flex items-center justify-between gap-3">
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-2">
                            <span className="text-sm font-medium text-fg">
                              {t("dynamicMemory.page.keepModelLoaded")}
                            </span>
                            <span className="rounded-md border border-info/30 bg-info/10 px-1.5 py-0.5 text-[10px] font-medium text-info/80">
                              {t("dynamicMemory.page.experimental")}
                            </span>
                          </div>
                          <p className="mt-1 text-[11px] leading-relaxed text-fg/45">
                            {t("dynamicMemory.page.keepModelLoadedDescription")}
                          </p>
                        </div>
                        <Switch
                          checked={embeddingKeepModelLoaded}
                          onChange={handleEmbeddingKeepModelLoadedChange}
                        />
                      </div>
                    </div>
                  )}

                  {modelVersion && (
                    <p className="px-1 text-[11px] text-fg/40">
                      {t("dynamicMemory.page.installedMemoryModel", {
                        version: (modelSourceVersion ?? modelVersion).toUpperCase(),
                        tokens: embeddingMaxTokens,
                        dimensions: effectiveEmbeddingDimensions,
                      })}
                    </p>
                  )}
                </section>
              )}

              {/* Model Management */}
              <section className="space-y-3">
                <SectionLabel>{t("dynamicMemory.page.modelManagement")}</SectionLabel>
                <div className="grid grid-cols-2 gap-3 lg:grid-cols-3">
                  <button
                    onClick={() => navigate("/settings/embedding-test")}
                    className={cn(
                      "flex items-center justify-center gap-2 rounded-xl",
                      "border border-fg/10 bg-fg/5 px-4 py-3",
                      "text-sm font-medium text-fg",
                      interactive.transition.fast,
                      "hover:bg-fg/10",
                    )}
                  >
                    <RefreshCw className="h-4 w-4" />
                    {t("dynamicMemory.page.testModel")}
                  </button>
                  {(!hasBothMajorEmbeddingVersionsInstalled || !installBundleComplete) && (
                    <button
                      onClick={() => setShowDownloadModelMenu(true)}
                      className={cn(
                        "flex items-center justify-center gap-2 rounded-xl border px-4 py-3 text-sm font-medium",
                        "border-info/25 bg-info/10 text-info",
                        interactive.transition.fast,
                        "hover:bg-info/20",
                      )}
                    >
                      <Sparkles className="h-4 w-4" />
                      {t("dynamicMemory.page.downloadModel")}
                    </button>
                  )}
                  <button
                    onClick={handleDeleteSelectedEmbeddingModel}
                    className={cn(
                      "flex items-center justify-center gap-2 rounded-xl",
                      "border border-danger/20 bg-danger/10 px-4 py-3",
                      "text-sm font-medium text-danger/80",
                      interactive.transition.fast,
                      "hover:bg-danger/20",
                    )}
                  >
                    <Trash2 className="h-4 w-4" />
                    {t("dynamicMemory.page.delete")}
                  </button>
                </div>
              </section>
            </div>
          )}
        </div>
      </main>

      {/* Download Model BottomMenu */}
      <BottomMenu
        isOpen={showDownloadModelMenu}
        onClose={() => setShowDownloadModelMenu(false)}
        title={t("dynamicMemory.page.downloadEmbeddingModel")}
      >
        <div className="space-y-3">
          <p className="text-xs text-fg/55">
            {t("dynamicMemory.page.downloadEmbeddingDescription")}
          </p>
          <button
            onClick={() => {
              setShowDownloadModelMenu(false);
              navigateToBundleDownload("v4");
            }}
            disabled={hasV4Installed && installBundleComplete}
            className={cn(
              "flex w-full items-center justify-between rounded-xl border px-4 py-3 text-left transition",
              hasV4Installed && installBundleComplete
                ? "cursor-not-allowed border-fg/10 bg-fg/5 text-fg/35"
                : "border-fg/10 bg-fg/5 text-fg hover:bg-fg/10",
            )}
          >
            <div className="flex items-center gap-2">
              <div className="rounded-md border border-fg/10 bg-fg/5 p-1.5">
                <Rocket className="h-4 w-4" />
              </div>
              <div>
                <div className="text-sm font-medium">
                  {t("dynamicMemory.page.downloadVersion", { version: "v4" })}
                </div>
                <div className="text-[11px] text-fg/45">
                  {t("dynamicMemory.page.downloadV4Description")}
                </div>
              </div>
            </div>
            {hasV4Installed && installBundleComplete && (
              <span className="flex items-center gap-1 text-xs text-fg/45">
                <Check className="h-3.5 w-3.5" />
                {t("dynamicMemory.page.installed")}
              </span>
            )}
          </button>
        </div>
      </BottomMenu>

      <ModelSelectionBottomMenu
        isOpen={showModelMenu}
        onClose={() => setShowModelMenu(false)}
        title={t("dynamicMemory.page.selectModel")}
        models={models}
        selectedModelIds={summarisationModelId ? [summarisationModelId] : []}
        searchPlaceholder={t("dynamicMemory.page.searchModels")}
        onSelectModel={(modelId) => {
          handleSummarisationModelChange(modelId);
          setShowModelMenu(false);
        }}
        clearOption={{
          label: t("dynamicMemory.page.useGlobalDefaultModel"),
          icon: Cpu,
          selected: !summarisationModelId,
          onClick: () => {
            handleSummarisationModelChange(null);
            setShowModelMenu(false);
          },
        }}
      />
    </div>
  );
}

// Compact setting row component
interface SettingRowProps {
  label: string;
  description: string;
  value: number;
  unit?: string;
  min: number;
  max: number;
  step: number;
  decimals?: number;
  onChange: (value: number) => void;
}

function SettingRow({
  label,
  description,
  value,
  unit,
  min,
  max,
  step,
  decimals = 0,
  onChange,
}: SettingRowProps) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div className="min-w-0 flex-1">
        <div className="text-sm text-fg">{label}</div>
        <div className="text-[10px] text-fg/40">{description}</div>
      </div>
      <div className="grid grid-cols-[96px_56px] items-center gap-2 shrink-0">
        <NumberInput
          value={value}
          onChange={(next) => {
            if (next !== null) onChange(next);
          }}
          min={min}
          max={max}
          step={step}
          decimals={decimals}
          className={cn(
            "w-full rounded-lg border border-fg/10 bg-surface-el/30",
            "px-2.5 py-1.5 text-sm text-fg text-right",
            "focus:border-fg/20 focus:outline-none",
          )}
        />
        <span className="text-[11px] text-fg/40 text-right">{unit || ""}</span>
      </div>
    </div>
  );
}
