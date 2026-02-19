import { useState, useEffect, useMemo } from "react";
import {
  Sparkles,
  Cpu,
  Image,
  Wand2,
  Check,
  Zap,
  PenTool,
  Eye,
  MessageSquare,
  User,
  FileImage,
  Palette,
  Settings2,
  CheckCircle2,
  BookOpen,
  List,
  Info,
  ChevronDown,
} from "lucide-react";
import { readSettings, saveAdvancedSettings } from "../../../core/storage/repo";
import type { Model } from "../../../core/storage/schemas";
import { cn, colors } from "../../design-tokens";
import { getProviderIcon } from "../../../core/utils/providerIcons";
import { BottomMenu } from "../../components/BottomMenu";

// Tool definitions matching the Rust backend
const CREATION_HELPER_TOOLS = [
  {
    id: "set_character_name",
    name: "Set Name",
    description: "Set the character's name",
    icon: User,
    category: "basic",
  },
  {
    id: "set_character_definition",
    name: "Set Definition",
    description: "Set personality and background",
    icon: PenTool,
    category: "basic",
  },
  {
    id: "add_scene",
    name: "Add Scene",
    description: "Add a starting scene for roleplay",
    icon: BookOpen,
    category: "content",
  },
  {
    id: "update_scene",
    name: "Update Scene",
    description: "Modify an existing scene",
    icon: PenTool,
    category: "content",
  },
  {
    id: "toggle_avatar_gradient",
    name: "Avatar Gradient",
    description: "Toggle gradient overlay on avatar",
    icon: Palette,
    category: "visual",
  },
  {
    id: "set_default_model",
    name: "Set Model",
    description: "Set the AI model for conversations",
    icon: Cpu,
    category: "settings",
  },
  {
    id: "set_system_prompt",
    name: "System Prompt",
    description: "Set behavioral guidelines",
    icon: Settings2,
    category: "settings",
  },
  {
    id: "get_system_prompt_list",
    name: "List Prompts",
    description: "View available prompts",
    icon: List,
    category: "settings",
  },
  {
    id: "get_model_list",
    name: "List Models",
    description: "View available models",
    icon: List,
    category: "settings",
  },
  {
    id: "use_uploaded_image_as_avatar",
    name: "Image as Avatar",
    description: "Use uploaded image as avatar",
    icon: FileImage,
    category: "visual",
  },
  {
    id: "use_uploaded_image_as_chat_background",
    name: "Image as Background",
    description: "Use uploaded image as background",
    icon: Image,
    category: "visual",
  },
  {
    id: "generate_image",
    name: "Generate Image",
    description: "Generate an image with the AI model",
    icon: Wand2,
    category: "visual",
  },
  {
    id: "show_preview",
    name: "Show Preview",
    description: "Preview the character",
    icon: Eye,
    category: "flow",
  },
  {
    id: "request_confirmation",
    name: "Request Confirmation",
    description: "Ask to save or continue",
    icon: CheckCircle2,
    category: "flow",
  },
  {
    id: "list_personas",
    name: "List Personas",
    description: "Browse personas",
    icon: List,
    category: "persona",
  },
  {
    id: "upsert_persona",
    name: "Save Persona",
    description: "Create or update a persona",
    icon: User,
    category: "persona",
  },
  {
    id: "use_uploaded_image_as_persona_avatar",
    name: "Persona Avatar",
    description: "Use uploaded image as persona avatar",
    icon: FileImage,
    category: "persona",
  },
  {
    id: "delete_persona",
    name: "Delete Persona",
    description: "Remove a persona",
    icon: Check,
    category: "persona",
  },
  {
    id: "get_default_persona",
    name: "Default Persona",
    description: "Fetch the default persona",
    icon: User,
    category: "persona",
  },
  {
    id: "list_lorebooks",
    name: "List Lorebooks",
    description: "Browse lorebooks",
    icon: List,
    category: "lorebook",
  },
  {
    id: "upsert_lorebook",
    name: "Save Lorebook",
    description: "Create or update a lorebook",
    icon: BookOpen,
    category: "lorebook",
  },
  {
    id: "delete_lorebook",
    name: "Delete Lorebook",
    description: "Remove a lorebook",
    icon: Check,
    category: "lorebook",
  },
  {
    id: "list_lorebook_entries",
    name: "List Entries",
    description: "View lorebook entries",
    icon: List,
    category: "lorebook",
  },
  {
    id: "get_lorebook_entry",
    name: "Get Entry",
    description: "Fetch a lorebook entry",
    icon: BookOpen,
    category: "lorebook",
  },
  {
    id: "upsert_lorebook_entry",
    name: "Save Entry",
    description: "Create or update an entry",
    icon: PenTool,
    category: "lorebook",
  },
  {
    id: "delete_lorebook_entry",
    name: "Delete Entry",
    description: "Remove a lorebook entry",
    icon: Check,
    category: "lorebook",
  },
  {
    id: "create_blank_lorebook_entry",
    name: "Blank Entry",
    description: "Create a placeholder entry",
    icon: PenTool,
    category: "lorebook",
  },
  {
    id: "reorder_lorebook_entries",
    name: "Reorder Entries",
    description: "Change entry ordering",
    icon: List,
    category: "lorebook",
  },
  {
    id: "list_character_lorebooks",
    name: "List Character Lorebooks",
    description: "See lorebooks for a character",
    icon: BookOpen,
    category: "lorebook",
  },
  {
    id: "set_character_lorebooks",
    name: "Set Character Lorebooks",
    description: "Assign lorebooks to a character",
    icon: BookOpen,
    category: "lorebook",
  },
] as const;

