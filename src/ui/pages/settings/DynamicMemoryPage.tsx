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
  Check,
  Zap,
  Scale,
  Brain,
  Boxes,
  Rocket,
} from "lucide-react";
import {
  readSettings,
  saveAdvancedSettings,
  getEmbeddingModelInfo,
} from "../../../core/storage/repo";
import { storageBridge } from "../../../core/storage/files";
import type {
  DynamicMemorySettings,
  DynamicMemoryStructuredFallbackFormat,
  Model,
  Settings,
} from "../../../core/storage/schemas";
import { cn, typography, interactive } from "../../design-tokens";
import { useNavigate } from "react-router-dom";
import { EmbeddingUpgradePrompt } from "../../components/EmbeddingUpgradePrompt";
import { BottomMenu } from "../../components/BottomMenu";
import { confirmBottomMenu } from "../../components/ConfirmBottomMenu";
import { ModelSelectionBottomMenu } from "../../components/ModelSelectionBottomMenu";
import { getProviderIcon } from "../../../core/utils/providerIcons";
import { useI18n } from "../../../core/i18n/context";
import { Switch } from "../../components/Switch";

const DYNAMIC_MEMORY_LLAMA_OVERWRITE_ORDER = [
  "penalties",
  "grammar",
  "top_k",
  "top_p",
  "temp",
  "min_p",
  "typical",
] as const;

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
};

type MemoryPreset = "minimal" | "balanced" | "comprehensive" | "custom";

const PRESETS: Record<
  Exclude<MemoryPreset, "custom">,
  Omit<
    DynamicMemorySettings,
    | "enabled"
    | "contextEnrichmentEnabled"
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
  if (advanced.dynamicMemoryLlamaSamplerOverwriteEnabled === undefined) {
    advanced.dynamicMemoryLlamaSamplerOverwriteEnabled = true;
  }
  advanced.dynamicMemory = hydrateDynamicMemorySettings(advanced.dynamicMemory);
  settings.advancedSettings = advanced;
  return advanced;
};

