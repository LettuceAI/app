import { useState, useEffect, useMemo } from "react";
import { Sparkles, Cpu, Image, Zap, MessageSquare, ChevronDown } from "lucide-react";
import { readSettings, saveAdvancedSettings } from "../../../core/storage/repo";
import type { Model } from "../../../core/storage/schemas";
import { cn } from "../../design-tokens";
import { getProviderIcon } from "../../../core/utils/providerIcons";
import { ModelSelectionBottomMenu } from "../../components/ModelSelectionBottomMenu";
import { useI18n } from "../../../core/i18n/context";
import { isImageGenerationModelAvailable } from "../../../core/image-generation";

export function CreationHelperPage() {
  const { t } = useI18n();
  const [isLoading, setIsLoading] = useState(true);
  const [models, setModels] = useState<Model[]>([]);
  const [defaultModelId, setDefaultModelId] = useState<string | null>(null);

  // Settings state
  const [selectedModelId, setSelectedModelId] = useState<string | null>(null);
  const [streamingEnabled, setStreamingEnabled] = useState(true);
  const [imageModelId, setImageModelId] = useState<string | null>(null);

  // Menu states
  const [showModelMenu, setShowModelMenu] = useState(false);
  const [showImageModelMenu, setShowImageModelMenu] = useState(false);

  useEffect(() => {
    const loadData = async () => {
      try {
        const settings = await readSettings();
        setModels(settings.models);
        setDefaultModelId(settings.defaultModelId);
        setSelectedModelId(settings.advancedSettings?.creationHelperModelId ?? null);
        setStreamingEnabled(settings.advancedSettings?.creationHelperStreaming ?? true);
        setImageModelId(settings.advancedSettings?.creationHelperImageModelId ?? null);
        setIsLoading(false);
      } catch (err) {
        console.error("Failed to load settings:", err);
        setIsLoading(false);
      }
    };

    loadData();
  }, []);

  const saveSettings = async (
    updates: Partial<{
      creationHelperModelId: string | undefined;
      creationHelperStreaming: boolean;
      creationHelperImageModelId: string | undefined;
    }>,
  ) => {
    try {
      const settings = await readSettings();
      const advanced = settings.advancedSettings ?? {
        creationHelperEnabled: false,
        helpMeReplyEnabled: true,
      };
      Object.assign(advanced, updates);
      await saveAdvancedSettings(advanced);
    } catch (err) {
      console.error("Failed to save settings:", err);
    }
  };

  const handleModelChange = async (modelId: string | null) => {
    setSelectedModelId(modelId);
    await saveSettings({ creationHelperModelId: modelId ?? undefined });
  };

  const handleStreamingToggle = async () => {
    const newValue = !streamingEnabled;
    setStreamingEnabled(newValue);
    await saveSettings({ creationHelperStreaming: newValue });
  };

  const handleImageModelChange = async (modelId: string | null) => {
    setImageModelId(modelId);
    await saveSettings({ creationHelperImageModelId: modelId ?? undefined });
  };

  const textModels = useMemo(
    () => models.filter((m) => !m.outputScopes || m.outputScopes.includes("text")),
    [models],
  );

  const imageModels = useMemo(() => models.filter(isImageGenerationModelAvailable), [models]);

  const selectedModel = selectedModelId ? models.find((m) => m.id === selectedModelId) : null;
  const defaultModel = defaultModelId ? models.find((m) => m.id === defaultModelId) : null;
  const selectedImageModel = imageModelId
    ? (imageModels.find((m) => m.id === imageModelId) ?? null)
    : null;
  const selectedModelLabel = selectedModel?.displayName || t("creationHelper.page.selectedModel");
  const chatDefaultLabel = t("creationHelper.page.useAppDefault", {
    model: defaultModel ? ` (${defaultModel.displayName})` : "",
  });

  if (isLoading) {
    return null;
  }

  return (
    <div className="flex min-h-screen flex-col">
      <main className="flex-1 px-4 pb-24 pt-4">
        <div className="mx-auto w-full max-w-5xl space-y-6">
          {/* Info Card */}
          <div className={cn("rounded-xl border border-danger/20 bg-danger/5 p-3")}>
            <div className="flex items-start gap-2">
              <Sparkles className="h-4 w-4 text-danger shrink-0 mt-0.5" />
              <p className="text-xs text-danger/80 leading-relaxed">
                {t("creationHelper.page.info")}
              </p>
            </div>
          </div>

          <div className="grid grid-cols-1 gap-6">
            <div className="space-y-4">
              <h3 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35 px-1">
                {t("creationHelper.page.modelConfiguration")}
              </h3>

              {/* Chat Model Selector */}
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-danger/30 bg-danger/10 p-1.5">
                    <MessageSquare className="h-4 w-4 text-danger" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">
                    {t("creationHelper.page.chatModel")}
                  </h3>
                </div>

                {textModels.length > 0 ? (
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
                        {selectedModelId ? selectedModelLabel : chatDefaultLabel}
                      </span>
                    </div>
                    <ChevronDown className="h-4 w-4 text-fg/40" />
                  </button>
                ) : (
                  <div className="rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                    <p className="text-sm text-fg/50">
                      {t("creationHelper.page.noModelsAvailable")}
                    </p>
                  </div>
                )}
                <p className="text-xs text-fg/50 px-1">
                  {t("creationHelper.page.chatModelDescription")}
                </p>
              </div>

              {/* Streaming Toggle */}
              <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-3">
                    <div className="rounded-lg border border-accent/30 bg-accent/10 p-1.5">
                      <Zap className="h-4 w-4 text-accent/80" />
                    </div>
                    <div>
                      <span className="text-sm font-medium text-fg">
                        {t("creationHelper.page.streamingOutput")}
                      </span>
                      <p className="text-[11px] text-fg/45">
                        {t("creationHelper.page.streamingDescription")}
                      </p>
                    </div>
                  </div>
                  <label className="relative inline-flex items-center cursor-pointer">
                    <input
                      type="checkbox"
                      checked={streamingEnabled}
                      onChange={handleStreamingToggle}
                      className="sr-only peer"
                    />
                    <div
                      className={cn(
                        "w-9 h-5 rounded-full transition-colors",
                        streamingEnabled ? "bg-accent" : "bg-fg/20",
                      )}
                    >
                      <div
                        className={cn(
                          "absolute top-0.5 left-0.5 w-4 h-4 bg-fg rounded-full transition-transform shadow-sm",
                          streamingEnabled && "translate-x-4",
                        )}
                      />
                    </div>
                  </label>
                </div>
              </div>

              {/* Image Model Selector */}
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-warning/30 bg-warning/10 p-1.5">
                    <Image className="h-4 w-4 text-warning/80" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">
                    {t("creationHelper.page.imageGenerationModel")}
                  </h3>
                </div>

                {imageModels.length > 0 ? (
                  <button
                    type="button"
                    onClick={() => setShowImageModelMenu(true)}
                    className="flex w-full items-center justify-between rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-left transition hover:bg-surface-el/30 focus:border-fg/25 focus:outline-none"
                  >
                    <div className="flex items-center gap-2">
                      {imageModelId ? (
                        getProviderIcon(selectedImageModel?.providerId || "")
                      ) : (
                        <Image className="h-5 w-5 text-fg/40" />
                      )}
                      <span className={`text-sm ${imageModelId ? "text-fg" : "text-fg/50"}`}>
                        {imageModelId
                          ? selectedImageModel?.displayName ||
                            t("creationHelper.page.selectedModel")
                          : t("creationHelper.page.noModelSelected")}
                      </span>
                    </div>
                    <ChevronDown className="h-4 w-4 text-fg/40" />
                  </button>
                ) : (
                  <div className="rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                    <p className="text-sm text-fg/50">
                      {t("creationHelper.page.noImageModelsAvailable")}
                    </p>
                  </div>
                )}
                <p className="text-xs text-fg/50 px-1">
                  {t("creationHelper.page.imageModelDescription")}
                </p>
              </div>
            </div>
          </div>
        </div>
      </main>

      <ModelSelectionBottomMenu
        isOpen={showModelMenu}
        onClose={() => setShowModelMenu(false)}
        title={t("creationHelper.page.selectChatModel")}
        models={textModels}
        selectedModelIds={selectedModelId ? [selectedModelId] : []}
        searchPlaceholder={t("creationHelper.page.searchModels")}
        onSelectModel={(modelId) => {
          handleModelChange(modelId);
          setShowModelMenu(false);
        }}
        clearOption={{
          label: t("creationHelper.page.useAppDefaultBase"),
          description: defaultModel?.displayName,
          icon: Cpu,
          selected: !selectedModelId,
          onClick: () => {
            handleModelChange(null);
            setShowModelMenu(false);
          },
        }}
      />

      <ModelSelectionBottomMenu
        isOpen={showImageModelMenu}
        onClose={() => setShowImageModelMenu(false)}
        title={t("creationHelper.page.selectImageModel")}
        models={imageModels}
        selectedModelIds={imageModelId ? [imageModelId] : []}
        searchPlaceholder={t("creationHelper.page.searchModels")}
        onSelectModel={(modelId) => {
          handleImageModelChange(modelId);
          setShowImageModelMenu(false);
        }}
        clearOption={{
          label: t("creationHelper.page.noModelSelected"),
          icon: Image,
          selected: !imageModelId,
          onClick: () => {
            handleImageModelChange(null);
            setShowImageModelMenu(false);
          },
        }}
      />
    </div>
  );
}
