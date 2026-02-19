import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import {
  Sparkles,
  ChevronRight,
  HelpCircle,
  PenLine,
  Brain,
  Cpu,
  Info,
  MessageSquare,
  Zap,
  DollarSign,
  AlertTriangle,
  Loader2,
} from "lucide-react";
import {
  readSettings,
  saveAdvancedSettings,
  checkEmbeddingModel,
} from "../../../core/storage/repo";
import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "../../../core/storage/schemas";
import { cn, typography, spacing, interactive, colors } from "../../design-tokens";
import { EmbeddingDownloadPrompt } from "../../components/EmbeddingDownloadPrompt";
import { BottomMenu } from "../../components/BottomMenu";
import { openDocs, DOCS } from "../../../core/utils/docs";

type DocsKey = keyof typeof DOCS;

interface FeatureCardProps {
  title: string;
  description: string;
  detailText?: string;
  icon: React.ReactNode;
  enabled: boolean;
  onToggle: () => void;
  onNavigate: () => void;
  colorScheme: "rose" | "blue" | "emerald" | "amber" | "violet";
  badge?: string;
  helpKey?: DocsKey;
}

function FeatureCard({
  title,
  description,
  detailText,
  icon,
  enabled,
  onToggle,
  onNavigate,
  colorScheme,
  badge,
  helpKey,
}: FeatureCardProps) {
  const colorStyles = {
    rose: {
      border: enabled ? "border-danger/25" : "border-fg/10",
      bg: enabled ? "bg-danger/8" : "bg-fg/5",
      hoverBorder: enabled ? "hover:border-danger/40" : "hover:border-fg/20",
      iconBorder: enabled ? "border-danger/40" : "border-fg/15",
      iconBg: enabled ? "bg-danger/15" : "bg-fg/8",
      iconShadow: enabled ? "shadow-danger/20" : "",
      iconColor: enabled ? "text-danger/80" : "text-fg/60",
      toggleBg: enabled ? "bg-danger" : "bg-fg/20",
      toggleShadow: enabled ? "shadow-danger/30" : "",
      badgeBorder: enabled ? "border-danger/50" : "border-fg/20",
      badgeBg: enabled ? "bg-danger/20" : "bg-fg/10",
      badgeText: enabled ? "text-danger" : "text-fg/60",
      gradient: enabled
        ? "radial-gradient(circle at 15% 15%, rgba(244,63,94,0.08) 0%, transparent 50%)"
        : "none",
    },
    blue: {
      border: enabled ? "border-info/25" : "border-fg/10",
      bg: enabled ? "bg-info/8" : "bg-fg/5",
      hoverBorder: enabled ? "hover:border-info/40" : "hover:border-fg/20",
      iconBorder: enabled ? "border-info/40" : "border-fg/15",
      iconBg: enabled ? "bg-info/15" : "bg-fg/8",
      iconShadow: enabled ? "shadow-info/20" : "",
      iconColor: enabled ? "text-info/80" : "text-fg/60",
      toggleBg: enabled ? "bg-info" : "bg-fg/20",
      toggleShadow: enabled ? "shadow-info/30" : "",
      badgeBorder: enabled ? "border-info/50" : "border-warning/40",
      badgeBg: enabled ? "bg-info/20" : "bg-warning/15",
      badgeText: enabled ? "text-info" : "text-warning",
      gradient: enabled
        ? "radial-gradient(circle at 15% 15%, rgba(59,130,246,0.06) 0%, transparent 50%)"
        : "none",
    },
    emerald: {
      border: enabled ? "border-accent/25" : "border-fg/10",
      bg: enabled ? "bg-accent/8" : "bg-fg/5",
      hoverBorder: enabled ? "hover:border-accent/40" : "hover:border-fg/20",
      iconBorder: enabled ? "border-accent/40" : "border-fg/15",
      iconBg: enabled ? "bg-accent/15" : "bg-fg/8",
      iconShadow: enabled ? "shadow-accent/20" : "",
      iconColor: enabled ? "text-accent/80" : "text-fg/60",
      toggleBg: enabled ? "bg-accent" : "bg-fg/20",
      toggleShadow: enabled ? "shadow-accent/30" : "",
      badgeBorder: enabled ? "border-accent/50" : "border-warning/40",
      badgeBg: enabled ? "bg-accent/20" : "bg-warning/15",
      badgeText: enabled ? "text-accent/80" : "text-warning",
      gradient: enabled
        ? "radial-gradient(circle at 15% 15%, rgba(16,185,129,0.08) 0%, transparent 50%)"
        : "none",
    },
    amber: {
      border: enabled ? "border-warning/25" : "border-fg/10",
      bg: enabled ? "bg-warning/8" : "bg-fg/5",
      hoverBorder: enabled ? "hover:border-warning/40" : "hover:border-fg/20",
      iconBorder: enabled ? "border-warning/40" : "border-fg/15",
      iconBg: enabled ? "bg-warning/15" : "bg-fg/8",
      iconShadow: enabled ? "shadow-warning/20" : "",
      iconColor: enabled ? "text-warning/80" : "text-fg/60",
      toggleBg: enabled ? "bg-warning" : "bg-fg/20",
      toggleShadow: enabled ? "shadow-warning/30" : "",
      badgeBorder: enabled ? "border-warning/50" : "border-fg/20",
      badgeBg: enabled ? "bg-warning/20" : "bg-fg/10",
      badgeText: enabled ? "text-warning/80" : "text-fg/60",
      gradient: enabled
        ? "radial-gradient(circle at 15% 15%, rgba(251,191,36,0.08) 0%, transparent 50%)"
        : "none",
    },
    violet: {
      border: enabled ? "border-secondary/25" : "border-fg/10",
      bg: enabled ? "bg-secondary/8" : "bg-fg/5",
      hoverBorder: enabled ? "hover:border-secondary/40" : "hover:border-fg/20",
      iconBorder: enabled ? "border-secondary/40" : "border-fg/15",
      iconBg: enabled ? "bg-secondary/15" : "bg-fg/8",
      iconShadow: enabled ? "shadow-secondary/20" : "",
      iconColor: enabled ? "text-secondary/80" : "text-fg/60",
      toggleBg: enabled ? "bg-secondary" : "bg-fg/20",
      toggleShadow: enabled ? "shadow-secondary/30" : "",
      badgeBorder: enabled ? "border-secondary/50" : "border-fg/20",
      badgeBg: enabled ? "bg-secondary/20" : "bg-fg/10",
      badgeText: enabled ? "text-secondary" : "text-fg/60",
      gradient: enabled
        ? "radial-gradient(circle at 15% 15%, rgba(139,92,246,0.08) 0%, transparent 50%)"
        : "none",
    },
  };

  const style = colorStyles[colorScheme];
  const toggleId = `toggle-${title.toLowerCase().replace(/\s+/g, "-")}`;

  return (
    <button
      onClick={onNavigate}
      className={cn(
        "group w-full text-left",
        "relative overflow-hidden rounded-xl border px-4 py-3.5",
        "transition-all duration-300",
        style.border,
        style.bg,
        style.hoverBorder,
        interactive.active.scale,
        interactive.focus.ring,
      )}
    >
      {enabled && (
        <div
          className="pointer-events-none absolute inset-0 opacity-70"
          style={{ background: style.gradient }}
        />
      )}

      <div className="relative flex items-start gap-3">
        <div
          className={cn(
            "flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border",
            "transition-all duration-300",
            style.iconBorder,
            style.iconBg,
            enabled && "shadow-lg",
            style.iconShadow,
          )}
        >
          <span className={cn("transition-colors duration-300", style.iconColor)}>{icon}</span>
        </div>

        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between gap-2">
            <div className="flex-1">
              <div className="flex items-center gap-2">
                <span className={cn(typography.body.size, "font-medium text-fg")}>{title}</span>
                <span
                  className={cn(
                    "rounded-md border px-1.5 py-0.5",
                    "text-[9px] font-semibold leading-none uppercase tracking-[0.2em]",
                    "transition-all duration-300",
                    style.badgeBorder,
                    style.badgeBg,
                    style.badgeText,
                  )}
                >
                  {enabled ? "On" : "Off"}
                </span>
                {badge && (
                  <span
                    className={cn(
                      "rounded-md border px-1.5 py-0.5",
                      "text-[9px] font-medium leading-none uppercase tracking-wider",
                      "border-fg/10 bg-fg/5 text-fg/40",
                    )}
                  >
                    {badge}
                  </span>
                )}
                {helpKey && (
                  <span
                    role="button"
                    tabIndex={0}
                    onClick={(e) => {
                      e.stopPropagation();
                      openDocs(helpKey);
                    }}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        e.stopPropagation();
                        openDocs(helpKey);
                      }
                    }}
                    className="p-0.5 text-fg/30 transition-colors hover:text-fg/60"
                    aria-label={`Help with ${title}`}
                  >
                    <HelpCircle size={14} />
                  </span>
                )}
              </div>
              <p className="mt-0.5 text-[11px] leading-relaxed text-fg/50">{description}</p>
            </div>

            <div className="flex shrink-0 items-center gap-2">
              <input
                id={toggleId}
                type="checkbox"
                checked={enabled}
                onChange={(e) => {
                  e.stopPropagation();
                  onToggle();
                }}
                onClick={(e) => e.stopPropagation()}
                className="peer sr-only"
              />
              <label
                htmlFor={toggleId}
                onClick={(e) => e.stopPropagation()}
                className={cn(
                  "relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full",
                  "border-2 border-transparent transition-all duration-200 ease-in-out",
                  "focus:outline-none focus:ring-2 focus:ring-fg/20",
                  style.toggleBg,
                  enabled && "shadow-md",
                  style.toggleShadow,
                )}
              >
                <span
                  className={cn(
                    "inline-block h-4 w-4 transform rounded-full bg-fg shadow-sm",
                    "ring-0 transition duration-200 ease-in-out",
                    enabled ? "translate-x-4" : "translate-x-0",
                  )}
                />
              </label>
              <ChevronRight
                className={cn(
                  "h-4 w-4 shrink-0 text-fg/25 transition-colors",
                  "group-hover:text-fg/50",
                )}
              />
            </div>
          </div>

          {detailText && (
            <p className="mt-1.5 text-[10px] leading-relaxed text-fg/40">{detailText}</p>
          )}
        </div>
      </div>
    </button>
  );
}

