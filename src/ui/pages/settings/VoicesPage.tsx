import { useState, useEffect, useCallback } from "react";
import {
  ChevronRight,
  Plus,
  Trash2,
  Play,
  Mic,
  Volume2,
  Loader2,
  Edit3,
  RefreshCw,
  HardDrive,
} from "lucide-react";

import {
  listAudioProviders,
  listUserVoices,
  upsertUserVoice,
  deleteUserVoice,
  upsertAudioProvider,
  deleteAudioProvider,
  listAudioModels,
  listVoiceDesignModels,
  refreshProviderVoices,
  getProviderVoices,
  verifyAudioProvider,
  generateTtsPreview,
  playAudioFromBase64,
  designVoicePreview,
  createVoiceFromPreview,
  getTtsCacheStats,
  clearTtsCache,
  DEVICE_TTS_PROVIDER_ID,
  type AudioProvider,
  type AudioProviderType,
  type AudioModel,
  type UserVoice,
  type CachedVoice,
  type TtsCacheStats,
} from "../../../core/storage/audioProviders";

import { BottomMenu, MenuButton } from "../../components/BottomMenu";

export function VoicesPage() {
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

  // Provider editor state
  const [isProviderEditorOpen, setIsProviderEditorOpen] = useState(false);
  const [editingProvider, setEditingProvider] = useState<AudioProvider | null>(null);

  // Selection menu state
  const [selectedVoice, setSelectedVoice] = useState<UserVoice | null>(null);
  const [selectedProvider, setSelectedProvider] = useState<AudioProvider | null>(null);
  const [selectedCustomVoice, setSelectedCustomVoice] = useState<
    (CachedVoice & { provider: AudioProvider }) | null
  >(null);
  const editableProviders = providers.filter((provider) => provider.providerType !== "device_tts");
  const isDeviceProvider = (provider: AudioProvider | null) =>
    !!provider &&
    (provider.providerType === "device_tts" || provider.id === DEVICE_TTS_PROVIDER_ID);

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

      // Load voices for each editable provider
      const voicesMap: Record<string, CachedVoice[]> = {};
      for (const provider of loadedProviders) {
        if (provider.providerType !== "device_tts") {
          try {
            const voices = await getProviderVoices(provider.id);
            voicesMap[provider.id] = voices;
          } catch (e) {
            console.error(`Failed to load voices for ${provider.label}:`, e);
            voicesMap[provider.id] = [];
          }
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
    const firstEditable = editableProviders[0];
    if (!firstEditable) {
      return;
    }
    setEditingVoice({
      id: "",
      providerId: firstEditable.id,
      name: "",
      modelId: "",
      voiceId: "", // Not used but required by type
      prompt: "",
    });
    setIsVoiceEditorOpen(true);
  };

  const handleEditVoice = (voice: UserVoice) => {
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

  const handleCreateProvider = useCallback(() => {
    setEditingProvider({
      id: "",
      providerType: "elevenlabs",
      label: "",
      apiKey: "",
    });
    setIsProviderEditorOpen(true);
  }, []);

  const handleEditProvider = (provider: AudioProvider) => {
    if (provider.providerType === "device_tts") {
      return;
    }
    setEditingProvider({ ...provider });
    setIsProviderEditorOpen(true);
    setSelectedProvider(null);
  };

  const handleDeleteProvider = async (id: string) => {
    if (id === DEVICE_TTS_PROVIDER_ID) {
      return;
    }
    try {
      await deleteAudioProvider(id);
      await loadData();
      setSelectedProvider(null);
    } catch (e) {
      console.error("Failed to delete provider:", e);
    }
  };

  useEffect(() => {
    const listener = () => handleCreateProvider();
    window.addEventListener("audioProviders:add", listener);
    return () => {
      window.removeEventListener("audioProviders:add", listener);
    };
  }, [handleCreateProvider]);

  if (isLoading) {
    return (
      <div className="flex h-64 items-center justify-center">
        <Loader2 className="h-8 w-8 animate-spin text-fg/40" />
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col space-y-6">
      {/* Audio Providers Section */}
      <section>
        <div className="mb-2 flex items-center justify-between px-1">
          <h2 className="text-xs font-medium uppercase tracking-wider text-fg/40">
            Audio Providers
          </h2>
          <button
            onClick={handleCreateProvider}
            className="flex items-center gap-1 rounded-lg border border-fg/10 bg-fg/5 px-2 py-1 text-xs text-fg/70 transition hover:border-fg/20 hover:bg-fg/10"
          >
            <Plus className="h-3 w-3" />
            Add
          </button>
        </div>

        {providers.length === 0 ? (
          <div className="flex flex-col items-center justify-center rounded-xl border border-dashed border-fg/10 py-8">
            <Mic className="mb-2 h-8 w-8 text-fg/20" />
            <p className="text-sm text-fg/50">No audio providers configured</p>
            <p className="text-xs text-fg/30">Add Gemini TTS or ElevenLabs to get started</p>
          </div>
        ) : (
          <div className="space-y-2">
            {providers.map((provider) => (
              <button
                key={provider.id}
                onClick={() => setSelectedProvider(provider)}
                className="group w-full rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-left transition hover:border-fg/20 hover:bg-fg/10"
              >
                <div className="flex items-center gap-3">
                  <div className="flex h-8 w-8 items-center justify-center rounded-full border border-fg/10 bg-fg/10">
                    {provider.providerType === "gemini_tts" ? (
                      <span className="text-xs">G</span>
                    ) : provider.providerType === "device_tts" ? (
                      <span className="text-[10px]">OS</span>
                    ) : (
                      <span className="text-xs">11</span>
                    )}
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="truncate text-sm font-medium text-fg">{provider.label}</p>
                    <p className="text-xs text-fg/50">
                      {provider.providerType === "gemini_tts"
                        ? "Gemini TTS"
                        : provider.providerType === "device_tts"
                          ? "Device TTS"
                          : "ElevenLabs"}
                    </p>
                  </div>
                  <ChevronRight className="h-4 w-4 text-fg/30 group-hover:text-fg/60" />
                </div>
              </button>
            ))}
          </div>
        )}
      </section>

      {/* My Voices Section (Voice Designer + Custom Provider Voices) */}
      <section>
        <div className="mb-2 flex items-center justify-between px-1">
          <h2 className="text-xs font-medium uppercase tracking-wider text-fg/40">My Voices</h2>
          {editableProviders.length > 0 && (
            <button
              onClick={handleCreateVoice}
              className="flex items-center gap-1 rounded-lg border border-fg/10 bg-fg/5 px-2 py-1 text-xs text-fg/70 transition hover:border-fg/20 hover:bg-fg/10"
            >
              <Plus className="h-3 w-3" />
              Create
            </button>
          )}
        </div>

        {userVoices.length === 0 && customProviderVoices.length === 0 ? (
          <div className="flex flex-col items-center justify-center rounded-xl border border-dashed border-fg/10 py-8">
            <Volume2 className="mb-2 h-8 w-8 text-fg/20" />
            <p className="text-sm text-fg/50">No voices created yet</p>
            <p className="text-xs text-fg/30">
              {editableProviders.length > 0
                ? "Create voices with custom prompts for your characters"
                : "Add an audio provider first"}
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
                        {voice.prompt ? `"${voice.prompt.slice(0, 30)}..."` : "No prompt"}
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
                      <span className="capitalize">{voice.labels?.category || "custom"}</span>
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
      {editableProviders.length > 0 && (
        <section>
          <div className="mb-2 px-1">
            <h2 className="text-xs font-medium uppercase tracking-wider text-fg/40">
              Provider Voices
            </h2>
          </div>

          <div className="space-y-4">
            {editableProviders.map((provider) => {
              const voices = getPremadeVoices(provider.id);
              const isLoadingThis = loadingVoicesFor === provider.id;
              const isExpanded = expandedProviders[provider.id] ?? false;
              const displayedVoices = isExpanded ? voices : voices.slice(0, 10);

              return (
                <div key={provider.id} className="rounded-xl border border-fg/10 bg-fg/5 p-3">
                  <div className="mb-2 flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <div className="flex h-6 w-6 items-center justify-center rounded-full border border-fg/10 bg-fg/10">
                        {provider.providerType === "gemini_tts" ? (
                          <span className="text-[10px]">G</span>
                        ) : (
                          <span className="text-[10px]">11</span>
                        )}
                      </div>
                      <span className="text-sm font-medium text-fg">{provider.label}</span>
                      <span className="text-xs text-fg/40">({voices.length} voices)</span>
                    </div>
                    <button
                      onClick={() => void handleRefreshProviderVoices(provider.id)}
                      disabled={isLoadingThis}
                      className="flex items-center gap-1 rounded-lg border border-fg/10 bg-fg/5 px-2 py-1 text-[10px] text-fg/60 transition hover:border-fg/20 hover:bg-fg/10 disabled:opacity-50"
                    >
                      <RefreshCw className={`h-3 w-3 ${isLoadingThis ? "animate-spin" : ""}`} />
                      Refresh
                    </button>
                  </div>

                  {isLoadingThis ? (
                    <div className="flex items-center justify-center py-4">
                      <Loader2 className="h-5 w-5 animate-spin text-fg/40" />
                    </div>
                  ) : voices.length === 0 ? (
                    <p className="py-2 text-center text-xs text-fg/40">
                      No voices found. Click Refresh to fetch voices.
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
                            <p className="truncate text-xs font-medium text-fg/80">
                              {voice.name}
                            </p>
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
                      {isExpanded ? "Show Less" : `Show All ${voices.length} Voices`}
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
            Audio Cache
          </h2>
        </div>
        <div className="rounded-xl border border-fg/10 bg-fg/5 p-4">
          <div className="flex items-start gap-3">
            <div className="flex h-8 w-8 items-center justify-center rounded-full border border-fg/10 bg-fg/10">
              <HardDrive className="h-4 w-4 text-fg/70" />
            </div>
            <div className="flex-1 min-w-0">
              <p className="text-sm font-medium text-fg">TTS Audio Cache</p>
              <p className="text-xs text-fg/50 mt-0.5">
                Generated voice audio is cached to reduce regenerations
              </p>
              {cacheStats && (
                <div className="mt-2 flex items-center gap-3 text-xs text-fg/60">
                  <span>
                    {cacheStats.count} file{cacheStats.count !== 1 ? "s" : ""}
                  </span>
                  <span>•</span>
                  <span>{formatBytes(cacheStats.sizeBytes)}</span>
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
                Clearing...
              </>
            ) : (
              <>
                <Trash2 className="h-3 w-3" />
                Clear Cache
              </>
            )}
          </button>
        </div>
      </section>

      {/* Voice Selection Menu */}
      <BottomMenu
        isOpen={!!selectedVoice}
        onClose={() => setSelectedVoice(null)}
        title={selectedVoice?.name || "Voice"}
      >
        {selectedVoice && (
          <div className="space-y-4">
            <MenuButton
              icon={Edit3}
              title="Edit"
              description="Modify this voice"
              onClick={() => handleEditVoice(selectedVoice)}
              color="from-info to-info/80"
            />
            <MenuButton
              icon={Trash2}
              title="Delete"
              description="Remove this voice"
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
        title={selectedCustomVoice?.name || "Voice"}
      >
        {selectedCustomVoice && (
          <div className="space-y-4">
            <div className="rounded-lg border border-fg/10 bg-fg/5 p-3">
              <p className="text-xs text-fg/50">Provider</p>
              <p className="text-sm text-fg">{selectedCustomVoice.provider.label}</p>
              <p className="mt-2 text-xs text-fg/50">Category</p>
              <p className="text-sm capitalize text-fg">
                {selectedCustomVoice.labels?.category || "custom"}
              </p>
              {selectedCustomVoice.labels?.description && (
                <>
                  <p className="mt-2 text-xs text-fg/50">Description</p>
                  <p className="text-sm text-fg">{selectedCustomVoice.labels.description}</p>
                </>
              )}
            </div>
            <MenuButton
              icon={Plus}
              title="Create Voice Config"
              description="Use this voice with custom settings"
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

      {/* Provider Selection Menu */}
      <BottomMenu
        isOpen={!!selectedProvider}
        onClose={() => setSelectedProvider(null)}
        title={selectedProvider?.label || "Provider"}
      >
        {selectedProvider &&
          (isDeviceProvider(selectedProvider) ? (
            <p className="text-sm text-fg/60">This is a built-in system provider.</p>
          ) : (
            <div className="space-y-4">
              <MenuButton
                icon={Edit3}
                title="Edit"
                description="Modify provider settings"
                onClick={() => handleEditProvider(selectedProvider)}
                color="from-info to-info/80"
              />
              <MenuButton
                icon={Trash2}
                title="Delete"
                description="Remove this provider"
                onClick={() => void handleDeleteProvider(selectedProvider.id)}
                color="from-danger to-danger/80"
              />
            </div>
          ))}
      </BottomMenu>

      {/* Voice Editor (Voice Designer) */}
      <VoiceEditor
        isOpen={isVoiceEditorOpen}
        voice={editingVoice}
        providers={editableProviders}
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

      {/* Provider Editor */}
      <ProviderEditor
        isOpen={isProviderEditorOpen}
        provider={editingProvider}
        onClose={() => {
          setIsProviderEditorOpen(false);
          setEditingProvider(null);
        }}
        onSave={async (provider) => {
          await upsertAudioProvider(provider);
          await loadData();
          setIsProviderEditorOpen(false);
          setEditingProvider(null);
        }}
      />
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
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
  const [formData, setFormData] = useState<UserVoice | null>(null);
  const [models, setModels] = useState<AudioModel[]>([]);
  const [providerVoices, setProviderVoices] = useState<CachedVoice[]>([]);
  const [isLoadingVoices, setIsLoadingVoices] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isPlaying, setIsPlaying] = useState(false);

  const [generatedPreviewId, setGeneratedPreviewId] = useState<string | null>(null);
  const [textSample, setTextSample] = useState(
    "Hello! This is how I sound when speaking. I can read longer passages with warmth, clarity, and emotion so you can judge my tone and pace.",
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
        "Hello! This is how I sound when speaking. I can read longer passages with warmth, clarity, and emotion so you can judge my tone and pace.",
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

    if (provider.providerType === "elevenlabs") {
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
          throw new Error("Please preview the voice first to generate it.");
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

      await onSave(finalVoiceData);
    } catch (e) {
      console.error("Failed to save voice:", e);
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
        `Example text must be at least ${minVoiceDesignChars} characters for voice design.`,
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
            throw new Error("Unable to resolve ElevenLabs voice ID. Please recreate the voice.");
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
      console.error("TTS preview failed:", e);
    } finally {
      setIsPlaying(false);
    }
  };

  if (!formData) return null;
  const activeProvider = providers.find((p) => p.id === formData.providerId);
  const isNewVoice = !formData.id;
  const isElevenLabsVoiceDesign = activeProvider?.providerType === "elevenlabs" && isNewVoice;
  const requiresVoiceId = activeProvider?.providerType === "elevenlabs" && !isElevenLabsVoiceDesign;
  const sampleLength = textSample.trim().length;
  const previewDisabled =
    isPlaying ||
    !formData.providerId ||
    !textSample.trim() ||
    (isElevenLabsVoiceDesign && sampleLength < minVoiceDesignChars) ||
    (!isElevenLabsVoiceDesign &&
      (!formData.modelId || (requiresVoiceId && !formData.voiceId && !formData.name.trim())));

  return (
    <BottomMenu
      isOpen={isOpen}
      onClose={onClose}
      title={formData.id ? "Edit Voice" : "Create Voice"}
    >
      <div className="space-y-4 pb-2">
        {/* Voice Name */}
        <div>
          <label className="mb-1 block text-[11px] font-medium text-fg/70">Voice Name</label>
          <input
            type="text"
            value={formData.name}
            onChange={(e) => setFormData({ ...formData, name: e.target.value })}
            placeholder="My Character Voice"
            className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
          />
        </div>

        {/* Provider Selection */}
        <div>
          <label className="mb-1 block text-[11px] font-medium text-fg/70">Provider</label>
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

        {/* Model Selection - shown for all providers */}
        <div>
          <label className="mb-1 block text-[11px] font-medium text-fg/70">Model</label>
          <select
            value={formData.modelId}
            onChange={(e) => setFormData({ ...formData, modelId: e.target.value })}
            className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg focus:border-fg/30 focus:outline-none"
            disabled={models.length === 0}
          >
            {models.length === 0 ? (
              <option value="" className="bg-surface-el">
                Loading models...
              </option>
            ) : (
              models.map((m) => (
                <option key={m.id} value={m.id} className="bg-surface-el">
                  {m.name}
                </option>
              ))
            )}
          </select>
        </div>

        {activeProvider?.providerType === "elevenlabs" && !isElevenLabsVoiceDesign && (
          <div>
            <div className="mb-1 flex items-center justify-between">
              <label className="text-[11px] font-medium text-fg/70">ElevenLabs Voice</label>
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
                Refresh
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
                  Loading voices...
                </option>
              ) : providerVoices.length === 0 ? (
                <option value="" className="bg-surface-el">
                  No voices available
                </option>
              ) : (
                <>
                  <option value="" className="bg-surface-el">
                    Select a voice...
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
            <p className="mt-1 text-[10px] text-fg/40">Select from your ElevenLabs voices</p>
          </div>
        )}

        {/* Gemini Voice Selection */}
        {activeProvider?.providerType === "gemini_tts" && (
          <div>
            <label className="mb-1 block text-[11px] font-medium text-fg/70">Gemini Voice</label>
            <select
              value={formData.voiceId}
              onChange={(e) => setFormData({ ...formData, voiceId: e.target.value })}
              className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg focus:border-fg/30 focus:outline-none"
            >
              <option value="kore" className="bg-surface-el">
                Kore (Warm and friendly)
              </option>
              <option value="aoede" className="bg-surface-el">
                Aoede (Bright and articulate)
              </option>
              <option value="charon" className="bg-surface-el">
                Charon (Deep and authoritative)
              </option>
              <option value="fenrir" className="bg-surface-el">
                Fenrir (Strong and bold)
              </option>
              <option value="puck" className="bg-surface-el">
                Puck (Energetic and youthful)
              </option>
              <option value="leda" className="bg-surface-el">
                Leda (Calm and soothing)
              </option>
              <option value="orus" className="bg-surface-el">
                Orus (Warm and resonant)
              </option>
              <option value="zephyr" className="bg-surface-el">
                Zephyr (Light and airy)
              </option>
              <option value="algieba" className="bg-surface-el">
                Algieba (Professional and clear)
              </option>
              <option value="callirrhoe" className="bg-surface-el">
                Callirrhoe (Expressive and dynamic)
              </option>
            </select>
            <p className="mt-1 text-[10px] text-fg/40">Select a Gemini TTS voice</p>
          </div>
        )}

        {/* Prompt */}
        <div>
          <label className="mb-1 block text-[11px] font-medium text-fg/70">Voice Prompt</label>
          <textarea
            value={formData.prompt ?? ""}
            onChange={(e) => setFormData({ ...formData, prompt: e.target.value })}
            placeholder="A warm, friendly voice with a cheerful tone..."
            rows={3}
            className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none resize-none"
          />
          <p className="mt-1 text-[10px] text-fg/40">Describe how the voice should sound</p>
        </div>

        {/* Example Text */}
        <div>
          <label className="mb-1 block text-[11px] font-medium text-fg/70">Example Text</label>
          <textarea
            value={textSample}
            onChange={(e) => {
              setTextSample(e.target.value);
              if (previewError) setPreviewError(null);
            }}
            placeholder="Hello! This is how I sound when speaking..."
            rows={2}
            className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none resize-none"
          />
          <p className="mt-1 text-[10px] text-fg/40">Sample text for testing the voice</p>
          {isElevenLabsVoiceDesign && (
            <p
              className={`mt-1 text-[10px] ${sampleLength < minVoiceDesignChars ? "text-danger/80" : "text-fg/40"}`}
            >
              {sampleLength}/{minVoiceDesignChars} characters required for voice design preview
            </p>
          )}
          {previewError && <p className="mt-1 text-xs font-medium text-danger/80">{previewError}</p>}
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
              Playing...
            </>
          ) : (
            <>
              <Play className="h-4 w-4" />
              Preview Voice
            </>
          )}
        </button>

        {/* Actions */}
        <div className="flex gap-3 pt-1">
          <button
            onClick={onClose}
            className="flex-1 rounded-lg border border-fg/10 bg-fg/5 px-4 py-2 text-sm font-medium text-fg/70 transition hover:border-fg/20 hover:bg-fg/10 hover:text-fg"
          >
            Cancel
          </button>
          <button
            onClick={() => void handleSave()}
            disabled={isSaving || !formData.name.trim()}
            className="flex-1 rounded-lg border border-accent/40 bg-accent/20 px-4 py-2 text-sm font-semibold text-accent/90 transition hover:border-accent/60 hover:bg-accent/30 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {isSaving ? "Saving..." : "Save"}
          </button>
        </div>
      </div>
    </BottomMenu>
  );
}

interface ProviderEditorProps {
  isOpen: boolean;
  provider: AudioProvider | null;
  onClose: () => void;
  onSave: (provider: AudioProvider) => Promise<void>;
}

function ProviderEditor({ isOpen, provider, onClose, onSave }: ProviderEditorProps) {
  const [formData, setFormData] = useState<AudioProvider | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [validationError, setValidationError] = useState<string | null>(null);

  useEffect(() => {
    if (provider) {
      setFormData({ ...provider });
      setValidationError(null);
    }
  }, [provider]);

  const handleSave = async () => {
    if (!formData || !formData.label.trim()) return;

    if (!formData.apiKey?.trim()) {
      setValidationError("API key is required");
      return;
    }

    if (formData.providerType === "gemini_tts" && !formData.projectId?.trim()) {
      setValidationError("Project ID is required for Gemini TTS");
      return;
    }

    setIsSaving(true);
    setValidationError(null);

    try {
      const isValid = await verifyAudioProvider(
        formData.providerType,
        formData.apiKey,
        formData.projectId,
      );

      if (!isValid) {
        setValidationError("Invalid API key or credentials");
        return;
      }

      await onSave(formData);
    } catch (e) {
      setValidationError(e instanceof Error ? e.message : "Verification failed");
    } finally {
      setIsSaving(false);
    }
  };

  if (!formData) return null;

  return (
    <BottomMenu
      isOpen={isOpen}
      onClose={onClose}
      title={formData.id ? "Edit Provider" : "Add Audio Provider"}
    >
      <div className="space-y-4 pb-2">
        {/* Provider Type */}
        <div>
          <label className="mb-1 block text-[11px] font-medium text-fg/70">Provider Type</label>
          <select
            value={formData.providerType}
            onChange={(e) =>
              setFormData({
                ...formData,
                providerType: e.target.value as AudioProviderType,
                projectId: undefined,
                location: undefined,
              })
            }
            className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg focus:border-fg/30 focus:outline-none"
          >
            <option value="elevenlabs" className="bg-surface-el">
              ElevenLabs
            </option>
            <option value="gemini_tts" className="bg-surface-el">
              Gemini TTS (Google)
            </option>
          </select>
        </div>

        {/* Label */}
        <div>
          <label className="mb-1 block text-[11px] font-medium text-fg/70">Label</label>
          <input
            type="text"
            value={formData.label}
            onChange={(e) => setFormData({ ...formData, label: e.target.value })}
            placeholder={formData.providerType === "gemini_tts" ? "My Gemini TTS" : "My ElevenLabs"}
            className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
          />
        </div>

        {/* API Key */}
        <div>
          <label className="mb-1 block text-[11px] font-medium text-fg/70">API Key</label>
          <input
            type="password"
            value={formData.apiKey ?? ""}
            onChange={(e) => {
              setFormData({ ...formData, apiKey: e.target.value });
              if (validationError) setValidationError(null);
            }}
            placeholder="Enter your API key"
            className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
          />
        </div>

        {/* Gemini-specific fields */}
        {formData.providerType === "gemini_tts" && (
          <>
            <div>
              <label className="mb-1 block text-[11px] font-medium text-fg/70">
                Google Cloud Project ID
              </label>
              <input
                type="text"
                value={formData.projectId ?? ""}
                onChange={(e) => {
                  setFormData({ ...formData, projectId: e.target.value });
                  if (validationError) setValidationError(null);
                }}
                placeholder="your-project-id"
                className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
              />
            </div>
            <div>
              <label className="mb-1 block text-[11px] font-medium text-fg/70">
                Region (optional)
              </label>
              <input
                type="text"
                value={formData.location ?? ""}
                onChange={(e) => setFormData({ ...formData, location: e.target.value })}
                placeholder="us-central1"
                className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
              />
            </div>
          </>
        )}

        {validationError && <p className="text-xs font-medium text-danger/80">{validationError}</p>}

        {/* Actions */}
        <div className="flex gap-3 pt-1">
          <button
            onClick={onClose}
            className="flex-1 rounded-lg border border-fg/10 bg-fg/5 px-4 py-2 text-sm font-medium text-fg/70 transition hover:border-fg/20 hover:bg-fg/10 hover:text-fg"
          >
            Cancel
          </button>
          <button
            onClick={() => void handleSave()}
            disabled={isSaving || !formData.label.trim()}
            className="flex-1 rounded-lg border border-accent/40 bg-accent/20 px-4 py-2 text-sm font-semibold text-accent/90 transition hover:border-accent/60 hover:bg-accent/30 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {isSaving ? "Verifying..." : "Save"}
          </button>
        </div>
      </div>
    </BottomMenu>
  );
}
