import { useState, useEffect } from "react";
import {
  MessageSquare,
  Cpu,
  Zap,
  Hash,
  Info,
  Check,
  ChevronDown,
  MessageCircle,
  BookOpen,
} from "lucide-react";
import { readSettings, saveAdvancedSettings } from "../../../core/storage/repo";
import type { Model } from "../../../core/storage/schemas";
import { cn, colors } from "../../design-tokens";
import { getProviderIcon } from "../../../core/utils/providerIcons";
import { ModelSelectionBottomMenu } from "../../components/ModelSelectionBottomMenu";
import { useI18n } from "../../../core/i18n/context";

type ReplyStyle = "conversational" | "roleplay";

export function HelpMeReplyPage() {
  const { t } = useI18n();
  const [isLoading, setIsLoading] = useState(true);
  const [models, setModels] = useState<Model[]>([]);
  const [defaultModelId, setDefaultModelId] = useState<string | null>(null);

  // Settings state
  const [selectedModelId, setSelectedModelId] = useState<string | null>(null);
  const [streamingEnabled, setStreamingEnabled] = useState(true);
  const [maxTokens, setMaxTokens] = useState(150);
  const [maxTokensInput, setMaxTokensInput] = useState("150");
  const [replyStyle, setReplyStyle] = useState<ReplyStyle>("conversational");

  // Menu states
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
        setSelectedModelId(settings.advancedSettings?.helpMeReplyModelId ?? null);
        setStreamingEnabled(settings.advancedSettings?.helpMeReplyStreaming ?? true);
        const tokens = settings.advancedSettings?.helpMeReplyMaxTokens ?? 150;
        setMaxTokens(tokens);
        setMaxTokensInput(String(tokens));
        setReplyStyle(settings.advancedSettings?.helpMeReplyStyle ?? "conversational");
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
      helpMeReplyModelId: string | undefined;
      helpMeReplyStreaming: boolean;
      helpMeReplyMaxTokens: number;
      helpMeReplyStyle: ReplyStyle;
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
    await saveSettings({ helpMeReplyModelId: modelId ?? undefined });
  };

  const handleStreamingToggle = async () => {
    const newValue = !streamingEnabled;
    setStreamingEnabled(newValue);
    await saveSettings({ helpMeReplyStreaming: newValue });
  };

  const handleMaxTokensChange = async (value: number) => {
    setMaxTokens(value);
    setMaxTokensInput(String(value));
    await saveSettings({ helpMeReplyMaxTokens: value });
  };

  const handleMaxTokensBlur = async () => {
    const val = parseInt(maxTokensInput);
    if (!isNaN(val) && val >= 1) {
      setMaxTokens(val);
      await saveSettings({ helpMeReplyMaxTokens: val });
    } else {
      setMaxTokensInput(String(maxTokens));
    }
  };

  const handleStyleChange = async (style: ReplyStyle) => {
    setReplyStyle(style);
    await saveSettings({ helpMeReplyStyle: style });
  };

  const selectedModel = selectedModelId ? models.find((m) => m.id === selectedModelId) : null;
  const defaultModel = defaultModelId ? models.find((m) => m.id === defaultModelId) : null;
  const selectedModelLabel = selectedModel?.displayName || t("helpMeReply.labels.selectedModel");
  const appDefaultLabel = t("helpMeReply.labels.useAppDefault", {
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
          <div className={cn("rounded-xl border border-accent/20 bg-accent/5 p-3")}>
            <div className="flex items-start gap-2">
              <MessageSquare className="h-4 w-4 text-accent shrink-0 mt-0.5" />
              <p className="text-xs text-accent/80 leading-relaxed">{t("helpMeReply.page.info")}</p>
            </div>
          </div>

          {/* Desktop: Two Column Layout / Mobile: Single Column */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            {/* Left Column - Model Configuration */}
            <div className="space-y-4">
              <h3 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35 px-1">
                {t("helpMeReply.sectionTitles.modelConfiguration")}
              </h3>

              {/* Model Selector */}
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <div className="rounded-lg border border-accent/30 bg-accent/10 p-1.5">
                    <Cpu className="h-4 w-4 text-accent" />
                  </div>
                  <h3 className="text-sm font-semibold text-fg">
                    {t("helpMeReply.labels.replyModel")}
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
                      {t("helpMeReply.labels.noModelsAvailable")}
                    </p>
                  </div>
                )}
                <p className="text-xs text-fg/50 px-1">
                  {t("helpMeReply.labels.replyModelDescription")}
                </p>
              </div>

              {/* Streaming Toggle */}
              <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-3">
                    <div className="rounded-lg border border-info/30 bg-info/10 p-1.5">
                      <Zap className="h-4 w-4 text-info" />
                    </div>
                    <div>
                      <span className="text-sm font-medium text-fg">
                        {t("helpMeReply.labels.streamingOutput")}
                      </span>
                      <p className="text-[11px] text-fg/45">
                        {t("helpMeReply.labels.streamingDescription")}
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
                        streamingEnabled ? "bg-info" : "bg-fg/20",
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

              {/* Max Tokens */}
              <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
                <div className="flex items-center justify-between gap-2 mb-3">
                  <div className="flex items-center gap-3">
                    <div className="rounded-lg border border-warning/30 bg-warning/10 p-1.5">
                      <Hash className="h-4 w-4 text-warning" />
                    </div>
                    <div>
                      <span className="text-sm font-medium text-fg">
                        {t("helpMeReply.labels.maxTokens")}
                      </span>
                      <p className="text-[11px] text-fg/45">
                        {t("helpMeReply.labels.maxTokensDescription")}
                      </p>
                    </div>
                  </div>
                  <input
                    type="number"
                    value={maxTokensInput}
                    onChange={(e) => setMaxTokensInput(e.target.value)}
                    onBlur={handleMaxTokensBlur}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        e.currentTarget.blur();
                      }
                    }}
                    min={1}
                    className={cn(
                      "w-20 rounded-lg border border-fg/15 bg-surface-el/30 px-3 py-1.5",
                      "text-center font-mono text-sm text-fg",
                      "focus:border-fg/30 focus:outline-none",
                    )}
                  />
                </div>
                <div className="grid grid-cols-4 gap-2">
                  {[100, 150, 250, 400].map((val) => (
                    <button
                      key={val}
                      onClick={() => handleMaxTokensChange(val)}
                      className={cn(
                        "px-3 py-2 rounded-lg text-xs font-medium transition-all",
                        maxTokens === val
                          ? "bg-warning/20 border border-warning/40 text-warning/80"
                          : "border border-fg/10 bg-fg/5 text-fg/60 hover:border-fg/20",
                      )}
                    >
                      {val}
                    </button>
                  ))}
                </div>
              </div>
            </div>

            {/* Right Column - Response Style */}
            <div className="space-y-4">
              <h3 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35 px-1">
                {t("helpMeReply.sectionTitles.responseStyle")}
              </h3>

              <div className="grid grid-cols-2 gap-3">
                {/* Conversational Style */}
                <button
                  onClick={() => handleStyleChange("conversational")}
                  className={cn(
                    "flex flex-col items-center gap-3 rounded-xl border p-4 transition-all",
                    replyStyle === "conversational"
                      ? "border-accent/40 bg-accent/10"
                      : "border-fg/10 bg-fg/5 hover:border-fg/20",
                  )}
                >
                  <div
                    className={cn(
                      "flex h-12 w-12 items-center justify-center rounded-full border transition-all",
                      replyStyle === "conversational"
                        ? "border-accent/50 bg-accent/20"
                        : "border-fg/15 bg-fg/5",
                    )}
                  >
                    <MessageCircle
                      className={cn(
                        "h-6 w-6 transition-colors",
                        replyStyle === "conversational" ? "text-accent/80" : "text-fg/50",
                      )}
                    />
                  </div>
                  <div className="text-center">
                    <span
                      className={cn(
                        "text-sm font-semibold block",
                        replyStyle === "conversational" ? "text-accent/90" : "text-fg/70",
                      )}
                    >
                      {t("helpMeReply.responseStyle.conversational")}
                    </span>
                    <span className="text-[10px] text-fg/40 mt-1 block">
                      {t("helpMeReply.responseStyle.conversationalDesc")}
                    </span>
                  </div>
                  {replyStyle === "conversational" && (
                    <div className="flex h-5 w-5 items-center justify-center rounded-full bg-accent">
                      <Check className="h-3 w-3 text-fg" />
                    </div>
                  )}
                </button>

                {/* Roleplay Style */}
                <button
                  onClick={() => handleStyleChange("roleplay")}
                  className={cn(
                    "flex flex-col items-center gap-3 rounded-xl border p-4 transition-all",
                    replyStyle === "roleplay"
                      ? "border-danger/40 bg-danger/10"
                      : "border-fg/10 bg-fg/5 hover:border-fg/20",
                  )}
                >
                  <div
                    className={cn(
                      "flex h-12 w-12 items-center justify-center rounded-full border transition-all",
                      replyStyle === "roleplay"
                        ? "border-danger/50 bg-danger/20"
                        : "border-fg/15 bg-fg/5",
                    )}
                  >
                    <BookOpen
                      className={cn(
                        "h-6 w-6 transition-colors",
                        replyStyle === "roleplay" ? "text-danger/80" : "text-fg/50",
                      )}
                    />
                  </div>
                  <div className="text-center">
                    <span
                      className={cn(
                        "text-sm font-semibold block",
                        replyStyle === "roleplay" ? "text-danger" : "text-fg/70",
                      )}
                    >
                      {t("helpMeReply.responseStyle.roleplay")}
                    </span>
                    <span className="text-[10px] text-fg/40 mt-1 block">
                      {t("helpMeReply.responseStyle.roleplayDesc")}
                    </span>
                  </div>
                  {replyStyle === "roleplay" && (
                    <div className="flex h-5 w-5 items-center justify-center rounded-full bg-danger">
                      <Check className="h-3 w-3 text-fg" />
                    </div>
                  )}
                </button>
              </div>

              <p className="text-xs text-fg/50 px-1">
                {replyStyle === "conversational"
                  ? t("helpMeReply.labels.conversationalHint")
                  : t("helpMeReply.labels.roleplayHint")}
              </p>
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
              <p>{t("helpMeReply.labels.footerInfo")}</p>
            </div>
          </div>
        </div>
      </main>

      <ModelSelectionBottomMenu
        isOpen={showModelMenu}
        onClose={() => setShowModelMenu(false)}
        title={t("helpMeReply.labels.selectReplyModel")}
        models={models}
        selectedModelIds={selectedModelId ? [selectedModelId] : []}
        searchPlaceholder={t("helpMeReply.labels.searchModels")}
        onSelectModel={(modelId) => {
          handleModelChange(modelId);
          setShowModelMenu(false);
        }}
        clearOption={{
          label: t("helpMeReply.labels.useAppDefaultBase"),
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
