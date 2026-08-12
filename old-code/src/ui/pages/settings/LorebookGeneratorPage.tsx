import { useEffect, useState } from "react";
import { BookOpen, Check, ChevronDown, Code2, Cpu, Gauge, Hash, Info, Sparkles } from "lucide-react";

import type {
  DynamicMemoryStructuredFallbackFormat,
  Model,
  Settings,
  SystemPromptTemplate,
} from "../../../core/storage/schemas";
import { readSettings, saveAdvancedSettings } from "../../../core/storage/repo";
import { listPromptTemplates } from "../../../core/prompts/service";
import {
  APP_LOREBOOK_GENERATOR_COHERENCE_TEMPLATE_ID,
  APP_LOREBOOK_GENERATOR_PLANNER_TEMPLATE_ID,
  APP_LOREBOOK_GENERATOR_REFINE_TEMPLATE_ID,
  APP_LOREBOOK_GENERATOR_WRITER_TEMPLATE_ID,
} from "../../../core/prompts/constants";
import { getProviderIcon } from "../../../core/utils/providerIcons";
import { cn } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import type { TranslationKey } from "../../../core/i18n/context";
import { ModelSelectionBottomMenu } from "../../components/ModelSelectionBottomMenu";
import { NumberInput } from "../../components/NumberInput";

const FALLBACK_OPTIONS = [
  {
    value: "json",
    titleKey: "lorebookGen.full.fallbackJsonTitle",
    descriptionKey: "lorebookGen.full.fallbackJsonDescription",
  },
  {
    value: "xml",
    titleKey: "lorebookGen.full.fallbackXmlTitle",
    descriptionKey: "lorebookGen.full.fallbackXmlDescription",
  },
] satisfies Array<{
  value: DynamicMemoryStructuredFallbackFormat;
  titleKey: TranslationKey;
  descriptionKey: TranslationKey;
}>;

const MIN_TARGET = 5;
const MAX_TARGET = 50;
const MIN_MAX_TOKENS = 256;
const MAX_MAX_TOKENS = 32768;
const DEFAULT_MAX_TOKENS = 4096;

interface PromptStage {
  appTemplateId: string;
  promptType:
    | "lorebookGeneratorPlanner"
    | "lorebookGeneratorWriter"
    | "lorebookGeneratorRefine"
    | "lorebookGeneratorCoherence";
  titleKey: TranslationKey;
  descriptionKey: TranslationKey;
  selectedId: string | null;
  onSelect: (id: string | null) => void;
}

function ensureAdvancedSettings(settings: Settings): NonNullable<Settings["advancedSettings"]> {
  const advanced = settings.advancedSettings ?? {
    creationHelperEnabled: false,
    helpMeReplyEnabled: true,
    lorebookEntryGeneratorStructuredFallbackFormat: "json",
    lorebookGeneratorStructuredFallbackFormat: "json",
    lorebookGeneratorDefaultTargetCount: 12,
  };
  if (advanced.lorebookGeneratorStructuredFallbackFormat === undefined) {
    advanced.lorebookGeneratorStructuredFallbackFormat = "json";
  }
  if (advanced.lorebookGeneratorDefaultTargetCount === undefined) {
    advanced.lorebookGeneratorDefaultTargetCount = 12;
  }
  settings.advancedSettings = advanced;
  return advanced;
}

