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
import { ModelSelectionBottomMenu } from "../../components/ModelSelectionBottomMenu";
import { useI18n, type TranslationKey } from "../../../core/i18n/context";

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

const TOOL_TEXT_KEYS: Record<string, { name: TranslationKey; desc: TranslationKey }> = {
  set_character_name: {
    name: "creationHelper.tools.set_character_name.name",
    desc: "creationHelper.tools.set_character_name.desc",
  },
  set_character_definition: {
    name: "creationHelper.tools.set_character_definition.name",
    desc: "creationHelper.tools.set_character_definition.desc",
  },
  add_scene: {
    name: "creationHelper.tools.add_scene.name",
    desc: "creationHelper.tools.add_scene.desc",
  },
  update_scene: {
    name: "creationHelper.tools.update_scene.name",
    desc: "creationHelper.tools.update_scene.desc",
  },
  toggle_avatar_gradient: {
    name: "creationHelper.tools.toggle_avatar_gradient.name",
    desc: "creationHelper.tools.toggle_avatar_gradient.desc",
  },
  set_default_model: {
    name: "creationHelper.tools.set_default_model.name",
    desc: "creationHelper.tools.set_default_model.desc",
  },
  set_system_prompt: {
    name: "creationHelper.tools.set_system_prompt.name",
    desc: "creationHelper.tools.set_system_prompt.desc",
  },
  get_system_prompt_list: {
    name: "creationHelper.tools.get_system_prompt_list.name",
    desc: "creationHelper.tools.get_system_prompt_list.desc",
  },
  get_model_list: {
    name: "creationHelper.tools.get_model_list.name",
    desc: "creationHelper.tools.get_model_list.desc",
  },
  use_uploaded_image_as_avatar: {
    name: "creationHelper.tools.use_uploaded_image_as_avatar.name",
    desc: "creationHelper.tools.use_uploaded_image_as_avatar.desc",
  },
  use_uploaded_image_as_chat_background: {
    name: "creationHelper.tools.use_uploaded_image_as_chat_background.name",
    desc: "creationHelper.tools.use_uploaded_image_as_chat_background.desc",
  },
  generate_image: {
    name: "creationHelper.tools.generate_image.name",
    desc: "creationHelper.tools.generate_image.desc",
  },
  show_preview: {
    name: "creationHelper.tools.show_preview.name",
    desc: "creationHelper.tools.show_preview.desc",
  },
  request_confirmation: {
    name: "creationHelper.tools.request_confirmation.name",
    desc: "creationHelper.tools.request_confirmation.desc",
  },
  list_personas: {
    name: "creationHelper.tools.list_personas.name",
    desc: "creationHelper.tools.list_personas.desc",
  },
  upsert_persona: {
    name: "creationHelper.tools.upsert_persona.name",
    desc: "creationHelper.tools.upsert_persona.desc",
  },
  use_uploaded_image_as_persona_avatar: {
    name: "creationHelper.tools.use_uploaded_image_as_persona_avatar.name",
    desc: "creationHelper.tools.use_uploaded_image_as_persona_avatar.desc",
  },
  delete_persona: {
    name: "creationHelper.tools.delete_persona.name",
    desc: "creationHelper.tools.delete_persona.desc",
  },
  get_default_persona: {
    name: "creationHelper.tools.get_default_persona.name",
    desc: "creationHelper.tools.get_default_persona.desc",
  },
  list_lorebooks: {
    name: "creationHelper.tools.list_lorebooks.name",
    desc: "creationHelper.tools.list_lorebooks.desc",
  },
  upsert_lorebook: {
    name: "creationHelper.tools.upsert_lorebook.name",
    desc: "creationHelper.tools.upsert_lorebook.desc",
  },
  delete_lorebook: {
    name: "creationHelper.tools.delete_lorebook.name",
    desc: "creationHelper.tools.delete_lorebook.desc",
  },
  list_lorebook_entries: {
    name: "creationHelper.tools.list_lorebook_entries.name",
    desc: "creationHelper.tools.list_lorebook_entries.desc",
  },
  get_lorebook_entry: {
    name: "creationHelper.tools.get_lorebook_entry.name",
    desc: "creationHelper.tools.get_lorebook_entry.desc",
  },
  upsert_lorebook_entry: {
    name: "creationHelper.tools.upsert_lorebook_entry.name",
    desc: "creationHelper.tools.upsert_lorebook_entry.desc",
  },
  delete_lorebook_entry: {
    name: "creationHelper.tools.delete_lorebook_entry.name",
    desc: "creationHelper.tools.delete_lorebook_entry.desc",
  },
  create_blank_lorebook_entry: {
    name: "creationHelper.tools.create_blank_lorebook_entry.name",
    desc: "creationHelper.tools.create_blank_lorebook_entry.desc",
  },
  reorder_lorebook_entries: {
    name: "creationHelper.tools.reorder_lorebook_entries.name",
    desc: "creationHelper.tools.reorder_lorebook_entries.desc",
  },
  list_character_lorebooks: {
    name: "creationHelper.tools.list_character_lorebooks.name",
    desc: "creationHelper.tools.list_character_lorebooks.desc",
  },
  set_character_lorebooks: {
    name: "creationHelper.tools.set_character_lorebooks.name",
    desc: "creationHelper.tools.set_character_lorebooks.desc",
  },
};

