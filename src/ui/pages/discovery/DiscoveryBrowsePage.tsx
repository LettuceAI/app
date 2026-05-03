import { useState, useEffect, useCallback } from "react";
import { motion } from "framer-motion";
import { useNavigate, useSearchParams } from "react-router-dom";
import { TrendingUp, Flame, Clock, AlertCircle, ArrowUpDown, Check } from "lucide-react";
import { cn, interactive } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import { DiscoveryCard, DiscoveryGridSkeleton } from "./components";
import { useNavigationManager } from "../../navigation";
import {
  fetchDiscoveryCards,
  type DiscoveryCard as DiscoveryCardType,
  type CardType,
  type SortOption,
} from "../../../core/discovery";
import { BottomMenu } from "../../components";

interface SectionConfig {
  title: string;
  subtitle: string;
  icon: typeof TrendingUp;
  accentColor: string;
  defaultSort: SortOption;
}

interface SortOptionItem {
  value: SortOption;
  label: string;
}

export function DiscoveryBrowsePage() {
  const navigate = useNavigate();
  const { } = useNavigationManager();
  const { t } = useI18n();

  const SECTION_CONFIGS: Record<CardType, SectionConfig> = {
    trending: {
      title: t("discovery.tabs.trending"),
      subtitle: t("discovery.sections.trendingSubtitle"),
      icon: TrendingUp,
      accentColor: "from-accent to-accent/80",
      defaultSort: "updated",
    },
    popular: {
      title: t("discovery.tabs.popular"),
      subtitle: t("discovery.sections.popularSubtitle"),
      icon: Flame,
      accentColor: "from-accent/80 to-accent/80",
      defaultSort: "likes",
    },
    newest: {
      title: t("discovery.browse.newArrivals"),
      subtitle: t("discovery.browse.freshCharacters"),
      icon: Clock,
      accentColor: "from-accent to-info/80",
      defaultSort: "created",
    },
  };

  const SORT_OPTIONS: SortOptionItem[] = [
    { value: "likes", label: t("discovery.sort.mostLiked") },
    { value: "downloads", label: t("discovery.sort.mostDownloaded") },
    { value: "views", label: t("discovery.sort.mostViewed") },
    { value: "messages", label: t("discovery.sort.mostMessages") },
    { value: "created", label: t("discovery.sort.newestFirst") },
    { value: "updated", label: t("discovery.sort.recentlyUpdated") },
    { value: "name", label: t("discovery.sort.nameAZ") },
  ];
  const [searchParams] = useSearchParams();

  const sectionParam = searchParams.get("section") as CardType | null;
  const section: CardType =
    sectionParam && SECTION_CONFIGS[sectionParam] ? sectionParam : "trending";
  const config = SECTION_CONFIGS[section];

  const [cards, setCards] = useState<DiscoveryCardType[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [sortBy, setSortBy] = useState<SortOption>(config.defaultSort);
  const [showSortMenu, setShowSortMenu] = useState(false);

  const loadCards = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      const data = await fetchDiscoveryCards(section, sortBy, true);
      setCards(data);
    } catch (err) {
      console.error("Failed to load cards:", err);
      setError(err instanceof Error ? err.message : t("discovery.errors.loadContent"));
    } finally {
      setLoading(false);
    }
  }, [section, sortBy, t]);

  useEffect(() => {
    loadCards();
  }, [loadCards]);

  const handleCardClick = useCallback(
    (card: DiscoveryCardType) => {
      if (card.path) {
        navigate(`/discover/card/${encodeURIComponent(card.path)}`);
      }
    },
    [navigate],
  );

  const handleSortChange = (newSort: SortOption) => {
    setSortBy(newSort);
    setShowSortMenu(false);
  };

  const currentSortLabel = SORT_OPTIONS.find((opt) => opt.value === sortBy)?.label || t("discovery.sortBy");

  return (
    <div className="flex h-full flex-col bg-surface">
      {/* Main content */}
      <main
        className="flex-1 overflow-y-auto"
        style={{
          paddingBottom: "calc(env(safe-area-inset-bottom) + 24px)",
        }}
      >
        {/* Toolbar */}
        <div className="flex items-center justify-between px-4 py-3 lg:px-8">
          <div className="text-xs text-fg/50">
            {!loading && cards.length > 0 && `${cards.length} ${t("discovery.resultsUnit")}`}
          </div>
          <button
            onClick={() => setShowSortMenu(true)}
            className={cn(
              "flex items-center gap-1.5 rounded-lg border border-fg/10 bg-fg/5 px-3 py-1.5",
              "text-xs font-medium text-fg/70",
              "transition-all hover:border-fg/20 hover:bg-fg/10 hover:text-fg",
              interactive.active.scale,
            )}
          >
            <ArrowUpDown className="h-3.5 w-3.5" />
            {currentSortLabel}
          </button>
        </div>

        {/* Error state */}
        {error && (
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            className="flex flex-col items-center justify-center px-6 py-20"
          >
            <div className="mb-4 flex h-16 w-16 items-center justify-center rounded-2xl border border-danger/30 bg-danger/10">
              <AlertCircle className="h-8 w-8 text-danger" />
            </div>
            <h3 className="mb-2 text-lg font-semibold text-fg">{t("discovery.errorTitle")}</h3>
            <p className="mb-6 text-center text-sm text-fg/50">{error}</p>
            <button
              onClick={loadCards}
              className="flex items-center gap-2 rounded-xl border border-fg/20 bg-fg/10 px-5 py-2.5 text-sm font-medium text-fg transition-all hover:bg-fg/15 active:scale-95"
            >
              {t("common.buttons.retry")}
            </button>
          </motion.div>
        )}

        {/* Loading state */}
        {loading && (
          <div className="pt-4">
            <DiscoveryGridSkeleton cardCount={12} />
          </div>
        )}

        {/* Content */}
        {!loading && !error && cards.length > 0 && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.2 }}
            className="px-4 pt-4"
          >
            <div className="grid grid-cols-2 gap-3 lg:grid-cols-3 xl:grid-cols-4">
              {cards.map((card, index) => (
                <DiscoveryCard key={card.id} card={card} onClick={handleCardClick} index={index} />
              ))}
            </div>
          </motion.div>
        )}

        {/* Empty state */}
        {!loading && !error && cards.length === 0 && (
          <div className="flex flex-col items-center justify-center py-20">
            <div className="mb-4 flex h-16 w-16 items-center justify-center rounded-2xl border border-fg/10 bg-fg/5">
              <config.icon className="h-8 w-8 text-fg/30" />
            </div>
            <h3 className="mb-2 text-lg font-semibold text-fg">{t("discovery.browse.noCharactersFound")}</h3>
            <p className="text-sm text-fg/50">{t("discovery.browse.noCharactersSubtitle")}</p>
          </div>
        )}
      </main>

      {/* Sort Menu */}
      <BottomMenu isOpen={showSortMenu} onClose={() => setShowSortMenu(false)} title={t("discovery.sortBy")}>
        <div className="space-y-1">
          {SORT_OPTIONS.map((option) => {
            const isSelected = sortBy === option.value;

            return (
              <button
                key={option.value}
                onClick={() => handleSortChange(option.value)}
                className={cn(
                  "flex w-full items-center justify-between rounded-xl px-4 py-3 text-left transition-all",
                  isSelected
                    ? "bg-fg/10 text-fg"
                    : "text-fg/60 hover:bg-fg/5 hover:text-fg",
                )}
              >
                <span className="text-sm font-medium">{option.label}</span>
                {isSelected && <Check className="h-4 w-4 text-accent" />}
              </button>
            );
          })}
        </div>
      </BottomMenu>
    </div>
  );
}

export default DiscoveryBrowsePage;
