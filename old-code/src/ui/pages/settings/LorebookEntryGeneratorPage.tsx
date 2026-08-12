import { useEffect, useState } from "react";
import { BookOpen, Check, ChevronDown, Code2, Cpu, Info, Sparkles } from "lucide-react";

import type {
  DynamicMemoryStructuredFallbackFormat,
  Model,
  Settings,
  SystemPromptTemplate,
} from "../../../core/storage/schemas";
import { readSettings, saveAdvancedSettings } from "../../../core/storage/repo";
import { listPromptTemplates } from "../../../core/prompts/service";
import {
  APP_LOREBOOK_ENTRY_WRITER_TEMPLATE_ID,
  APP_LOREBOOK_KEYWORD_GENERATOR_TEMPLATE_ID,
} from "../../../core/prompts/constants";
import { getProviderIcon } from "../../../core/utils/providerIcons";
import { cn } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import type { TranslationKey } from "../../../core/i18n/context";
import { ModelSelectionBottomMenu } from "../../components/ModelSelectionBottomMenu";

const FALLBACK_OPTIONS = [
  {
    value: "json",
    titleKey: "lorebookGen.entry.fallbackJsonTitle",
    descriptionKey: "lorebookGen.entry.fallbackJsonDescription",
  },
  {
    value: "xml",
    titleKey: "lorebookGen.entry.fallbackXmlTitle",
    descriptionKey: "lorebookGen.entry.fallbackXmlDescription",
  },
] satisfies Array<{
  value: DynamicMemoryStructuredFallbackFormat;
  titleKey: TranslationKey;
  descriptionKey: TranslationKey;
}>;

function ensureAdvancedSettings(settings: Settings): NonNullable<Settings["advancedSettings"]> {
  const advanced = settings.advancedSettings ?? {
    creationHelperEnabled: false,
    helpMeReplyEnabled: true,
    lorebookEntryGeneratorStructuredFallbackFormat: "json",
  };
  if (advanced.creationHelperEnabled === undefined) advanced.creationHelperEnabled = false;
  if (advanced.helpMeReplyEnabled === undefined) advanced.helpMeReplyEnabled = true;
  if (advanced.lorebookEntryGeneratorStructuredFallbackFormat === undefined) {
    advanced.lorebookEntryGeneratorStructuredFallbackFormat = "json";
  }
  settings.advancedSettings = advanced;
  return advanced;
}

