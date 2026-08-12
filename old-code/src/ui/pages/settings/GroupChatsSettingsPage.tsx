import { useState, useEffect } from "react";
import { ChevronDown, Cpu, Users } from "lucide-react";
import { readSettings, saveAdvancedSettings } from "../../../core/storage/repo";
import type { Model } from "../../../core/storage/schemas";
import { cn } from "../../design-tokens";
import { getProviderIcon } from "../../../core/utils/providerIcons";
import { ModelSelectionBottomMenu } from "../../components/ModelSelectionBottomMenu";
import { useI18n } from "../../../core/i18n/context";

export function GroupChatsSettingsPage() {
  const { t } = useI18n();
  const [isLoading, setIsLoading] = useState(true);
  const [models, setModels] = useState<Model[]>([]);
  const [defaultModelId, setDefaultModelId] = useState<string | null>(null);
  const [selectedModelId, setSelectedModelId] = useState<string | null>(null);
  const [showModelMenu, setShowModelMenu] = useState(false);

  useEffect(() => {
    const loadData = async () => {
      try {
        const settings = await readSettings();
        const textModels = settings.models.filter(
          (m) => !m.outputScopes || m.outputScopes.includes("text"),
        );
        setModels(textModels);
        setDefaultModelId(settings.defaultModelId);
        setSelectedModelId(settings.advancedSettings?.groupSpeakerSelectionModelId ?? null);
        setIsLoading(false);
      } catch (err) {
        console.error("Failed to load settings:", err);
        setIsLoading(false);
      }
    };

    loadData();
  }, []);

  const handleModelChange = async (modelId: string | null) => {
    setSelectedModelId(modelId);
    try {
      const settings = await readSettings();
      const advanced = settings.advancedSettings ?? {
        creationHelperEnabled: false,
        helpMeReplyEnabled: true,
      };
      advanced.groupSpeakerSelectionModelId = modelId ?? undefined;
      await saveAdvancedSettings(advanced);
    } catch (err) {
      console.error("Failed to save group speaker selection model:", err);
    }
  };

  const selectedModel = selectedModelId ? models.find((m) => m.id === selectedModelId) : null;
  const defaultModel = defaultModelId ? models.find((m) => m.id === defaultModelId) : null;
  const selectedModelLabel =
    selectedModel?.displayName || t("groupChatsSettings.labels.selectedModel");
  const appDefaultLabel = t("groupChatsSettings.labels.useAppDefault", {
    model: defaultModel ? ` (${defaultModel.displayName})` : "",
  });

  if (isLoading) {
    return null;
  }

  return (
    <div className="flex min-h-screen flex-col">
      <main className="flex-1 px-4 pb-24 pt-4">
        <div className="mx-auto w-full max-w-5xl space-y-6">
          <div className={cn("rounded-xl border border-accent/20 bg-accent/5 p-3")}>
            <div className="flex items-start gap-2">
              <Users className="h-4 w-4 text-accent shrink-0 mt-0.5" />
              <p className="text-xs text-accent/80 leading-relaxed">
                {t("groupChatsSettings.page.info")}
              </p>
            </div>
          </div>

          <div className="space-y-4">
            <h3 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35 px-1">
              {t("groupChatsSettings.sectionTitles.speakerSelection")}
            </h3>

            <div className="space-y-3">
              <div className="flex items-center gap-2">
                <div className="rounded-lg border border-accent/30 bg-accent/10 p-1.5">
                  <Cpu className="h-4 w-4 text-accent" />
                </div>
                <h3 className="text-sm font-semibold text-fg">
                  {t("groupChatsSettings.labels.speakerModel")}
                </h3>
              </div>

              {models.length > 0 ? (
                <button
                  type="button"
                  onClick={() => setShowModelMenu(true)}
                  className="flex w-full items-center justify-between rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-left transition hover:bg-surface-el/30 focus:border-fg/25 focus:outline-none"
                >
                  <div className="flex items-center gap-2">
                    {selectedModelId ? (
                      getProviderIcon(selectedModel?.providerId || "")
                    ) : (
                      <Cpu className="h-5 w-5 text-fg/40" />
                    )}
                    <span className={`text-sm ${selectedModelId ? "text-fg" : "text-fg/50"}`}>
                      {selectedModelId ? selectedModelLabel : appDefaultLabel}
                    </span>
                  </div>
                  <ChevronDown className="h-4 w-4 text-fg/40" />
                </button>
              ) : (
                <div className="rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                  <p className="text-sm text-fg/50">
                    {t("groupChatsSettings.labels.noModelsAvailable")}
                  </p>
                </div>
              )}
              <p className="text-xs text-fg/50 px-1">
                {t("groupChatsSettings.labels.speakerModelDescription")}
              </p>
            </div>
          </div>
        </div>
      </main>

      <ModelSelectionBottomMenu
        isOpen={showModelMenu}
        onClose={() => setShowModelMenu(false)}
        title={t("groupChatsSettings.labels.selectSpeakerModel")}
        models={models}
        selectedModelIds={selectedModelId ? [selectedModelId] : []}
        searchPlaceholder={t("groupChatsSettings.labels.searchModels")}
        onSelectModel={(modelId) => {
          handleModelChange(modelId);
          setShowModelMenu(false);
        }}
        clearOption={{
          label: t("groupChatsSettings.labels.useAppDefaultBase"),
          description: defaultModel?.displayName,
          icon: Cpu,
          selected: !selectedModelId,
          onClick: () => {
            handleModelChange(null);
            setShowModelMenu(false);
          },
        }}
      />
    </div>
  );
}
