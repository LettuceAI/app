import React from "react";
import { useParams, useNavigate } from "react-router-dom";
import {
  Loader2,
  Plus,
  X,
  Sparkles,
  BookOpen,
  Cpu,
  Image,
  Download,
  Layers,
  Edit2,
  ChevronDown,
  Crop,
  Upload,
  User,
  Settings,
  Volume2,
  EyeOff,
  Check,
  Info,
  MessageSquare,
  ChevronRight,
} from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { useEditCharacterForm } from "./hooks/useEditCharacterForm";
import { AvatarPicker } from "../../components/AvatarPicker";
import { BottomMenu, MenuSection } from "../../components/BottomMenu";
import { BackgroundPositionModal } from "../../components/BackgroundPositionModal";
import { CharacterExportMenu } from "../../components/CharacterExportMenu";
import { cn, radius, colors, interactive } from "../../design-tokens";
import { getProviderIcon } from "../../../core/utils/providerIcons";
import type { CharacterFileFormat } from "../../../core/storage/characterTransfer";
import {
  listAudioProviders,
  listUserVoices,
  getProviderVoices,
  refreshProviderVoices,
  type AudioProvider,
  type CachedVoice,
  type UserVoice,
} from "../../../core/storage/audioProviders";

const wordCount = (text: string) => {
  const trimmed = text.trim();
  if (!trimmed) return 0;
  return trimmed.split(/\s+/).length;
};

