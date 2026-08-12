import {
  Trash2,
  ChevronRight,
  Edit3,
  EthernetPort,
  Cpu,
  Volume2,
  Mic,
  Leaf,
  Sparkles,
  LayoutDashboard,
  AlertTriangle,
  Loader2,
  Download,
  ExternalLink,
} from "lucide-react";
import { useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from "react";
import { openExternalUrl } from "../../../core/utils/openExternal";
import { useSearchParams, useNavigate } from "react-router-dom";

import { useProvidersPageController } from "./hooks/useProvidersPageController";
import { AudioProviderEditor } from "./components/AudioProviderEditor";
import { TextField, SelectField, ToggleRow } from "./components/Field";

import type { ProviderCapabilitiesCamel } from "../../../core/providers/capabilities";
import { getProviderIcon } from "../../../core/utils/providerIcons";
import { Routes } from "../../navigation";
import {
  listAudioProviders,
  upsertAudioProvider,
  deleteAudioProvider,
  kokoroValidateAssets,
  type AudioProvider,
  type AudioProviderType,
} from "../../../core/storage/audioProviders";
import { KokoroSetupMenu } from "./components/KokoroSetupMenu";
import { InlineDownloadCards } from "./components/DownloadQueueBar";
import { useDownloadQueue } from "../../../core/downloads/DownloadQueueContext";

import { BottomMenu, MenuButton } from "../../components/BottomMenu";
import { cn, colors, interactive, radius } from "../../design-tokens";
import { getPlatform } from "../../../core/utils/platform";
import { useI18n } from "../../../core/i18n/context";
import { Switch } from "../../components/Switch";

const AUDIO_PROVIDER_TYPE_LABEL: Record<AudioProviderType, string> = {
  elevenlabs: "ElevenLabs",
  fish_tts: "fishTts",
  fish_speech: "fishSpeech",
  gemini_tts: "geminiTts",
  openai_tts: "openaiTts",
  kokoro: "kokoro",
};

const AUDIO_PROVIDER_TYPE_TRANSLATION_KEY: Record<
  Exclude<AudioProviderType, "elevenlabs">,
  | "providers.extra.audioProviderLabel.fishTts"
  | "providers.extra.audioProviderLabel.fishSpeech"
  | "providers.extra.audioProviderLabel.geminiTts"
  | "providers.extra.audioProviderLabel.openaiTts"
  | "providers.extra.audioProviderLabel.kokoro"
> = {
  fish_tts: "providers.extra.audioProviderLabel.fishTts",
  fish_speech: "providers.extra.audioProviderLabel.fishSpeech",
  gemini_tts: "providers.extra.audioProviderLabel.geminiTts",
  openai_tts: "providers.extra.audioProviderLabel.openaiTts",
  kokoro: "providers.extra.audioProviderLabel.kokoro",
};

type ProviderTab = "llm" | "audio";

function ResolvedEndpointPreview({
  baseUrl,
  endpoint,
  duplicateWarning,
}: {
  baseUrl: string;
  endpoint: string;
  duplicateWarning: (segment: string) => string;
}) {
  const { t } = useI18n();
  const base = baseUrl.trim().replace(/\/+$/, "");
  const rawEndpoint = endpoint.trim();
  const isAbsolute = /^https?:\/\//.test(rawEndpoint);
  if (!rawEndpoint || (!base && !isAbsolute)) return null;
  const resolved = isAbsolute
    ? rawEndpoint
    : `${base}${rawEndpoint.startsWith("/") ? "" : "/"}${rawEndpoint}`;
  const baseTail = base.split("/").filter(Boolean).pop();
  const endpointHead = rawEndpoint
    .replace(/^https?:\/\/[^/]+/, "")
    .split("/")
    .filter(Boolean)[0];
  const duplicated = !!baseTail && !!endpointHead && baseTail === endpointHead && !isAbsolute;
  return (
    <div className="-mt-2 space-y-1 px-1">
      <p className="break-all font-mono text-[11px] text-fg/40">
        {t("providers.editor.resolvedChatUrl")}: {resolved}
      </p>
      {duplicated && baseTail && (
        <p className="text-[11px] text-amber-400/90">{duplicateWarning(baseTail)}</p>
      )}
    </div>
  );
}

export function ProvidersPage() {
  const { t } = useI18n();
  const isMobile = getPlatform().type === "mobile";
  const [searchParams, setSearchParams] = useSearchParams();
  const [activeTab, setActiveTab] = useState<ProviderTab>(() => {
    const tab = searchParams.get("tab");
    return tab === "audio" ? "audio" : "llm";
  });
  const tabsId = useId();
  const llmTabId = `${tabsId}-llm-tab`;
  const audioTabId = `${tabsId}-audio-tab`;
  const llmPanelId = `${tabsId}-llm-panel`;
  const audioPanelId = `${tabsId}-audio-panel`;
  const getAudioProviderTypeLabel = useCallback(
    (providerType: AudioProviderType) =>
      providerType === "elevenlabs"
        ? AUDIO_PROVIDER_TYPE_LABEL[providerType]
        : t(AUDIO_PROVIDER_TYPE_TRANSLATION_KEY[providerType]),
    [t],
  );
  const navigate = useNavigate();
  const {
    state: {
      providers,
      selectedProvider,
      isEditorOpen,
      editorProvider,
      apiKey,
      isSaving,
      isDeleting,
      validationError,
      capabilities,
      engineSetupResult,
      loading,
    },
    openEditor,
    closeEditor,
    setSelectedProvider,
    setApiKey,
    setValidationError,
    updateEditorProvider,
    handleSaveProvider,
    handleDeleteProvider,
    dismissEngineSetup,
  } = useProvidersPageController();

  useLayoutEffect(() => {
    const tab = searchParams.get("tab");
    const nextTab = tab === "audio" ? "audio" : "llm";
    setActiveTab((prev) => (prev === nextTab ? prev : nextTab));
  }, [searchParams]);

  const handleTabChange = (tab: ProviderTab) => {
    setActiveTab(tab);
    const nextParams = new URLSearchParams(searchParams);
    nextParams.set("tab", tab);
    setSearchParams(nextParams, { replace: true });
  };

  const isEngineProvider = !!editorProvider && editorProvider.providerId === "lettuce-engine";
  const isHostProvider = !!editorProvider && editorProvider.providerId === "lettuce-host";
  const isLocalProvider =
    !!editorProvider &&
    ["ollama", "lmstudio", "intenserp", "automatic1111", "comfyui", "diffusers"].includes(
      editorProvider.providerId,
    );
  const isComfyUiProvider = !!editorProvider && editorProvider.providerId === "comfyui";
  const isCustomProvider =
    !!editorProvider &&
    (editorProvider.providerId === "custom" || editorProvider.providerId === "custom-anthropic");
  const allowsTlsException = !!editorProvider && (isLocalProvider || isCustomProvider);
  const showBaseUrl =
    !!editorProvider && (isLocalProvider || isCustomProvider || isEngineProvider || isHostProvider);
  const isOllamaProvider = !!editorProvider && editorProvider.providerId === "ollama";
  const customConfig = (editorProvider?.config ?? {}) as Record<string, any>;
  const customFetchModelsEnabled = customConfig.fetchModelsEnabled === true;
  const providerStreamingEnabled = customConfig.streamingEnabled !== false;
  const providerAllowInvalidTls = customConfig.allowInvalidTls === true;
  const sproutEnabled = customConfig.sproutEnabled === true;
  const customAuthMode = (customConfig.authMode ?? "header") as
    | "bearer"
    | "header"
    | "query"
    | "none";
  const selectedCapability = editorProvider
    ? capabilities.find((provider) => provider.id === editorProvider.providerId)
    : null;
  const providerRequiresApiKey = isCustomProvider
    ? customAuthMode !== "none"
    : selectedCapability
      ? selectedCapability.requiresApiKey
      : true;
  const showApiKeyInput = providerRequiresApiKey && !isEngineProvider;
  const showOfficialProviderStreamingToggle =
    !!editorProvider && !isCustomProvider && selectedCapability?.supportsStream === true;
  const visibleCapabilities = isMobile
    ? capabilities.filter((provider) => provider.id !== "llamacpp")
    : capabilities;

  const [audioProviders, setAudioProviders] = useState<AudioProvider[]>([]);
  const [audioLoading, setAudioLoading] = useState(true);
  const [selectedAudioProvider, setSelectedAudioProvider] = useState<AudioProvider | null>(null);
  const [isAudioEditorOpen, setIsAudioEditorOpen] = useState(false);
  const [editingAudioProvider, setEditingAudioProvider] = useState<AudioProvider | null>(null);
  const [isAudioDeleting, setIsAudioDeleting] = useState(false);
  const [kokoroSetupProvider, setKokoroSetupProvider] = useState<AudioProvider | null>(null);
  const [kokoroInstalled, setKokoroInstalled] = useState<Record<string, boolean>>({});
  const { queue: downloadQueue } = useDownloadQueue();

  const loadAudioProviders = useCallback(async () => {
    setAudioLoading(true);
    try {
      const list = await listAudioProviders();
      setAudioProviders(list);
    } catch (e) {
      console.error("Failed to load audio providers:", e);
    } finally {
      setAudioLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadAudioProviders();
  }, [loadAudioProviders]);

  const refreshKokoroStatus = useCallback(async () => {
    const kokoro = audioProviders.filter((p) => p.providerType === "kokoro");
    if (kokoro.length === 0) return;
    const entries = await Promise.all(
      kokoro.map(async (p) => {
        const root = p.assetRoot?.trim();
        if (!root || !p.kokoroVariant) return [p.id, false] as const;
        try {
          const status = await kokoroValidateAssets(root, p.kokoroVariant);
          return [p.id, Boolean(status.resolvedModelPath)] as const;
        } catch {
          return [p.id, false] as const;
        }
      }),
    );
    setKokoroInstalled((prev) => ({ ...prev, ...Object.fromEntries(entries) }));
  }, [audioProviders]);

  useEffect(() => {
    void refreshKokoroStatus();
  }, [refreshKokoroStatus]);

  const kokoroDownloadingRoots = useMemo(() => {
    const roots = new Set<string>();
    for (const item of downloadQueue) {
      if (
        item.queueKind === "kokoro" &&
        item.assetRoot &&
        (item.status === "downloading" || item.status === "queued")
      ) {
        roots.add(item.assetRoot);
      }
    }
    return roots;
  }, [downloadQueue]);

  const activeKokoroInstallIds = useMemo(() => {
    const ids = new Set<string>();
    for (const item of downloadQueue) {
      if (
        item.queueKind === "kokoro" &&
        item.installId &&
        (item.status === "downloading" || item.status === "queued")
      ) {
        ids.add(item.installId);
      }
    }
    return ids;
  }, [downloadQueue]);

  const prevDownloadingCount = useRef(0);
  useEffect(() => {
    if (prevDownloadingCount.current > 0 && kokoroDownloadingRoots.size === 0) {
      void refreshKokoroStatus();
    }
    prevDownloadingCount.current = kokoroDownloadingRoots.size;
  }, [kokoroDownloadingRoots, refreshKokoroStatus]);

  const openAudioEditor = useCallback((provider?: AudioProvider) => {
    setEditingAudioProvider(
      provider
        ? { ...provider }
        : {
            id: "",
            providerType: "elevenlabs",
            label: "",
            apiKey: "",
            requestPath: "/v1/audio/speech",
          },
    );
    setIsAudioEditorOpen(true);
    setSelectedAudioProvider(null);
  }, []);

  const closeAudioEditor = useCallback(() => {
    setIsAudioEditorOpen(false);
    setEditingAudioProvider(null);
  }, []);

  const handleSaveAudioProvider = useCallback(
    async (provider: AudioProvider) => {
      await upsertAudioProvider(provider);
      await loadAudioProviders();
      closeAudioEditor();
    },
    [loadAudioProviders, closeAudioEditor],
  );

  const handleDeleteAudioProvider = useCallback(
    async (id: string) => {
      setIsAudioDeleting(true);
      try {
        await deleteAudioProvider(id);
        await loadAudioProviders();
        setSelectedAudioProvider(null);
      } catch (e) {
        console.error("Failed to delete audio provider:", e);
      } finally {
        setIsAudioDeleting(false);
      }
    },
    [loadAudioProviders],
  );

  useEffect(() => {
    const handleAddProvider = () => {
      if (activeTab === "audio") {
        openAudioEditor();
        return;
      }
      openEditor();
    };

    (window as any).__openAddProvider = handleAddProvider;
    const listener = () => handleAddProvider();
    window.addEventListener("providers:add", listener);
    return () => {
      if ((window as any).__openAddProvider) delete (window as any).__openAddProvider;
      window.removeEventListener("providers:add", listener);
    };
  }, [activeTab, openEditor, openAudioEditor]);

  const EmptyState = ({ onCreate }: { onCreate: () => void }) => (
    <div className="flex h-64 flex-col items-center justify-center lg:col-span-2">
      <EthernetPort className="mb-3 h-12 w-12 text-fg/20" />
      <h3 className="mb-1 text-lg font-medium text-fg">{t("providers.empty.title")}</h3>
      <p className="mb-4 text-center text-sm text-fg/50">{t("providers.empty.description")}</p>
      <button
        onClick={onCreate}
        className="rounded-full border border-accent/40 bg-accent/20 px-6 py-2 text-sm font-medium text-accent/90 transition hover:bg-accent/30 active:scale-[0.99]"
      >
        {t("providers.empty.addButton")}
      </button>
    </div>
  );

  const AudioEmptyState = ({ onCreate }: { onCreate: () => void }) => (
    <div className="flex h-64 flex-col items-center justify-center lg:col-span-2">
      <Mic className="mb-3 h-12 w-12 text-fg/20" />
      <h3 className="mb-1 text-lg font-medium text-fg">
        {t("providers.extra.audioEmpty.title")}
      </h3>
      <p className="mb-4 text-center text-sm text-fg/50">
        {t("providers.extra.audioEmpty.description")}
      </p>
      <button
        onClick={onCreate}
        className="rounded-full border border-accent/40 bg-accent/20 px-6 py-2 text-sm font-medium text-accent/90 transition hover:bg-accent/30 active:scale-[0.99]"
      >
        {t("providers.extra.audioEmpty.addButton")}
      </button>
    </div>
  );

  return (
    <div className="flex h-full flex-col">
      <div className="flex-1 overflow-y-auto px-3 py-3 pb-[calc(env(safe-area-inset-bottom)+96px)] lg:px-8 lg:pt-8 lg:pb-10">
        <div className="mx-auto w-full max-w-5xl">
        {activeTab === "llm" ? (
          <div
            role="tabpanel"
            id={llmPanelId}
            aria-labelledby={llmTabId}
            tabIndex={0}
            className="grid grid-cols-1 gap-2 lg:grid-cols-2 lg:gap-3"
          >
            {!loading && providers.length === 0 && <EmptyState onCreate={() => openEditor()} />}
            {providers.map((provider) => {
              const cap: ProviderCapabilitiesCamel | undefined = capabilities.find(
                (p) => p.id === provider.providerId,
              );
              return (
                <button
                  key={provider.id}
                  onClick={() => setSelectedProvider(provider)}
                  className="group w-full rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-left transition hover:border-fg/20 hover:bg-fg/10 focus:outline-none focus:ring-2 focus:ring-fg/20 active:scale-[0.99]"
                >
                  <div className="flex items-center gap-3">
                    {getProviderIcon(cap?.id ?? "custom")}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="truncate text-sm font-medium text-fg">
                          {provider.label || cap?.name}
                        </span>
                      </div>
                      <div className="mt-0.5 flex items-center gap-1 text-[11px] text-fg/50">
                        <span className="truncate">{cap?.name}</span>
                        {provider.baseUrl && (
                          <>
                            <span className="opacity-40">•</span>
                            <span className="truncate max-w-30">
                              {provider.baseUrl.replace(/^https?:\/\//, "")}
                            </span>
                          </>
                        )}
                      </div>
                    </div>
                    <ChevronRight className="h-4 w-4 text-fg/30 group-hover:text-fg/60 transition" />
                  </div>
                </button>
              );
            })}
          </div>
        ) : (
          <div
            role="tabpanel"
            id={audioPanelId}
            aria-labelledby={audioTabId}
            tabIndex={0}
            className="grid grid-cols-1 gap-2 lg:grid-cols-2 lg:gap-3"
          >
            {activeKokoroInstallIds.size > 0 && (
              <div className="lg:col-span-2">
                <InlineDownloadCards
                  compact
                  filter={(item) =>
                    item.queueKind === "kokoro" &&
                    !!item.installId &&
                    activeKokoroInstallIds.has(item.installId)
                  }
                />
              </div>
            )}
            {!audioLoading && audioProviders.length === 0 && (
              <AudioEmptyState onCreate={() => openAudioEditor()} />
            )}
            {audioProviders.map((provider) => {
              const isKokoro = provider.providerType === "kokoro";
              const isDownloading =
                isKokoro && !!provider.assetRoot && kokoroDownloadingRoots.has(provider.assetRoot);
              const needsSetup =
                isKokoro && !isDownloading && kokoroInstalled[provider.id] === false;
              return (
                <button
                  key={provider.id}
                  onClick={() => {
                    if (needsSetup) setKokoroSetupProvider(provider);
                    else setSelectedAudioProvider(provider);
                  }}
                  className={cn(
                    "group w-full rounded-xl border px-4 py-3 text-left transition focus:outline-none focus:ring-2 focus:ring-fg/20 active:scale-[0.99]",
                    needsSetup
                      ? "border-warning/30 bg-warning/[0.06] hover:border-warning/45 hover:bg-warning/10"
                      : "border-fg/10 bg-fg/5 hover:border-fg/20 hover:bg-fg/10",
                  )}
                >
                  <div className="flex items-center gap-3">
                    {getProviderIcon(provider.providerType)}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="truncate text-sm font-medium text-fg">
                          {provider.label}
                        </span>
                      </div>
                      <div className="mt-0.5 flex items-center gap-1 text-[11px] text-fg/50">
                        {isDownloading ? (
                          <span className="flex items-center gap-1 text-info/80">
                            <Loader2 className="h-3 w-3 animate-spin" />
                            {t("providers.list.downloading")}
                          </span>
                        ) : needsSetup ? (
                          <span className="flex items-center gap-1 font-medium text-warning/90">
                            <AlertTriangle className="h-3 w-3" />
                            {t("providers.list.notSetUp")}
                          </span>
                        ) : (
                          <>
                            <span className="truncate">
                              {getAudioProviderTypeLabel(provider.providerType)}
                            </span>
                            {provider.baseUrl && (
                              <>
                                <span className="opacity-40">•</span>
                                <span className="truncate max-w-30">
                                  {provider.baseUrl.replace(/^https?:\/\//, "")}
                                </span>
                              </>
                            )}
                          </>
                        )}
                      </div>
                    </div>
                    {needsSetup ? (
                      <Download className="h-4 w-4 text-warning/70 group-hover:text-warning transition" />
                    ) : (
                      <ChevronRight className="h-4 w-4 text-fg/30 group-hover:text-fg/60 transition" />
                    )}
                  </div>
                </button>
              );
            })}
            {audioProviders.length > 0 && (
              <p className="px-1 pt-3 text-[11px] text-fg/40 lg:col-span-2">
                {t("providers.list.manageVoicesPrefix")}{" "}
                <button
                  onClick={() => navigate("/settings/voices")}
                  className="text-fg/60 underline-offset-2 hover:text-fg hover:underline"
                >
                  {t("providers.list.voicesLink")}
                </button>
                .
              </p>
            )}
          </div>
        )}
        </div>
      </div>

      {activeTab === "llm" && (
        <>
          <BottomMenu
            isOpen={!!selectedProvider}
            onClose={() => setSelectedProvider(null)}
            title={selectedProvider?.label || t("providers.fallbackTitle")}
          >
            {selectedProvider && (
              <div className="space-y-4">
                <div className="rounded-lg border border-fg/10 bg-fg/5 px-3 py-2">
                  <p className="truncate text-sm font-medium text-fg">
                    {selectedProvider.label ||
                      capabilities.find((p) => p.id === selectedProvider.providerId)?.name}
                  </p>
                  <p className="mt-0.5 truncate text-[11px] text-fg/50">
                    {capabilities.find((p) => p.id === selectedProvider.providerId)?.name}
                  </p>
                </div>
                {selectedProvider.providerId === "lettuce-engine" ? (
                  <MenuButton
                    icon={LayoutDashboard}
                    title={t("providers.actions.openDashboard")}
                    description={t("providers.actions.openDashboardDesc")}
                    onClick={() => {
                      setSelectedProvider(null);
                      navigate(Routes.engineHome(selectedProvider.id));
                    }}
                    color="from-accent to-accent/80"
                  />
                ) : (
                  <MenuButton
                    icon={Edit3}
                    title={t("providers.actions.edit")}
                    description={t("providers.actions.editDesc")}
                    onClick={() => openEditor(selectedProvider)}
                    color="from-info to-info/80"
                  />
                )}
                <MenuButton
                  icon={Trash2}
                  title={isDeleting ? t("common.buttons.deleting") : t("common.buttons.delete")}
                  description={t("providers.actions.deleteDesc")}
                  onClick={() => void handleDeleteProvider(selectedProvider.id)}
                  disabled={isDeleting}
                  color="from-danger to-danger/80"
                />
              </div>
            )}
          </BottomMenu>

          <BottomMenu
            isOpen={isEditorOpen}
            onClose={closeEditor}
            title={editorProvider?.label ? t("providers.editor.titleEdit") : t("providers.editor.titleCreate")}
          >
            {editorProvider && (
              <div className="space-y-4 pb-2">
                <SelectField
                  label={t("providers.editor.providerType")}
                  value={editorProvider.providerId}
                  onChange={(providerId) => {
                    updateEditorProvider({
                      providerId,
                      config:
                        providerId === "custom"
                          ? {
                              chatEndpoint: "/v1/chat/completions",
                              modelsEndpoint: "",
                              fetchModelsEnabled: false,
                              modelsListPath: "data",
                              modelsIdPath: "id",
                              modelsDisplayNamePath: "name",
                              modelsDescriptionPath: "description",
                              modelsContextLengthPath: "",
                              authMode: "header",
                              authHeaderName: "x-api-key",
                              authQueryParamName: "api_key",
                              systemRole: "system",
                              userRole: "user",
                              assistantRole: "assistant",
                              toolChoiceMode: "auto",
                              supportsStream: true,
                              mergeSameRoleMessages: true,
                            }
                          : providerId === "custom-anthropic"
                            ? {
                                chatEndpoint: "/v1/messages",
                                modelsEndpoint: "",
                                fetchModelsEnabled: false,
                                modelsListPath: "data",
                                modelsIdPath: "id",
                                modelsDisplayNamePath: "name",
                                modelsDescriptionPath: "description",
                                modelsContextLengthPath: "",
                                authMode: "header",
                                authHeaderName: "x-api-key",
                                authQueryParamName: "api_key",
                                systemRole: "system",
                                userRole: "user",
                                assistantRole: "assistant",
                                supportsStream: true,
                                mergeSameRoleMessages: true,
                              }
                            : undefined,
                    });
                    setValidationError(null);
                  }}
                >
                  {visibleCapabilities.map((p) => (
                    <option key={p.id} value={p.id} className="bg-surface-el">
                      {p.name}
                    </option>
                  ))}
                </SelectField>
                <TextField
                  label={t("providers.editor.label")}
                  value={editorProvider.label}
                  onChange={(value) => updateEditorProvider({ label: value })}
                  placeholder={t("providers.editor.labelPlaceholder", {
                    name:
                      visibleCapabilities.find((p) => p.id === editorProvider.providerId)?.name ||
                      t("providers.editor.labelPlaceholderFallback"),
                  })}
                />
                {showApiKeyInput && (
                  <TextField
                    label={t("providers.editor.apiKey")}
                    type="password"
                    value={apiKey}
                    onChange={(value) => {
                      setApiKey(value);
                      if (validationError) setValidationError(null);
                    }}
                    placeholder={t("providers.editor.apiKeyPlaceholder")}
                  />
                )}
                {showBaseUrl && (
                  <TextField
                    label={t("providers.editor.baseUrl")}
                    type="url"
                    value={editorProvider.baseUrl || ""}
                    onChange={(value) => {
                      updateEditorProvider({ baseUrl: value || undefined });
                      if (validationError) setValidationError(null);
                    }}
                    placeholder={
                      isEngineProvider
                        ? "http://localhost:8000"
                        : isHostProvider
                          ? "http://192.168.1.10:3333"
                          : editorProvider.providerId === "intenserp"
                            ? "http://127.0.0.1:7777/v1"
                            : isLocalProvider
                              ? "http://localhost:11434"
                              : "https://api.provider.com"
                    }
                  />
                )}
                {isEngineProvider && (
                  <TextField
                    label={t("providers.editor.apiKeyOptional")}
                    type="password"
                    value={apiKey}
                    onChange={(value) => {
                      setApiKey(value);
                      if (validationError) setValidationError(null);
                    }}
                    placeholder={t("providers.editor.apiKeyOptionalPlaceholder")}
                  />
                )}
                {isComfyUiProvider && (
                  <>
                    <div>
                      <label className="mb-1 block text-[11px] font-medium text-fg/70">
                        {t("providers.editor.comfyTxt2imgWorkflow")}
                      </label>
                      <textarea
                        value={(customConfig.txt2imgWorkflow as string | undefined) || ""}
                        onChange={(e) =>
                          updateEditorProvider({
                            config: { ...editorProvider.config, txt2imgWorkflow: e.target.value },
                          })
                        }
                        placeholder='{ "3": { "class_type": "KSampler", ... } }'
                        rows={8}
                        spellCheck={false}
                        className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 font-mono text-xs text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
                      />
                    </div>
                    <div>
                      <label className="mb-1 block text-[11px] font-medium text-fg/70">
                        {t("providers.editor.comfyImg2imgWorkflow")}
                      </label>
                      <textarea
                        value={(customConfig.img2imgWorkflow as string | undefined) || ""}
                        onChange={(e) =>
                          updateEditorProvider({
                            config: { ...editorProvider.config, img2imgWorkflow: e.target.value },
                          })
                        }
                        placeholder='{ "10": { "class_type": "LoadImage", ... } }'
                        rows={8}
                        spellCheck={false}
                        className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 font-mono text-xs text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
                      />
                    </div>
                    <p className="text-[11px] leading-relaxed text-fg/50">
                      {t("providers.editor.comfyWorkflowHelp")}
                    </p>
                  </>
                )}
                {showOfficialProviderStreamingToggle && (
                  <ToggleRow
                    id="providerStreamingEnabled"
                    title={t("providers.editor.streaming")}
                    description={t("providers.editor.streamingDesc")}
                    checked={providerStreamingEnabled}
                    onChange={(next) =>
                      updateEditorProvider({
                        config: { ...editorProvider.config, streamingEnabled: next },
                      })
                    }
                  />
                )}
                {allowsTlsException && (
                  <ToggleRow
                    id="providerAllowInvalidTls"
                    title={t("providers.editor.allowInvalidTls")}
                    description={t("providers.editor.allowInvalidTlsDesc")}
                    checked={providerAllowInvalidTls}
                    onChange={(next) =>
                      updateEditorProvider({
                        config: { ...editorProvider.config, allowInvalidTls: next },
                      })
                    }
                    variant="warning"
                  />
                )}
                {isOllamaProvider && (
                  <>
                    <ToggleRow
                      id="providerSproutEnabled"
                      title={t("providers.editor.sprout")}
                      description={t("providers.editor.sproutDesc")}
                      checked={sproutEnabled}
                      onChange={(next) =>
                        updateEditorProvider({
                          config: { ...editorProvider.config, sproutEnabled: next },
                        })
                      }
                    />
                    <button
                      type="button"
                      onClick={() =>
                        void openExternalUrl("https://github.com/LettuceAI/Sprout")
                      }
                      className="inline-flex items-center gap-1.5 text-xs font-medium text-accent hover:underline"
                    >
                      <ExternalLink size={12} />
                      {t("providers.editor.sproutGet")}
                    </button>
                    {sproutEnabled && (
                      <>
                        <TextField
                          label={t("providers.editor.sproutUrl")}
                          type="url"
                          value={(customConfig.sproutUrl as string) || ""}
                          onChange={(value) =>
                            updateEditorProvider({
                              config: { ...editorProvider.config, sproutUrl: value || undefined },
                            })
                          }
                          placeholder="http://192.168.1.10:8477"
                        />
                        <TextField
                          label={t("providers.editor.sproutApiKey")}
                          type="password"
                          value={(customConfig.sproutApiKey as string) || ""}
                          onChange={(value) =>
                            updateEditorProvider({
                              config: {
                                ...editorProvider.config,
                                sproutApiKey: value || undefined,
                              },
                            })
                          }
                          placeholder={t("providers.editor.sproutApiKeyPlaceholder")}
                        />
                      </>
                    )}
                  </>
                )}
                {isCustomProvider && (
                  <>
                    <TextField
                      label={t("providers.editor.chatEndpoint")}
                      value={
                        (customConfig.chatEndpoint as string | undefined) ?? "/v1/chat/completions"
                      }
                      onChange={(value) =>
                        updateEditorProvider({
                          config: { ...editorProvider.config, chatEndpoint: value },
                        })
                      }
                      placeholder="/v1/chat/completions"
                    />
                    <ResolvedEndpointPreview
                      baseUrl={editorProvider.baseUrl || ""}
                      endpoint={
                        (customConfig.chatEndpoint as string | undefined) ?? "/v1/chat/completions"
                      }
                      duplicateWarning={(segment) =>
                        t("providers.editor.resolvedChatUrlDuplicate", { segment })
                      }
                    />
                    <ToggleRow
                      id="fetchModelsEnabled"
                      title={t("providers.editor.fetchModels")}
                      description={t("providers.editor.fetchModelsDesc")}
                      checked={customFetchModelsEnabled}
                      onChange={(next) =>
                        updateEditorProvider({
                          config: { ...editorProvider.config, fetchModelsEnabled: next },
                        })
                      }
                    />
                    <SelectField
                      label={t("providers.editor.authMode")}
                      value={customAuthMode}
                      onChange={(value) =>
                        updateEditorProvider({
                          config: { ...editorProvider.config, authMode: value },
                        })
                      }
                    >
                      <option value="bearer" className="bg-surface-el">
                        {t("providers.editor.authModeBearer")}
                      </option>
                      <option value="header" className="bg-surface-el">
                        {t("providers.editor.authModeHeader")}
                      </option>
                      <option value="query" className="bg-surface-el">
                        {t("providers.editor.authModeQuery")}
                      </option>
                      <option value="none" className="bg-surface-el">
                        {t("providers.editor.authModeNone")}
                      </option>
                    </SelectField>
                    {editorProvider.providerId === "custom" && (
                      <SelectField
                        label={t("providers.editor.toolChoiceMode")}
                        value={(customConfig.toolChoiceMode as string | undefined) ?? "auto"}
                        onChange={(value) =>
                          updateEditorProvider({
                            config: { ...editorProvider.config, toolChoiceMode: value },
                          })
                        }
                      >
                        <option value="auto" className="bg-surface-el">
                          {t("providers.editor.toolChoiceAuto")}
                        </option>
                        <option value="required" className="bg-surface-el">
                          {t("providers.editor.toolChoiceRequired")}
                        </option>
                        <option value="none" className="bg-surface-el">
                          {t("providers.editor.toolChoiceNone")}
                        </option>
                        <option value="omit" className="bg-surface-el">
                          {t("providers.editor.toolChoiceOmit")}
                        </option>
                        <option value="passthrough" className="bg-surface-el">
                          {t("providers.editor.toolChoicePassthrough")}
                        </option>
                      </SelectField>
                    )}
                    {customAuthMode === "header" && (
                      <TextField
                        label={t("providers.editor.authHeaderName")}
                        value={(customConfig.authHeaderName as string | undefined) ?? "x-api-key"}
                        onChange={(value) =>
                          updateEditorProvider({
                            config: { ...editorProvider.config, authHeaderName: value },
                          })
                        }
                        placeholder="x-api-key"
                      />
                    )}
                    {customAuthMode === "query" && (
                      <TextField
                        label={t("providers.editor.authQueryParamName")}
                        value={
                          (customConfig.authQueryParamName as string | undefined) ?? "api_key"
                        }
                        onChange={(value) =>
                          updateEditorProvider({
                            config: { ...editorProvider.config, authQueryParamName: value },
                          })
                        }
                        placeholder="api_key"
                      />
                    )}
                    {customFetchModelsEnabled && (
                      <>
                        <TextField
                          label={t("providers.editor.modelsEndpoint")}
                          value={(customConfig.modelsEndpoint as string | undefined) ?? ""}
                          onChange={(value) =>
                            updateEditorProvider({
                              config: { ...editorProvider.config, modelsEndpoint: value },
                            })
                          }
                          placeholder="/v1/models"
                        />
                        <ResolvedEndpointPreview
                          baseUrl={editorProvider.baseUrl || ""}
                          endpoint={(customConfig.modelsEndpoint as string | undefined) ?? ""}
                          duplicateWarning={(segment) =>
                            t("providers.editor.resolvedModelsUrlDuplicate", { segment })
                          }
                        />
                        <div className="grid grid-cols-2 gap-3">
                          <TextField
                            label={t("providers.editor.listPath")}
                            value={(customConfig.modelsListPath as string | undefined) ?? "data"}
                            onChange={(value) =>
                              updateEditorProvider({
                                config: { ...editorProvider.config, modelsListPath: value },
                              })
                            }
                            placeholder="data"
                          />
                          <TextField
                            label={t("providers.editor.modelIdPath")}
                            value={(customConfig.modelsIdPath as string | undefined) ?? "id"}
                            onChange={(value) =>
                              updateEditorProvider({
                                config: { ...editorProvider.config, modelsIdPath: value },
                              })
                            }
                            placeholder="id"
                          />
                        </div>
                        <div className="grid grid-cols-2 gap-3">
                          <TextField
                            label={t("providers.editor.displayNamePath")}
                            value={
                              (customConfig.modelsDisplayNamePath as string | undefined) ?? "name"
                            }
                            onChange={(value) =>
                              updateEditorProvider({
                                config: {
                                  ...editorProvider.config,
                                  modelsDisplayNamePath: value,
                                },
                              })
                            }
                            placeholder="name"
                          />
                          <TextField
                            label={t("providers.editor.descriptionPath")}
                            value={
                              (customConfig.modelsDescriptionPath as string | undefined) ??
                              "description"
                            }
                            onChange={(value) =>
                              updateEditorProvider({
                                config: {
                                  ...editorProvider.config,
                                  modelsDescriptionPath: value,
                                },
                              })
                            }
                            placeholder="description"
                          />
                        </div>
                        <TextField
                          label={t("providers.editor.contextLengthPath")}
                          value={
                            (customConfig.modelsContextLengthPath as string | undefined) ?? ""
                          }
                          onChange={(value) =>
                            updateEditorProvider({
                              config: {
                                ...editorProvider.config,
                                modelsContextLengthPath: value,
                              },
                            })
                          }
                          placeholder="context_length"
                        />
                      </>
                    )}
                    <TextField
                      label={t("providers.editor.systemRole")}
                      value={(customConfig.systemRole as string | undefined) ?? "system"}
                      onChange={(value) =>
                        updateEditorProvider({
                          config: { ...editorProvider.config, systemRole: value },
                        })
                      }
                      placeholder="system"
                    />
                    <div className="grid grid-cols-2 gap-3">
                      <TextField
                        label={t("providers.editor.userRole")}
                        value={(customConfig.userRole as string | undefined) ?? "user"}
                        onChange={(value) =>
                          updateEditorProvider({
                            config: { ...editorProvider.config, userRole: value },
                          })
                        }
                        placeholder="user"
                      />
                      <TextField
                        label={t("providers.editor.assistantRole")}
                        value={(customConfig.assistantRole as string | undefined) ?? "assistant"}
                        onChange={(value) =>
                          updateEditorProvider({
                            config: { ...editorProvider.config, assistantRole: value },
                          })
                        }
                        placeholder="assistant"
                      />
                    </div>
                    <div className="flex items-center justify-between pt-1">
                      <span className="text-sm font-medium text-fg/70">
                        {t("providers.editor.supportsStreaming")}
                      </span>
                      <Switch
                        id="supportsStream"
                        checked={(customConfig.supportsStream as boolean | undefined) ?? true}
                        onChange={(next) =>
                          updateEditorProvider({
                            config: {
                              ...editorProvider.config,
                              supportsStream: next,
                            },
                          })
                        }
                      />
                    </div>
                    <div className="flex items-center justify-between pt-1">
                      <span className="text-sm font-medium text-fg/70">
                        {t("providers.editor.mergeSameRoleMessages")}
                      </span>
                      <Switch
                        id="mergeSameRoleMessages"
                        checked={
                          (customConfig.mergeSameRoleMessages as boolean | undefined) ?? true
                        }
                        onChange={(next) =>
                          updateEditorProvider({
                            config: {
                              ...editorProvider.config,
                              mergeSameRoleMessages: next,
                            },
                          })
                        }
                      />
                    </div>
                  </>
                )}
                {isCustomProvider && (
                  <div className="flex items-center justify-between pt-1">
                    <div className="min-w-0">
                      <p className="text-sm font-medium text-fg/70">
                        {t("providers.editor.syncReasoningState")}
                      </p>
                      <p className="text-[10px] text-fg/40 leading-tight">
                        {t("providers.editor.syncReasoningStateDesc")}
                      </p>
                    </div>
                    <Switch
                      id="sendChatTemplateKwargs"
                      checked={
                        (editorProvider.config?.sendChatTemplateKwargs as boolean | undefined) ??
                        false
                      }
                      onChange={(next) =>
                        updateEditorProvider({
                          config: {
                            ...editorProvider.config,
                            sendChatTemplateKwargs: next,
                          },
                        })
                      }
                    />
                  </div>
                )}
                {validationError && (
                  <p className="text-xs font-medium text-danger/80">{validationError}</p>
                )}
                <div className="flex gap-3 pt-1">
                  <button
                    onClick={closeEditor}
                    className="flex-1 rounded-lg border border-fg/10 bg-fg/5 px-4 py-2 text-sm font-medium text-fg/70 transition hover:border-fg/20 hover:bg-fg/10 hover:text-fg"
                  >
                    {t("common.buttons.cancel")}
                  </button>
                  <button
                    onClick={() => void handleSaveProvider()}
                    disabled={isSaving || !editorProvider.label}
                    className="flex-1 rounded-lg border border-accent/40 bg-accent/20 px-4 py-2 text-sm font-semibold text-accent/90 transition hover:border-accent/60 hover:bg-accent/30 disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    {isSaving ? t("common.buttons.saving") : t("common.buttons.save")}
                  </button>
                </div>
              </div>
            )}
          </BottomMenu>
        </>
      )}

      {activeTab === "audio" && (
        <>
          <BottomMenu
            isOpen={!!selectedAudioProvider}
            onClose={() => setSelectedAudioProvider(null)}
            title={selectedAudioProvider?.label || t("providers.fallbackTitle")}
          >
            {selectedAudioProvider && (
              <div className="space-y-4">
                <div className="rounded-lg border border-fg/10 bg-fg/5 px-3 py-2">
                  <p className="truncate text-sm font-medium text-fg">
                    {selectedAudioProvider.label}
                  </p>
                  <p className="mt-0.5 truncate text-[11px] text-fg/50">
                    {getAudioProviderTypeLabel(selectedAudioProvider.providerType)}
                  </p>
                </div>
                {selectedAudioProvider.providerType === "kokoro" &&
                  kokoroInstalled[selectedAudioProvider.id] === false && (
                    <MenuButton
                      icon={Download}
                      title={t("providers.actions.downloadModel")}
                      description={t("providers.actions.downloadModelDesc")}
                      onClick={() => {
                        const provider = selectedAudioProvider;
                        setSelectedAudioProvider(null);
                        setKokoroSetupProvider(provider);
                      }}
                      color="from-accent to-accent/80"
                    />
                  )}
                {selectedAudioProvider.providerType === "kokoro" && (
                  <MenuButton
                    icon={LayoutDashboard}
                    title={t("providers.actions.openKokoroStudio")}
                    description={t("providers.actions.openKokoroStudioDesc")}
                    onClick={() => {
                      const id = selectedAudioProvider.id;
                      setSelectedAudioProvider(null);
                      navigate(`/settings/voices/kokoro/${id}`);
                    }}
                    color="from-accent to-accent/80"
                  />
                )}
                <MenuButton
                  icon={Edit3}
                  title={t("providers.actions.edit")}
                  description={t("providers.actions.editAudioDesc")}
                  onClick={() => openAudioEditor(selectedAudioProvider)}
                  color="from-info to-info/80"
                />
                {selectedAudioProvider.id !== "system-kokoro" && (
                  <MenuButton
                    icon={Trash2}
                    title={isAudioDeleting ? t("common.buttons.deleting") : t("common.buttons.delete")}
                    description={t("providers.actions.deleteDesc")}
                    onClick={() => void handleDeleteAudioProvider(selectedAudioProvider.id)}
                    disabled={isAudioDeleting}
                    color="from-danger to-danger/80"
                  />
                )}
              </div>
            )}
          </BottomMenu>

          <AudioProviderEditor
            isOpen={isAudioEditorOpen}
            provider={editingAudioProvider}
            onClose={closeAudioEditor}
            onSave={handleSaveAudioProvider}
          />

          <KokoroSetupMenu
            provider={kokoroSetupProvider}
            onClose={() => setKokoroSetupProvider(null)}
            onStarted={() => void refreshKokoroStatus()}
          />
        </>
      )}

      {/* Engine Setup Bottom Sheet */}
      <BottomMenu isOpen={!!engineSetupResult} onClose={dismissEngineSetup} title={t("providers.engineSetup.title")}>
        {engineSetupResult && (
          <div className="space-y-4 pb-2">
            {engineSetupResult.needsSetup ? (
              <>
                <div className="flex flex-col items-center gap-3 py-4">
                  <div className="flex h-14 w-14 items-center justify-center rounded-2xl border border-accent/30 bg-accent/15">
                    <Sparkles className="h-7 w-7 text-accent/80" />
                  </div>
                  <div className="text-center">
                    <h3 className="text-base font-semibold text-fg">
                      {t("providers.engineSetup.newEngineTitle")}
                    </h3>
                    <p className="mt-1 text-sm text-fg/60">
                      {t("providers.engineSetup.newEngineDesc")}
                    </p>
                  </div>
                </div>
                <button
                  onClick={() => {
                    dismissEngineSetup();
                    navigate(Routes.engineSetup(engineSetupResult.credentialId));
                  }}
                  className="w-full rounded-lg border border-accent/40 bg-accent/20 px-4 py-3 text-sm font-semibold text-accent/90 transition hover:border-accent/60 hover:bg-accent/30"
                >
                  {t("providers.engineSetup.startSetup")}
                </button>
              </>
            ) : (
              <>
                <div className="flex flex-col items-center gap-3 py-4">
                  <div className="flex h-14 w-14 items-center justify-center rounded-2xl border border-accent/30 bg-accent/15">
                    <Leaf className="h-7 w-7 text-accent/80" />
                  </div>
                  <div className="text-center">
                    <h3 className="text-base font-semibold text-fg">
                      {t("providers.engineSetup.connectedTitle")}
                    </h3>
                    <p className="mt-1 text-sm text-fg/60">
                      {t("providers.engineSetup.connectedDesc")}
                    </p>
                  </div>
                </div>
                <button
                  onClick={() => {
                    dismissEngineSetup();
                    navigate(Routes.engineHome(engineSetupResult.credentialId));
                  }}
                  className="w-full rounded-lg border border-accent/40 bg-accent/20 px-4 py-3 text-sm font-semibold text-accent/90 transition hover:border-accent/60 hover:bg-accent/30"
                >
                  {t("providers.engineSetup.openDashboard")}
                </button>
              </>
            )}
            <button
              onClick={dismissEngineSetup}
              className="w-full rounded-lg border border-fg/10 bg-fg/5 px-4 py-2.5 text-sm font-medium text-fg/70 transition hover:border-fg/20 hover:bg-fg/10 hover:text-fg"
            >
              {t("providers.engineSetup.dismiss")}
            </button>
          </div>
        )}
      </BottomMenu>

      <div
        className={cn(
          "fixed bottom-0 left-0 right-0 z-30 border-t px-3 pb-[calc(env(safe-area-inset-bottom)+12px)] pt-3 lg:hidden",
          colors.glass.strong,
        )}
      >
        <div
          role="tablist"
          aria-label={t("providers.tabs.ariaLabel")}
          className={cn(radius.lg, "grid grid-cols-2 gap-2 p-1", colors.surface.elevated)}
        >
          {[
            { id: "llm" as const, icon: Cpu, label: t("providers.tabs.ai") },
            { id: "audio" as const, icon: Volume2, label: t("providers.tabs.audio") },
          ].map(({ id, icon: Icon, label }) => (
            <button
              key={id}
              type="button"
              onClick={() => handleTabChange(id)}
              role="tab"
              id={id === "llm" ? llmTabId : audioTabId}
              aria-selected={activeTab === id}
              aria-controls={id === "llm" ? llmPanelId : audioPanelId}
              className={cn(
                radius.md,
                "px-3 py-2.5 text-sm font-semibold transition flex items-center justify-center gap-2",
                interactive.active.scale,
                activeTab === id ? "bg-fg/10 text-fg" : cn(colors.text.tertiary, "hover:text-fg"),
              )}
            >
              <Icon size={16} />
              <span className="pt-1">{label}</span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