export function LorebookGeneratorPage() {
  const { t } = useI18n();
  const [isLoading, setIsLoading] = useState(true);
  const [models, setModels] = useState<Model[]>([]);
  const [defaultModelId, setDefaultModelId] = useState<string | null>(null);
  const [selectedModelId, setSelectedModelId] = useState<string | null>(null);
  const [fallbackFormat, setFallbackFormat] =
    useState<DynamicMemoryStructuredFallbackFormat>("json");
  const [targetCount, setTargetCount] = useState<number>(12);
  const [maxTokens, setMaxTokens] = useState<number>(DEFAULT_MAX_TOKENS);
  const [templates, setTemplates] = useState<SystemPromptTemplate[]>([]);
  const [plannerTemplateId, setPlannerTemplateId] = useState<string | null>(null);
  const [writerTemplateId, setWriterTemplateId] = useState<string | null>(null);
  const [refineTemplateId, setRefineTemplateId] = useState<string | null>(null);
  const [coherenceTemplateId, setCoherenceTemplateId] = useState<string | null>(null);
  const [showModelMenu, setShowModelMenu] = useState(false);

  useEffect(() => {
    const load = async () => {
      try {
        const [settings, promptTemplates] = await Promise.all([
          readSettings(),
          listPromptTemplates(),
        ]);
        const advanced = ensureAdvancedSettings(settings);
        const textModels = settings.models.filter(
          (m) => !m.outputScopes || m.outputScopes.includes("text"),
        );
        setModels(textModels);
        setDefaultModelId(settings.defaultModelId ?? null);
        setSelectedModelId(advanced.lorebookGeneratorModelId ?? null);
        setFallbackFormat(advanced.lorebookGeneratorStructuredFallbackFormat ?? "json");
        setTargetCount(
          Math.min(MAX_TARGET, Math.max(MIN_TARGET, advanced.lorebookGeneratorDefaultTargetCount ?? 12)),
        );
        setMaxTokens(
          Math.min(
            MAX_MAX_TOKENS,
            Math.max(MIN_MAX_TOKENS, advanced.lorebookGeneratorMaxTokens ?? DEFAULT_MAX_TOKENS),
          ),
        );
        setPlannerTemplateId(advanced.lorebookGeneratorPlannerPromptTemplateId ?? null);
        setWriterTemplateId(advanced.lorebookGeneratorWriterPromptTemplateId ?? null);
        setRefineTemplateId(advanced.lorebookGeneratorRefinePromptTemplateId ?? null);
        setCoherenceTemplateId(advanced.lorebookGeneratorCoherencePromptTemplateId ?? null);
        setTemplates(promptTemplates);
      } catch (error) {
        console.error("Failed to load lorebook generator settings:", error);
      } finally {
        setIsLoading(false);
      }
    };
    void load();
  }, []);

  const updateAdvanced = async (
    updater: (advanced: NonNullable<Settings["advancedSettings"]>) => void,
    errorMessage: string,
  ) => {
    try {
      const settings = await readSettings();
      const advanced = ensureAdvancedSettings(settings);
      updater(advanced);
      await saveAdvancedSettings(advanced);
    } catch (error) {
      console.error(errorMessage, error);
    }
  };

  const selectedModel = selectedModelId
    ? models.find((m) => m.id === selectedModelId) ?? null
    : null;
  const defaultModel = defaultModelId
    ? models.find((m) => m.id === defaultModelId) ?? null
    : null;
  const selectedModelLabel = selectedModel?.displayName ?? t("lorebookGen.full.generationModelFallback");
  const appDefaultLabel = defaultModel
    ? t("lorebookGen.full.useAppDefaultWith", { name: defaultModel.displayName })
    : t("lorebookGen.full.useAppDefault");

  const handleModelChange = async (modelId: string | null) => {
    setSelectedModelId(modelId);
    await updateAdvanced((advanced) => {
      advanced.lorebookGeneratorModelId = modelId ?? undefined;
    }, "Failed to save lorebook generator model:");
  };

  const handleFallbackChange = async (value: DynamicMemoryStructuredFallbackFormat) => {
    setFallbackFormat(value);
    await updateAdvanced((advanced) => {
      advanced.lorebookGeneratorStructuredFallbackFormat = value;
    }, "Failed to save lorebook generator fallback format:");
  };

  const handleTargetCountChange = async (next: number) => {
    const clamped = Math.min(MAX_TARGET, Math.max(MIN_TARGET, Math.round(next)));
    setTargetCount(clamped);
    await updateAdvanced((advanced) => {
      advanced.lorebookGeneratorDefaultTargetCount = clamped;
    }, "Failed to save lorebook generator default target count:");
  };

  const handleMaxTokensChange = async (next: number) => {
    const clamped = Math.min(
      MAX_MAX_TOKENS,
      Math.max(MIN_MAX_TOKENS, Math.round(next)),
    );
    setMaxTokens(clamped);
    await updateAdvanced((advanced) => {
      advanced.lorebookGeneratorMaxTokens = clamped;
    }, "Failed to save lorebook generator max tokens:");
  };

  const stages: PromptStage[] = [
    {
      appTemplateId: APP_LOREBOOK_GENERATOR_PLANNER_TEMPLATE_ID,
      promptType: "lorebookGeneratorPlanner",
      titleKey: "lorebookGen.full.plannerTitle",
      descriptionKey: "lorebookGen.full.plannerDescription",
      selectedId: plannerTemplateId,
      onSelect: (id) => {
        setPlannerTemplateId(id);
        void updateAdvanced((advanced) => {
          advanced.lorebookGeneratorPlannerPromptTemplateId = id ?? undefined;
        }, "Failed to save planner prompt:");
      },
    },
    {
      appTemplateId: APP_LOREBOOK_GENERATOR_WRITER_TEMPLATE_ID,
      promptType: "lorebookGeneratorWriter",
      titleKey: "lorebookGen.full.writerTitle",
      descriptionKey: "lorebookGen.full.writerDescription",
      selectedId: writerTemplateId,
      onSelect: (id) => {
        setWriterTemplateId(id);
        void updateAdvanced((advanced) => {
          advanced.lorebookGeneratorWriterPromptTemplateId = id ?? undefined;
        }, "Failed to save writer prompt:");
      },
    },
    {
      appTemplateId: APP_LOREBOOK_GENERATOR_REFINE_TEMPLATE_ID,
      promptType: "lorebookGeneratorRefine",
      titleKey: "lorebookGen.full.refineTitle",
      descriptionKey: "lorebookGen.full.refineDescription",
      selectedId: refineTemplateId,
      onSelect: (id) => {
        setRefineTemplateId(id);
        void updateAdvanced((advanced) => {
          advanced.lorebookGeneratorRefinePromptTemplateId = id ?? undefined;
        }, "Failed to save refine prompt:");
      },
    },
    {
      appTemplateId: APP_LOREBOOK_GENERATOR_COHERENCE_TEMPLATE_ID,
      promptType: "lorebookGeneratorCoherence",
      titleKey: "lorebookGen.full.coherenceTitle",
      descriptionKey: "lorebookGen.full.coherenceDescription",
      selectedId: coherenceTemplateId,
      onSelect: (id) => {
        setCoherenceTemplateId(id);
        void updateAdvanced((advanced) => {
          advanced.lorebookGeneratorCoherencePromptTemplateId = id ?? undefined;
        }, "Failed to save coherence prompt:");
      },
    },
  ];

  if (isLoading) return null;

  return (
    <div className="flex min-h-screen flex-col">
      <main className="flex-1 px-4 pb-24 pt-4">
        <div className="mx-auto w-full max-w-5xl space-y-6">
          <div className="rounded-xl border border-accent/20 bg-accent/5 p-3">
            <div className="flex items-start gap-2">
              <Sparkles className="mt-0.5 h-4 w-4 shrink-0 text-accent" />
              <p className="text-xs leading-relaxed text-accent/80">
                {t("lorebookGen.full.infoText", { format: fallbackFormat.toUpperCase() })}
              </p>
            </div>
          </div>

          <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
            <div className="space-y-4">
              <h3 className="px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
                {t("lorebookGen.full.generationHeading")}
              </h3>

              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-accent/30 bg-accent/10 p-1.5">
                    <Cpu className="h-4 w-4 text-accent" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">{t("lorebookGen.full.generationModel")}</h3>
                </div>

                {models.length > 0 ? (
                  <button
                    type="button"
                    onClick={() => setShowModelMenu(true)}
                    className="flex w-full items-center justify-between rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-left transition hover:bg-surface-el/30 focus:border-fg/25 focus:outline-none"
                  >
                    <div className="flex min-w-0 items-center gap-2">
                      {selectedModelId ? (
                        getProviderIcon(selectedModel?.providerId ?? "")
                      ) : (
                        <Cpu className="h-5 w-5 shrink-0 text-fg/40" />
                      )}
                      <span
                        className={`truncate text-sm ${
                          selectedModelId ? "text-fg" : "text-fg/50"
                        }`}
                      >
                        {selectedModelId ? selectedModelLabel : appDefaultLabel}
                      </span>
                    </div>
                    <ChevronDown className="h-4 w-4 shrink-0 text-fg/40" />
                  </button>
                ) : (
                  <div className="rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                    <p className="text-sm text-fg/50">{t("lorebookGen.full.noTextModels")}</p>
                  </div>
                )}
                <p className="px-1 text-xs text-fg/50">
                  {t("lorebookGen.full.modelHint")}
                </p>
              </div>

              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-info/30 bg-info/10 p-1.5">
                    <Code2 className="h-4 w-4 text-info" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">{t("lorebookGen.full.structuredFallback")}</h3>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  {FALLBACK_OPTIONS.map((option) => {
                    const active = fallbackFormat === option.value;
                    return (
                      <button
                        key={option.value}
                        type="button"
                        onClick={() => void handleFallbackChange(option.value)}
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
                            {t(option.titleKey)}
                          </span>
                          {active && (
                            <div className="flex h-5 w-5 items-center justify-center rounded-full bg-info">
                              <Check className="h-3 w-3 text-fg" />
                            </div>
                          )}
                        </div>
                        <span className="text-[11px] leading-relaxed text-fg/50">
                          {t(option.descriptionKey)}
                        </span>
                      </button>
                    );
                  })}
                </div>
              </div>

              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-warning/30 bg-warning/10 p-1.5">
                    <Hash className="h-4 w-4 text-warning" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">
                    {t("lorebookGen.full.defaultEntryCount", { count: targetCount })}
                  </h3>
                </div>
                <div className="flex items-center gap-3">
                  <input
                    type="range"
                    min={MIN_TARGET}
                    max={MAX_TARGET}
                    step={1}
                    value={targetCount}
                    onChange={(e) => void handleTargetCountChange(Number(e.target.value))}
                    className="min-w-0 flex-1 accent-accent"
                  />
                  <NumberInput
                    min={MIN_TARGET}
                    max={MAX_TARGET}
                    step={1}
                    value={targetCount}
                    onChange={(next) => {
                      if (next !== null) void handleTargetCountChange(next);
                    }}
                    className="w-16 shrink-0 rounded-lg border border-fg/10 bg-surface-el/20 px-2 py-2 text-center text-sm tabular-nums focus:border-fg/25 focus:outline-none"
                  />
                </div>
                <p className="px-1 text-xs text-fg/50">
                  {t("lorebookGen.full.entryCountHint", { min: MIN_TARGET, max: MAX_TARGET })}
                </p>
              </div>

              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-info/30 bg-info/10 p-1.5">
                    <Gauge className="h-4 w-4 text-info" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">{t("lorebookGen.full.maxOutputTokens")}</h3>
                </div>
                <NumberInput
                  min={MIN_MAX_TOKENS}
                  max={MAX_MAX_TOKENS}
                  step={128}
                  value={maxTokens}
                  onChange={(next) => {
                    if (next !== null) void handleMaxTokensChange(next);
                  }}
                  className="w-full rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-sm focus:border-fg/25 focus:outline-none"
                />
                <p className="px-1 text-xs text-fg/50">
                  {t("lorebookGen.full.maxTokensHint", { min: MIN_MAX_TOKENS, max: MAX_MAX_TOKENS })}
                </p>
              </div>
            </div>

            <div className="space-y-4">
              <h3 className="px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
                {t("lorebookGen.full.stagePromptsHeading")}
              </h3>

              {stages.map((stage) => (
                <div key={stage.appTemplateId} className="space-y-3">
                  <div className="flex items-center gap-2">
                    <div className="rounded-lg border border-warning/30 bg-warning/10 p-1.5">
                      <BookOpen className="h-4 w-4 text-warning" />
                    </div>
                    <h3 className="text-sm font-semibold text-fg">{t(stage.titleKey)}</h3>
                  </div>
                  <select
                    value={stage.selectedId ?? ""}
                    onChange={(e) => stage.onSelect(e.target.value || null)}
                    className="w-full appearance-none rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-sm text-fg transition focus:border-fg/25 focus:outline-none"
                  >
                    <option value="">{t("lorebookGen.full.useBuiltInDefault")}</option>
                    {templates
                      .filter((t) => t.promptType === stage.promptType)
                      .filter((t) => t.id !== stage.appTemplateId)
                      .map((t) => (
                        <option key={t.id} value={t.id}>
                          {t.name}
                        </option>
                      ))}
                  </select>
                  <p className="px-1 text-xs leading-relaxed text-fg/50">{t(stage.descriptionKey)}</p>
                </div>
              ))}
            </div>
          </div>

          <div className="flex items-start gap-3 rounded-xl border border-fg/10 bg-fg/3 px-4 py-3.5">
            <Info className="mt-0.5 h-4 w-4 shrink-0 text-fg/30" />
            <div className="text-[11px] leading-relaxed text-fg/45">
              <p>{t("lorebookGen.full.footerHint")}</p>
            </div>
          </div>
        </div>
      </main>

      <ModelSelectionBottomMenu
        isOpen={showModelMenu}
        onClose={() => setShowModelMenu(false)}
        title={t("lorebookGen.full.generationModel")}
        models={models}
        selectedModelIds={selectedModelId ? [selectedModelId] : []}
        onSelectModel={(modelId) => {
          void handleModelChange(modelId);
          setShowModelMenu(false);
        }}
        clearOption={{
          label: t("lorebookGen.full.useAppDefault"),
          description: defaultModel ? defaultModel.displayName : t("lorebookGen.full.noAppDefaultModel"),
          icon: Cpu,
          selected: !selectedModelId,
          onClick: () => {
            void handleModelChange(null);
            setShowModelMenu(false);
          },
        }}
      />
    </div>
  );
}