const CATEGORY_LABEL_KEYS: Record<string, TranslationKey> = {
  basic: "creationHelper.categories.basic",
  content: "creationHelper.categories.content",
  visual: "creationHelper.categories.visual",
  settings: "creationHelper.categories.settings",
  flow: "creationHelper.categories.flow",
  persona: "creationHelper.categories.persona",
  lorebook: "creationHelper.categories.lorebook",
};

const PRESET_TEXT_KEYS: Record<string, { name: TranslationKey; desc: TranslationKey }> = {
  all: { name: "creationHelper.presets.all.name", desc: "creationHelper.presets.all.desc" },
  essential: {
    name: "creationHelper.presets.essential.name",
    desc: "creationHelper.presets.essential.desc",
  },
  minimal: {
    name: "creationHelper.presets.minimal.name",
    desc: "creationHelper.presets.minimal.desc",
  },
};

export function CreationHelperPage() {
  const { t } = useI18n();
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
  const selectedModelLabel = selectedModel?.displayName || t("creationHelper.page.selectedModel");
  const chatDefaultLabel = t("creationHelper.page.useAppDefault", {
    model: defaultModel ? ` (${defaultModel.displayName})` : "",
  });

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
                {t("creationHelper.page.info")}
              </p>
            </div>
          </div>

          {/* Desktop: Two Column Layout / Mobile: Single Column */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            {/* Left Column - Model Configuration */}
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

            {/* Right Column - Tool Selection */}
            <div className="space-y-4">
              <h3 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35 px-1">
                {t("creationHelper.page.toolSelection")}
              </h3>

              {/* Smart Tool Selection Toggle */}
              <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-3">
                    <div className="rounded-lg border border-info/30 bg-info/10 p-1.5">
                      <Wand2 className="h-4 w-4 text-info" />
                    </div>
                    <div>
                      <span className="text-sm font-medium text-fg">
                        {t("creationHelper.page.smartToolSelection")}
                      </span>
                      <p className="text-[11px] text-fg/45">
                        {t("creationHelper.page.smartToolDescription")}
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
                    ? t("creationHelper.page.smartToolEnabledHint")
                    : t("creationHelper.page.smartToolDisabledHint")}
                </div>
              </div>

              {/* Tool Presets - shown when smart selection is OFF */}
              {!smartToolSelection && (
                <>
                  <div className="space-y-3">
                    <p className="text-xs text-fg/50 px-1">
                      {t("creationHelper.page.quickPresets")}
                    </p>
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
                          <span className="text-xs font-medium">
                            {PRESET_TEXT_KEYS[preset.id]
                              ? t(PRESET_TEXT_KEYS[preset.id].name)
                              : preset.name}
                          </span>
                        </button>
                      ))}
                    </div>
                    {currentPreset === "custom" && (
                      <p className="text-[11px] text-warning/70 px-1">
                        {t("creationHelper.page.customSelection", { count: enabledTools.length })}
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
                              {CATEGORY_LABEL_KEYS[category]
                                ? t(CATEGORY_LABEL_KEYS[category])
                                : categoryInfo.label}
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
                                      isEnabled ? "border-fg/20 bg-fg/10" : "border-fg/10 bg-fg/5",
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
                                      {TOOL_TEXT_KEYS[tool.id]
                                        ? t(TOOL_TEXT_KEYS[tool.id].name)
                                        : tool.name}
                                    </span>
                                    <p className="text-[11px] text-fg/40 truncate">
                                      {TOOL_TEXT_KEYS[tool.id]
                                        ? t(TOOL_TEXT_KEYS[tool.id].desc)
                                        : tool.description}
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
              <p>{t("creationHelper.page.footerInfo")}</p>
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