const TOOL_CATEGORIES = {
  basic: { label: "Basic", color: "blue" },
  content: { label: "Content", color: "emerald" },
  visual: { label: "Visual", color: "amber" },
  settings: { label: "Settings", color: "rose" },
  flow: { label: "Flow", color: "cyan" },
  persona: { label: "Personas", color: "purple" },
  lorebook: { label: "Lorebooks", color: "amber" },
} as const;

const TOOL_PRESETS = [
  {
    id: "all",
    name: "All Tools",
    description: "Enable all available tools",
    tools: CREATION_HELPER_TOOLS.map((t) => t.id),
  },
  {
    id: "essential",
    name: "Essential",
    description: "Name, definition, and scenes only",
    tools: [
      "set_character_name",
      "set_character_definition",
      "add_scene",
      "show_preview",
      "request_confirmation",
      "list_character_lorebooks",
      "set_character_lorebooks",
    ],
  },
  {
    id: "minimal",
    name: "Minimal",
    description: "Just name and definition",
    tools: ["set_character_name", "set_character_definition", "request_confirmation"],
  },
] as const;

export function CreationHelperPage() {
  const [isLoading, setIsLoading] = useState(true);
  const [models, setModels] = useState<Model[]>([]);
  const [defaultModelId, setDefaultModelId] = useState<string | null>(null);

  // Settings state
  const [selectedModelId, setSelectedModelId] = useState<string | null>(null);
  const [streamingEnabled, setStreamingEnabled] = useState(true);
  const [imageModelId, setImageModelId] = useState<string | null>(null);
  const [smartToolSelection, setSmartToolSelection] = useState(true);
  const [enabledTools, setEnabledTools] = useState<string[]>(
    CREATION_HELPER_TOOLS.map((t) => t.id),
  );

  // Menu states
  const [showModelMenu, setShowModelMenu] = useState(false);
  const [showImageModelMenu, setShowImageModelMenu] = useState(false);
  const [modelSearchQuery, setModelSearchQuery] = useState("");
  const [imageModelSearchQuery, setImageModelSearchQuery] = useState("");

  useEffect(() => {
    const loadData = async () => {
      try {
        const settings = await readSettings();
        setModels(settings.models);
        setDefaultModelId(settings.defaultModelId);
        setSelectedModelId(settings.advancedSettings?.creationHelperModelId ?? null);
        setStreamingEnabled(settings.advancedSettings?.creationHelperStreaming ?? true);
        setImageModelId(settings.advancedSettings?.creationHelperImageModelId ?? null);
        setSmartToolSelection(settings.advancedSettings?.creationHelperSmartToolSelection ?? true);
        setEnabledTools(
          settings.advancedSettings?.creationHelperEnabledTools ??
            CREATION_HELPER_TOOLS.map((t) => t.id),
        );
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
      creationHelperSmartToolSelection: boolean;
      creationHelperEnabledTools: string[];
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

  const handleSmartToolToggle = async () => {
    const newValue = !smartToolSelection;
    setSmartToolSelection(newValue);
    await saveSettings({ creationHelperSmartToolSelection: newValue });
  };

  const handleToolToggle = async (toolId: string) => {
    const newTools = enabledTools.includes(toolId)
      ? enabledTools.filter((t) => t !== toolId)
      : [...enabledTools, toolId];
    setEnabledTools(newTools);
    await saveSettings({ creationHelperEnabledTools: newTools });
  };

  const handlePresetSelect = async (presetId: string) => {
    const preset = TOOL_PRESETS.find((p) => p.id === presetId);
    if (preset) {
      setEnabledTools([...preset.tools]);
      await saveSettings({ creationHelperEnabledTools: [...preset.tools] });
    }
  };

  const textModels = useMemo(
    () => models.filter((m) => !m.outputScopes || m.outputScopes.includes("text")),
    [models],
  );

  const imageModels = useMemo(
    () => models.filter((m) => m.outputScopes?.includes("image")),
    [models],
  );

  const selectedModel = selectedModelId ? models.find((m) => m.id === selectedModelId) : null;
  const defaultModel = defaultModelId ? models.find((m) => m.id === defaultModelId) : null;
  const selectedImageModel = imageModelId ? models.find((m) => m.id === imageModelId) : null;

  const currentPreset = useMemo(() => {
    for (const preset of TOOL_PRESETS) {
      if (
        preset.tools.length === enabledTools.length &&
        preset.tools.every((t) => enabledTools.includes(t))
      ) {
        return preset.id;
      }
    }
    return "custom";
  }, [enabledTools]);

  const groupedTools = useMemo(() => {
    const groups: Record<string, (typeof CREATION_HELPER_TOOLS)[number][]> = {};
    for (const tool of CREATION_HELPER_TOOLS) {
      if (!groups[tool.category]) {
        groups[tool.category] = [];
      }
      groups[tool.category].push(tool);
    }
    return groups;
  }, []);

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
                Creation Helper guides you through building characters with AI assistance. Configure
                the model and tools used during character creation.
              </p>
            </div>
          </div>

          {/* Desktop: Two Column Layout / Mobile: Single Column */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            {/* Left Column - Model Configuration */}
            <div className="space-y-4">
              <h3 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35 px-1">
                Model Configuration
              </h3>

              {/* Chat Model Selector */}
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-danger/30 bg-danger/10 p-1.5">
                    <MessageSquare className="h-4 w-4 text-danger" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">Chat Model</h3>
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
                      <span
                        className={`text-sm ${selectedModelId ? "text-fg" : "text-fg/50"}`}
                      >
                        {selectedModelId
                          ? selectedModel?.displayName || "Selected Model"
                          : `Use app default${defaultModel ? ` (${defaultModel.displayName})` : ""}`}
                      </span>
                    </div>
                    <ChevronDown className="h-4 w-4 text-fg/40" />
                  </button>
                ) : (
                  <div className="rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                    <p className="text-sm text-fg/50">No models available</p>
                  </div>
                )}
                <p className="text-xs text-fg/50 px-1">
                  AI model for character creation conversations
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
                      <span className="text-sm font-medium text-fg">Streaming Output</span>
                      <p className="text-[11px] text-fg/45">
                        Show responses as they're generated
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
                  <h3 className="text-sm font-semibold text-fg">Image Generation Model</h3>
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
                          ? selectedImageModel?.displayName || "Selected Model"
                          : "No model selected"}
                      </span>
                    </div>
                    <ChevronDown className="h-4 w-4 text-fg/40" />
                  </button>
                ) : (
                  <div className="rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3">
                    <p className="text-sm text-fg/50">No image models available</p>
                  </div>
                )}
                <p className="text-xs text-fg/50 px-1">For generating character avatars</p>
              </div>
            </div>

            {/* Right Column - Tool Selection */}
            <div className="space-y-4">
              <h3 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35 px-1">
                Tool Selection
              </h3>

              {/* Smart Tool Selection Toggle */}
              <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-3">
                    <div className="rounded-lg border border-info/30 bg-info/10 p-1.5">
                      <Wand2 className="h-4 w-4 text-info" />
                    </div>
                    <div>
                      <span className="text-sm font-medium text-fg">Smart Tool Selection</span>
                      <p className="text-[11px] text-fg/45">
                        AI automatically chooses which tools to use
                      </p>
                    </div>
                  </div>
                  <label className="relative inline-flex items-center cursor-pointer">
                    <input
                      type="checkbox"
                      checked={smartToolSelection}
                      onChange={handleSmartToolToggle}
                      className="sr-only peer"
                    />
                    <div
                      className={cn(
                        "w-9 h-5 rounded-full transition-colors",
                        smartToolSelection ? "bg-info" : "bg-fg/20",
                      )}
                    >
                      <div
                        className={cn(
                          "absolute top-0.5 left-0.5 w-4 h-4 bg-fg rounded-full transition-transform shadow-sm",
                          smartToolSelection && "translate-x-4",
                        )}
                      />
                    </div>
                  </label>
                </div>
                <div className="mt-3 rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-[11px] text-fg/60">
                  {smartToolSelection
                    ? "When enabled, the AI Creator Helper asks what you want to create and loads only the relevant tool set."
                    : "When disabled, the AI Creator Helper opens directly and uses all enabled tools; the assistant decides what to build."}
                </div>
              </div>

              {/* Tool Presets - shown when smart selection is OFF */}
              {!smartToolSelection && (
                <>
                  <div className="space-y-3">
                    <p className="text-xs text-fg/50 px-1">Quick Presets</p>
                    <div className="grid grid-cols-3 gap-2">
                      {TOOL_PRESETS.map((preset) => (
                        <button
                          key={preset.id}
                          onClick={() => handlePresetSelect(preset.id)}
                          className={cn(
                            "rounded-xl border px-3 py-2.5 text-center transition-all",
                            currentPreset === preset.id
                              ? "border-danger/40 bg-danger/15 text-danger"
                              : "border-fg/10 bg-fg/5 text-fg/60 hover:border-fg/20",
                          )}
                        >
                          <span className="text-xs font-medium">{preset.name}</span>
                        </button>
                      ))}
                    </div>
                    {currentPreset === "custom" && (
                      <p className="text-[11px] text-warning/70 px-1">
                        Custom selection — {enabledTools.length} tools enabled
                      </p>
                    )}
                  </div>

                  {/* Tool List */}
                  <div className="space-y-4">
                    {Object.entries(groupedTools).map(([category, tools]) => {
                      const categoryInfo =
                        TOOL_CATEGORIES[category as keyof typeof TOOL_CATEGORIES];
                      const colorMap = {
                        blue: {
                          badge: "border-info/30 bg-info/10 text-info/80",
                        },
                        emerald: {
                          badge: "border-accent/30 bg-accent/10 text-accent/80",
                        },
                        amber: {
                          badge: "border-warning/30 bg-warning/10 text-warning/80",
                        },
                        rose: {
                          badge: "border-danger/30 bg-danger/10 text-danger/80",
                        },
                        cyan: {
                          badge: "border-info/30 bg-info/10 text-info/80",
                        },
                        purple: {
                          badge: "border-secondary/30 bg-secondary/10 text-secondary/80",
                        },
                      };

                      const categoryColors =
                        colorMap[categoryInfo.color as keyof typeof colorMap] ?? colorMap.blue;

                      return (
                        <div key={category} className="space-y-2">
                          <div className="flex items-center gap-2 px-1">
                            <span
                              className={cn(
                                "rounded-md border px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wider",
                                categoryColors.badge,
                              )}
                            >
                              {categoryInfo.label}
                            </span>
                          </div>

                          <div className="rounded-xl border border-fg/10 bg-fg/5 overflow-hidden divide-y divide-fg/5">
                            {tools.map((tool) => {
                              const Icon = tool.icon;
                              const isEnabled = enabledTools.includes(tool.id);

                              return (
                                <button
                                  key={tool.id}
                                  onClick={() => handleToolToggle(tool.id)}
                                  className={cn(
                                    "w-full flex items-center gap-3 px-4 py-3 text-left",
                                    "transition-colors hover:bg-fg/5",
                                  )}
                                >
                                  <div
                                    className={cn(
                                      "flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border",
                                      isEnabled
                                        ? "border-fg/20 bg-fg/10"
                                        : "border-fg/10 bg-fg/5",
                                    )}
                                  >
                                    <Icon
                                      className={cn(
                                        "h-4 w-4 transition-colors",
                                        isEnabled ? "text-fg/70" : "text-fg/30",
                                      )}
                                    />
                                  </div>
                                  <div className="min-w-0 flex-1">
                                    <span
                                      className={cn(
                                        "text-sm font-medium",
                                        isEnabled ? "text-fg" : "text-fg/50",
                                      )}
                                    >
                                      {tool.name}
                                    </span>
                                    <p className="text-[11px] text-fg/40 truncate">
                                      {tool.description}
                                    </p>
                                  </div>
                                  <div
                                    className={cn(
                                      "flex h-5 w-5 shrink-0 items-center justify-center rounded-md border transition-all",
                                      isEnabled
                                        ? "border-accent/50 bg-accent/20"
                                        : "border-fg/15 bg-fg/5",
                                    )}
                                  >
                                    {isEnabled && <Check className="h-3 w-3 text-accent/80" />}
                                  </div>
                                </button>
                              );
                            })}
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </>
              )}
            </div>
          </div>

          {/* Bottom Info Card - Full Width */}
          <div
            className={cn(
              "rounded-xl border px-4 py-3.5",
              colors.glass.subtle,
              "flex items-start gap-3",
            )}
          >
            <Info className="mt-0.5 h-4 w-4 shrink-0 text-fg/30" />
            <div className="text-[11px] leading-relaxed text-fg/45">
              <p>
                When <strong className="text-fg/60">Smart Tool Selection</strong> is enabled, the
                AI decides which tools to use based on context. Disable it to manually control which
                tools are available.
              </p>
            </div>
          </div>
        </div>
      </main>

      {/* Chat Model Selection BottomMenu */}
      <BottomMenu
        isOpen={showModelMenu}
        onClose={() => {
          setShowModelMenu(false);
          setModelSearchQuery("");
        }}
        title="Select Chat Model"
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
                handleModelChange(null);
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
              <div className="flex-1 min-w-0">
                <span className="text-sm text-fg">Use app default</span>
                {defaultModel && (
                  <span className="block truncate text-xs text-fg/40">
                    {defaultModel.displayName}
                  </span>
                )}
              </div>
              {!selectedModelId && <Check className="h-4 w-4 ml-auto text-accent/80" />}
            </button>
            {textModels
              .filter((model) => {
                if (!modelSearchQuery) return true;
                const q = modelSearchQuery.toLowerCase();
                return (
                  model.displayName?.toLowerCase().includes(q) ||
                  model.name?.toLowerCase().includes(q)
                );
              })
              .map((model) => (
                <button
                  key={model.id}
                  onClick={() => {
                    handleModelChange(model.id);
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
                    <Check className="h-4 w-4 shrink-0 text-accent/80" />
                  )}
                </button>
              ))}
          </div>
        </div>
      </BottomMenu>

      {/* Image Model Selection BottomMenu */}
      <BottomMenu
        isOpen={showImageModelMenu}
        onClose={() => {
          setShowImageModelMenu(false);
          setImageModelSearchQuery("");
        }}
        title="Select Image Model"
      >
        <div className="space-y-4">
          <div className="relative">
            <input
              type="text"
              value={imageModelSearchQuery}
              onChange={(e) => setImageModelSearchQuery(e.target.value)}
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
                handleImageModelChange(null);
                setShowImageModelMenu(false);
                setImageModelSearchQuery("");
              }}
              className={cn(
                "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition",
                !imageModelId
                  ? "border-accent/40 bg-accent/10"
                  : "border-fg/10 bg-fg/5 hover:bg-fg/10",
              )}
            >
              <Image className="h-5 w-5 text-fg/40" />
              <span className="text-sm text-fg">No model selected</span>
              {!imageModelId && <Check className="h-4 w-4 ml-auto text-accent/80" />}
            </button>
            {imageModels
              .filter((model) => {
                if (!imageModelSearchQuery) return true;
                const q = imageModelSearchQuery.toLowerCase();
                return (
                  model.displayName?.toLowerCase().includes(q) ||
                  model.name?.toLowerCase().includes(q)
                );
              })
              .map((model) => (
                <button
                  key={model.id}
                  onClick={() => {
                    handleImageModelChange(model.id);
                    setShowImageModelMenu(false);
                    setImageModelSearchQuery("");
                  }}
                  className={cn(
                    "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition",
                    imageModelId === model.id
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
                  {imageModelId === model.id && (
                    <Check className="h-4 w-4 shrink-0 text-accent/80" />
                  )}
                </button>
              ))}
          </div>
        </div>
      </BottomMenu>
    </div>
  );
}