export function EditCharacterPage() {
  const { characterId } = useParams();
  const navigate = useNavigate();
  const { state, actions, computed } = useEditCharacterForm(characterId);
  const [expandedSceneId, setExpandedSceneId] = React.useState<string | null>(null);
  const [newSceneEditorOpen, setNewSceneEditorOpen] = React.useState(false);

  // Background image positioning state
  const [pendingBackgroundSrc, setPendingBackgroundSrc] = React.useState<string | null>(null);
  const [showBackgroundChoiceMenu, setShowBackgroundChoiceMenu] = React.useState(false);
  const [showBackgroundPositionModal, setShowBackgroundPositionModal] = React.useState(false);

  // Tab state
  const [activeTab, setActiveTab] = React.useState<"character" | "settings">("character");
  const [showModelMenu, setShowModelMenu] = React.useState(false);
  const [modelSearchQuery, setModelSearchQuery] = React.useState("");
  const [showFallbackModelMenu, setShowFallbackModelMenu] = React.useState(false);
  const [fallbackModelSearchQuery, setFallbackModelSearchQuery] = React.useState("");
  const [showVoiceMenu, setShowVoiceMenu] = React.useState(false);
  const [voiceSearchQuery, setVoiceSearchQuery] = React.useState("");
  const [exportMenuOpen, setExportMenuOpen] = React.useState(false);
  const tabsId = React.useId();
  const tabPanelId = `${tabsId}-panel`;
  const characterTabId = `${tabsId}-tab-character`;
  const settingsTabId = `${tabsId}-tab-settings`;

  const {
    loading,
    saving,
    exporting,
    error,
    name,
    definition,
    description,
    nickname,
    creator,
    creatorNotes,
    creatorNotesMultilingualText,
    tagsText,
    avatarPath,
    avatarCrop,
    avatarRoundPath,
    backgroundImagePath,
    scenes,
    defaultSceneId,
    newSceneContent,
    newSceneDirection,
    selectedModelId,
    selectedFallbackModelId,

    disableAvatarGradient,
    customGradientEnabled,
    customGradientColors,
    customTextColor: _customTextColor,
    customTextSecondary: _customTextSecondary,
    memoryType,
    dynamicMemoryEnabled,
    models,
    loadingModels,
    promptTemplates,
    loadingTemplates,
    systemPromptTemplateId,
    voiceConfig,
    voiceAutoplay,

    editingSceneId,
    editingSceneContent,
    editingSceneDirection,
  } = state;

  const {
    setFields,
    handleSave,
    handleExport,
    addScene,
    deleteScene,
    startEditingScene,
    saveEditedScene,
    cancelEditingScene,
    resetToInitial,
  } = actions;

  const { avatarInitial, canSave } = computed;

  const closeNewSceneEditor = React.useCallback(() => {
    setFields({ newSceneContent: "", newSceneDirection: "" });
    setNewSceneEditorOpen(false);
  }, [setFields]);

  const saveNewScene = React.useCallback(() => {
    if (!newSceneContent.trim()) return;
    addScene();
    setNewSceneEditorOpen(false);
  }, [addScene, newSceneContent]);

  const handleExportFormat = React.useCallback(
    async (format: CharacterFileFormat) => {
      await handleExport(format);
      setExportMenuOpen(false);
    },
    [handleExport],
  );

  const [audioProviders, setAudioProviders] = React.useState<AudioProvider[]>([]);
  const [userVoices, setUserVoices] = React.useState<UserVoice[]>([]);
  const [providerVoices, setProviderVoices] = React.useState<Record<string, CachedVoice[]>>({});
  const [loadingVoices, setLoadingVoices] = React.useState(false);
  const [voiceError, setVoiceError] = React.useState<string | null>(null);
  const [hasLoadedVoices, setHasLoadedVoices] = React.useState(false);

  const buildUserVoiceValue = (id: string) => `user:${id}`;
  const buildProviderVoiceValue = (providerId: string, voiceId: string) =>
    `provider:${providerId}:${voiceId}`;

  const voiceSelectionValue = (() => {
    if (!voiceConfig) return "";
    if (voiceConfig.source === "user" && voiceConfig.userVoiceId) {
      return buildUserVoiceValue(voiceConfig.userVoiceId);
    }
    if (voiceConfig.source === "provider" && voiceConfig.providerId && voiceConfig.voiceId) {
      return buildProviderVoiceValue(voiceConfig.providerId, voiceConfig.voiceId);
    }
    return "";
  })();

  React.useEffect(() => {
    const globalWindow = window as any;
    globalWindow.__saveCharacter = handleSave;
    globalWindow.__saveCharacterCanSave = canSave;
    globalWindow.__saveCharacterSaving = saving;
    return () => {
      delete globalWindow.__saveCharacter;
      delete globalWindow.__saveCharacterCanSave;
      delete globalWindow.__saveCharacterSaving;
    };
  }, [handleSave, canSave, saving]);

  React.useEffect(() => {
    const handleDiscard = () => resetToInitial();
    window.addEventListener("unsaved:discard", handleDiscard);
    return () => window.removeEventListener("unsaved:discard", handleDiscard);
  }, [resetToInitial]);

  const loadVoices = React.useCallback(async () => {
    setLoadingVoices(true);
    setVoiceError(null);
    try {
      const [providers, voices] = await Promise.all([listAudioProviders(), listUserVoices()]);
      setAudioProviders(providers);
      setUserVoices(voices);

      const voicesByProvider: Record<string, CachedVoice[]> = {};
      await Promise.all(
        providers.map(async (provider) => {
          try {
            if (provider.providerType === "elevenlabs" && provider.apiKey) {
              voicesByProvider[provider.id] = await refreshProviderVoices(provider.id);
            } else {
              voicesByProvider[provider.id] = await getProviderVoices(provider.id);
            }
          } catch (err) {
            console.warn("Failed to refresh provider voices:", err);
            try {
              voicesByProvider[provider.id] = await getProviderVoices(provider.id);
            } catch (fallbackErr) {
              console.warn("Failed to load cached voices:", fallbackErr);
              voicesByProvider[provider.id] = [];
            }
          }
        }),
      );
      setProviderVoices(voicesByProvider);
      setHasLoadedVoices(true);
    } catch (err) {
      console.error("Failed to load voices:", err);
      setVoiceError("Failed to load voices");
    } finally {
      setLoadingVoices(false);
    }
  }, []);

  React.useEffect(() => {
    if (activeTab !== "settings" || hasLoadedVoices) return;
    void loadVoices();
  }, [activeTab, hasLoadedVoices, loadVoices]);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-fg/10 border-t-white/60" />
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col pb-16 text-fg/80">
      <main
        id={tabPanelId}
        role="tabpanel"
        aria-labelledby={activeTab === "character" ? characterTabId : settingsTabId}
        tabIndex={0}
        className="flex-1 overflow-y-auto px-4"
      >
        <motion.div
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.2, ease: "easeOut" }}
          className="space-y-5 pb-6 pt-4"
        >
          {/* Character Tab Content */}
          {activeTab === "character" && (
            <>
              {/* Error Message */}
              <AnimatePresence>
                {error && (
                  <motion.div
                    initial={{ opacity: 0, y: -10 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0, y: -10 }}
                    transition={{ duration: 0.15 }}
                    className="rounded-xl border border-danger/30 bg-danger/10 px-4 py-3"
                  >
                    <p className="text-sm text-danger">{error}</p>
                  </motion.div>
                )}
              </AnimatePresence>

              {/* Avatar Section - CreateCharacter Style */}
              <div className="flex flex-col items-center py-4">
                <div className="relative">
                  <AvatarPicker
                    currentAvatarPath={avatarPath}
                    onAvatarChange={(path) => setFields({ avatarPath: path })}
                    avatarCrop={avatarCrop}
                    onAvatarCropChange={(crop) => setFields({ avatarCrop: crop })}
                    avatarRoundPath={avatarRoundPath}
                    onAvatarRoundChange={(roundPath) => setFields({ avatarRoundPath: roundPath })}
                    placeholder={avatarInitial}
                  />

                  {/* Remove Button - top left */}
                  {avatarPath && (
                    <button
                      type="button"
                      onClick={() =>
                        setFields({ avatarPath: "", avatarCrop: null, avatarRoundPath: null })
                      }
                      className="absolute -top-1 -left-1 z-30 flex h-10 w-10 items-center justify-center rounded-full border border-fg/10 bg-surface-el text-fg/60 transition hover:bg-danger/80 hover:border-danger/50 hover:text-fg active:scale-95"
                      aria-label="Remove avatar"
                    >
                      <X size={14} strokeWidth={2.5} />
                    </button>
                  )}
                </div>
                <p className="mt-3 text-xs text-fg/40">Tap to add or generate avatar</p>
              </div>

              {/* Name Input */}
              <div className="space-y-2">
                <label className="text-[10px] font-medium uppercase tracking-wide text-fg/50">
                  Character Name
                </label>
                <input
                  value={name}
                  onChange={(e) => setFields({ name: e.target.value })}
                  placeholder="Enter character name..."
                  className="w-full rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3 text-base text-fg placeholder-fg/40 transition focus:border-fg/25 focus:outline-none"
                />
              </div>
            </>
          )}

          {/* Settings Tab Content */}
          {activeTab === "settings" && (
            <>
              {/* Avatar Gradient Toggle */}
              {avatarPath && (
                <div className="space-y-3">
                  <label className="flex cursor-pointer items-center justify-between rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3 transition hover:bg-surface-el/30">
                    <div className="flex-1">
                      <div className="flex items-center gap-2">
                        <Sparkles className="h-4 w-4 text-accent" />
                        <p className="text-sm font-medium text-fg">Avatar Gradient</p>
                      </div>
                      <p className="mt-0.5 text-xs text-fg/50">
                        Generate colorful gradients from avatar colors
                      </p>
                    </div>
                    <div className="relative ml-3">
                      <input
                        type="checkbox"
                        checked={!disableAvatarGradient}
                        onChange={(e) => setFields({ disableAvatarGradient: !e.target.checked })}
                        className="peer sr-only"
                      />
                      <div className="h-6 w-11 rounded-full bg-fg/20 transition peer-checked:bg-accent/80"></div>
                      <div className="absolute left-1 top-1 h-4 w-4 rounded-full bg-fg transition peer-checked:translate-x-5"></div>
                    </div>
                  </label>
                </div>
              )}

              {/* Custom Gradient Override */}
              {avatarPath && (
                <div className="space-y-3">
                  <div className="rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                    <div className="flex items-center justify-between">
                      <div className="flex-1">
                        <div className="flex items-center gap-2">
                          <Sparkles className="h-4 w-4 text-secondary" />
                          <p className="text-sm font-medium text-fg">Custom Gradient</p>
                        </div>
                        <p className="mt-0.5 text-xs text-fg/50">
                          Override auto-detected colors with your own
                        </p>
                      </div>
                      <label className="relative ml-3 cursor-pointer">
                        <input
                          type="checkbox"
                          checked={customGradientEnabled}
                          onChange={(e) => {
                            if (e.target.checked) {
                              // Enable - set default colors if none exist
                              const colors =
                                customGradientColors.length > 0
                                  ? customGradientColors
                                  : ["#4f46e5", "#7c3aed"];
                              setFields({
                                customGradientEnabled: true,
                                customGradientColors: colors,
                              });
                            } else {
                              // Disable but preserve colors
                              setFields({ customGradientEnabled: false });
                            }
                          }}
                          className="peer sr-only"
                        />
                        <div className="h-6 w-11 rounded-full bg-fg/20 transition peer-checked:bg-secondary/80"></div>
                        <div className="absolute left-1 top-1 h-4 w-4 rounded-full bg-fg transition peer-checked:translate-x-5"></div>
                      </label>
                    </div>

                    {/* Color Pickers - shown when custom gradient enabled */}
                    {customGradientEnabled && (
                      <div className="mt-4 space-y-3 border-t border-fg/10 pt-4">
                        {/* Gradient Preview */}
                        <div
                          className="h-16 w-full rounded-lg"
                          style={{
                            background:
                              customGradientColors.length >= 3
                                ? `linear-gradient(135deg, ${customGradientColors[0]}, ${customGradientColors[2]}, ${customGradientColors[1]})`
                                : customGradientColors.length >= 2
                                  ? `linear-gradient(135deg, ${customGradientColors[0]}, ${customGradientColors[1]})`
                                  : customGradientColors[0],
                          }}
                        />

                        {/* Color 1 */}
                        <div className="flex items-center gap-3">
                          <label className="text-xs text-fg/50 w-12">Start</label>
                          <div className="relative shrink-0">
                            <input
                              type="color"
                              value={customGradientColors[0] || "#4f46e5"}
                              onChange={(e) => {
                                const newColors = [...customGradientColors];
                                newColors[0] = e.target.value;
                                setFields({ customGradientColors: newColors });
                              }}
                              className="h-10 w-10 cursor-pointer rounded-lg border-2 border-fg/20 p-0.5"
                              style={{ backgroundColor: customGradientColors[0] || "#4f46e5" }}
                            />
                          </div>
                          <input
                            type="text"
                            value={customGradientColors[0] || ""}
                            onChange={(e) => {
                              const newColors = [...customGradientColors];
                              newColors[0] = e.target.value;
                              setFields({ customGradientColors: newColors });
                            }}
                            placeholder="#4f46e5"
                            className="flex-1 rounded-lg border border-fg/10 bg-surface-el/50 px-3 py-2 text-sm font-mono text-fg placeholder:text-fg/30 focus:border-secondary/50 focus:outline-none"
                          />
                        </div>

                        {/* Middle Color (optional) */}
                        {customGradientColors.length >= 3 ? (
                          <div className="flex items-center gap-3">
                            <label className="text-xs text-fg/50 w-12">Mid</label>
                            <div className="relative shrink-0">
                              <input
                                type="color"
                                value={customGradientColors[2] || "#a855f7"}
                                onChange={(e) => {
                                  const newColors = [...customGradientColors];
                                  newColors[2] = e.target.value;
                                  setFields({ customGradientColors: newColors });
                                }}
                                className="h-10 w-10 cursor-pointer rounded-lg border-2 border-fg/20 p-0.5"
                                style={{ backgroundColor: customGradientColors[2] || "#a855f7" }}
                              />
                            </div>
                            <input
                              type="text"
                              value={customGradientColors[2] || ""}
                              onChange={(e) => {
                                const newColors = [...customGradientColors];
                                newColors[2] = e.target.value;
                                setFields({ customGradientColors: newColors });
                              }}
                              placeholder="#a855f7"
                              className="flex-1 rounded-lg border border-fg/10 bg-surface-el/50 px-3 py-2 text-sm font-mono text-fg placeholder:text-fg/30 focus:border-secondary/50 focus:outline-none"
                            />
                            <button
                              type="button"
                              onClick={() => {
                                // Remove middle color - reorder so End stays at index 1
                                const newColors = [
                                  customGradientColors[0],
                                  customGradientColors[1],
                                ];
                                setFields({ customGradientColors: newColors });
                              }}
                              className="shrink-0 text-xs text-danger hover:text-danger"
                            >
                              ✕
                            </button>
                          </div>
                        ) : (
                          <button
                            type="button"
                            onClick={() => {
                              // Add middle color between Start and End
                              const newColors = [
                                customGradientColors[0],
                                customGradientColors[1],
                                "#a855f7",
                              ];
                              setFields({ customGradientColors: newColors });
                            }}
                            className="text-xs text-secondary hover:text-secondary py-1"
                          >
                            + Add middle color
                          </button>
                        )}

                        {/* Color 2 (End) */}
                        <div className="flex items-center gap-3">
                          <label className="text-xs text-fg/50 w-12">End</label>
                          <div className="relative shrink-0">
                            <input
                              type="color"
                              value={customGradientColors[1] || "#7c3aed"}
                              onChange={(e) => {
                                const newColors = [...customGradientColors];
                                newColors[1] = e.target.value;
                                setFields({ customGradientColors: newColors });
                              }}
                              className="h-10 w-10 cursor-pointer rounded-lg border-2 p-0.5"
                              style={{ backgroundColor: customGradientColors[1] || "#7c3aed" }}
                            />
                          </div>
                          <input
                            type="text"
                            value={customGradientColors[1] || ""}
                            onChange={(e) => {
                              const newColors = [...customGradientColors];
                              newColors[1] = e.target.value;
                              setFields({ customGradientColors: newColors });
                            }}
                            placeholder="#7c3aed"
                            className="flex-1 rounded-lg border border-fg/10 bg-surface-el/50 px-3 py-2 text-sm font-mono text-fg placeholder:text-fg/30 focus:border-secondary/50 focus:outline-none"
                          />
                        </div>

                        {/* Optional: Text color override hint */}
                        <p className="text-[10px] text-fg/40 mt-2">
                          Text colors are auto-calculated based on gradient brightness
                        </p>
                      </div>
                    )}
                  </div>
                </div>
              )}

              {/* Background Image Section */}
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-secondary/30 bg-secondary/10 p-1.5">
                    <Image className="h-4 w-4 text-secondary" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">Chat Background</h3>
                  <span className="text-xs text-fg/40">(Optional)</span>
                </div>

                <div className="overflow-hidden rounded-xl border border-fg/10 bg-surface-el/20">
                  {backgroundImagePath ? (
                    <div className="relative">
                      <img
                        src={backgroundImagePath}
                        alt="Background preview"
                        className="h-32 w-full object-cover"
                      />
                      <div className="absolute inset-0 bg-surface-el/30 flex items-center justify-center">
                        <span className="text-xs text-fg/80 bg-surface-el/50 px-2 py-1 rounded">
                          Background Preview
                        </span>
                      </div>
                      <button
                        type="button"
                        onClick={() => setFields({ backgroundImagePath: "" })}
                        className="absolute top-2 right-2 rounded-full border border-fg/20 bg-surface-el/50 p-1 text-fg/70 transition hover:bg-surface-el/70 active:scale-95"
                        aria-label="Remove background image"
                      >
                        <X size={14} />
                      </button>
                    </div>
                  ) : (
                    <label className="flex h-32 cursor-pointer flex-col items-center justify-center gap-2 transition hover:bg-fg/5">
                      <div className="rounded-lg border border-fg/10 bg-fg/5 p-2">
                        <Image size={20} className="text-fg/40" />
                      </div>
                      <div className="text-center">
                        <p className="text-sm text-fg/70">Add Background Image</p>
                        <p className="text-xs text-fg/40">Tap to select an image</p>
                      </div>
                      <input
                        type="file"
                        accept="image/*"
                        onChange={(e) => {
                          const file = e.target.files?.[0];
                          if (!file) return;
                          const reader = new FileReader();
                          reader.onload = () => {
                            const dataUrl = reader.result as string;
                            setPendingBackgroundSrc(dataUrl);
                            setShowBackgroundChoiceMenu(true);
                          };
                          reader.readAsDataURL(file);
                          e.target.value = "";
                        }}
                        className="hidden"
                      />
                    </label>
                  )}
                </div>
                <p className="text-xs text-fg/50">
                  Optional background image for chat conversations with this character
                </p>
              </div>
            </>
          )}

          {/* Character Tab: Personality & Scenes */}
          {activeTab === "character" && (
            <>
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-fg/10 bg-fg/5 p-1.5">
                    <Info className="h-4 w-4 text-fg/60" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">Description</h3>
                </div>
                <textarea
                  value={description}
                  onChange={(e) => setFields({ description: e.target.value })}
                  rows={3}
                  placeholder="Short summary shown in lists and cards..."
                  className="w-full resize-none rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-sm leading-relaxed text-fg placeholder-fg/40 transition focus:border-fg/25 focus:outline-none"
                />
                <p className="text-xs text-fg/50">
                  Optional short description for display purposes.
                </p>
              </div>

              <div className="space-y-2">
                <label className="text-[10px] font-medium uppercase tracking-wide text-fg/50">
                  Nickname
                </label>
                <input
                  value={nickname}
                  onChange={(e) => setFields({ nickname: e.target.value })}
                  placeholder="Optional nickname..."
                  className="w-full rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3 text-sm text-fg placeholder-fg/40 transition focus:border-fg/25 focus:outline-none"
                />
              </div>

              <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                <div className="space-y-2">
                  <label className="text-[10px] font-medium uppercase tracking-wide text-fg/50">
                    Creator
                  </label>
                  <input
                    value={creator}
                    onChange={(e) => setFields({ creator: e.target.value })}
                    placeholder="Optional creator name..."
                    className="w-full rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3 text-sm text-fg placeholder-fg/40 transition focus:border-fg/25 focus:outline-none"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-[10px] font-medium uppercase tracking-wide text-fg/50">
                    Tags
                  </label>
                  <input
                    value={tagsText}
                    onChange={(e) => setFields({ tagsText: e.target.value })}
                    placeholder="tag1, tag2"
                    className="w-full rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3 text-sm text-fg placeholder-fg/40 transition focus:border-fg/25 focus:outline-none"
                  />
                </div>
              </div>

              <div className="space-y-2">
                <label className="text-[10px] font-medium uppercase tracking-wide text-fg/50">
                  Creator Notes
                </label>
                <textarea
                  value={creatorNotes}
                  onChange={(e) => setFields({ creatorNotes: e.target.value })}
                  rows={3}
                  placeholder="Optional creator notes..."
                  className="w-full resize-none rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3 text-sm text-fg placeholder-fg/40 transition focus:border-fg/25 focus:outline-none"
                />
              </div>

              <div className="space-y-2">
                <label className="text-[10px] font-medium uppercase tracking-wide text-fg/50">
                  Creator Notes Multilingual (JSON)
                </label>
                <textarea
                  value={creatorNotesMultilingualText}
                  onChange={(e) => setFields({ creatorNotesMultilingualText: e.target.value })}
                  rows={4}
                  placeholder='{"en":"note","ja":"メモ"}'
                  className="w-full resize-none rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3 font-mono text-xs text-fg placeholder-fg/40 transition focus:border-fg/25 focus:outline-none"
                />
              </div>

              {/* Personality Section */}
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-accent/30 bg-accent/10 p-1.5">
                    <Sparkles className="h-4 w-4 text-accent" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">Definition</h3>
                </div>
                <textarea
                  value={definition}
                  onChange={(e) => setFields({ definition: e.target.value })}
                  rows={8}
                  placeholder="Describe who this character is, their personality, background, speaking style, and how they should interact..."
                  className="w-full resize-none rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-sm leading-relaxed text-fg placeholder-fg/40 transition focus:border-fg/25 focus:outline-none"
                />
                <div className="flex justify-between text-[11px] text-fg/50">
                  <span>Be detailed to create a unique personality</span>
                  <span>{wordCount(definition)} words</span>
                </div>
                <div className="rounded-xl border border-info/20 bg-info/10 px-3.5 py-3">
                  <div className="text-[11px] font-medium text-info">
                    Available Placeholders
                  </div>
                  <div className="mt-2 space-y-1 text-xs text-info/70">
                    <div>
                      <code className="text-accent/80">{"{{char}}"}</code> - Character name
                    </div>
                    <div>
                      <code className="text-accent/80">{"{{user}}"}</code> - Persona name
                      (preferred, empty if none)
                    </div>
                    <div>
                      <code className="text-accent/80">{"{{persona}}"}</code> - Persona name
                      (alias)
                    </div>
                  </div>
                </div>
              </div>

              {/* Starting Scenes Section */}
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-info/30 bg-info/10 p-1.5">
                    <BookOpen className="h-4 w-4 text-info" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">Starting Scenes</h3>
                  {scenes.length > 0 && (
                    <span className="ml-auto rounded-full border border-fg/10 bg-fg/5 px-2 py-0.5 text-xs text-fg/70">
                      {scenes.length}
                    </span>
                  )}
                </div>

                {/* Existing Scenes */}
                <AnimatePresence mode="popLayout">
                  {scenes.length > 0 && (
                    <motion.div layout className="space-y-2">
                      {scenes.map((scene, index) => {
                        const isDefault = defaultSceneId === scene.id;
                        const isExpanded = expandedSceneId === scene.id;

                        return (
                          <motion.div
                            key={scene.id}
                            layout
                            initial={{ opacity: 0, scale: 0.95 }}
                            animate={{ opacity: 1, scale: 1 }}
                            exit={{ opacity: 0, scale: 0.9, x: -20 }}
                            transition={{ duration: 0.15 }}
                            className={`overflow-hidden rounded-xl border ${
                              isDefault
                                ? "border-accent/30 bg-accent/5"
                                : "border-fg/10 bg-fg/5"
                            }`}
                          >
                            {/* Scene Header - clickable to expand/collapse */}
                            <button
                              onClick={() => setExpandedSceneId(isExpanded ? null : scene.id)}
                              className={`flex w-full items-center gap-2 border-b px-3.5 py-2.5 text-left ${
                                isDefault
                                  ? "border-accent/20 bg-accent/10"
                                  : "border-fg/10 bg-fg/5"
                              }`}
                            >
                              {/* Scene number badge */}
                              <div
                                className={`flex h-6 w-6 shrink-0 items-center justify-center rounded-lg border text-xs font-medium ${
                                  isDefault
                                    ? "border-accent/40 bg-accent/20 text-accent/80"
                                    : "border-fg/10 bg-fg/5 text-fg/60"
                                }`}
                              >
                                {index + 1}
                              </div>

                              {/* Default badge */}
                              {isDefault && (
                                <div className="flex items-center gap-1 rounded-full border border-accent/40 bg-accent/20 px-2 py-0.5">
                                  <div className="h-1.5 w-1.5 rounded-full bg-accent" />
                                  <span className="text-[10px] font-medium text-accent/80">
                                    Default
                                  </span>
                                </div>
                              )}

                              {/* Direction indicator */}
                              {scene.direction && (
                                <div
                                  className="flex items-center gap-1 rounded-full border border-fg/10 bg-fg/5 px-1.5 py-0.5"
                                  title="Has scene direction"
                                >
                                  <EyeOff className="h-3 w-3 text-fg/40" />
                                </div>
                              )}

                              {/* Preview text when collapsed */}
                              {!isExpanded && (
                                <span className="flex-1 truncate text-sm text-fg/50">
                                  {scene.content.slice(0, 50)}
                                  {scene.content.length > 50 ? "..." : ""}
                                </span>
                              )}

                              {/* Expand indicator */}
                              <ChevronDown
                                className={cn(
                                  "h-4 w-4 text-fg/40 transition-transform ml-auto",
                                  isExpanded && "rotate-180",
                                )}
                              />
                            </button>

                            {/* Scene Content - collapsible */}
                            <AnimatePresence>
                              {isExpanded && (
                                <motion.div
                                  initial={{ height: 0, opacity: 0 }}
                                  animate={{ height: "auto", opacity: 1 }}
                                  exit={{ height: 0, opacity: 0 }}
                                  transition={{ duration: 0.2 }}
                                  className="overflow-hidden"
                                >
                                  <div className="p-3.5">
                                    <div className="space-y-3">
                                      <p className="text-sm leading-relaxed text-fg/90">
                                        {scene.content}
                                      </p>

                                      {/* Scene Direction (if set) */}
                                      {scene.direction && (
                                        <div className="pt-2 border-t border-fg/5">
                                          <p className="text-[10px] font-medium text-fg/40 mb-1">
                                            Scene Direction
                                          </p>
                                          <p className="text-xs leading-relaxed text-fg/50 italic">
                                            {scene.direction}
                                          </p>
                                        </div>
                                      )}

                                      {/* Actions when expanded */}
                                      <div className="flex items-center gap-2 pt-2 border-t border-fg/5">
                                        {!isDefault && (
                                          <button
                                            onClick={(e) => {
                                              e.stopPropagation();
                                              setFields({ defaultSceneId: scene.id });
                                            }}
                                            className="rounded-lg border border-fg/10 bg-fg/5 px-2.5 py-1.5 text-xs font-medium text-fg/60 transition active:scale-95 active:bg-fg/10"
                                          >
                                            Set as Default
                                          </button>
                                        )}
                                        <button
                                          onClick={(e) => {
                                            e.stopPropagation();
                                            setNewSceneEditorOpen(false);
                                            startEditingScene(scene);
                                          }}
                                          className="rounded-lg border border-fg/10 bg-fg/5 p-1.5 text-fg/60 transition active:scale-95 active:bg-fg/10"
                                          aria-label={`Edit scene ${index + 1}`}
                                        >
                                          <Edit2 className="h-3.5 w-3.5" />
                                        </button>
                                        <button
                                          onClick={(e) => {
                                            e.stopPropagation();
                                            deleteScene(scene.id);
                                          }}
                                          className="rounded-lg border border-fg/10 bg-fg/5 p-1.5 text-fg/50 transition active:bg-danger/10 active:text-danger"
                                          aria-label={`Delete scene ${index + 1}`}
                                        >
                                          <X className="h-3.5 w-3.5" />
                                        </button>
                                      </div>
                                    </div>
                                  </div>
                                </motion.div>
                              )}
                            </AnimatePresence>
                          </motion.div>
                        );
                      })}
                    </motion.div>
                  )}
                </AnimatePresence>

                {/* Add New Scene */}
                <motion.div layout className="space-y-2">
                  <div className="rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3">
                    <div className="text-sm font-medium text-fg">New starting scene</div>
                    <p className="mt-1 text-xs text-fg/50">
                      Create a scenario and optional direction for the opening moment.
                    </p>
                    <div className="mt-3 flex items-center gap-2">
                      <motion.button
                        onClick={() => setNewSceneEditorOpen(true)}
                        whileTap={{ scale: 0.97 }}
                        className="flex items-center gap-2 rounded-xl border border-info/40 bg-info/20 px-3.5 py-2 text-sm font-medium text-info transition active:bg-info/30"
                      >
                        <Plus className="h-4 w-4" />
                        Create Scene
                      </motion.button>
                      {newSceneContent.trim() && (
                        <button
                          type="button"
                          onClick={() => setNewSceneEditorOpen(true)}
                          className="text-xs text-fg/50 transition hover:text-fg/70"
                        >
                          Continue draft
                        </button>
                      )}
                    </div>
                  </div>
                </motion.div>

                <p className="text-xs text-fg/50">
                  Create multiple starting scenarios. One will be selected when starting a new chat.
                </p>
              </div>
            </>
          )}

          {/* Settings Tab: Model & Memory */}
          {activeTab === "settings" && (
            <>
              {/* Model Selection Section */}
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-secondary/30 bg-secondary/10 p-1.5">
                    <Cpu className="h-4 w-4 text-secondary" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">Default Model</h3>
                  <span className="ml-auto text-xs text-fg/40">(Optional)</span>
                </div>

                {loadingModels ? (
                  <div className="flex items-center gap-2 rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                    <Loader2 className="h-4 w-4 animate-spin text-fg/50" />
                    <span className="text-sm text-fg/50">Loading models...</span>
                  </div>
                ) : models.length > 0 ? (
                  <button
                    type="button"
                    onClick={() => setShowModelMenu(true)}
                    className="flex w-full items-center justify-between rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-left transition hover:bg-surface-el/30 focus:border-fg/25 focus:outline-none"
                  >
                    <div className="flex items-center gap-2">
                      {selectedModelId ? (
                        getProviderIcon(
                          models.find((m) => m.id === selectedModelId)?.providerId || "",
                        )
                      ) : (
                        <Cpu className="h-5 w-5 text-fg/40" />
                      )}
                      <span
                        className={`text-sm ${selectedModelId ? "text-fg" : "text-fg/50"}`}
                      >
                        {selectedModelId
                          ? models.find((m) => m.id === selectedModelId)?.displayName ||
                            "Selected Model"
                          : "Use global default model"}
                      </span>
                    </div>
                    <ChevronDown className="h-4 w-4 text-fg/40" />
                  </button>
                ) : (
                  <div className="rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                    <p className="text-sm text-fg/50">No models available</p>
                  </div>
                )}
                <p className="text-xs text-fg/50">
                  Override the default AI model for this character
                </p>
              </div>

              {/* Fallback Model Section */}
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-info/30 bg-info/10 p-1.5">
                    <Cpu className="h-4 w-4 text-info" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">Fallback Model</h3>
                  <span className="ml-auto text-xs text-fg/40">(Optional)</span>
                </div>

                {loadingModels ? (
                  <div className="flex items-center gap-2 rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                    <Loader2 className="h-4 w-4 animate-spin text-fg/50" />
                    <span className="text-sm text-fg/50">Loading models...</span>
                  </div>
                ) : (
                  <button
                    type="button"
                    onClick={() => setShowFallbackModelMenu(true)}
                    className="flex w-full items-center justify-between rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-left transition hover:bg-surface-el/30 focus:border-fg/25 focus:outline-none"
                  >
                    <div className="flex items-center gap-2">
                      {selectedFallbackModelId ? (
                        getProviderIcon(
                          models.find((m) => m.id === selectedFallbackModelId)?.providerId || "",
                        )
                      ) : (
                        <Cpu className="h-5 w-5 text-fg/40" />
                      )}
                      <span
                        className={`text-sm ${selectedFallbackModelId ? "text-fg" : "text-fg/50"}`}
                      >
                        {selectedFallbackModelId
                          ? models.find((m) => m.id === selectedFallbackModelId)?.displayName ||
                            "Selected Fallback Model"
                          : "Off (no fallback)"}
                      </span>
                    </div>
                    <ChevronDown className="h-4 w-4 text-fg/40" />
                  </button>
                )}
                <p className="text-xs text-fg/50">
                  Retry with this model only when the primary model fails
                </p>
              </div>

              {/* System Prompt Template Section */}
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-info/30 bg-info/10 p-1.5">
                    <BookOpen className="h-4 w-4 text-info" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">System Prompt</h3>
                  <span className="ml-auto text-xs text-fg/40">(Optional)</span>
                </div>

                {loadingTemplates ? (
                  <div className="flex items-center gap-2 rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                    <Loader2 className="h-4 w-4 animate-spin text-fg/50" />
                    <span className="text-sm text-fg/50">Loading templates...</span>
                  </div>
                ) : promptTemplates.length > 0 ? (
                  <select
                    value={systemPromptTemplateId || ""}
                    onChange={(e) => setFields({ systemPromptTemplateId: e.target.value || null })}
                    className="w-full appearance-none rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-sm text-fg transition focus:border-fg/25 focus:outline-none"
                  >
                    <option value="">Use default system prompt</option>
                    {promptTemplates.map((template) => (
                      <option key={template.id} value={template.id}>
                        {template.name}
                      </option>
                    ))}
                  </select>
                ) : (
                  <div className="rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                    <p className="text-sm text-fg/50">No templates available</p>
                    <p className="text-xs text-fg/40 mt-1">
                      Create templates in Settings → Prompts
                    </p>
                  </div>
                )}
                <p className="text-xs text-fg/50">
                  Override the default system prompt for this character
                </p>
              </div>

              {/* Voice Selection */}
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-accent/30 bg-accent/10 p-1.5">
                    <Volume2 className="h-4 w-4 text-accent/80" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">Voice</h3>
                  <span className="ml-auto text-xs text-fg/40">(Optional)</span>
                </div>

                {loadingVoices ? (
                  <div className="flex items-center gap-2 rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                    <Loader2 className="h-4 w-4 animate-spin text-fg/50" />
                    <span className="text-sm text-fg/50">Loading voices...</span>
                  </div>
                ) : (
                  <button
                    type="button"
                    onClick={() => setShowVoiceMenu(true)}
                    className="flex w-full items-center justify-between rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-left transition hover:bg-surface-el/30 focus:border-fg/25 focus:outline-none"
                  >
                    <div className="flex items-center gap-2">
                      <Volume2 className="h-5 w-5 text-fg/40" />
                      <span
                        className={`text-sm ${voiceSelectionValue ? "text-fg" : "text-fg/50"}`}
                      >
                        {voiceSelectionValue
                          ? (() => {
                              if (voiceConfig?.source === "user") {
                                const v = userVoices.find(
                                  (uv) => uv.id === voiceConfig.userVoiceId,
                                );
                                return v?.name || "Custom Voice";
                              }
                              if (voiceConfig?.source === "provider") {
                                const pv = providerVoices[voiceConfig.providerId || ""]?.find(
                                  (pv) => pv.voiceId === voiceConfig.voiceId,
                                );
                                return pv?.name || "Provider Voice";
                              }
                              return "Selected Voice";
                            })()
                          : "No voice assigned"}
                      </span>
                    </div>
                    <ChevronDown className="h-4 w-4 text-fg/40" />
                  </button>
                )}

                {voiceError && <p className="text-xs font-medium text-danger">{voiceError}</p>}
                {!loadingVoices && audioProviders.length === 0 && userVoices.length === 0 && (
                  <p className="text-xs text-fg/40">Add voices in Settings → Voices</p>
                )}
                <p className="text-xs text-fg/50">
                  Assign a voice for future text-to-speech playback
                </p>
                <div
                  className={cn(
                    "flex items-center justify-between rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3",
                    !voiceConfig && "opacity-50",
                  )}
                >
                  <div>
                    <p className="text-sm font-medium text-fg">Autoplay voice</p>
                    <p className="mt-1 text-xs text-fg/50">
                      {voiceConfig
                        ? "Play this character's replies automatically"
                        : "Select a voice first"}
                    </p>
                  </div>
                  <div className="flex items-center">
                    <input
                      id="character-voice-autoplay"
                      type="checkbox"
                      checked={voiceAutoplay}
                      onChange={() => setFields({ voiceAutoplay: !voiceAutoplay })}
                      disabled={!voiceConfig}
                      className="peer sr-only"
                    />
                    <label
                      htmlFor="character-voice-autoplay"
                      className={`relative inline-flex h-6 w-11 shrink-0 rounded-full transition-all ${
                        voiceAutoplay ? "bg-accent" : "bg-fg/20"
                      } ${voiceConfig ? "cursor-pointer" : "cursor-not-allowed"}`}
                    >
                      <span
                        className={`inline-block h-5 w-5 mt-0.5 transform rounded-full bg-fg transition ${
                          voiceAutoplay ? "translate-x-5" : "translate-x-0.5"
                        }`}
                      />
                    </label>
                  </div>
                </div>
              </div>

              {/* Chat Appearance */}
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-info/30 bg-info/10 p-1.5">
                    <MessageSquare className="h-4 w-4 text-info" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">Chat Appearance</h3>
                  <span className="ml-auto text-xs text-fg/40">(Optional)</span>
                </div>
                <button
                  type="button"
                  onClick={() => navigate(`/settings/accessibility/chat?characterId=${characterId}`)}
                  className={cn(
                    "group flex w-full items-center justify-between rounded-xl border px-3.5 py-3",
                    "border-fg/10 bg-surface-el/20",
                    interactive.transition.fast,
                    "hover:bg-surface-el/30",
                  )}
                >
                  <span className="text-sm text-fg/70">Customize bubbles, fonts & layout</span>
                  <ChevronRight className="h-4 w-4 text-fg/25 group-hover:text-fg/50" />
                </button>
              </div>

              {/* Memory Mode */}
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-warning/30 bg-warning/10 p-1.5">
                    <Layers className="h-4 w-4 text-warning" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">Memory Mode</h3>
                  {!dynamicMemoryEnabled && (
                    <span className="ml-auto text-xs text-fg/40">
                      Enable Dynamic Memory to switch
                    </span>
                  )}
                </div>
                <div className="grid grid-cols-2 gap-2">
                  <button
                    type="button"
                    onClick={() => setFields({ memoryType: "manual" })}
                    className={`rounded-xl border px-3.5 py-3 text-left transition ${
                      memoryType === "manual"
                        ? "border-accent/40 bg-accent/15 text-accent shadow-[0_0_0_1px_rgba(16,185,129,0.25)]"
                        : "border-fg/10 bg-surface-el/20 text-fg/70 hover:border-fg/20 hover:bg-surface-el/30"
                    }`}
                  >
                    <p className="text-sm font-semibold">Manual Memory</p>
                    <p className="mt-1 text-xs text-fg/60">
                      Manage notes yourself (current system).
                    </p>
                  </button>
                  <button
                    type="button"
                    disabled={!dynamicMemoryEnabled}
                    onClick={() => dynamicMemoryEnabled && setFields({ memoryType: "dynamic" })}
                    className={`rounded-xl border px-3.5 py-3 text-left transition ${
                      memoryType === "dynamic" && dynamicMemoryEnabled
                        ? "border-info/50 bg-info/20 text-info shadow-[0_0_0_1px_rgba(96,165,250,0.3)]"
                        : "border-fg/10 bg-surface-el/15 text-fg/60"
                    } ${!dynamicMemoryEnabled ? "cursor-not-allowed opacity-50" : "hover:border-fg/20 hover:bg-surface-el/25"}`}
                  >
                    <p className="text-sm font-semibold">Dynamic Memory</p>
                    <p className="mt-1 text-xs text-fg/60">
                      Automatic summaries when enabled globally.
                    </p>
                  </button>
                </div>
                <p className="text-xs text-fg/50">
                  Dynamic Memory must be turned on in Advanced settings; otherwise manual memory is
                  used.
                </p>
              </div>
            </>
          )}

          {/* Export Button - Character Tab */}
          {activeTab === "character" && (
            <motion.button
              onClick={() => setExportMenuOpen(true)}
              disabled={exporting}
              whileTap={{ scale: exporting ? 1 : 0.98 }}
              className="w-full rounded-xl border border-info/40 bg-info/20 px-4 py-3.5 text-sm font-semibold text-info transition hover:bg-info/30 disabled:opacity-50"
            >
              {exporting ? (
                <span className="flex items-center justify-center gap-2">
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Exporting...
                </span>
              ) : (
                <span className="flex items-center justify-center gap-2">
                  <Download className="h-4 w-4" />
                  Export Character
                </span>
              )}
            </motion.button>
          )}
        </motion.div>
      </main>

      <CharacterExportMenu
        isOpen={exportMenuOpen}
        onClose={() => setExportMenuOpen(false)}
        onSelect={handleExportFormat}
        exporting={exporting}
      />

      {/* Bottom Tab Bar */}
      <div
        className={cn(
          "fixed bottom-0 left-0 right-0 border-t px-3 pb-[calc(env(safe-area-inset-bottom)+12px)] pt-3",
          colors.glass.strong,
        )}
      >
        <div
          role="tablist"
          aria-label="Character editor tabs"
          className={cn(radius.lg, "grid grid-cols-2 gap-2 p-1", colors.surface.elevated)}
        >
          {[
            { id: "character" as const, icon: User, label: "Character" },
            { id: "settings" as const, icon: Settings, label: "Settings" },
          ].map(({ id, icon: Icon, label }) => (
            <button
              key={id}
              type="button"
              onClick={() => setActiveTab(id)}
              role="tab"
              id={id === "character" ? characterTabId : settingsTabId}
              aria-selected={activeTab === id}
              aria-controls={tabPanelId}
              className={cn(
                radius.md,
                "px-3 py-2.5 text-sm font-semibold transition flex items-center justify-center gap-2",
                interactive.active.scale,
                activeTab === id
                  ? "bg-fg/10 text-fg"
                  : cn(colors.text.tertiary, "hover:text-fg"),
              )}
            >
              <Icon size={16} className="block" />
              <span className="pt-1">{label}</span>
            </button>
          ))}
        </div>
      </div>

      {/* Edit/New Scene Fullscreen Panel */}
      <AnimatePresence>
        {(editingSceneId !== null || newSceneEditorOpen) && (
          <motion.div
            className="fixed inset-0 z-50 flex h-full flex-col bg-surface-el/90 backdrop-blur-sm"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 20 }}
            transition={{ duration: 0.2 }}
          >
            <div className="flex items-center justify-between border-b border-fg/10 px-4 py-3">
              <div className="text-base font-semibold text-fg">
                {editingSceneId !== null ? "Edit Scene" : "New Scene"}
              </div>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={editingSceneId !== null ? cancelEditingScene : closeNewSceneEditor}
                  className="rounded-full border border-fg/10 px-3 py-1.5 text-xs font-medium text-fg/70 transition hover:bg-fg/10 hover:text-fg"
                >
                  Close
                </button>
                <button
                  type="button"
                  onClick={editingSceneId !== null ? saveEditedScene : saveNewScene}
                  disabled={
                    editingSceneId !== null ? !editingSceneContent.trim() : !newSceneContent.trim()
                  }
                  className={cn(
                    "rounded-full px-3 py-1.5 text-xs font-semibold text-fg transition",
                    "bg-linear-to-r from-accent to-accent/80",
                    "hover:from-accent/80 hover:to-accent/60",
                    "disabled:cursor-not-allowed disabled:opacity-50",
                  )}
                >
                  {editingSceneId !== null ? "Save" : "Add"}
                </button>
              </div>
            </div>

            <div className="flex-1 overflow-y-auto px-4 pb-6 pt-4">
              <div className="space-y-6">
                <div className="space-y-2">
                  <div className="text-sm font-medium text-fg/80">Scene</div>
                  <textarea
                    value={editingSceneId !== null ? editingSceneContent : newSceneContent}
                    onChange={(e) =>
                      setFields(
                        editingSceneId !== null
                          ? { editingSceneContent: e.target.value }
                          : { newSceneContent: e.target.value },
                      )
                    }
                    rows={14}
                    className="min-h-[40vh] w-full resize-none rounded-2xl border border-fg/10 bg-surface-el/40 px-4 py-4 text-sm leading-relaxed text-fg placeholder-fg/40 transition focus:border-fg/20 focus:outline-none"
                    placeholder="Enter scene content..."
                  />
                  <div className="flex items-center justify-between text-[11px] text-fg/40">
                    <span>
                      {wordCount(editingSceneId !== null ? editingSceneContent : newSceneContent)}{" "}
                      words
                    </span>
                    <span>
                      Use <code className="text-accent/80">{"{{char}}"}</code>,{" "}
                      <code className="text-accent/80">{"{{user}}"}</code>
                    </span>
                  </div>
                </div>

                <div className="space-y-2">
                  <div className="flex items-center gap-1.5 text-sm font-medium text-fg/80">
                    <EyeOff className="h-4 w-4 text-fg/50" />
                    Scene Direction
                  </div>
                  <textarea
                    value={editingSceneId !== null ? editingSceneDirection : newSceneDirection}
                    onChange={(e) =>
                      setFields(
                        editingSceneId !== null
                          ? { editingSceneDirection: e.target.value }
                          : { newSceneDirection: e.target.value },
                      )
                    }
                    rows={6}
                    className="min-h-[18vh] w-full resize-none rounded-2xl border border-fg/10 bg-surface-el/35 px-4 py-3 text-sm leading-relaxed text-fg placeholder-fg/30 transition focus:border-fg/20 focus:outline-none"
                    placeholder="e.g., 'The hostage will be rescued' or 'Build tension gradually'"
                  />
                  <p className="text-[11px] text-fg/40">
                    Hidden guidance for the AI on how this scene should unfold.
                  </p>
                </div>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Background Upload Choice Menu */}
      <BottomMenu
        isOpen={showBackgroundChoiceMenu}
        onClose={() => {
          setShowBackgroundChoiceMenu(false);
          setPendingBackgroundSrc(null);
        }}
        title=""
      >
        <div className="space-y-4 p-2">
          <div className="text-center">
            <h3 className="text-lg font-semibold text-fg">Background Image</h3>
            <p className="text-sm text-fg/50">Choose how to add your image</p>
          </div>

          <div className="space-y-2">
            {/* Quick Upload Option */}
            <button
              onClick={() => {
                if (pendingBackgroundSrc) {
                  setFields({ backgroundImagePath: pendingBackgroundSrc });
                }
                setShowBackgroundChoiceMenu(false);
                setPendingBackgroundSrc(null);
              }}
              className="flex w-full items-center gap-4 rounded-xl border border-fg/10 bg-fg/5 p-2 transition hover:bg-fg/10 active:scale-[0.98]"
            >
              <div className="flex h-12 w-12 items-center justify-center rounded-lg border border-accent/30 bg-accent/15 text-accent/80">
                <Upload size={20} />
              </div>
              <div className="flex-1 text-left">
                <p className="font-medium text-fg">Quick Upload</p>
                <p className="text-xs text-fg/50">Use full image without cropping</p>
              </div>
            </button>

            {/* Position & Crop Option */}
            <button
              onClick={() => {
                setShowBackgroundChoiceMenu(false);
                setShowBackgroundPositionModal(true);
              }}
              className="flex w-full items-center gap-4 rounded-xl border border-fg/10 bg-fg/5 p-4 transition hover:bg-fg/10 active:scale-[0.98]"
            >
              <div className="flex h-12 w-12 items-center justify-center rounded-lg border border-info/30 bg-info/15 text-info">
                <Crop size={20} />
              </div>
              <div className="flex-1 text-left">
                <p className="font-medium text-fg">Position & Crop</p>
                <p className="text-xs text-fg/50">Adjust to fit portrait view</p>
              </div>
            </button>
          </div>
        </div>
      </BottomMenu>

      {/* Background Position Modal */}
      {pendingBackgroundSrc && (
        <BackgroundPositionModal
          isOpen={showBackgroundPositionModal}
          onClose={() => {
            setShowBackgroundPositionModal(false);
            setPendingBackgroundSrc(null);
          }}
          imageSrc={pendingBackgroundSrc}
          onConfirm={(croppedDataUrl) => {
            setFields({ backgroundImagePath: croppedDataUrl });
            setPendingBackgroundSrc(null);
          }}
        />
      )}

      {/* Model Selection BottomMenu */}
      <BottomMenu
        isOpen={showModelMenu}
        onClose={() => {
          setShowModelMenu(false);
          setModelSearchQuery("");
        }}
        title="Select Model"
      >
        <div className="space-y-4">
          <div className="relative">
            <input
              type="text"
              value={modelSearchQuery}
              onChange={(e) => setModelSearchQuery(e.target.value)}
              placeholder="Search models..."
              className="w-full rounded-xl border border-fg/10 bg-surface-el/30 px-4 py-2.5 pl-10 text-sm text-fg placeholder-fg/40 focus:border-fg/20 focus:outline-none"
            />
            <svg
              className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-fg/40"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
              />
            </svg>
          </div>
          <div className="space-y-2 max-h-[50vh] overflow-y-auto">
            <button
              onClick={() => {
                setFields({ selectedModelId: null });
                setShowModelMenu(false);
                setModelSearchQuery("");
              }}
              className={cn(
                "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition",
                !selectedModelId
                  ? "border-accent/40 bg-accent/10"
                  : "border-fg/10 bg-fg/5 hover:bg-fg/10",
              )}
            >
              <Cpu className="h-5 w-5 text-fg/40" />
              <span className="text-sm text-fg">Use global default model</span>
              {!selectedModelId && <Check className="h-4 w-4 ml-auto text-accent" />}
            </button>
            {models
              .filter((m) => {
                if (!modelSearchQuery) return true;
                const q = modelSearchQuery.toLowerCase();
                return (
                  m.displayName?.toLowerCase().includes(q) || m.name?.toLowerCase().includes(q)
                );
              })
              .map((model) => (
                <button
                  key={model.id}
                  onClick={() => {
                    setFields({
                      selectedModelId: model.id,
                      selectedFallbackModelId:
                        selectedFallbackModelId === model.id ? null : selectedFallbackModelId,
                    });
                    setShowModelMenu(false);
                    setModelSearchQuery("");
                  }}
                  className={cn(
                    "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition",
                    selectedModelId === model.id
                      ? "border-accent/40 bg-accent/10"
                      : "border-fg/10 bg-fg/5 hover:bg-fg/10",
                  )}
                >
                  {getProviderIcon(model.providerId)}
                  <div className="flex-1 min-w-0">
                    <span className="block truncate text-sm text-fg">
                      {model.displayName || model.name}
                    </span>
                    <span className="block truncate text-xs text-fg/40">{model.name}</span>
                  </div>
                  {selectedModelId === model.id && (
                    <Check className="h-4 w-4 shrink-0 text-accent" />
                  )}
                </button>
              ))}
          </div>
        </div>
      </BottomMenu>

      {/* Fallback Model Selection BottomMenu */}
      <BottomMenu
        isOpen={showFallbackModelMenu}
        onClose={() => {
          setShowFallbackModelMenu(false);
          setFallbackModelSearchQuery("");
        }}
        title="Select Fallback Model"
      >
        <div className="space-y-4">
          <div className="relative">
            <input
              type="text"
              value={fallbackModelSearchQuery}
              onChange={(e) => setFallbackModelSearchQuery(e.target.value)}
              placeholder="Search models..."
              className="w-full rounded-xl border border-fg/10 bg-surface-el/30 px-4 py-2.5 pl-10 text-sm text-fg placeholder-fg/40 focus:border-fg/20 focus:outline-none"
            />
            <svg
              className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-fg/40"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
              />
            </svg>
          </div>
          <div className="space-y-2 max-h-[50vh] overflow-y-auto">
            <button
              onClick={() => {
                setFields({ selectedFallbackModelId: null });
                setShowFallbackModelMenu(false);
                setFallbackModelSearchQuery("");
              }}
              className={cn(
                "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition",
                !selectedFallbackModelId
                  ? "border-accent/40 bg-accent/10"
                  : "border-fg/10 bg-fg/5 hover:bg-fg/10",
              )}
            >
              <Cpu className="h-5 w-5 text-fg/40" />
              <span className="text-sm text-fg">Off (no fallback)</span>
              {!selectedFallbackModelId && <Check className="h-4 w-4 ml-auto text-accent" />}
            </button>
            {models
              .filter((m) => m.id !== selectedModelId)
              .filter((m) => {
                if (!fallbackModelSearchQuery) return true;
                const q = fallbackModelSearchQuery.toLowerCase();
                return (
                  m.displayName?.toLowerCase().includes(q) || m.name?.toLowerCase().includes(q)
                );
              })
              .map((model) => (
                <button
                  key={model.id}
                  onClick={() => {
                    setFields({ selectedFallbackModelId: model.id });
                    setShowFallbackModelMenu(false);
                    setFallbackModelSearchQuery("");
                  }}
                  className={cn(
                    "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition",
                    selectedFallbackModelId === model.id
                      ? "border-accent/40 bg-accent/10"
                      : "border-fg/10 bg-fg/5 hover:bg-fg/10",
                  )}
                >
                  {getProviderIcon(model.providerId)}
                  <div className="flex-1 min-w-0">
                    <span className="block truncate text-sm text-fg">
                      {model.displayName || model.name}
                    </span>
                    <span className="block truncate text-xs text-fg/40">{model.name}</span>
                  </div>
                  {selectedFallbackModelId === model.id && (
                    <Check className="h-4 w-4 shrink-0 text-accent" />
                  )}
                </button>
              ))}
          </div>
        </div>
      </BottomMenu>

      {/* Voice Selection BottomMenu */}
      <BottomMenu
        isOpen={showVoiceMenu}
        onClose={() => {
          setShowVoiceMenu(false);
          setVoiceSearchQuery("");
        }}
        title="Select Voice"
      >
        <div className="space-y-4">
          <div className="relative">
            <input
              type="text"
              value={voiceSearchQuery}
              onChange={(e) => setVoiceSearchQuery(e.target.value)}
              placeholder="Search voices..."
              className="w-full rounded-xl border border-fg/10 bg-surface-el/30 px-4 py-2.5 pl-10 text-sm text-fg placeholder-fg/40 focus:border-fg/20 focus:outline-none"
            />
            <svg
              className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-fg/40"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
              />
            </svg>
          </div>
          <div className="space-y-2 max-h-[50vh] overflow-y-auto">
            <button
              onClick={() => {
                setFields({ voiceConfig: null });
                setShowVoiceMenu(false);
                setVoiceSearchQuery("");
              }}
              className={cn(
                "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition",
                !voiceSelectionValue
                  ? "border-accent/40 bg-accent/10"
                  : "border-fg/10 bg-fg/5 hover:bg-fg/10",
              )}
            >
              <Volume2 className="h-5 w-5 text-fg/40" />
              <span className="text-sm text-fg">No voice assigned</span>
              {!voiceSelectionValue && <Check className="h-4 w-4 ml-auto text-accent" />}
            </button>

            {/* User Voices */}
            {userVoices.length > 0 && (
              <MenuSection label="My Voices">
                {userVoices
                  .filter((v) => {
                    if (!voiceSearchQuery) return true;
                    return v.name.toLowerCase().includes(voiceSearchQuery.toLowerCase());
                  })
                  .map((voice) => {
                    const value = buildUserVoiceValue(voice.id);
                    const isSelected = voiceSelectionValue === value;
                    const providerLabel =
                      audioProviders.find((p) => p.id === voice.providerId)?.label ?? "Provider";
                    return (
                      <button
                        key={voice.id}
                        onClick={() => {
                          setFields({
                            voiceConfig: {
                              source: "user",
                              userVoiceId: voice.id,
                              providerId: voice.providerId,
                              modelId: voice.modelId,
                              voiceName: voice.name,
                            },
                          });
                          setShowVoiceMenu(false);
                          setVoiceSearchQuery("");
                        }}
                        className={cn(
                          "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition",
                          isSelected
                            ? "border-accent/40 bg-accent/10"
                            : "border-fg/10 bg-fg/5 hover:bg-fg/10",
                        )}
                      >
                        <User className="h-5 w-5 text-fg/40" />
                        <div className="flex-1 min-w-0">
                          <span className="block truncate text-sm text-fg">{voice.name}</span>
                          <span className="block truncate text-xs text-fg/40">
                            {providerLabel}
                          </span>
                        </div>
                        {isSelected && <Check className="h-4 w-4 shrink-0 text-accent" />}
                      </button>
                    );
                  })}
              </MenuSection>
            )}

            {/* Provider Voices */}
            {audioProviders.map((provider) => {
              const voices = (providerVoices[provider.id] ?? []).filter((v) => {
                if (!voiceSearchQuery) return true;
                return v.name.toLowerCase().includes(voiceSearchQuery.toLowerCase());
              });
              if (voices.length === 0) return null;
              return (
                <MenuSection key={provider.id} label={`${provider.label} Voices`}>
                  {voices.map((voice) => {
                    const value = buildProviderVoiceValue(provider.id, voice.voiceId);
                    const isSelected = voiceSelectionValue === value;
                    return (
                      <button
                        key={`${provider.id}:${voice.voiceId}`}
                        onClick={() => {
                          setFields({
                            voiceConfig: {
                              source: "provider",
                              providerId: provider.id,
                              voiceId: voice.voiceId,
                              voiceName: voice.name,
                            },
                          });
                          setShowVoiceMenu(false);
                          setVoiceSearchQuery("");
                        }}
                        className={cn(
                          "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition",
                          isSelected
                            ? "border-accent/40 bg-accent/10"
                            : "border-fg/10 bg-fg/5 hover:bg-fg/10",
                        )}
                      >
                        <Volume2 className="h-5 w-5 text-fg/40" />
                        <span className="flex-1 truncate text-sm text-fg">{voice.name}</span>
                        {isSelected && <Check className="h-4 w-4 shrink-0 text-accent" />}
                      </button>
                    );
                  })}
                </MenuSection>
              );
            })}
          </div>
        </div>
      </BottomMenu>
    </div>
  );
}
