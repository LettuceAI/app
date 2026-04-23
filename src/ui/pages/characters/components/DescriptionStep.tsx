import { useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  Sparkles,
  AlertCircle,
  FileText,
  Loader2,
  Layers,
  ChevronDown,
  Check,
  Cpu,
  Volume2,
  User,
} from "lucide-react";
import type {
  CharacterMode,
  CompanionConfig,
  Model,
  SystemPromptTemplate,
} from "../../../../core/storage/schemas";
import { typography, radius, spacing, interactive, shadows, cn } from "../../../design-tokens";
import { BottomMenu, MenuSection } from "../../../components/BottomMenu";
import { ModelSelectionBottomMenu } from "../../../components/ModelSelectionBottomMenu";
import { Switch } from "../../../components/Switch";
import { getProviderIcon } from "../../../../core/utils/providerIcons";
import { useI18n } from "../../../../core/i18n/context";
import {
  APP_COMPANION_TEMPLATE_ID,
  APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID,
  APP_GROUP_CHAT_TEMPLATE_ID,
} from "../../../../core/prompts/constants";
import { InteractionModeSelector } from "./InteractionModeSelector";
import { CompanionSoulEditor } from "./CompanionSoulEditor";

interface DescriptionStepProps {
  definition: string;
  onDefinitionChange: (value: string) => void;
  description: string;
  onDescriptionChange: (value: string) => void;
  mode: CharacterMode;
  onModeChange: (value: CharacterMode) => void;
  companion: CompanionConfig | null | undefined;
  onCompanionChange: (value: CompanionConfig | null) => void;
  models: Model[];
  loadingModels: boolean;
  selectedModelId: string | null;
  onSelectModel: (value: string | null) => void;
  selectedFallbackModelId: string | null;
  onSelectFallbackModel: (value: string | null) => void;
  memoryType: "manual" | "dynamic";
  dynamicMemoryEnabled: boolean;
  onMemoryTypeChange: (value: "manual" | "dynamic") => void;
  promptTemplates: SystemPromptTemplate[];
  loadingTemplates: boolean;
  systemPromptTemplateId: string | null;
  onSelectSystemPrompt: (value: string | null) => void;
  companionPromptTemplateId: string | null;
  onSelectCompanionPrompt: (value: string | null) => void;
  groupChatPromptTemplateId: string | null;
  onSelectGroupChatPrompt: (value: string | null) => void;
  groupChatRoleplayPromptTemplateId: string | null;
  onSelectGroupChatRoleplayPrompt: (value: string | null) => void;
  voiceConfig: any | null;
  onVoiceConfigChange: (value: any | null) => void;
  voiceAutoplay: boolean;
  onVoiceAutoplayChange: (value: boolean) => void;
  audioProviders: any[];
  userVoices: any[];
  providerVoices: Record<string, any[]>;
  loadingVoices: boolean;
  voiceError: string | null;
  onSave: () => void;
  canSave: boolean;
  saving: boolean;
  error: string | null;
  submitLabel?: string;
}

