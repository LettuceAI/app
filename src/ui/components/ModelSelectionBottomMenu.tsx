import { createElement, isValidElement, ReactNode, useEffect, useMemo, useState } from "react";
import { Check, Loader2, LucideIcon, Search } from "lucide-react";

import type { Model } from "../../core/storage/schemas";
import { getProviderIcon } from "../../core/utils/providerIcons";
import { cn } from "../design-tokens";
import { BottomMenu } from "./BottomMenu";

type MenuTheme = "default" | "dark";
type SelectionTone = "accent" | "emerald" | "info";

type ClearOption = {
  label: ReactNode;
  description?: ReactNode;
  icon?: ReactNode | LucideIcon;
  selected?: boolean;
  onClick: () => void;
};

export type ModelSelectionBottomMenuProps = {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  models: Model[];
  selectedModelIds?: string[];
  onSelectModel?: (modelId: string) => void;
  onToggleModel?: (model: Model, isSelected: boolean) => void;
  searchQuery?: string;
  onSearchChange?: (value: string) => void;
  searchPlaceholder?: string;
  filterModels?: boolean;
  modelMatchesQuery?: (model: Model, query: string) => boolean;
  renderModelTitle?: (model: Model) => ReactNode;
  renderModelDescription?: (model: Model) => ReactNode;
  renderModelMeta?: (model: Model) => ReactNode;
  renderModelIcon?: (model: Model) => ReactNode;
  renderEmptyState?: (query: string, hasModels: boolean) => ReactNode;
  loading?: boolean;
  loadingContent?: ReactNode;
  clearOption?: ClearOption;
  rightAction?: ReactNode;
  theme?: MenuTheme;
  tone?: SelectionTone;
  includeExitIcon?: boolean;
  location?: "top" | "bottom";
};

const TONE_STYLES: Record<SelectionTone, { selectedItem: string; check: string }> = {
  accent: {
    selectedItem: "border-accent/40 bg-accent/10",
    check: "text-accent/80",
  },
  emerald: {
    selectedItem: "border-emerald-400/40 bg-emerald-400/10",
    check: "text-emerald-400",
  },
  info: {
    selectedItem: "border-info/40 bg-info/10",
    check: "text-info",
  },
};

const THEME_STYLES: Record<
  MenuTheme,
  {
    input: string;
    icon: string;
    idleItem: string;
    primaryText: string;
    secondaryText: string;
  }
> = {
  default: {
    input:
      "w-full rounded-xl border border-fg/10 bg-surface-el/30 px-4 py-2.5 pl-10 text-sm text-fg placeholder-fg/40 focus:border-fg/20 focus:outline-none",
    icon: "text-fg/40",
    idleItem: "border-fg/10 bg-fg/5 hover:bg-fg/10",
    primaryText: "text-fg",
    secondaryText: "text-fg/40",
  },
  dark: {
    input:
      "w-full rounded-xl border border-fg/12 bg-surface-el/45 px-4 py-2.5 pl-10 text-sm text-fg placeholder-fg/40 focus:border-fg/20 focus:outline-none",
    icon: "text-fg/40",
    idleItem: "border-fg/12 bg-fg/6 hover:bg-fg/10",
    primaryText: "text-fg",
    secondaryText: "text-fg/40",
  },
};

function renderIcon(icon: ClearOption["icon"], fallbackClassName: string) {
  if (!icon) return null;
  if (isValidElement(icon)) {
    return icon;
  }
  if (typeof icon === "function" || typeof icon === "object") {
    return createElement(icon as LucideIcon, { className: fallbackClassName });
  }
  return icon;
}

