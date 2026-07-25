import { useState, useEffect, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import {
  ChevronRight,
  Plus,
  Trash2,
  Play,
  Volume2,
  Loader2,
  Edit3,
  RefreshCw,
  HardDrive,
  EthernetPort,
} from "lucide-react";

import {
  listAudioProviders,
  listUserVoices,
  upsertUserVoice,
  deleteUserVoice,
  listAudioModels,
  listVoiceDesignModels,
  refreshProviderVoices,
  getProviderVoices,
  generateTtsPreview,
  playAudioFromBase64,
  designVoicePreview,
  createVoiceFromPreview,
  getTtsCacheStats,
  clearTtsCache,
  type AudioProvider,
  type AudioProviderType,
  type AudioModel,
  type UserVoice,
  type CachedVoice,
  type TtsCacheStats,
} from "../../../core/storage/audioProviders";

import { BottomMenu, MenuButton } from "../../components/BottomMenu";
import { useI18n, type TranslationKey } from "../../../core/i18n/context";

const GEMINI_VOICES = [
  { id: "kore", name: "Kore", descriptionKey: "voices.extra.geminiVoices.kore" },
  { id: "aoede", name: "Aoede", descriptionKey: "voices.extra.geminiVoices.aoede" },
  { id: "charon", name: "Charon", descriptionKey: "voices.extra.geminiVoices.charon" },
  { id: "fenrir", name: "Fenrir", descriptionKey: "voices.extra.geminiVoices.fenrir" },
  { id: "puck", name: "Puck", descriptionKey: "voices.extra.geminiVoices.puck" },
  { id: "leda", name: "Leda", descriptionKey: "voices.extra.geminiVoices.leda" },
  { id: "orus", name: "Orus", descriptionKey: "voices.extra.geminiVoices.orus" },
  { id: "zephyr", name: "Zephyr", descriptionKey: "voices.extra.geminiVoices.zephyr" },
  { id: "algieba", name: "Algieba", descriptionKey: "voices.extra.geminiVoices.algieba" },
  {
    id: "callirrhoe",
    name: "Callirrhoe",
    descriptionKey: "voices.extra.geminiVoices.callirrhoe",
  },
] as const satisfies ReadonlyArray<{
  id: string;
  name: string;
  descriptionKey: TranslationKey;
}>;

export function VoicesPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [providers, setProviders] = useState<AudioProvider[]>([]);
  const [userVoices, setUserVoices] = useState<UserVoice[]>([]);
  const [providerVoices, setProviderVoices] = useState<Record<string, CachedVoice[]>>({});
  const [loadingVoicesFor, setLoadingVoicesFor] = useState<string | null>(null);
  const [expandedProviders, setExpandedProviders] = useState<Record<string, boolean>>({});
  const [isLoading, setIsLoading] = useState(true);
  const [cacheStats, setCacheStats] = useState<TtsCacheStats | null>(null);
  const [isClearingCache, setIsClearingCache] = useState(false);

  // Voice editor state
  const [isVoiceEditorOpen, setIsVoiceEditorOpen] = useState(false);
  const [editingVoice, setEditingVoice] = useState<UserVoice | null>(null);

  // Selection menu state
  const [selectedVoice, setSelectedVoice] = useState<UserVoice | null>(null);
  const [selectedCustomVoice, setSelectedCustomVoice] = useState<
    (CachedVoice & { provider: AudioProvider }) | null
  >(null);
  const editableProviders = providers;
  const libraryProviders = editableProviders.filter(
    (provider) => provider.providerType !== "openai_tts",
  );

  const getProviderBadge = (providerType: AudioProviderType) => {
    if (providerType === "gemini_tts") return t("voices.extra.badge.gemini");
    if (providerType === "fish_tts") return t("voices.extra.badge.fish");
    if (providerType === "fish_speech") return t("voices.extra.badge.fishSpeech");
    if (providerType === "openai_tts") return t("voices.extra.badge.openai");
    return t("voices.extra.badge.elevenlabs");
  };

  // Separate custom voices (cloned, generated) from premade library voices
  const isCustomVoice = (voice: CachedVoice) => {
    const category = voice.labels?.category?.toLowerCase() || "";
    // Custom voices are: cloned, generated, professional (user-created), or anything not "premade"
    return category !== "premade" && category !== "library" && category !== "";
  };

  // Get all custom voices from all providers (to show in My Voices section)
  const customProviderVoices = editableProviders.flatMap((provider) => {
    const voices = providerVoices[provider.id] || [];
    return voices.filter(isCustomVoice).map((v) => ({ ...v, provider }));
  });

  // Get premade/library voices for Provider Voices section
  const getPremadeVoices = (providerId: string) => {
    const voices = providerVoices[providerId] || [];
    return voices.filter((v) => !isCustomVoice(v));
  };

  const loadData = useCallback(async () => {
    setIsLoading(true);
    try {
      const [loadedProviders, loadedVoices, stats] = await Promise.all([
        listAudioProviders(),
        listUserVoices(),
        getTtsCacheStats(),
      ]);
      setCacheStats(stats);
      setProviders(loadedProviders);
      setUserVoices(loadedVoices);

      // Load voices for each provider
      const voicesMap: Record<string, CachedVoice[]> = {};
      for (const provider of loadedProviders) {
        try {
          const voices = await getProviderVoices(provider.id);
          voicesMap[provider.id] = voices;
        } catch (e) {
          console.error(`Failed to load voices for ${provider.label}:`, e);
          voicesMap[provider.id] = [];
        }
      }
      setProviderVoices(voicesMap);
    } catch (e) {
      console.error("Failed to load voices data:", e);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const handleRefreshProviderVoices = async (providerId: string) => {
    setLoadingVoicesFor(providerId);
    try {
      const voices = await refreshProviderVoices(providerId);
      setProviderVoices((prev) => ({ ...prev, [providerId]: voices }));
    } catch (e) {
      console.error("Failed to refresh voices:", e);
    } finally {
      setLoadingVoicesFor(null);
    }
  };

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const handleCreateVoice = () => {
    const firstNonKokoro = editableProviders.find((p) => p.providerType !== "kokoro");
    if (firstNonKokoro) {
      setEditingVoice({
        id: "",
        providerId: firstNonKokoro.id,
        name: "",
        modelId: "",
        voiceId: "",
        prompt: "",
      });
      setIsVoiceEditorOpen(true);
      return;
    }
    const kokoroProvider = editableProviders.find((p) => p.providerType === "kokoro");
    if (kokoroProvider) {
      navigate(`/settings/voices/kokoro/${kokoroProvider.id}`);
    }
  };

  const handleEditVoice = (voice: UserVoice) => {
    const owningProvider = providers.find((p) => p.id === voice.providerId);
    if (owningProvider?.providerType === "kokoro") {
      setSelectedVoice(null);
      navigate(`/settings/voices/kokoro/${voice.providerId}/blend/${voice.id}`);
      return;
    }
    setEditingVoice({ ...voice });
    setIsVoiceEditorOpen(true);
    setSelectedVoice(null);
  };

  const handleDeleteVoice = async (id: string) => {
    try {
      await deleteUserVoice(id);
      await loadData();
      setSelectedVoice(null);
    } catch (e) {
      console.error("Failed to delete voice:", e);
    }
  };

  if (isLoading) {
    return (
      <div className="flex h-64 items-center justify-center">
        <Loader2 className="h-8 w-8 animate-spin text-fg/40" />
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col space-y-6">
      {providers.length === 0 && (
        <button
          onClick={() => navigate("/settings/providers?tab=audio")}
          className="group w-full rounded-xl border border-dashed border-fg/15 bg-fg/5 px-4 py-3 text-left transition hover:border-fg/25 hover:bg-fg/10"
        >
          <div className="flex items-center gap-3">
            <div className="flex h-8 w-8 items-center justify-center rounded-full border border-fg/10 bg-fg/10">
              <EthernetPort className="h-4 w-4 text-fg/60" />
            </div>
            <div className="flex-1 min-w-0">
              <p className="truncate text-sm font-medium text-fg">
                {t("providers.extra.audioEmpty.title")}
              </p>
              <p className="text-xs text-fg/50">
                {t("voices.extra.page.noAudioProvidersHint")}
              </p>
            </div>
            <ChevronRight className="h-4 w-4 text-fg/30 group-hover:text-fg/60" />
          </div>
        </button>
      )}

      {/* My Voices Section (Voice Designer + Custom Provider Voices) */}
      <section>
        <div className="mb-2 flex items-center justify-between px-1">
          <h2 className="text-xs font-medium uppercase tracking-wider text-fg/40">
            {t("common.nav.voices")}
          </h2>
          {editableProviders.length > 0 && (
            <button
              onClick={handleCreateVoice}
              className="flex items-center gap-1 rounded-lg border border-fg/10 bg-fg/5 px-2 py-1 text-xs text-fg/70 transition hover:border-fg/20 hover:bg-fg/10"
            >
              <Plus className="h-3 w-3" />
              {t("common.buttons.create")}
            </button>
          )}
        </div>

        {userVoices.length === 0 && customProviderVoices.length === 0 ? (
          <div className="flex flex-col items-center justify-center rounded-xl border border-dashed border-fg/10 py-8">
            <Volume2 className="mb-2 h-8 w-8 text-fg/20" />
            <p className="text-sm text-fg/50">{t("voices.extra.page.noVoicesTitle")}</p>
            <p className="text-xs text-fg/30">
              {editableProviders.length > 0
                ? t("voices.extra.page.noVoicesDescription")
                : t("voices.extra.page.addProviderFirst")}
            </p>
          </div>
        ) : (
          <div className="space-y-2">
            {/* User-created voice configs */}
            {userVoices.map((voice) => {
              const provider = providers.find((p) => p.id === voice.providerId);
              return (
                <button
                  key={voice.id}
                  onClick={() => setSelectedVoice(voice)}
                  className="group w-full rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-left transition hover:border-fg/20 hover:bg-fg/10"
                >
                  <div className="flex items-center gap-3">
                    <div className="flex h-8 w-8 items-center justify-center rounded-full border border-accent/30 bg-accent/10">
                      <Volume2 className="h-4 w-4 text-accent" />
                    </div>
                    <div className="flex-1 min-w-0">
                      <p className="truncate text-sm font-medium text-fg">{voice.name}</p>
                      <p className="truncate text-xs text-fg/50">
                        {provider?.label} •{" "}
                        {voice.prompt
                          ? `"${voice.prompt.slice(0, 30)}..."`
                          : t("voices.extra.page.noPrompt")}
                      </p>
                    </div>
                    <ChevronRight className="h-4 w-4 text-fg/30 group-hover:text-fg/60" />
                  </div>
                </button>
              );
            })}

            {/* Custom provider voices (cloned, generated, etc.) */}
            {customProviderVoices.map((voice) => (
              <button
                key={`${voice.providerId}-${voice.voiceId}`}
                onClick={() => setSelectedCustomVoice(voice)}
                className="group w-full rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-left transition hover:border-fg/20 hover:bg-fg/10"
              >
                <div className="flex items-center gap-3">
                  <div className="flex h-8 w-8 items-center justify-center rounded-full border border-warning/30 bg-warning/10">
                    <Volume2 className="h-4 w-4 text-warning" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="truncate text-sm font-medium text-fg">{voice.name}</p>
                    <p className="truncate text-xs text-fg/50">
                      {voice.provider.label} •{" "}
                      <span className="capitalize">
                        {voice.labels?.category || t("common.labels.custom")}
                      </span>
                    </p>
                  </div>
                  <ChevronRight className="h-4 w-4 text-fg/30 group-hover:text-fg/60" />
                </div>
              </button>
            ))}
          </div>
        )}
      </section>

      {/* Provider Voices Section */}
      {libraryProviders.length > 0 && (
        <section>
          <div className="mb-2 px-1">
            <h2 className="text-xs font-medium uppercase tracking-wider text-fg/40">
              {t("voices.extra.providerVoices")}
            </h2>
          </div>

          <div className="space-y-4">
            {libraryProviders.map((provider) => {
              const voices = getPremadeVoices(provider.id);
              const isLoadingThis = loadingVoicesFor === provider.id;
              const isExpanded = expandedProviders[provider.id] ?? false;
              const displayedVoices = isExpanded ? voices : voices.slice(0, 10);

              return (
                <div key={provider.id} className="rounded-xl border border-fg/10 bg-fg/5 p-3">
                  <div className="mb-2 flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <div className="flex h-6 w-6 items-center justify-center rounded-full border border-fg/10 bg-fg/10">
                        <span className="text-[10px]">
                          {getProviderBadge(provider.providerType)}
                        </span>
                      </div>
                      <span className="text-sm font-medium text-fg">{provider.label}</span>
                      <span className="text-xs text-fg/40">
                        ({t("voices.extra.page.voiceCount", { count: voices.length })})
                      </span>
                    </div>
                    <button
                      onClick={() => void handleRefreshProviderVoices(provider.id)}
                      disabled={isLoadingThis}
                      className="flex items-center gap-1 rounded-lg border border-fg/10 bg-fg/5 px-2 py-1 text-[10px] text-fg/60 transition hover:border-fg/20 hover:bg-fg/10 disabled:opacity-50"
                    >
                      <RefreshCw className={`h-3 w-3 ${isLoadingThis ? "animate-spin" : ""}`} />
                      {t("common.buttons.refresh")}
                    </button>
                  </div>

                  {isLoadingThis ? (
                    <div className="flex items-center justify-center py-4">
                      <Loader2 className="h-5 w-5 animate-spin text-fg/40" />
                    </div>
                  ) : voices.length === 0 ? (
                    <p className="py-2 text-center text-xs text-fg/40">
                      {t("voices.extra.page.noProviderVoices")}
                    </p>
                  ) : (
                    <div className="grid grid-cols-2 gap-2">
                      {displayedVoices.map((voice) => (
                        <div
                          key={voice.voiceId}
                          className="flex items-center gap-2 rounded-lg border border-fg/5 bg-surface-el/20 px-2 py-1.5"
                        >
                          <Volume2 className="h-3 w-3 shrink-0 text-fg/40" />
                          <div className="min-w-0 flex-1">
                            <p className="truncate text-xs font-medium text-fg/80">{voice.name}</p>
                            {voice.labels?.category && (
                              <p className="truncate text-[10px] text-fg/40">
                                {voice.labels.category}
                              </p>
                            )}
                            {voice.labels?.gender && !voice.labels?.category && (
                              <p className="truncate text-[10px] text-fg/40">
                                {voice.labels.gender}
                              </p>
                            )}
                          </div>
                        </div>
                      ))}
                    </div>
                  )}

                  {voices.length > 10 && (
                    <button
                      onClick={() =>
                        setExpandedProviders((prev) => ({
                          ...prev,
                          [provider.id]: !isExpanded,
                        }))
                      }
                      className="mt-2 w-full rounded-lg border border-fg/10 bg-fg/5 py-1.5 text-[11px] text-fg/60 transition hover:border-fg/20 hover:bg-fg/10"
                    >
                      {isExpanded
                        ? t("voices.extra.page.showLess")
                        : t("voices.extra.page.showAllVoices", { count: voices.length })}
                    </button>
                  )}
                </div>
              );
            })}
          </div>
        </section>
      )}

      {/* TTS Audio Cache Section */}
      <section>
        <div className="mb-2 px-1">
          <h2 className="text-xs font-medium uppercase tracking-wider text-fg/40">
            {t("voices.extra.cache.section")}
          </h2>
        </div>
        <div className="rounded-xl border border-fg/10 bg-fg/5 p-4">
          <div className="flex items-start gap-3">
            <div className="flex h-8 w-8 items-center justify-center rounded-full border border-fg/10 bg-fg/10">
              <HardDrive className="h-4 w-4 text-fg/70" />
            </div>
            <div className="flex-1 min-w-0">
              <p className="text-sm font-medium text-fg">{t("voices.extra.cache.title")}</p>
              <p className="text-xs text-fg/50 mt-0.5">
                {t("voices.extra.cache.description")}
              </p>
              {cacheStats && (
                <div className="mt-2 flex items-center gap-3 text-xs text-fg/60">
                  <span>
                    {t("voices.extra.page.cacheFiles", { count: cacheStats.count })}
                  </span>
                  <span>•</span>
                  <span>
                    {formatBytes(cacheStats.sizeBytes, [
                      t("common.units.bytes"),
                      t("common.units.kb"),
                      t("common.units.mb"),
                      t("common.units.gb"),
                    ])}
                  </span>
                </div>
              )}
            </div>
          </div>
          <button
            onClick={async () => {
              if (isClearingCache) return;
              setIsClearingCache(true);
              try {
                await clearTtsCache();
                const stats = await getTtsCacheStats();
                setCacheStats(stats);
              } catch (err) {
                console.error("Failed to clear TTS cache:", err);
              } finally {
                setIsClearingCache(false);
              }
            }}
            disabled={isClearingCache || !cacheStats || cacheStats.count === 0}
            className="mt-3 w-full flex items-center justify-center gap-2 rounded-lg border border-fg/10 bg-fg/5 py-2 text-xs text-fg/70 transition hover:border-fg/20 hover:bg-fg/10 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {isClearingCache ? (
              <>
                <Loader2 className="h-3 w-3 animate-spin" />
                {t("voices.extra.cache.clearing")}
              </>
            ) : (
              <>
                <Trash2 className="h-3 w-3" />
                {t("voices.extra.cache.clear")}
              </>
            )}
          </button>
        </div>
      </section>

      {/* Voice Selection Menu */}
      <BottomMenu
        isOpen={!!selectedVoice}
        onClose={() => setSelectedVoice(null)}
        title={selectedVoice?.name || t("voices.extra.page.voiceFallbackTitle")}
      >
        {selectedVoice && (
          <div className="space-y-4">
            <MenuButton
              icon={Edit3}
              title={t("common.buttons.edit")}
              description={t("voices.extra.menu.editDescription")}
              onClick={() => handleEditVoice(selectedVoice)}
              color="from-info to-info/80"
            />
            <MenuButton
              icon={Trash2}
              title={t("common.buttons.delete")}
              description={t("voices.extra.menu.deleteDescription")}
              onClick={() => void handleDeleteVoice(selectedVoice.id)}
              color="from-danger to-danger/80"
            />
          </div>
        )}
      </BottomMenu>

      {/* Custom Provider Voice Selection Menu */}
      <BottomMenu
        isOpen={!!selectedCustomVoice}
        onClose={() => setSelectedCustomVoice(null)}
        title={selectedCustomVoice?.name || t("voices.extra.page.voiceFallbackTitle")}
      >
        {selectedCustomVoice && (
          <div className="space-y-4">
            <div className="rounded-lg border border-fg/10 bg-fg/5 p-3">
              <p className="text-xs text-fg/50">{t("voices.extra.menu.provider")}</p>
              <p className="text-sm text-fg">{selectedCustomVoice.provider.label}</p>
              <p className="mt-2 text-xs text-fg/50">{t("voices.extra.menu.category")}</p>
              <p className="text-sm capitalize text-fg">
                {selectedCustomVoice.labels?.category || t("common.labels.custom")}
              </p>
              {selectedCustomVoice.labels?.description && (
                <>
                  <p className="mt-2 text-xs text-fg/50">{t("common.labels.description")}</p>
                  <p className="text-sm text-fg">{selectedCustomVoice.labels.description}</p>
                </>
              )}
            </div>
            <MenuButton
              icon={Plus}
              title={t("voices.extra.menu.createVoiceConfig")}
              description={t("voices.extra.menu.createVoiceConfigDescription")}
              onClick={() => {
                setEditingVoice({
                  id: "",
                  providerId: selectedCustomVoice.provider.id,
                  name: selectedCustomVoice.name,
                  modelId: "",
                  voiceId: selectedCustomVoice.voiceId,
                  prompt: "",
                });
                setIsVoiceEditorOpen(true);
                setSelectedCustomVoice(null);
              }}
              color="from-accent to-accent/80"
            />
          </div>
        )}
      </BottomMenu>

      {/* Voice Editor (Voice Designer) */}
      <VoiceEditor
        isOpen={isVoiceEditorOpen}
        voice={editingVoice}
        providers={editableProviders.filter((p) => p.providerType !== "kokoro")}
        onClose={() => {
          setIsVoiceEditorOpen(false);
          setEditingVoice(null);
        }}
        onSave={async (voice) => {
          await upsertUserVoice(voice);
          await loadData();
          setIsVoiceEditorOpen(false);
          setEditingVoice(null);
        }}
      />
    </div>
  );
}

function formatBytes(bytes: number, sizes: string[]): string {
  if (bytes === 0) return `0 ${sizes[0]}`;
  const k = 1024;
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

interface VoiceEditorProps {
  isOpen: boolean;
  voice: UserVoice | null;
  providers: AudioProvider[];
  onClose: () => void;
  onSave: (voice: UserVoice) => Promise<void>;
}

function VoiceEditor({ isOpen, voice, providers, onClose, onSave }: VoiceEditorProps) {
  const { t } = useI18n();
  const [formData, setFormData] = useState<UserVoice | null>(null);
  const [models, setModels] = useState<AudioModel[]>([]);
  const [providerVoices, setProviderVoices] = useState<CachedVoice[]>([]);
  const [isLoadingVoices, setIsLoadingVoices] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isPlaying, setIsPlaying] = useState(false);

  const [generatedPreviewId, setGeneratedPreviewId] = useState<string | null>(null);
  const [textSample, setTextSample] = useState(
    t("voices.extra.editor.defaultSample"),
  );
  const [previewError, setPreviewError] = useState<string | null>(null);
  const minVoiceDesignChars = 100;

  useEffect(() => {
    if (isOpen) {
      if (voice) {
        setFormData({ ...voice });
      } else {
        // Initialize for NEW voice
        setFormData({
          id: "",
          providerId: providers.length > 0 ? providers[0].id : "",
          name: "",
          modelId: "",
          voiceId: "",
          prompt: "",
        });
      }
      setGeneratedPreviewId(null);
      setTextSample(
        t("voices.extra.editor.defaultSample"),
      );
      setPreviewError(null);
      setProviderVoices([]);
    }
  }, [isOpen, voice, providers]);

  // Load models when provider changes
  useEffect(() => {
    if (!formData?.providerId) return;
    const provider = providers.find((p) => p.id === formData.providerId);
    if (!provider) return;

    void (async () => {
      try {
        const isNewVoice = !formData.id;

        let loadedModels: AudioModel[];
        if (isNewVoice) {
          loadedModels = await listVoiceDesignModels(provider.providerType);
        } else {
          loadedModels = await listAudioModels(provider.providerType);
        }

        setModels(loadedModels);

        const currentModelValid = loadedModels.some((m) => m.id === formData.modelId);
        if ((!formData.modelId || !currentModelValid) && loadedModels.length > 0) {
          setFormData((f) => (f ? { ...f, modelId: loadedModels[0].id } : f));
        }
      } catch (e) {
        console.error("Failed to load models:", e);
        setModels([]);
      }
    })();
  }, [formData?.providerId, formData?.id, providers]);

  useEffect(() => {
    if (!formData?.providerId) return;
    const provider = providers.find((p) => p.id === formData.providerId);
    if (!provider) return;

    if (provider.providerType === "elevenlabs" || provider.providerType === "fish_tts") {
      setIsLoadingVoices(true);
      void (async () => {
        try {
          let voices = await getProviderVoices(formData.providerId);
          if (voices.length === 0) {
            voices = await refreshProviderVoices(formData.providerId);
          }
          setProviderVoices(voices);
        } catch (e) {
          console.error("Failed to load provider voices:", e);
          setProviderVoices([]);
        } finally {
          setIsLoadingVoices(false);
        }
      })();
    } else {
      setProviderVoices([]);
    }
  }, [formData?.providerId, providers]);

  const handleSave = async () => {
    if (!formData || !formData.name.trim()) return;
    setIsSaving(true);
    try {
      let finalVoiceData = { ...formData };
      const provider = providers.find((p) => p.id === formData.providerId);

      if (provider?.providerType === "elevenlabs" && !formData.id) {
        if (!generatedPreviewId) {
          throw new Error(t("voices.extra.editor.nameVoiceFirst"));
        }

        const result = await createVoiceFromPreview(
          formData.providerId,
          formData.name,
          generatedPreviewId,
          formData.prompt || "",
        );

        finalVoiceData.voiceId = result.voiceId;
      }

      if (provider?.providerType === "gemini_tts" && !finalVoiceData.voiceId.trim()) {
        finalVoiceData.voiceId = "kore";
      }

      if (provider?.providerType === "fish_tts" && !finalVoiceData.voiceId.trim()) {
        throw new Error(t("voices.extra.editor.fishVoiceIdRequired"));
      }

      if (provider?.providerType === "openai_tts") {
        if (!finalVoiceData.modelId.trim()) {
          throw new Error(t("voices.extra.editor.openaiModelIdRequired"));
        }
        if (!finalVoiceData.voiceId.trim()) {
          throw new Error(t("voices.extra.editor.openaiVoiceIdRequired"));
        }
      }

      await onSave(finalVoiceData);
    } catch (e) {
      console.error("Failed to save voice:", e);
      setPreviewError(e instanceof Error ? e.message : t("voices.extra.editor.saveVoiceFailed"));
    } finally {
      setIsSaving(false);
    }
  };

  const handlePreview = async () => {
    console.log(
      "handlePreview called. Provider:",
      formData?.providerId,
      "Model:",
      formData?.modelId,
      "Sample:",
      textSample,
    );
    if (!formData?.providerId || !textSample.trim()) return;

    const provider = providers.find((p) => p.id === formData.providerId);
    if (!provider) return;

    // For ElevenLabs Voice Design, model is optional (has a default)
    // For other cases, require modelId
    const isElevenLabsVoiceDesign = provider.providerType === "elevenlabs" && !formData.id;
    if (!isElevenLabsVoiceDesign && !formData?.modelId) return;
    const trimmedSample = textSample.trim();
    if (isElevenLabsVoiceDesign && trimmedSample.length < minVoiceDesignChars) {
      setPreviewError(
        t("voices.extra.editor.exampleTextMinChars", { minimum: minVoiceDesignChars }),
      );
      return;
    }

    setIsPlaying(true);
    try {
      setPreviewError(null);
      if (isElevenLabsVoiceDesign) {
        // Voice Design Preview (New Voice)
        const description = formData.prompt || "";
        console.log("Requesting Voice Design Preview...");

        // Pass modelId if selected, otherwise API uses default
        const previews = await designVoicePreview(
          formData.providerId,
          trimmedSample,
          description,
          formData.modelId || undefined,
          1, // Generate 1 preview
        );

        if (previews.length > 0) {
          console.log("Got previews:", previews);
          const preview = previews[0];
          console.log("Setting generatedPreviewId:", preview.generatedVoiceId);
          setGeneratedPreviewId(preview.generatedVoiceId);
          playAudioFromBase64(preview.audioBase64, preview.mediaType);
        } else {
          console.warn("No previews returned!");
        }
      } else {
        let resolvedVoiceId = formData.voiceId;
        if (
          provider.providerType === "elevenlabs" &&
          (!resolvedVoiceId || resolvedVoiceId === formData.id)
        ) {
          const voices = await refreshProviderVoices(formData.providerId);
          const match = voices.find(
            (voice) => voice.name.trim().toLowerCase() === formData.name.trim().toLowerCase(),
          );
          if (!match) {
            throw new Error(t("voices.extra.editor.resolveElevenlabsFailed"));
          }
          resolvedVoiceId = match.voiceId;
          const nextVoice = { ...formData, voiceId: resolvedVoiceId };
          setFormData(nextVoice);
          await upsertUserVoice(nextVoice);
        }
        // Standard TTS Preview (Existing Voice or Gemini)
        // Use the REAL voice ID stored in formData
        const response = await generateTtsPreview(
          formData.providerId,
          formData.modelId,
          // For Gemini, we might need a placeholder if voiceId is empty (new config)
          // But if it's existing, use the ID.
          resolvedVoiceId || "preview",
          textSample,
          formData.prompt,
        );
        playAudioFromBase64(response.audioBase64, response.format);
      }
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      console.error("TTS preview failed:", e);
      setPreviewError(message || "TTS preview failed.");
    } finally {
      setIsPlaying(false);
    }
  };

  if (!formData) return null;
  const activeProvider = providers.find((p) => p.id === formData.providerId);
  const isNewVoice = !formData.id;
  const isElevenLabsVoiceDesign = activeProvider?.providerType === "elevenlabs" && isNewVoice;
  const allowsVoiceLookupByName =
    activeProvider?.providerType === "elevenlabs" && !isElevenLabsVoiceDesign;
  const usesProviderVoicePicker =
    activeProvider?.providerType === "elevenlabs" || activeProvider?.providerType === "fish_tts";
  const requiresManualVoiceId =
    activeProvider?.providerType === "openai_tts" ||
    activeProvider?.providerType === "fish_tts" ||
    activeProvider?.providerType === "fish_speech";
  const sampleLength = textSample.trim().length;
  const previewDisabled =
    isPlaying ||
    !formData.providerId ||
    !textSample.trim() ||
    (isElevenLabsVoiceDesign && sampleLength < minVoiceDesignChars) ||
    (!isElevenLabsVoiceDesign &&
      (!formData.modelId ||
        (requiresManualVoiceId && !formData.voiceId.trim()) ||
        (allowsVoiceLookupByName && !formData.voiceId && !formData.name.trim())));

  return (
    <BottomMenu
      isOpen={isOpen}
      onClose={onClose}
      title={formData.id ? t("voices.extra.editor.editTitle") : t("voices.extra.editor.createTitle")}
    >
      <div className="space-y-4 pb-2">
        {/* Voice Name */}
        <div>
          <label className="mb-1 block text-[11px] font-medium text-fg/70">
            {t("voices.extra.editor.voiceName")}
          </label>
          <input
            type="text"
            value={formData.name}
            onChange={(e) => setFormData({ ...formData, name: e.target.value })}
            placeholder={t("voices.extra.editor.voiceNamePlaceholder")}
            className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
          />
        </div>

        {/* Provider Selection */}
        <div>
          <label className="mb-1 block text-[11px] font-medium text-fg/70">
            {t("voices.extra.editor.provider")}
          </label>
          <select
            value={formData.providerId}
            onChange={(e) => setFormData({ ...formData, providerId: e.target.value, modelId: "" })}
            className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg focus:border-fg/30 focus:outline-none"
          >
            {providers.map((p) => (
              <option key={p.id} value={p.id} className="bg-surface-el">
                {p.label}
              </option>
            ))}
          </select>
        </div>

        {/* Model Selection */}
        <div>
          <label className="mb-1 block text-[11px] font-medium text-fg/70">
            {t("voices.extra.editor.model")}
          </label>
          {activeProvider?.providerType === "openai_tts" ? (
            <>
              <input
                type="text"
                value={formData.modelId}
                onChange={(e) => setFormData({ ...formData, modelId: e.target.value })}
                placeholder={t("voices.extra.editor.modelIdPlaceholder")}
                className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
              />
              <p className="mt-1 text-[10px] text-fg/40">
                {t("voices.extra.editor.modelIdHint")}
              </p>
            </>
          ) : (
            <select
              value={formData.modelId}
              onChange={(e) => setFormData({ ...formData, modelId: e.target.value })}
              className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg focus:border-fg/30 focus:outline-none"
              disabled={models.length === 0}
            >
              {models.length === 0 ? (
                <option value="" className="bg-surface-el">
                  {t("characters.description.loadingModels")}
                </option>
              ) : (
                models.map((m) => (
                  <option key={m.id} value={m.id} className="bg-surface-el">
                    {m.name}
                  </option>
                ))
              )}
            </select>
          )}
        </div>

        {usesProviderVoicePicker && !isElevenLabsVoiceDesign && (
          <div>
            <div className="mb-1 flex items-center justify-between">
              <label className="text-[11px] font-medium text-fg/70">
                {activeProvider?.providerType === "fish_tts"
                  ? t("voices.extra.editor.fishVoice")
                  : t("voices.extra.editor.elevenlabsVoice")}
              </label>
              <button
                type="button"
                onClick={async () => {
                  setIsLoadingVoices(true);
                  try {
                    const voices = await refreshProviderVoices(formData.providerId);
                    setProviderVoices(voices);
                  } catch (e) {
                    console.error("Failed to refresh voices:", e);
                  } finally {
                    setIsLoadingVoices(false);
                  }
                }}
                disabled={isLoadingVoices}
                className="flex items-center gap-1 text-[10px] text-fg/50 hover:text-fg/70 disabled:opacity-50"
              >
                <RefreshCw className={`h-3 w-3 ${isLoadingVoices ? "animate-spin" : ""}`} />
                {t("common.buttons.refresh")}
              </button>
            </div>
            <select
              value={formData.voiceId}
              onChange={(e) => {
                const selectedVoice = providerVoices.find((v) => v.voiceId === e.target.value);
                setFormData({
                  ...formData,
                  voiceId: e.target.value,
                  name: formData.name || selectedVoice?.name || "",
                });
              }}
              className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg focus:border-fg/30 focus:outline-none"
              disabled={isLoadingVoices}
            >
              {isLoadingVoices ? (
                <option value="" className="bg-surface-el">
                  {t("characters.description.loadingVoices")}
                </option>
              ) : providerVoices.length === 0 ? (
                <option value="" className="bg-surface-el">
                  {t("voices.extra.editor.noVoicesAvailable")}
                </option>
              ) : (
                <>
                  <option value="" className="bg-surface-el">
                    {t("voices.extra.editor.selectVoice")}
                  </option>
                  {providerVoices.map((v) => (
                    <option key={v.voiceId} value={v.voiceId} className="bg-surface-el">
                      {v.name}
                      {v.labels?.category ? ` (${v.labels.category})` : ""}
                    </option>
                  ))}
                </>
              )}
            </select>
            <p className="mt-1 text-[10px] text-fg/40">
              {activeProvider?.providerType === "fish_tts"
                ? t("voices.extra.editor.fishVoiceHint")
                : t("voices.extra.editor.elevenlabsVoiceHint")}
            </p>
          </div>
        )}

        {/* Gemini Voice Selection */}
        {activeProvider?.providerType === "gemini_tts" && (
          <div>
            <label className="mb-1 block text-[11px] font-medium text-fg/70">
              {t("voices.extra.editor.geminiVoice")}
            </label>
            <select
              value={formData.voiceId}
              onChange={(e) => setFormData({ ...formData, voiceId: e.target.value })}
              className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg focus:border-fg/30 focus:outline-none"
            >
              {GEMINI_VOICES.map((v) => (
                <option key={v.id} value={v.id} className="bg-surface-el">
                  {v.name} ({t(v.descriptionKey)})
                </option>
              ))}
            </select>
            <p className="mt-1 text-[10px] text-fg/40">
              {t("voices.extra.editor.geminiVoiceHint")}
            </p>
          </div>
        )}

        {requiresManualVoiceId && (
          <div>
            <label className="mb-1 block text-[11px] font-medium text-fg/70">
              {t("voices.extra.editor.voiceId")}
            </label>
            <input
              type="text"
              value={formData.voiceId}
              onChange={(e) => setFormData({ ...formData, voiceId: e.target.value })}
              placeholder={
                activeProvider?.providerType === "fish_tts"
                  ? t("voices.extra.editor.fishVoiceIdPlaceholder")
                  : activeProvider?.providerType === "fish_speech"
                    ? t("voices.extra.editor.fishSpeechVoiceIdPlaceholder")
                  : t("voices.extra.editor.voiceIdPlaceholder")
              }
              className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
            />
            <p className="mt-1 text-[10px] text-fg/40">
              {activeProvider?.providerType === "fish_tts"
                ? t("voices.extra.editor.fishVoiceIdHint")
                : activeProvider?.providerType === "fish_speech"
                  ? t("voices.extra.editor.fishSpeechVoiceIdHint")
                : t("voices.extra.editor.voiceIdHint")}
            </p>
          </div>
        )}

        {/* Prompt */}
        <div>
          <label className="mb-1 block text-[11px] font-medium text-fg/70">
            {t("voices.extra.editor.voicePrompt")}
          </label>
          <textarea
            value={formData.prompt ?? ""}
            onChange={(e) => setFormData({ ...formData, prompt: e.target.value })}
            placeholder={t("voices.extra.editor.voicePromptPlaceholder")}
            rows={3}
            className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none resize-none"
          />
          <p className="mt-1 text-[10px] text-fg/40">{t("voices.extra.editor.voicePromptHint")}</p>
        </div>

        {/* Example Text */}
        <div>
          <label className="mb-1 block text-[11px] font-medium text-fg/70">
            {t("voices.extra.editor.exampleText")}
          </label>
          <textarea
            value={textSample}
            onChange={(e) => {
              setTextSample(e.target.value);
              if (previewError) setPreviewError(null);
            }}
            placeholder={t("voices.extra.editor.exampleTextPlaceholder")}
            rows={2}
            className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none resize-none"
          />
          <p className="mt-1 text-[10px] text-fg/40">{t("voices.extra.editor.exampleTextHint")}</p>
          {isElevenLabsVoiceDesign && (
            <p
              className={`mt-1 text-[10px] ${sampleLength < minVoiceDesignChars ? "text-danger/80" : "text-fg/40"}`}
            >
              {t("voices.extra.editor.voiceDesignChars", {
                current: sampleLength,
                minimum: minVoiceDesignChars,
              })}
            </p>
          )}
          {previewError && (
            <p className="mt-1 text-xs font-medium text-danger/80">{previewError}</p>
          )}
        </div>

        {/* Preview Button */}
        <button
          onClick={() => void handlePreview()}
          disabled={previewDisabled}
          className="flex w-full items-center justify-center gap-2 rounded-lg border border-accent/30 bg-accent/10 px-4 py-2 text-sm font-medium text-accent/80 transition hover:border-accent/50 hover:bg-accent/20 disabled:opacity-50"
        >
          {isPlaying ? (
            <>
              <Loader2 className="h-4 w-4 animate-spin" />
              {t("voices.extra.editor.playing")}
            </>
          ) : (
            <>
              <Play className="h-4 w-4" />
              {t("voices.extra.editor.previewVoice")}
            </>
          )}
        </button>

        {/* Actions */}
        <div className="flex gap-3 pt-1">
          <button
            onClick={onClose}
            className="flex-1 rounded-lg border border-fg/10 bg-fg/5 px-4 py-2 text-sm font-medium text-fg/70 transition hover:border-fg/20 hover:bg-fg/10 hover:text-fg"
          >
            {t("common.buttons.cancel")}
          </button>
          <button
            onClick={() => void handleSave()}
            disabled={isSaving || !formData.name.trim()}
            className="flex-1 rounded-lg border border-accent/40 bg-accent/20 px-4 py-2 text-sm font-semibold text-accent/90 transition hover:border-accent/60 hover:bg-accent/30 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {isSaving ? t("common.buttons.saving") : t("common.buttons.save")}
          </button>
        </div>
      </div>
    </BottomMenu>
  );
}