export function LorebookEntryGeneratorPage() {
  const { t } = useI18n();
  const [isLoading, setIsLoading] = useState(true);
  const [models, setModels] = useState<Model[]>([]);
  const [defaultModelId, setDefaultModelId] = useState<string | null>(null);
  const [selectedModelId, setSelectedModelId] = useState<string | null>(null);
  const [fallbackFormat, setFallbackFormat] =
    useState<DynamicMemoryStructuredFallbackFormat>("json");
  const [templates, setTemplates] = useState<SystemPromptTemplate[]>([]);
  const [selectedPromptTemplateId, setSelectedPromptTemplateId] = useState<string | null>(null);
  const [selectedKeywordPromptTemplateId, setSelectedKeywordPromptTemplateId] = useState<
    string | null
  >(null);
  const [showModelMenu, setShowModelMenu] = useState(false);

  const loadData = async () => {
    try {
      const [settings, promptTemplates] = await Promise.all([readSettings(), listPromptTemplates()]);
      const advanced = ensureAdvancedSettings(settings);
      const textModels = settings.models.filter(
        (m) => !m.outputScopes || m.outputScopes.includes("text"),
      );
      setModels(textModels);
      setDefaultModelId(settings.defaultModelId ?? null);
      setSelectedModelId(advanced.lorebookEntryGeneratorModelId ?? null);
      setFallbackFormat(advanced.lorebookEntryGeneratorStructuredFallbackFormat ?? "json");
      setSelectedPromptTemplateId(advanced.lorebookEntryGeneratorPromptTemplateId ?? null);
      setSelectedKeywordPromptTemplateId(
        advanced.lorebookKeywordGeneratorPromptTemplateId ?? null,
      );
      setTemplates(promptTemplates);
    } catch (error) {
      console.error("Failed to load lorebook entry generator settings:", error);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    void loadData();
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
    } catch (error) {
      console.error(errorMessage, error);
    }
  };

  const selectedModel = selectedModelId
    ? models.find((model) => model.id === selectedModelId) ?? null
    : null;
  const defaultModel = defaultModelId
    ? models.find((model) => model.id === defaultModelId) ?? null
    : null;
  const selectedModelLabel = selectedModel?.displayName ?? t("lorebookGen.entry.generationModelFallback");
  const appDefaultLabel = defaultModel
    ? t("lorebookGen.entry.useAppDefaultWith", { name: defaultModel.displayName })
    : t("lorebookGen.entry.useAppDefault");

  const handleModelChange = async (modelId: string | null) => {
    setSelectedModelId(modelId);
    await updateAdvancedSettings((advanced) => {
      advanced.lorebookEntryGeneratorModelId = modelId ?? undefined;
    }, "Failed to save lorebook entry generator model:");
  };

  const handleFallbackChange = async (value: DynamicMemoryStructuredFallbackFormat) => {
    setFallbackFormat(value);
    await updateAdvancedSettings((advanced) => {
      advanced.lorebookEntryGeneratorStructuredFallbackFormat = value;
    }, "Failed to save lorebook entry generator fallback format:");
  };

  const handlePromptSelection = async (templateId: string | null) => {
    setSelectedPromptTemplateId(templateId);
    await updateAdvancedSettings((advanced) => {
      advanced.lorebookEntryGeneratorPromptTemplateId = templateId ?? undefined;
    }, "Failed to save lorebook entry generator prompt:");
  };

  const handleKeywordPromptSelection = async (templateId: string | null) => {
    setSelectedKeywordPromptTemplateId(templateId);
    await updateAdvancedSettings((advanced) => {
      advanced.lorebookKeywordGeneratorPromptTemplateId = templateId ?? undefined;
    }, "Failed to save lorebook keyword generator prompt:");
  };

  if (isLoading) return null;

  return (
    <div className="flex min-h-screen flex-col">
      <main className="flex-1 px-4 pb-24 pt-4">
        <div className="mx-auto w-full max-w-5xl space-y-6">
          {/* Info Card */}
          <div className="rounded-xl border border-accent/20 bg-accent/5 p-3">
            <div className="flex items-start gap-2">
              <Sparkles className="mt-0.5 h-4 w-4 shrink-0 text-accent" />
              <p className="text-xs leading-relaxed text-accent/80">
                {t("lorebookGen.entry.infoText", { format: fallbackFormat.toUpperCase() })}
              </p>
            </div>
          </div>

          {/* Two-column on desktop, single on mobile */}
          <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
            {/* Left — Generation */}
            <div className="space-y-4">
              <h3 className="px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
                {t("lorebookGen.entry.generationHeading")}
              </h3>

              {/* Model */}
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-accent/30 bg-accent/10 p-1.5">
                    <Cpu className="h-4 w-4 text-accent" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">{t("lorebookGen.entry.generationModel")}</h3>
                </div>

                {models.length > 0 ? (
                  <button
                    type="button"
                    onClick={() => setShowModelMenu(true)}
                    className="flex w-full items-center justify-between rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-left transition hover:bg-surface-el/30 focus:border-fg/25 focus:outline-none"
                  >
                    <div className="flex items-center gap-2 min-w-0">
                      {selectedModelId ? (
                        getProviderIcon(selectedModel?.providerId ?? "")
                      ) : (
                        <Cpu className="h-5 w-5 shrink-0 text-fg/40" />
                      )}
                      <span
                        className={`truncate text-sm ${selectedModelId ? "text-fg" : "text-fg/50"}`}
                      >
                        {selectedModelId ? selectedModelLabel : appDefaultLabel}
                      </span>
                    </div>
                    <ChevronDown className="h-4 w-4 shrink-0 text-fg/40" />
                  </button>
                ) : (
                  <div className="rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                    <p className="text-sm text-fg/50">{t("lorebookGen.entry.noTextModels")}</p>
                  </div>
                )}
                <p className="px-1 text-xs text-fg/50">
                  {t("lorebookGen.entry.modelHint")}
                </p>
              </div>

              {/* Structured Fallback */}
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-info/30 bg-info/10 p-1.5">
                    <Code2 className="h-4 w-4 text-info" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">{t("lorebookGen.entry.structuredFallback")}</h3>
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
                <p className="px-1 text-xs text-fg/50">
                  {t("lorebookGen.entry.fallbackHint")}
                </p>
              </div>
            </div>

            {/* Right — Prompt Template */}
            <div className="space-y-4">
              <h3 className="px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
                {t("lorebookGen.entry.promptTemplateHeading")}
              </h3>

              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-warning/30 bg-warning/10 p-1.5">
                    <BookOpen className="h-4 w-4 text-warning" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">{t("lorebookGen.entry.entryWriterPrompt")}</h3>
                </div>

                <select
                  value={selectedPromptTemplateId ?? ""}
                  onChange={(e) => void handlePromptSelection(e.target.value || null)}
                  className="w-full appearance-none rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-sm text-fg transition focus:border-fg/25 focus:outline-none"
                >
                  <option value="">{t("lorebookGen.entry.useBuiltInDefault")}</option>
                  {templates
                    .filter((template) => template.promptType === "lorebookEntryWriter")
                    .filter((template) => template.id !== APP_LOREBOOK_ENTRY_WRITER_TEMPLATE_ID)
                    .map((template) => (
                      <option key={template.id} value={template.id}>
                        {template.name}
                      </option>
                    ))}
                </select>

                <p className="px-1 text-xs leading-relaxed text-fg/50">
                  {t("lorebookGen.entry.entryWriterHint")}
                </p>
              </div>

              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-warning/30 bg-warning/10 p-1.5">
                    <BookOpen className="h-4 w-4 text-warning" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">{t("lorebookGen.entry.keywordGeneratorPrompt")}</h3>
                </div>

                <select
                  value={selectedKeywordPromptTemplateId ?? ""}
                  onChange={(e) => void handleKeywordPromptSelection(e.target.value || null)}
                  className="w-full appearance-none rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-sm text-fg transition focus:border-fg/25 focus:outline-none"
                >
                  <option value="">{t("lorebookGen.entry.useBuiltInDefault")}</option>
                  {templates
                    .filter((template) => template.promptType === "lorebookKeywordGenerator")
                    .filter((template) => template.id !== APP_LOREBOOK_KEYWORD_GENERATOR_TEMPLATE_ID)
                    .map((template) => (
                      <option key={template.id} value={template.id}>
                        {template.name}
                      </option>
                    ))}
                </select>

                <p className="px-1 text-xs leading-relaxed text-fg/50">
                  {t("lorebookGen.entry.keywordGeneratorHint")}
                </p>
              </div>
            </div>
          </div>

          {/* Footer hint */}
          <div className="flex items-start gap-3 rounded-xl border border-fg/10 bg-fg/[0.03] px-4 py-3.5">
            <Info className="mt-0.5 h-4 w-4 shrink-0 text-fg/30" />
            <div className="text-[11px] leading-relaxed text-fg/45">
              <p>{t("lorebookGen.entry.footerHint")}</p>
            </div>
          </div>
        </div>
      </main>

      <ModelSelectionBottomMenu
        isOpen={showModelMenu}
        onClose={() => setShowModelMenu(false)}
        title={t("lorebookGen.entry.generationModel")}
        models={models}
        selectedModelIds={selectedModelId ? [selectedModelId] : []}
        onSelectModel={(modelId) => {
          void handleModelChange(modelId);
          setShowModelMenu(false);
        }}
        clearOption={{
          label: t("lorebookGen.entry.useAppDefault"),
          description: defaultModel ? defaultModel.displayName : t("lorebookGen.entry.noAppDefaultModel"),
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
