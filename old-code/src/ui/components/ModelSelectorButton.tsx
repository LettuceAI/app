import { useState, useMemo } from "react";
import { Cpu, ChevronDown, Check, Search, Loader2 } from "lucide-react";
import { BottomMenu } from "./BottomMenu";
import { cn, typography, interactive } from "../design-tokens";
import type { Model } from "../../core/storage/schemas";
import { useI18n } from "../../core/i18n/context";
import GroqIcon from "../../assets/groq.svg";
import CerebrasIcon from "../../assets/cerebras.svg";

// Provider icons - matching the character settings
function getProviderIcon(providerId: string) {
  const iconClass = "h-5 w-5";

  switch (providerId?.toLowerCase()) {
    case "openai":
      return (
        <svg className={iconClass} viewBox="0 0 24 24" fill="currentColor">
          <path d="M22.282 9.821a5.985 5.985 0 0 0-.516-4.91 6.046 6.046 0 0 0-6.51-2.9A6.065 6.065 0 0 0 4.981 4.18a5.985 5.985 0 0 0-3.998 2.9 6.046 6.046 0 0 0 .743 7.097 5.98 5.98 0 0 0 .51 4.911 6.051 6.051 0 0 0 6.515 2.9A5.985 5.985 0 0 0 13.26 24a6.056 6.056 0 0 0 5.772-4.206 5.99 5.99 0 0 0 3.997-2.9 6.056 6.056 0 0 0-.747-7.073zM13.26 22.43a4.476 4.476 0 0 1-2.876-1.04l.141-.081 4.779-2.758a.795.795 0 0 0 .392-.681v-6.737l2.02 1.168a.071.071 0 0 1 .038.052v5.583a4.504 4.504 0 0 1-4.494 4.494zM3.6 18.304a4.47 4.47 0 0 1-.535-3.014l.142.085 4.783 2.759a.771.771 0 0 0 .78 0l5.843-3.369v2.332a.08.08 0 0 1-.033.062L9.74 19.95a4.5 4.5 0 0 1-6.14-1.646zM2.34 7.896a4.485 4.485 0 0 1 2.366-1.973V11.6a.766.766 0 0 0 .388.676l5.815 3.355-2.02 1.168a.076.076 0 0 1-.071 0l-4.83-2.786A4.504 4.504 0 0 1 2.34 7.872zm16.597 3.855l-5.833-3.387L15.119 7.2a.076.076 0 0 1 .071 0l4.83 2.791a4.494 4.494 0 0 1-.676 8.105v-5.678a.79.79 0 0 0-.407-.667zm2.01-3.023l-.141-.085-4.774-2.782a.776.776 0 0 0-.785 0L9.409 9.23V6.897a.066.066 0 0 1 .028-.061l4.83-2.787a4.5 4.5 0 0 1 6.68 4.66zm-12.64 4.135l-2.02-1.164a.08.08 0 0 1-.038-.057V6.075a4.5 4.5 0 0 1 7.375-3.453l-.142.08-4.778 2.758a.795.795 0 0 0-.393.681zm1.097-2.365l2.602-1.5 2.607 1.5v2.999l-2.597 1.5-2.607-1.5z" />
        </svg>
      );
    case "cerebras":
      return <img src={CerebrasIcon} alt="Cerebras" className={iconClass} />;
    case "anthropic":
      return (
        <svg className={iconClass} viewBox="0 0 24 24" fill="currentColor">
          <path d="M13.827 3.52h3.603L24 20.48h-3.603l-6.57-16.96zm-7.258 0h3.767L16.906 20.48h-3.674l-1.343-3.461H5.017l-1.344 3.46H0L6.57 3.522zm3.357 10.51L7.636 7.722l-2.29 6.308h4.58z" />
        </svg>
      );
    case "google":
      return (
        <svg className={iconClass} viewBox="0 0 24 24" fill="currentColor">
          <path d="M12.48 10.92v3.28h7.84c-.24 1.84-.853 3.187-1.787 4.133-1.147 1.147-2.933 2.4-6.053 2.4-4.827 0-8.6-3.893-8.6-8.72s3.773-8.72 8.6-8.72c2.6 0 4.507 1.027 5.907 2.347l2.307-2.307C18.747 1.44 16.133 0 12.48 0 5.867 0 .307 5.387.307 12s5.56 12 12.173 12c3.573 0 6.267-1.173 8.373-3.36 2.16-2.16 2.84-5.213 2.84-7.667 0-.76-.053-1.467-.173-2.053H12.48z" />
        </svg>
      );
    case "mistral":
      return (
        <svg className={iconClass} viewBox="0 0 24 24" fill="currentColor">
          <path d="M3.428 0v4.963h4.963V0zM15.61 0v4.963h4.964V0zM3.428 7.036V12h4.963V7.036zm6.09 0V12h4.964V7.036zm6.092 0V12h4.964V7.036zm-6.092 7.035v4.963h4.964v-4.963zm6.092 0v4.963h4.964v-4.963zM3.428 19.036V24h4.963v-4.964zm6.09 0V24h4.964v-4.964zm6.092 0V24h4.964v-4.964z" />
        </svg>
      );
    case "groq":
      return <img src={GroqIcon} alt="Groq" className={iconClass} />;
    default:
      return <Cpu className={iconClass + " text-white/50"} />;
  }
}