const normalizeModelId = (value?: string | null) => (value && value.trim() ? value : null);

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
  const [embeddingMaxTokens, setEmbeddingMaxTokens] = useState<number>(2048);
  const [embeddingKeepModelLoaded, setEmbeddingKeepModelLoaded] = useState(false);
  const [modelVersion, setModelVersion] = useState<string | null>(null);
  const [modelSourceVersion, setModelSourceVersion] = useState<string | null>(null);
  const [availableEmbeddingVersions, setAvailableEmbeddingVersions] = useState<string[]>([]);
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
        const [settings, modelInfo] = await Promise.all([readSettings(), getEmbeddingModelInfo()]);

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
        setStructuredFallbackFormat(
          settings.advancedSettings?.dynamicMemoryStructuredFallbackFormat ?? "xml",
        );
        setDynamicMemoryLlamaSamplerOverwriteEnabled(
          settings.advancedSettings?.dynamicMemoryLlamaSamplerOverwriteEnabled ?? true,
        );
        setEmbeddingMaxTokens(settings.advancedSettings?.embeddingMaxTokens ?? 2048);
        setEmbeddingKeepModelLoaded(settings.advancedSettings?.embeddingKeepModelLoaded ?? false);
        setModels(settings.models);

        if (modelInfo.installed) {
          setModelVersion(modelInfo.version);
          const sourceVersion =
            modelInfo.selectedSourceVersion ?? modelInfo.sourceVersion ?? modelInfo.version;
          setModelSourceVersion(sourceVersion);
          setSelectedEmbeddingVersion(sourceVersion);
          const available = modelInfo.availableVersions ?? [];
          setAvailableEmbeddingVersions(available);
          if ((sourceVersion === "v1" || sourceVersion === "v2") && !available.includes("v3")) {
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

  const handleEmbeddingModelVersionChange = async (version: "v2" | "v3") => {
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

  const handleEmbeddingKeepModelLoadedChange = async (enabled: boolean) => {
    setEmbeddingKeepModelLoaded(enabled);
    await updateAdvancedSettings((advanced) => {
      advanced.embeddingKeepModelLoaded = enabled;
    }, "Failed to save keep-model-loaded setting:");
  };

  const handleDeleteSelectedEmbeddingModel = async () => {
    const version = selectedEmbeddingVersion === "v2" ? "v2" : "v3";
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
  const isLocalLlamaSummaryModel = effectiveSummarisationModel?.providerId === "llamacpp";
  const hasV2Installed = availableEmbeddingVersions.includes("v2");
  const hasV3Installed = availableEmbeddingVersions.includes("v3");
  const hasBothMajorEmbeddingVersionsInstalled = hasV2Installed && hasV3Installed;
  const effectiveEmbeddingVersion =
    selectedEmbeddingVersion ?? modelSourceVersion ?? modelVersion ?? null;
  const supportsExtendedTokenCapacity =
    effectiveEmbeddingVersion === "v2" || effectiveEmbeddingVersion === "v3";
  const selectedSummarisationModelLabel =
    selectedSummarisationModel?.displayName || t("dynamicMemory.page.selectedModel");

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
                currentVersion={modelSourceVersion === "v1" ? "v1" : "v2"}
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
                    <button
                      onClick={async () => {
                        const next = !enabled;
                        setEnabled(next);
                        await handleDirectSettingChange("enabled", next);
                      }}
                      className={cn(
                        "relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors",
                        enabled ? "bg-accent" : "bg-fg/20",
                      )}
                    >
                      <span
                        className={cn(
                          "inline-block h-4 w-4 rounded-full bg-white shadow-sm transition-transform",
                          enabled ? "translate-x-6" : "translate-x-1",
                        )}
                      />
                    </button>
                  </div>
                </div>
              )}

              {activeTab === "group" && (
                <div className={cn("rounded-xl border border-fg/8 bg-fg/3 px-4 py-3")}>
                  <p className="text-xs text-fg/50">{t("dynamicMemory.page.groupChatsInfo")}</p>
                </div>
              )}

              <div className={cn(!currentEnabled && "opacity-50 pointer-events-none", "space-y-4")}>
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
                {supportsExtendedTokenCapacity && currentEnabled && (
                  <div className={cn("rounded-xl border border-fg/10 bg-fg/5 px-4 py-3")}>
                    <div className="flex items-center justify-between gap-3">
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
                            label="Delete Confidence Default"
                            description="Used when the model omits delete confidence. Lower values prefer cold storage instead of hard delete."
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
                            label="Max Hard Delete Ratio"
                            description="Caps how much of the starting memory set can be hard-deleted in one cycle. Extra deletes are downgraded to cold storage."
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
                        </div>
                      </motion.div>
                    )}
                  </AnimatePresence>
                </div>
              </div>
            </motion.div>
          </AnimatePresence>

          {/* Shared Settings (always visible) - Desktop: Two Column Grid */}
          {isAnyEnabled && (
            <div className="space-y-4 pt-2">
              <h3 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35 px-1">
                {t("dynamicMemory.page.sharedSettings")}
              </h3>

              <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
                {/* Left: Summarisation Model */}
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
                  <p className="text-xs text-fg/50">
                    {t("dynamicMemory.page.summarisationModelDescription")}
                  </p>

                  <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 space-y-3">
                    <div>
                      <div className="text-sm font-medium text-fg">Structured Fallback Format</div>
                      <div className="mt-1 text-[11px] leading-relaxed text-fg/45">
                        Used when tool calling fails during dynamic memory updates and the model is
                        asked to return structured output directly.
                      </div>
                    </div>
                    <div className="grid grid-cols-2 gap-2">
                      {(["xml", "json"] as const).map((format) => (
                        <button
                          key={format}
                          onClick={() => handleStructuredFallbackFormatChange(format)}
                          className={cn(
                            "rounded-lg border px-3 py-2 text-xs font-medium uppercase transition-colors",
                            structuredFallbackFormat === format
                              ? "border-info/50 bg-info/20 text-info"
                              : "border-fg/10 bg-fg/5 text-fg/60 hover:border-fg/20",
                          )}
                        >
                          {format}
                        </button>
                      ))}
                    </div>
                    <p className="text-[11px] text-fg/45">
                      XML is the default. JSON can be useful for models that follow JSON-only
                      instructions more reliably than XML.
                    </p>
                  </div>

                  {isLocalLlamaSummaryModel && (
                    <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 space-y-3">
                      <div className="flex items-center justify-between gap-3">
                        <div className="min-w-0 flex-1">
                          <div className="text-sm font-medium text-fg">
                            Overwrite Model Sampler Configuration
                          </div>
                          <div className="mt-1 text-[11px] leading-relaxed text-fg/45">
                            Use a fixed llama.cpp sampler setup for dynamic memory instead of the
                            summarisation model&apos;s saved sampler configuration.
                          </div>
                        </div>
                        <Switch
                          checked={dynamicMemoryLlamaSamplerOverwriteEnabled}
                          onChange={handleDynamicMemoryLlamaSamplerOverwriteChange}
                        />
                      </div>

                      {dynamicMemoryLlamaSamplerOverwriteEnabled && (
                        <div className="rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2.5 space-y-2">
                          <div className="text-[11px] font-medium uppercase tracking-[0.18em] text-fg/35">
                            Overwrite Values
                          </div>
                          <div className="grid grid-cols-2 gap-2 text-xs text-fg/65">
                            <div>Temperature: 0.4</div>
                            <div>Top P: 1.0</div>
                            <div>Top K: 40</div>
                            <div>Frequency Penalty: 0.0</div>
                            <div>Presence Penalty: 0.0</div>
                            <div>Min P: disabled</div>
                            <div>Typical P: disabled</div>
                          </div>
                          <div className="text-xs text-fg/55">
                            Order:{" "}
                            <span className="font-mono text-[11px] text-fg/70">
                              {DYNAMIC_MEMORY_LLAMA_OVERWRITE_ORDER.join(" -> ")}
                            </span>
                          </div>
                        </div>
                      )}
                    </div>
                  )}

                  {/* Desktop: Model Management under Summarisation to avoid large left-column gap */}
                  <div className="hidden lg:block space-y-3 pt-4">
                    <h3 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35 px-1">
                      {t("dynamicMemory.page.modelManagement")}
                    </h3>
                    <div className="grid grid-cols-2 gap-3">
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
                      {!hasBothMajorEmbeddingVersionsInstalled && (
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
                  </div>
                </div>

                {/* Right: Token Capacity (v2/v3) or Model Info */}
                <div className="space-y-3">
                  {availableEmbeddingVersions.filter((v) => v === "v2" || v === "v3").length >
                    1 && (
                    <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
                      <div className="mb-2 text-sm font-medium text-fg">
                        {t("dynamicMemory.page.embeddingModel")}
                      </div>
                      <div className="grid grid-cols-2 gap-2">
                        {(["v2", "v3"] as const)
                          .filter((version) => availableEmbeddingVersions.includes(version))
                          .map((version) => (
                            <button
                              key={version}
                              onClick={() => handleEmbeddingModelVersionChange(version)}
                              className={cn(
                                "px-3 py-2.5 rounded-lg text-sm font-medium transition-all uppercase",
                                selectedEmbeddingVersion === version
                                  ? "bg-info text-fg"
                                  : "border border-fg/10 bg-fg/5 text-fg/70 hover:border-fg/20",
                              )}
                            >
                              {version}
                            </button>
                          ))}
                      </div>
                    </div>
                  )}

                  {supportsExtendedTokenCapacity && (
                    <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
                      <div className="flex items-center justify-between gap-2 mb-2">
                        <span className="text-sm font-medium text-fg">
                          {t("dynamicMemory.page.tokenCapacity")}
                        </span>
                        <span
                          className={cn(
                            "rounded-md border border-fg/10 bg-fg/10 px-2 py-1",
                            typography.caption.size,
                            "text-fg/70",
                          )}
                        >
                          {embeddingMaxTokens} {t("dynamicMemory.page.tokensUnit")}
                        </span>
                      </div>
                      <p className="text-[11px] text-fg/45 mb-3">
                        {t("dynamicMemory.page.tokenCapacityDescription")}
                      </p>
                      <div className="grid grid-cols-3 gap-2">
                        {[1024, 2048, 4096].map((val) => (
                          <button
                            key={val}
                            onClick={() => handleEmbeddingMaxTokensChange(val)}
                            className={cn(
                              "px-3 py-2.5 rounded-lg text-sm font-medium transition-all",
                              embeddingMaxTokens === val
                                ? "bg-info text-fg"
                                : "border border-fg/10 bg-fg/5 text-fg/70 hover:border-fg/20",
                            )}
                          >
                            {val / 1024}K
                          </button>
                        ))}
                      </div>
                    </div>
                  )}

                  {supportsExtendedTokenCapacity && (
                    <div className={cn("rounded-xl border border-fg/10 bg-fg/5 px-4 py-3")}>
                      <div className="flex items-center justify-between gap-3">
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-2 mb-1">
                            <span className="text-sm font-medium text-fg">
                              {t("dynamicMemory.page.keepModelLoaded")}
                            </span>
                            <span className="rounded-md border border-info/30 bg-info/10 px-1.5 py-0.5 text-[10px] font-medium text-info/80">
                              {t("dynamicMemory.page.experimental")}
                            </span>
                          </div>
                          <div className="text-[11px] text-fg/45 leading-relaxed">
                            {t("dynamicMemory.page.keepModelLoadedDescription")}
                          </div>
                        </div>
                        <Switch
                          checked={embeddingKeepModelLoaded}
                          onChange={handleEmbeddingKeepModelLoadedChange}
                        />
                      </div>
                    </div>
                  )}

                  {/* Model info */}
                  {modelVersion && (
                    <div className="text-xs text-fg/40 px-1">
                      {t("dynamicMemory.page.installedModel", {
                        version: modelSourceVersion ?? modelVersion,
                        tokens: embeddingMaxTokens,
                      })}
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}

          {/* Model Management */}
          {isAnyEnabled && (
            <div className="space-y-3 pt-2 lg:hidden">
              <h3 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35 px-1">
                {t("dynamicMemory.page.modelManagement")}
              </h3>

              <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
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
                {!hasBothMajorEmbeddingVersionsInstalled && (
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
              navigate("/settings/embedding-download?version=v2");
            }}
            disabled={hasV2Installed}
            className={cn(
              "flex w-full items-center justify-between rounded-xl border px-4 py-3 text-left transition",
              hasV2Installed
                ? "cursor-not-allowed border-fg/10 bg-fg/5 text-fg/35"
                : "border-fg/10 bg-fg/5 text-fg hover:bg-fg/10",
            )}
          >
            <div className="flex items-center gap-2">
              <div className="rounded-md border border-fg/10 bg-fg/5 p-1.5">
                <Boxes className="h-4 w-4" />
              </div>
              <div>
                <div className="text-sm font-medium">
                  {t("dynamicMemory.page.downloadVersion", { version: "v2" })}
                </div>
                <div className="text-[11px] text-fg/45">
                  {t("dynamicMemory.page.downloadV2Description")}
                </div>
              </div>
            </div>
            {hasV2Installed && (
              <span className="flex items-center gap-1 text-xs text-fg/45">
                <Check className="h-3.5 w-3.5" />
                {t("dynamicMemory.page.installed")}
              </span>
            )}
          </button>
          <button
            onClick={() => {
              setShowDownloadModelMenu(false);
              navigate("/settings/embedding-download?version=v3");
            }}
            disabled={hasV3Installed}
            className={cn(
              "flex w-full items-center justify-between rounded-xl border px-4 py-3 text-left transition",
              hasV3Installed
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
                  {t("dynamicMemory.page.downloadVersion", { version: "v3" })}
                </div>
                <div className="text-[11px] text-fg/45">
                  {t("dynamicMemory.page.downloadV3Description")}
                </div>
              </div>
            </div>
            {hasV3Installed && (
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
  const displayValue = decimals > 0 ? value.toFixed(decimals) : value;

  return (
    <div className="flex items-center justify-between gap-4">
      <div className="min-w-0 flex-1">
        <div className="text-sm text-fg">{label}</div>
        <div className="text-[10px] text-fg/40">{description}</div>
      </div>
      <div className="grid grid-cols-[96px_56px] items-center gap-2 shrink-0">
        <input
          type="number"
          inputMode={decimals > 0 ? "decimal" : "numeric"}
          min={min}
          max={max}
          step={step}
          value={displayValue}
          onChange={(e) => {
            const raw = e.target.value;
            const next = Number(raw);
            if (raw && Number.isFinite(next)) {
              onChange(Math.min(max, Math.max(min, next)));
            }
          }}
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