interface SettingsSectionProps {
  title: string;
  children: React.ReactNode;
  icon?: React.ReactNode;
}

function SettingsSection({ title, children, icon }: SettingsSectionProps) {
  return (
    <div>
      <div className="mb-2.5 flex items-center gap-2 px-1">
        {icon && <span className="text-fg/30">{icon}</span>}
        <h2
          className={cn(
            typography.overline.size,
            typography.overline.weight,
            typography.overline.tracking,
            typography.overline.transform,
            "text-fg/40",
          )}
        >
          {title}
        </h2>
      </div>
      <div className="space-y-2.5">{children}</div>
    </div>
  );
}

export function AdvancedPage() {
  const navigate = useNavigate();
  const [isLoading, setIsLoading] = useState(true);
  const [showDownloadPrompt, setShowDownloadPrompt] = useState(false);

  // Settings state
  const [creationHelperEnabled, setCreationHelperEnabled] = useState(false);
  const [dynamicMemoryEnabled, setDynamicMemoryEnabled] = useState(false);
  const [helpMeReplyEnabled, setHelpMeReplyEnabled] = useState(true);
  const [manualWindow, setManualWindow] = useState<number | null>(50);

  // Usage recalculation state
  const [showRecalculateWarning, setShowRecalculateWarning] = useState(false);
  const [recalculating, setRecalculating] = useState(false);
  const [recalculateResult, setRecalculateResult] = useState<string | null>(null);
  const [openRouterApiKey, setOpenRouterApiKey] = useState<string>("");

  const getAdvancedSettings = (settings: Settings) => {
    const advanced = settings.advancedSettings ?? {
      creationHelperEnabled: false,
      helpMeReplyEnabled: true,
    };
    if (advanced.creationHelperEnabled === undefined) {
      advanced.creationHelperEnabled = false;
    }
    if (advanced.helpMeReplyEnabled === undefined) {
      advanced.helpMeReplyEnabled = true;
    }
    settings.advancedSettings = advanced;
    return advanced;
  };

  useEffect(() => {
    const loadData = async () => {
      try {
        const settings = await readSettings();

        setCreationHelperEnabled(settings.advancedSettings?.creationHelperEnabled ?? false);
        setDynamicMemoryEnabled(settings.advancedSettings?.dynamicMemory?.enabled ?? false);
        setHelpMeReplyEnabled(settings.advancedSettings?.helpMeReplyEnabled ?? true);
        setManualWindow(settings.advancedSettings?.manualModeContextWindow ?? 50);

        // Get OpenRouter API key for recalculation
        const openRouterCred = settings.providerCredentials?.find(
          (c) => c.providerId === "openrouter",
        );
        setOpenRouterApiKey(openRouterCred?.apiKey ?? "");

        setIsLoading(false);
      } catch (err) {
        console.error("Failed to load settings:", err);
        setIsLoading(false);
      }
    };

    loadData();
  }, []);

  const handleToggleCreationHelper = async () => {
    const newValue = !creationHelperEnabled;
    setCreationHelperEnabled(newValue);

    try {
      const settings = await readSettings();
      const advanced = getAdvancedSettings(settings);
      advanced.creationHelperEnabled = newValue;

      if (newValue && !advanced.creationHelperModelId && settings.defaultModelId) {
        advanced.creationHelperModelId = settings.defaultModelId;
      }

      await saveAdvancedSettings(advanced);
    } catch (err) {
      console.error("Failed to save creation helper setting:", err);
      setCreationHelperEnabled(!newValue);
    }
  };

  const handleToggleDynamicMemory = async () => {
    const newValue = !dynamicMemoryEnabled;

    if (newValue) {
      try {
        const modelExists = await checkEmbeddingModel();
        if (!modelExists) {
          setShowDownloadPrompt(true);
          return;
        }
      } catch (err) {
        console.error("Failed to check embedding model:", err);
        return;
      }
    }

    setDynamicMemoryEnabled(newValue);

    try {
      const settings = await readSettings();
      const advanced = getAdvancedSettings(settings);
      if (!advanced.dynamicMemory) {
        advanced.dynamicMemory = {
          enabled: false,
          summaryMessageInterval: 20,
          maxEntries: 50,
          minSimilarityThreshold: 0.35,
          retrievalLimit: 5,
          retrievalStrategy: "smart",
          hotMemoryTokenBudget: 2000,
          decayRate: 0.08,
          coldThreshold: 0.3,
          contextEnrichmentEnabled: true,
        };
      }

      if (newValue && !advanced.summarisationModelId && settings.defaultModelId) {
        advanced.summarisationModelId = settings.defaultModelId;
      }

      advanced.dynamicMemory.enabled = newValue;
      await saveAdvancedSettings(advanced);
    } catch (err) {
      console.error("Failed to save dynamic memory setting:", err);
      setDynamicMemoryEnabled(!newValue);
    }
  };

  const handleManualWindowChange = async (value: number | null) => {
    setManualWindow(value);

    try {
      const settings = await readSettings();
      const advanced = getAdvancedSettings(settings);
      advanced.manualModeContextWindow = value === null ? 50 : value;
      await saveAdvancedSettings(advanced);
    } catch (err) {
      console.error("Failed to save manual window setting:", err);
    }
  };

  const handleRecalculateCosts = async () => {
    if (!openRouterApiKey) {
      setRecalculateResult(
        "Error: No OpenRouter API key found. Please configure it in Settings > Providers.",
      );
      return;
    }

    setRecalculating(true);
    setRecalculateResult(null);

    try {
      const result = await invoke<string>("usage_recalculate_costs", {
        apiKey: openRouterApiKey,
      });
      setRecalculateResult(result);
    } catch (err) {
      setRecalculateResult(`Error: ${err}`);
    } finally {
      setRecalculating(false);
    }
  };

  const handleToggleHelpMeReply = async () => {
    const newValue = !helpMeReplyEnabled;
    setHelpMeReplyEnabled(newValue);

    try {
      const settings = await readSettings();
      const advanced = getAdvancedSettings(settings);
      advanced.helpMeReplyEnabled = newValue;
      await saveAdvancedSettings(advanced);
    } catch (err) {
      console.error("Failed to save help me reply setting:", err);
      setHelpMeReplyEnabled(!newValue);
    }
  };

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-6 w-6 animate-spin rounded-full border-2 border-fg/20 border-t-fg/60" />
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col pb-16">
      <section className={cn("flex-1 overflow-y-auto px-3 pt-3", spacing.section)}>
        {/* AI Features Section */}
        <SettingsSection title="AI Features" icon={<Zap size={12} />}>
          <FeatureCard
            title="Creation Helper"
            description="AI-guided character creation wizard"
            detailText="Get intelligent suggestions for personality traits, backstory, and dialogue style"
            icon={<Sparkles className="h-4 w-4" />}
            enabled={creationHelperEnabled}
            onToggle={handleToggleCreationHelper}
            onNavigate={() => navigate("/settings/advanced/creation-helper")}
            colorScheme="rose"
            helpKey="creationHelper"
          />

          <FeatureCard
            title="Help Me Reply"
            description="AI-assisted reply suggestions"
            detailText="Generate contextual response options based on conversation history"
            icon={<PenLine className="h-4 w-4" />}
            enabled={helpMeReplyEnabled}
            onToggle={handleToggleHelpMeReply}
            onNavigate={() => navigate("/settings/advanced/help-me-reply")}
            colorScheme="emerald"
          />
        </SettingsSection>

        {/* Memory System Section */}
        <SettingsSection title="Memory System" icon={<Brain size={12} />}>
          <FeatureCard
            title="Dynamic Memory"
            description={
              dynamicMemoryEnabled
                ? "AI automatically manages conversation context"
                : "Switch to automatic memory management"
            }
            detailText="Semantic search enables intelligent memory recall across conversations"
            icon={<Cpu className="h-4 w-4" />}
            enabled={dynamicMemoryEnabled}
            onToggle={handleToggleDynamicMemory}
            onNavigate={() => navigate("/settings/advanced/memory")}
            colorScheme="blue"
          />

          {/* Context Window Settings Card */}
          <div className={cn("rounded-xl border px-4 py-4", "border-fg/10 bg-fg/5")}>
            <div className="flex items-start gap-3">
              <div
                className={cn(
                  "flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border",
                  "border-fg/10 bg-fg/5",
                )}
              >
                <MessageSquare className="h-4 w-4 text-fg/50" />
              </div>

              <div className="min-w-0 flex-1">
                <div className="flex items-center justify-between">
                  <div>
                    <span className={cn(typography.body.size, "font-medium text-fg")}>
                      Context Window
                    </span>
                    <p className="mt-0.5 text-[11px] text-fg/45">
                      Number of recent messages to include (1-1000)
                    </p>
                  </div>
                  <input
                    type="number"
                    min={1}
                    max={1000}
                    value={manualWindow ?? 50}
                    onChange={(e) => {
                      const val = parseInt(e.target.value);
                      handleManualWindowChange(isNaN(val) ? null : val);
                    }}
                    className={cn(
                      "w-20 rounded-lg border border-fg/15 bg-surface-el/30 px-3 py-1.5",
                      "text-center font-mono text-sm text-fg",
                      "focus:border-fg/30 focus:outline-none",
                      interactive.transition.fast,
                    )}
                  />
                </div>
              </div>
            </div>
          </div>
        </SettingsSection>

        {/* Usage Analytics Section */}
        <SettingsSection title="Usage Analytics" icon={<DollarSign size={12} />}>
          <div className={cn("rounded-xl border px-4 py-4", "border-fg/10 bg-fg/5")}>
            <div className="flex items-start gap-3">
              <div
                className={cn(
                  "flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border",
                  "border-warning/20 bg-warning/10",
                )}
              >
                <DollarSign className="h-4 w-4 text-warning" />
              </div>

              <div className="min-w-0 flex-1">
                <div className="flex items-center justify-between mb-3">
                  <div>
                    <span className={cn(typography.body.size, "font-medium text-fg")}>
                      Recalculate Usage Costs
                    </span>
                    <p className="mt-0.5 text-[11px] text-fg/45">
                      Update all historical usage records with correct pricing
                    </p>
                  </div>
                </div>

                <button
                  onClick={() => setShowRecalculateWarning(true)}
                  disabled={recalculating || !openRouterApiKey}
                  className={cn(
                    "w-full rounded-lg border px-4 py-2.5 text-sm font-medium",
                    "transition-all",
                    recalculating || !openRouterApiKey
                      ? "border-fg/10 bg-fg/5 text-fg/30 cursor-not-allowed"
                      : "border-warning/30 bg-warning/10 text-warning/90 hover:bg-warning/20 hover:border-warning/40",
                  )}
                >
                  {recalculating ? (
                    <span className="flex items-center justify-center gap-2">
                      <Loader2 className="h-4 w-4 animate-spin" />
                      Recalculating...
                    </span>
                  ) : (
                    "Recalculate All Costs"
                  )}
                </button>

                {!openRouterApiKey && (
                  <p className="mt-2 text-[11px] text-danger/70">
                    OpenRouter API key required. Configure it in Settings → Providers.
                  </p>
                )}

                {recalculateResult && (
                  <div
                    className={cn(
                      "mt-3 rounded-lg border px-3 py-2 text-[11px]",
                      recalculateResult.startsWith("Error")
                        ? "border-danger/30 bg-danger/10 text-danger/80"
                        : "border-accent/30 bg-accent/10 text-accent/80",
                    )}
                  >
                    {recalculateResult}
                  </div>
                )}
              </div>
            </div>
          </div>
        </SettingsSection>

        {/* Info Card */}
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
              <strong className="text-fg/60">Dynamic Memory</strong> uses AI to automatically
              summarize and manage conversation context, enabling longer, more coherent
              conversations.
            </p>
            <p className="mt-2">
              When disabled, the app uses a simple sliding window of recent messages determined by
              the Context Window setting.
            </p>
          </div>
        </div>
      </section>

      <EmbeddingDownloadPrompt
        isOpen={showDownloadPrompt}
        onClose={() => setShowDownloadPrompt(false)}
      />

      {/* Recalculate Warning Bottom Menu */}
      <BottomMenu
        isOpen={showRecalculateWarning}
        onClose={() => setShowRecalculateWarning(false)}
        title="Recalculate Usage Costs?"
        includeExitIcon={false}
      >
        <div className="space-y-4 pb-4">
          <div className="flex items-start gap-3">
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-warning/30 bg-warning/20">
              <AlertTriangle className="h-5 w-5 text-warning" />
            </div>
            <div className="flex-1">
              <p className="text-sm text-fg/80 leading-relaxed">
                This will update all historical OpenRouter usage records with current pricing from
                the OpenRouter API.
              </p>
            </div>
          </div>

          <div className={cn("rounded-xl border border-fg/10 bg-fg/5 p-3")}>
            <p className="text-xs font-medium text-fg/70 mb-2">Important:</p>
            <ul className="space-y-1.5 text-xs text-fg/60">
              <li className="flex items-start gap-2">
                <span className="text-warning mt-0.5">•</span>
                <span>This operation cannot be undone</span>
              </li>
              <li className="flex items-start gap-2">
                <span className="text-warning mt-0.5">•</span>
                <span>It may take time if you have many records</span>
              </li>
              <li className="flex items-start gap-2">
                <span className="text-warning mt-0.5">•</span>
                <span>Only OpenRouter records with tokens will be updated</span>
              </li>
              <li className="flex items-start gap-2">
                <span className="text-warning mt-0.5">•</span>
                <span>Existing cost values will be overwritten</span>
              </li>
            </ul>
          </div>

          <div className="flex gap-2 pt-2">
            <button
              onClick={() => {
                setShowRecalculateWarning(false);
                setRecalculateResult(null);
              }}
              className={cn(
                "flex-1 rounded-xl border border-fg/10 bg-fg/5 px-4 py-2.5",
                "text-sm font-medium text-fg/60",
                "hover:bg-fg/10 hover:text-fg transition-all",
              )}
            >
              Cancel
            </button>
            <button
              onClick={() => {
                setShowRecalculateWarning(false);
                handleRecalculateCosts();
              }}
              className={cn(
                "flex-1 rounded-xl border border-warning/30 bg-warning/20 px-4 py-2.5",
                "text-sm font-medium text-warning/90",
                "hover:bg-warning/30 hover:border-warning/40 transition-all",
              )}
            >
              Proceed
            </button>
          </div>
        </div>
      </BottomMenu>
    </div>
  );
}