interface ModelSelectorButtonProps {
  models: Model[];
  selectedModelId: string | null;
  onSelect: (modelId: string | null) => void;
  loading?: boolean;
  disabled?: boolean;
  placeholder?: string;
  allowClear?: boolean;
  clearLabel?: string;
  label?: string;
  description?: string;
  className?: string;
}

export function ModelSelectorButton({
  models,
  selectedModelId,
  onSelect,
  loading = false,
  disabled = false,
  placeholder: placeholderProp,
  allowClear = false,
  clearLabel: clearLabelProp,
  label,
  description,
  className,
}: ModelSelectorButtonProps) {
  const { t } = useI18n();
  const placeholder = placeholderProp ?? t("components.modelSelector.placeholder");
  const clearLabel = clearLabelProp ?? t("components.modelSelector.clearLabel");
  const [isOpen, setIsOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");

  const selectedModel = useMemo(
    () => models.find((m) => m.id === selectedModelId),
    [models, selectedModelId],
  );

  const filteredModels = useMemo(() => {
    if (!searchQuery.trim()) return models;
    const q = searchQuery.toLowerCase();
    return models.filter(
      (m) =>
        m.displayName?.toLowerCase().includes(q) ||
        m.name?.toLowerCase().includes(q) ||
        m.providerId?.toLowerCase().includes(q),
    );
  }, [models, searchQuery]);

  // Group models by provider
  const groupedModels = useMemo(() => {
    const groups: Record<string, Model[]> = {};
    for (const model of filteredModels) {
      const provider = model.providerId || "Other";
      if (!groups[provider]) {
        groups[provider] = [];
      }
      groups[provider].push(model);
    }
    return groups;
  }, [filteredModels]);

  const handleSelect = (modelId: string | null) => {
    onSelect(modelId);
    setIsOpen(false);
    setSearchQuery("");
  };

  if (loading) {
    return (
      <div className={cn("space-y-2", className)}>
        {label && (
          <label className={cn(typography.label.size, typography.label.weight, "text-white/60")}>
            {label}
          </label>
        )}
        <div className="flex items-center gap-3 rounded-xl border border-white/10 bg-black/20 px-4 py-3">
          <Loader2 className="h-4 w-4 animate-spin text-white/50" />
          <span className={cn(typography.body.size, "text-white/50")}>{t("components.modelSelector.loading")}</span>
        </div>
      </div>
    );
  }

  if (models.length === 0) {
    return (
      <div className={cn("space-y-2", className)}>
        {label && (
          <label className={cn(typography.label.size, typography.label.weight, "text-white/60")}>
            {label}
          </label>
        )}
        <div className="rounded-xl border border-amber-400/20 bg-amber-400/10 px-4 py-3">
          <p className={cn(typography.body.size, "text-amber-200/90")}>{t("components.modelSelector.noModels")}</p>
          <p className={cn(typography.bodySmall.size, "mt-1 text-amber-200/60")}>
            {t("components.modelSelector.addProviderFirst")}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className={cn("space-y-2", className)}>
      {label && (
        <label className={cn(typography.label.size, typography.label.weight, "text-white/60")}>
          {label}
        </label>
      )}

      <button
        type="button"
        onClick={() => !disabled && setIsOpen(true)}
        disabled={disabled}
        className={cn(
          "flex w-full items-center justify-between rounded-xl border bg-black/20 px-4 py-3 text-left",
          interactive.transition.default,
          "focus:border-white/25 focus:outline-none",
          disabled ? "cursor-not-allowed opacity-50" : "hover:bg-black/30",
          selectedModelId ? "border-white/20" : "border-white/10",
        )}
      >
        <div className="flex items-center gap-3 min-w-0 flex-1">
          {selectedModel ? (
            <>
              <div className="shrink-0 text-white/70">
                {getProviderIcon(selectedModel.providerId)}
              </div>
              <div className="min-w-0 flex-1">
                <span className="block truncate text-sm text-white">
                  {selectedModel.displayName || selectedModel.name}
                </span>
                {selectedModel.displayName && selectedModel.name !== selectedModel.displayName && (
                  <span className="block truncate text-xs text-white/40">{selectedModel.name}</span>
                )}
              </div>
            </>
          ) : (
            <>
              <Cpu className="h-5 w-5 shrink-0 text-white/40" />
              <span className="text-sm text-white/50">{placeholder}</span>
            </>
          )}
        </div>
        <ChevronDown className="h-4 w-4 shrink-0 text-white/40" />
      </button>

      {description && (
        <p className={cn(typography.bodySmall.size, "text-white/40")}>{description}</p>
      )}

      {/* Model Selection Bottom Menu */}
      <BottomMenu
        isOpen={isOpen}
        onClose={() => {
          setIsOpen(false);
          setSearchQuery("");
        }}
        title={t("components.modelSelector.title")}
      >
        <div className="space-y-4">
          {/* Search */}
          <div className="relative">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-white/40" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder={t("components.modelSelector.searchPlaceholder")}
              className={cn(
                "w-full rounded-xl border border-white/10 bg-black/30",
                "px-4 py-2.5 pl-10 text-sm text-white placeholder-white/40",
                "focus:border-white/20 focus:outline-none",
              )}
              autoFocus
            />
          </div>

          {/* Model List */}
          <div className="space-y-3 max-h-[50vh] overflow-y-auto -mx-2 px-2">
            {/* Clear option */}
            {allowClear && (
              <button
                onClick={() => handleSelect(null)}
                className={cn(
                  "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition",
                  !selectedModelId
                    ? "border-emerald-400/40 bg-emerald-400/10"
                    : "border-white/10 bg-white/5 hover:bg-white/10",
                )}
              >
                <Cpu className="h-5 w-5 text-white/40" />
                <span className="flex-1 text-sm text-white">{clearLabel}</span>
                {!selectedModelId && <Check className="h-4 w-4 text-emerald-400" />}
              </button>
            )}

            {/* Grouped models */}
            {Object.entries(groupedModels).map(([provider, providerModels]) => (
              <div key={provider} className="space-y-1.5">
                <div className="flex items-center gap-2 px-1 py-1">
                  <div className="text-white/50 scale-75 origin-left">
                    {getProviderIcon(provider)}
                  </div>
                  <span className="text-[10px] font-semibold uppercase tracking-wider text-white/40">
                    {provider}
                  </span>
                  <div className="flex-1 h-px bg-white/5" />
                </div>
                {providerModels.map((model) => (
                  <button
                    key={model.id}
                    onClick={() => handleSelect(model.id)}
                    className={cn(
                      "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition",
                      selectedModelId === model.id
                        ? "border-emerald-400/40 bg-emerald-400/10"
                        : "border-white/10 bg-white/5 hover:bg-white/10",
                    )}
                  >
                    <div className="min-w-0 flex-1">
                      <span className="block truncate text-sm text-white">
                        {model.displayName || model.name}
                      </span>
                      {model.displayName && model.name !== model.displayName && (
                        <span className="block truncate text-xs text-white/40">{model.name}</span>
                      )}
                    </div>
                    {selectedModelId === model.id && (
                      <Check className="h-4 w-4 shrink-0 text-emerald-400" />
                    )}
                  </button>
                ))}
              </div>
            ))}

            {/* No results */}
            {filteredModels.length === 0 && (
              <div className="py-8 text-center">
                <p className="text-sm text-white/50">{t("components.modelSelector.noResults")}</p>
                <p className="text-xs text-white/30 mt-1">{t("components.modelSelector.noResultsHint")}</p>
              </div>
            )}
          </div>
        </div>
      </BottomMenu>
    </div>
  );
}

export { getProviderIcon };