export function ModelSelectionBottomMenu({
  isOpen,
  onClose,
  title,
  models,
  selectedModelIds = [],
  onSelectModel,
  onToggleModel,
  searchQuery,
  onSearchChange,
  searchPlaceholder = "Search models...",
  filterModels = true,
  modelMatchesQuery,
  renderModelTitle,
  renderModelDescription,
  renderModelMeta,
  renderModelIcon,
  renderEmptyState,
  loading = false,
  loadingContent,
  clearOption,
  rightAction,
  theme = "default",
  tone = "accent",
  includeExitIcon = true,
  location = "bottom",
}: ModelSelectionBottomMenuProps) {
  const [internalQuery, setInternalQuery] = useState("");
  const query = searchQuery ?? internalQuery;
  const setQuery = onSearchChange ?? setInternalQuery;
  const themeStyles = THEME_STYLES[theme];
  const toneStyles = TONE_STYLES[tone];

  useEffect(() => {
    if (!isOpen && searchQuery === undefined) {
      setInternalQuery("");
    }
  }, [isOpen, searchQuery]);

  const filteredModels = useMemo(() => {
    if (!filterModels || !query.trim()) return models;
    const normalizedQuery = query.toLowerCase();
    return models.filter((model) => {
      if (modelMatchesQuery) {
        return modelMatchesQuery(model, normalizedQuery);
      }
      return (
        model.displayName?.toLowerCase().includes(normalizedQuery) ||
        model.name?.toLowerCase().includes(normalizedQuery) ||
        model.providerId?.toLowerCase().includes(normalizedQuery) ||
        model.providerLabel?.toLowerCase().includes(normalizedQuery)
      );
    });
  }, [filterModels, modelMatchesQuery, models, query]);

  return (
    <BottomMenu
      isOpen={isOpen}
      onClose={onClose}
      title={title}
      rightAction={rightAction}
      includeExitIcon={includeExitIcon}
      location={location}
    >
      <div className="space-y-4">
        <div className="relative">
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={searchPlaceholder}
            className={themeStyles.input}
            autoFocus
          />
          <Search
            className={cn(
              "absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2",
              themeStyles.icon,
            )}
          />
        </div>

        <div className="max-h-[50vh] space-y-2 overflow-y-auto">
          {clearOption && (
            <button
              onClick={clearOption.onClick}
              className={cn(
                "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition",
                clearOption.selected ? toneStyles.selectedItem : themeStyles.idleItem,
              )}
            >
              {renderIcon(clearOption.icon, cn("h-5 w-5 shrink-0", themeStyles.icon))}
              <div className="min-w-0 flex-1">
                <span className={cn("block truncate text-sm", themeStyles.primaryText)}>
                  {clearOption.label}
                </span>
                {clearOption.description && (
                  <span className={cn("block truncate text-xs", themeStyles.secondaryText)}>
                    {clearOption.description}
                  </span>
                )}
              </div>
              {clearOption.selected && <Check className={cn("ml-auto h-4 w-4", toneStyles.check)} />}
            </button>
          )}

          {loading ? (
            loadingContent ?? (
              <div className="flex flex-col items-center justify-center py-12">
                <Loader2 className={cn("h-6 w-6 animate-spin", themeStyles.icon)} />
                <p className={cn("mt-3 text-sm", themeStyles.secondaryText)}>Loading models...</p>
              </div>
            )
          ) : filteredModels.length === 0 ? (
            renderEmptyState?.(query, models.length > 0) ?? (
              <div className={cn("py-8 text-center text-sm", themeStyles.secondaryText)}>
                {models.length === 0
                  ? "No models available."
                  : `No models found matching "${query}".`}
              </div>
            )
          ) : (
            filteredModels.map((model) => {
              const isSelected = selectedModelIds.includes(model.id);
              return (
                <button
                  key={model.id}
                  onClick={() => {
                    if (onToggleModel) {
                      onToggleModel(model, isSelected);
                    } else {
                      onSelectModel?.(model.id);
                    }
                  }}
                  className={cn(
                    "flex w-full items-start gap-3 rounded-xl border px-3.5 py-3 text-left transition",
                    isSelected ? toneStyles.selectedItem : themeStyles.idleItem,
                  )}
                >
                  {renderModelIcon?.(model) ?? getProviderIcon(model.providerId)}
                  <div className="min-w-0 flex-1">
                    <span className={cn("block truncate text-sm", themeStyles.primaryText)}>
                      {(renderModelTitle?.(model) ?? model.displayName) || model.name}
                    </span>
                    <span className={cn("block truncate text-xs", themeStyles.secondaryText)}>
                      {renderModelDescription?.(model) ?? model.name}
                    </span>
                    {renderModelMeta && (
                      <div className="mt-1">{renderModelMeta(model)}</div>
                    )}
                  </div>
                  {isSelected && <Check className={cn("h-4 w-4 shrink-0", toneStyles.check)} />}
                </button>
              );
            })
          )}
        </div>
      </div>
    </BottomMenu>
  );
}