export function DescriptionStep({
  definition,
  onDefinitionChange,
  description,
  onDescriptionChange,
  mode,
  onModeChange,
  companion,
  onCompanionChange,
  models,
  loadingModels,
  selectedModelId,
  onSelectModel,
  selectedFallbackModelId,
  onSelectFallbackModel,
  memoryType,
  dynamicMemoryEnabled,
  onMemoryTypeChange,
  promptTemplates,
  loadingTemplates,
  systemPromptTemplateId,
  onSelectSystemPrompt,
  companionPromptTemplateId,
  onSelectCompanionPrompt,
  groupChatPromptTemplateId,
  onSelectGroupChatPrompt,
  groupChatRoleplayPromptTemplateId,
  onSelectGroupChatRoleplayPrompt,
  voiceConfig,
  onVoiceConfigChange,
  voiceAutoplay,
  onVoiceAutoplayChange,
  audioProviders,
  userVoices,
  providerVoices,
  loadingVoices,
  voiceError,
  onSave,
  canSave,
  saving,
  error,
  submitLabel,
}: DescriptionStepProps) {
  const { t } = useI18n();
  const resolvedSubmitLabel = submitLabel ?? t("characters.identity.title");
  const wordCount = definition.trim().split(/\s+/).filter(Boolean).length;
  const [showModelMenu, setShowModelMenu] = useState(false);
  const [showFallbackModelMenu, setShowFallbackModelMenu] = useState(false);
  const [showVoiceMenu, setShowVoiceMenu] = useState(false);
  const [voiceSearchQuery, setVoiceSearchQuery] = useState("");
  const directPromptTemplates = promptTemplates.filter(
    (template) => template.promptType === "undefined" || template.promptType === "directChat",
  );
  const companionPromptTemplates = promptTemplates.filter(
    (template) =>
      template.promptType === "companionChat" && template.id !== APP_COMPANION_TEMPLATE_ID,
  );
  const groupChatTemplates = promptTemplates.filter(
    (template) =>
      template.promptType === "groupChatConversational" &&
      template.id !== APP_GROUP_CHAT_TEMPLATE_ID,
  );
  const groupChatRoleplayTemplates = promptTemplates.filter(
    (template) =>
      template.promptType === "groupChatRoleplay" &&
      template.id !== APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID,
  );

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

  return (
    <motion.div
      initial={{ opacity: 0, y: 16 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -16 }}
      transition={{ duration: 0.2, ease: "easeOut" }}
      className={spacing.section}
    >
      {/* Title */}
      <div className={spacing.tight}>
        <h2 className={cn(typography.h1.size, typography.h1.weight, "text-fg")}>
          {t("characters.details.title")}
        </h2>
        <p className={cn(typography.body.size, "text-fg/50")}>{t("characters.details.subtitle")}</p>
      </div>

      <InteractionModeSelector mode={mode} onChange={onModeChange} />

      {mode === "companion" && (
        <CompanionSoulEditor companion={companion} onChange={onCompanionChange} />
      )}

      {/* Desktop: Two-column layout / Mobile: stacked */}
      <div className="flex flex-col lg:flex-row lg:gap-8">
        {/* Left column: Definition + Description textareas */}
        <div className="lg:flex-1 lg:min-w-0 space-y-6">
          {/* Definition Textarea */}
          <div className={spacing.field}>
            <div className="flex items-center justify-between">
              <label
                className={cn(
                  typography.label.size,
                  typography.label.weight,
                  typography.label.tracking,
                  "uppercase text-fg/70",
                )}
              >
                {t("characters.details.definition")}
              </label>
              {definition.trim() && (
                <motion.span
                  initial={{ opacity: 0, scale: 0.9 }}
                  animate={{ opacity: 1, scale: 1 }}
                  className={cn(typography.caption.size, typography.caption.weight, "text-fg/40")}
                >
                  {t("characters.details.wordCount", { count: wordCount })}
                </motion.span>
              )}
            </div>
            <div className="relative">
              <textarea
                value={definition}
                onChange={(e) => onDefinitionChange(e.target.value)}
                rows={8}
                placeholder={t("characters.details.definitionPlaceholder")}
                className={cn(
                  "w-full resize-none border bg-surface-el/20 px-4 py-3 text-base leading-relaxed text-fg placeholder-fg/40 backdrop-blur-xl lg:rows-12",
                  radius.md,
                  interactive.transition.default,
                  "focus:bg-surface-el/30 focus:outline-none",
                  definition.trim()
                    ? "border-accent/30 focus:border-accent/40"
                    : "border-fg/10 focus:border-fg/30",
                )}
                autoFocus
              />
              {definition.trim() && (
                <motion.div
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  className="pointer-events-none absolute bottom-3 right-3"
                >
                  <div
                    className={cn(
                      "flex h-7 w-7 items-center justify-center",
                      radius.full,
                      "border border-accent/30 bg-accent/15",
                    )}
                  >
                    <Sparkles className="h-3.5 w-3.5 text-accent/80" />
                  </div>
                </motion.div>
              )}
            </div>
            <p className={cn(typography.bodySmall.size, "text-fg/40")}>
              {t("characters.details.definitionDesc")}
            </p>
            <div className="rounded-xl border border-info/20 bg-info/10 px-4 py-3">
              <div className="text-xs font-medium text-info">{t("characters.details.availablePlaceholders")}</div>
              <div className="mt-2 space-y-1 text-xs text-info/70">
                <div>
                  <code className="text-accent/80">{"{{char}}"}</code> - Character name
                </div>
                <div>
                  <code className="text-accent/80">{"{{user}}"}</code> - Persona name (preferred,
                  empty if none)
                </div>
                <div>
                  <code className="text-accent/80">{"{{persona}}"}</code> - Persona name (alias)
                </div>
              </div>
            </div>
          </div>

          {/* UI Description */}
          <div className={spacing.field}>
            <div className="flex items-center justify-between">
              <label
                className={cn(
                  typography.label.size,
                  typography.label.weight,
                  typography.label.tracking,
                  "uppercase text-fg/70",
                )}
              >
                Description
              </label>
            </div>
            <textarea
              value={description}
              onChange={(e) => onDescriptionChange(e.target.value)}
              rows={3}
              placeholder="Short summary shown on cards and lists..."
              className={cn(
                "w-full resize-none border bg-surface-el/20 px-4 py-3 text-base leading-relaxed text-fg placeholder-fg/40 backdrop-blur-xl",
                radius.md,
                interactive.transition.default,
                "focus:bg-surface-el/30 focus:outline-none",
                description.trim()
                  ? "border-fg/20 focus:border-fg/40"
                  : "border-fg/10 focus:border-fg/30",
              )}
            />
            <p className={cn(typography.bodySmall.size, "text-fg/40")}>
              Optional short description for the UI; the full definition is used in prompts.
            </p>
          </div>
        </div>

        {/* Right sidebar: Settings (desktop) / continues stacked (mobile) */}
        <div className="lg:w-80 lg:shrink-0 space-y-6 mt-6 lg:mt-0">
          {/* Model Selection */}
          <div className={spacing.field}>
            <label
              className={cn(
                typography.label.size,
                typography.label.weight,
                typography.label.tracking,
                "uppercase text-fg/70",
              )}
            >
              AI Model *
            </label>
            {loadingModels ? (
              <div
                className={cn(
                  "flex items-center gap-3 border border-fg/10 bg-surface-el/20 px-4 py-3 backdrop-blur-xl",
                  radius.md,
                )}
              >
                <div className="h-4 w-4 animate-spin rounded-full border-2 border-fg/10 border-t-white/60" />
                <span className={cn(typography.body.size, "text-fg/60")}>Loading models...</span>
              </div>
            ) : models.length ? (
              <button
                type="button"
                onClick={() => setShowModelMenu(true)}
                className={cn(
                  "flex w-full items-center justify-between border bg-surface-el/20 px-4 py-3.5 text-left backdrop-blur-xl",
                  radius.md,
                  interactive.transition.default,
                  "focus:border-fg/30 focus:bg-surface-el/30 focus:outline-none hover:bg-surface-el/30",
                  selectedModelId ? "border-fg/20" : "border-fg/10",
                )}
              >
                <div className="flex items-center gap-2">
                  {selectedModelId ? (
                    getProviderIcon(models.find((m) => m.id === selectedModelId)?.providerId || "")
                  ) : (
                    <Cpu className="h-5 w-5 text-fg/40" />
                  )}
                  <span className={cn("text-sm", selectedModelId ? "text-fg" : "text-fg/50")}>
                    {selectedModelId
                      ? models.find((m) => m.id === selectedModelId)?.displayName || "Selected Model"
                      : "Select a model"}
                  </span>
                </div>
                <ChevronDown className="h-4 w-4 text-fg/40" />
              </button>
            ) : (
              <div
                className={cn(
                  "border border-warning/20 bg-warning/10 px-4 py-3 backdrop-blur-xl",
                  radius.md,
                )}
              >
                <div className="flex items-start gap-3">
                  <AlertCircle className="mt-0.5 h-5 w-5 shrink-0 text-warning" />
                  <div>
                    <p className={cn(typography.body.size, typography.h3.weight, "text-warning/90")}>
                      No models configured
                    </p>
                    <p className={cn(typography.bodySmall.size, "mt-1 text-warning/70")}>
                      Add a provider in settings first to continue
                    </p>
                  </div>
                </div>
              </div>
            )}
            <p className={cn(typography.bodySmall.size, "text-fg/40")}>
              This model will power the character's responses
            </p>
          </div>

          {/* Fallback Model Selection */}
          <div className={spacing.field}>
            <label
              className={cn(
                typography.label.size,
                typography.label.weight,
                typography.label.tracking,
                "uppercase text-fg/70",
              )}
            >
              Fallback Model (Optional)
            </label>
            {loadingModels ? (
              <div
                className={cn(
                  "flex items-center gap-3 border border-fg/10 bg-surface-el/20 px-4 py-3 backdrop-blur-xl",
                  radius.md,
                )}
              >
                <div className="h-4 w-4 animate-spin rounded-full border-2 border-fg/10 border-t-white/60" />
                <span className={cn(typography.body.size, "text-fg/60")}>Loading models...</span>
              </div>
            ) : (
              <button
                type="button"
                onClick={() => setShowFallbackModelMenu(true)}
                className={cn(
                  "flex w-full items-center justify-between border bg-surface-el/20 px-4 py-3.5 text-left backdrop-blur-xl",
                  radius.md,
                  interactive.transition.default,
                  "focus:border-fg/30 focus:bg-surface-el/30 focus:outline-none hover:bg-surface-el/30",
                  selectedFallbackModelId ? "border-fg/20" : "border-fg/10",
                )}
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
                    className={cn(
                      "text-sm",
                      selectedFallbackModelId ? "text-fg" : "text-fg/50",
                    )}
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
            <p className={cn(typography.bodySmall.size, "text-fg/40")}>
              Retries with this model only if the primary model fails
            </p>
          </div>

          {/* Memory Mode */}
          <div className={spacing.field}>
            <div className="flex items-center justify-between">
              <label
                className={cn(
                  typography.label.size,
                  typography.label.weight,
                  typography.label.tracking,
                  "uppercase text-fg/70",
                )}
              >
                Memory Mode
              </label>
              {!dynamicMemoryEnabled && (
                <span className="text-[11px] text-fg/45">Enable in Settings to switch</span>
              )}
            </div>
            <div className="grid grid-cols-2 gap-2">
              <button
                type="button"
                onClick={() => onMemoryTypeChange("manual")}
                className={cn(
                  "flex flex-col items-start gap-1 rounded-xl border px-3 py-3 text-left transition",
                  memoryType === "manual"
                    ? "border-accent/50 bg-accent/10 text-accent/70 shadow-[0_0_0_1px_rgba(16,185,129,0.2)]"
                    : "border-fg/10 bg-fg/5 text-fg/70 hover:border-fg/20 hover:bg-fg/10",
                )}
              >
                <div className="flex items-center gap-2">
                  <Layers className="h-4 w-4" />
                  <span className="text-sm font-semibold">Manual</span>
                </div>
                <p className="text-xs text-fg/60 hidden lg:block">
                  Add and manage memory notes yourself.
                </p>
                <p className="text-xs text-fg/60 lg:hidden">
                  Current system: add and manage memory notes yourself.
                </p>
              </button>
              <button
                type="button"
                disabled={!dynamicMemoryEnabled}
                onClick={() => dynamicMemoryEnabled && onMemoryTypeChange("dynamic")}
                className={cn(
                  "flex flex-col items-start gap-1 rounded-xl border px-3 py-3 text-left transition",
                  memoryType === "dynamic" && dynamicMemoryEnabled
                    ? "border-info/60 bg-info/15 text-info shadow-[0_0_0_1px_rgba(96,165,250,0.3)]"
                    : "border-fg/10 bg-fg/5 text-fg/60",
                  !dynamicMemoryEnabled && "cursor-not-allowed opacity-50",
                )}
              >
                <div className="flex items-center gap-2">
                  <Sparkles className="h-4 w-4" />
                  <span className="text-sm font-semibold">Dynamic</span>
                </div>
                <p className="text-xs text-fg/60 hidden lg:block">
                  Automatic summaries and context updates.
                </p>
                <p className="text-xs text-fg/60 lg:hidden">
                  Automatic summaries and context updates for this character.
                </p>
              </button>
            </div>
            <p className={cn(typography.bodySmall.size, "text-fg/40")}>
              Dynamic memory requires it to be enabled in Advanced settings. Otherwise, manual memory is
              used.
            </p>
          </div>

          {/* Prompt Selection */}
          <div className={spacing.field}>
            <label
              className={cn(
                typography.label.size,
                typography.label.weight,
                typography.label.tracking,
                "uppercase text-fg/70",
              )}
            >
              {mode === "companion" ? "Companion Prompt (Optional)" : "System Prompt (Optional)"}
            </label>
            {loadingTemplates ? (
              <div
                className={cn(
                  "flex items-center gap-3 border border-fg/10 bg-surface-el/20 px-4 py-3 backdrop-blur-xl",
                  radius.md,
                )}
              >
                <Loader2 className="h-4 w-4 animate-spin text-fg/60" />
                <span className={cn(typography.body.size, "text-fg/60")}>Loading templates...</span>
              </div>
            ) : (
              <div className="relative">
                <select
                  value={
                    mode === "companion"
                      ? (companionPromptTemplateId ?? "")
                      : (systemPromptTemplateId ?? "")
                  }
                  onChange={(e) => {
                    const next = e.target.value || null;
                    if (mode === "companion") {
                      onSelectCompanionPrompt(next);
                    } else {
                      onSelectSystemPrompt(next);
                    }
                  }}
                  className={cn(
                    "w-full appearance-none border bg-surface-el/20 px-4 py-3.5 pr-10 text-sm text-fg backdrop-blur-xl",
                    radius.md,
                    interactive.transition.default,
                    "focus:border-fg/30 focus:bg-surface-el/30 focus:outline-none",
                    (mode === "companion" ? companionPromptTemplateId : systemPromptTemplateId)
                      ? "border-fg/20"
                      : "border-fg/10",
                  )}
                >
                  <option value="" className="bg-surface-el text-fg">
                    {mode === "companion" ? "Use app companion default" : "Use app default"}
                  </option>
                  {(mode === "companion" ? companionPromptTemplates : directPromptTemplates).map((template) => (
                    <option key={template.id} value={template.id} className="bg-surface-el text-fg">
                      {template.name}
                    </option>
                  ))}
                </select>
                {/* Custom dropdown icon */}
                <div className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2">
                  <FileText className="h-4 w-4 text-fg/40" />
                </div>
              </div>
            )}
            <p className={cn(typography.bodySmall.size, "text-fg/40")}>
              {mode === "companion"
                ? "Stored separately as the companion prompt. The normal roleplay system prompt is not changed."
                : "Choose a custom system prompt or use the default."}
            </p>
          </div>

          <div className={spacing.field}>
            <label
              className={cn(
                typography.label.size,
                typography.label.weight,
                typography.label.tracking,
                "uppercase text-fg/70",
              )}
            >
              Group Chat Prompt (Conversation)
            </label>
            {loadingTemplates ? (
              <div
                className={cn(
                  "flex items-center gap-3 border border-fg/10 bg-surface-el/20 px-4 py-3 backdrop-blur-xl",
                  radius.md,
                )}
              >
                <Loader2 className="h-4 w-4 animate-spin text-fg/60" />
                <span className={cn(typography.body.size, "text-fg/60")}>Loading templates...</span>
              </div>
            ) : (
              <div className="relative">
                <select
                  value={groupChatPromptTemplateId ?? ""}
                  onChange={(e) => onSelectGroupChatPrompt(e.target.value || null)}
                  className={cn(
                    "w-full appearance-none border bg-surface-el/20 px-4 py-3.5 pr-10 text-sm text-fg backdrop-blur-xl",
                    radius.md,
                    interactive.transition.default,
                    "focus:border-fg/30 focus:bg-surface-el/30 focus:outline-none",
                    groupChatPromptTemplateId ? "border-fg/20" : "border-fg/10",
                  )}
                >
                  <option value="" className="bg-surface-el text-fg">
                    Use app default
                  </option>
                  {groupChatTemplates.map((template) => (
                    <option key={template.id} value={template.id} className="bg-surface-el text-fg">
                      {template.name}
                    </option>
                  ))}
                </select>
                <div className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2">
                  <FileText className="h-4 w-4 text-fg/40" />
                </div>
              </div>
            )}
            <p className={cn(typography.bodySmall.size, "text-fg/40")}>
              Override this character&apos;s conversation prompt in group chats
            </p>
          </div>

          <div className={spacing.field}>
            <label
              className={cn(
                typography.label.size,
                typography.label.weight,
                typography.label.tracking,
                "uppercase text-fg/70",
              )}
            >
              Group Chat Prompt (Roleplay)
            </label>
            {loadingTemplates ? (
              <div
                className={cn(
                  "flex items-center gap-3 border border-fg/10 bg-surface-el/20 px-4 py-3 backdrop-blur-xl",
                  radius.md,
                )}
              >
                <Loader2 className="h-4 w-4 animate-spin text-fg/60" />
                <span className={cn(typography.body.size, "text-fg/60")}>Loading templates...</span>
              </div>
            ) : (
              <div className="relative">
                <select
                  value={groupChatRoleplayPromptTemplateId ?? ""}
                  onChange={(e) => onSelectGroupChatRoleplayPrompt(e.target.value || null)}
                  className={cn(
                    "w-full appearance-none border bg-surface-el/20 px-4 py-3.5 pr-10 text-sm text-fg backdrop-blur-xl",
                    radius.md,
                    interactive.transition.default,
                    "focus:border-fg/30 focus:bg-surface-el/30 focus:outline-none",
                    groupChatRoleplayPromptTemplateId ? "border-fg/20" : "border-fg/10",
                  )}
                >
                  <option value="" className="bg-surface-el text-fg">
                    Use app default
                  </option>
                  {groupChatRoleplayTemplates.map((template) => (
                    <option key={template.id} value={template.id} className="bg-surface-el text-fg">
                      {template.name}
                    </option>
                  ))}
                </select>
                <div className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2">
                  <FileText className="h-4 w-4 text-fg/40" />
                </div>
              </div>
            )}
            <p className={cn(typography.bodySmall.size, "text-fg/40")}>
              Override this character&apos;s roleplay prompt in group chats
            </p>
          </div>

          {/* Voice Configuration */}
          <div className={spacing.field}>
            <label
              className={cn(
                typography.label.size,
                typography.label.weight,
                typography.label.tracking,
                "uppercase text-fg/70",
              )}
            >
              Voice (Optional)
            </label>
            {loadingVoices ? (
              <div
                className={cn(
                  "flex items-center gap-3 border border-fg/10 bg-surface-el/20 px-4 py-3 backdrop-blur-xl",
                  radius.md,
                )}
              >
                <Loader2 className="h-4 w-4 animate-spin text-fg/60" />
                <span className={cn(typography.body.size, "text-fg/60")}>Loading voices...</span>
              </div>
            ) : (
              <button
                type="button"
                onClick={() => setShowVoiceMenu(true)}
                className={cn(
                  "flex w-full items-center justify-between border bg-surface-el/20 px-4 py-3.5 text-left backdrop-blur-xl",
                  radius.md,
                  interactive.transition.default,
                  "focus:border-fg/30 focus:bg-surface-el/30 focus:outline-none hover:bg-surface-el/30",
                  voiceSelectionValue ? "border-fg/20" : "border-fg/10",
                )}
              >
                <div className="flex items-center gap-2">
                  <Volume2 className="h-5 w-5 text-fg/40" />
                  <span
                    className={cn("text-sm", voiceSelectionValue ? "text-fg" : "text-fg/50")}
                  >
                    {voiceSelectionValue
                      ? (() => {
                          if (voiceConfig?.source === "user") {
                            const v = userVoices.find((uv) => uv.id === voiceConfig.userVoiceId);
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
            {voiceError && (
              <p className={cn(typography.bodySmall.size, "font-medium text-danger")}>{voiceError}</p>
            )}
            {!loadingVoices && audioProviders.length === 0 && userVoices.length === 0 && (
              <p className={cn(typography.bodySmall.size, "text-fg/40")}>
                Add voices in Settings → Voices
              </p>
            )}
            <p className={cn(typography.bodySmall.size, "text-fg/40")}>
              Assign a voice for future text-to-speech playback
            </p>

            {/* Voice Autoplay Toggle */}
            <div
              className={cn(
                "flex items-center justify-between border border-fg/10 bg-surface-el/20 px-4 py-3 backdrop-blur-xl",
                radius.md,
                !voiceConfig && "opacity-50",
              )}
            >
              <div>
                <p className={cn(typography.body.size, "font-medium text-fg")}>Autoplay voice</p>
                <p className={cn(typography.bodySmall.size, "mt-1 text-fg/50")}>
                  {voiceConfig ? "Play this character's replies automatically" : "Select a voice first"}
                </p>
              </div>
              <Switch
                id="character-voice-autoplay"
                checked={voiceAutoplay}
                onChange={(next) => onVoiceAutoplayChange(next)}
                disabled={!voiceConfig}
              />
            </div>
          </div>
        </div>
      </div>

      {/* Error Display */}
      <AnimatePresence>
        {error && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            className={cn(
              "overflow-hidden border border-danger/20 bg-danger/10 px-4 py-3 backdrop-blur-xl",
              radius.md,
            )}
          >
            <div className="flex items-start gap-3">
              <AlertCircle className="mt-0.5 h-5 w-5 shrink-0 text-danger" />
              <p className={cn(typography.body.size, "text-danger")}>{error}</p>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Create Button */}
      <div className="pt-2">
        <motion.button
          disabled={!canSave}
          onClick={onSave}
          whileTap={{ scale: canSave ? 0.97 : 1 }}
          className={cn(
            "w-full py-4 text-base font-semibold",
            radius.md,
            interactive.transition.fast,
            canSave
              ? cn(
                  "border border-accent/40 bg-accent/20 text-accent",
                  shadows.glow,
                  "active:border-accent/60 active:bg-accent/30",
                )
              : "cursor-not-allowed border border-fg/5 bg-fg/5 text-fg/30",
          )}
        >
          {saving ? (
            <div className="flex items-center justify-center gap-2">
              <div className="h-4 w-4 animate-spin rounded-full border-2 border-accent/30 border-t-accent/80" />
              <span>Creating Character...</span>
            </div>
          ) : (
            resolvedSubmitLabel
          )}
        </motion.button>
      </div>

      <ModelSelectionBottomMenu
        isOpen={showModelMenu}
        onClose={() => setShowModelMenu(false)}
        title="Select Model"
        models={models}
        selectedModelIds={selectedModelId ? [selectedModelId] : []}
        searchPlaceholder="Search models..."
        onSelectModel={(modelId) => {
          onSelectModel(modelId);
          setShowModelMenu(false);
        }}
      />

      <ModelSelectionBottomMenu
        isOpen={showFallbackModelMenu}
        onClose={() => setShowFallbackModelMenu(false)}
        title="Select Fallback Model"
        models={models.filter((m) => m.id !== selectedModelId)}
        selectedModelIds={selectedFallbackModelId ? [selectedFallbackModelId] : []}
        searchPlaceholder="Search models..."
        onSelectModel={(modelId) => {
          onSelectFallbackModel(modelId);
          setShowFallbackModelMenu(false);
        }}
        clearOption={{
          label: "Off (no fallback)",
          icon: Cpu,
          selected: !selectedFallbackModelId,
          onClick: () => {
            onSelectFallbackModel(null);
            setShowFallbackModelMenu(false);
          },
        }}
      />

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
          <div className="max-h-[50vh] space-y-2 overflow-y-auto">
            <button
              onClick={() => {
                onVoiceConfigChange(null);
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
              {!voiceSelectionValue && <Check className="ml-auto h-4 w-4 text-accent" />}
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
                          onVoiceConfigChange({
                            source: "user",
                            userVoiceId: voice.id,
                            providerId: voice.providerId,
                            modelId: voice.modelId,
                            voiceName: voice.name,
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
                        <div className="min-w-0 flex-1">
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
                          onVoiceConfigChange({
                            source: "provider",
                            providerId: provider.id,
                            voiceId: voice.voiceId,
                            voiceName: voice.name,
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
    </motion.div>
  );
}
