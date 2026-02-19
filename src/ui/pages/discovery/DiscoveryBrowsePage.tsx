import { useState, useEffect, useCallback } from "react";
import { motion } from "framer-motion";
import { useLocation, useNavigate, useSearchParams } from "react-router-dom";
import { ArrowLeft, TrendingUp, Flame, Clock, AlertCircle, ArrowUpDown, Check } from "lucide-react";
import { cn, typography, interactive } from "../../design-tokens";
import { DiscoveryCard, DiscoveryGridSkeleton } from "./components";
import { resolveBackTarget, Routes, useNavigationManager } from "../../navigation";
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

const SECTION_CONFIGS: Record<CardType, SectionConfig> = {
  trending: {
    title: "Trending",
    subtitle: "Hot this week",
    icon: TrendingUp,
    accentColor: "from-accent to-accent/80",
    defaultSort: "updated",
  },
  popular: {
    title: "Popular",
    subtitle: "Community favorites",
    icon: Flame,
    accentColor: "from-accent/80 to-accent/80",
    defaultSort: "likes",
  },
  newest: {
    title: "New Arrivals",
    subtitle: "Fresh characters",
    icon: Clock,
    accentColor: "from-accent to-info/80",
    defaultSort: "created",
  },
};

interface SortOptionItem {
  value: SortOption;
  label: string;
}

const SORT_OPTIONS: SortOptionItem[] = [
  { value: "likes", label: "Most Liked" },
  { value: "downloads", label: "Most Downloaded" },
  { value: "views", label: "Most Viewed" },
  { value: "messages", label: "Most Messages" },
  { value: "created", label: "Newest First" },
  { value: "updated", label: "Recently Updated" },
  { value: "name", label: "Name (A-Z)" },
];

export function DiscoveryBrowsePage() {
  const navigate = useNavigate();
  const location = useLocation();
  const { go, backOrReplace } = useNavigationManager();
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
      setError(err instanceof Error ? err.message : "Failed to load content");
    } finally {
      setLoading(false);
    }
  }, [section, sortBy]);

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

  const handleBack = useCallback(() => {
    const currentPath = location.pathname + location.search;
    const target = resolveBackTarget(currentPath);
    if (target) {
      go(target, { replace: true });
      return;
    }
    backOrReplace(Routes.discover);
  }, [location.pathname, location.search, go, backOrReplace]);

  const handleSortChange = (newSort: SortOption) => {
    setSortBy(newSort);
    setShowSortMenu(false);
  };

  const currentSortLabel = SORT_OPTIONS.find((opt) => opt.value === sortBy)?.label || "Sort";

  return (
    <div className="flex h-full flex-col bg-surface">
      {/* Header - matches TopNav style */}
      <header
        className="sticky top-0 z-30 border-b border-fg/10 bg-surface-el/80 backdrop-blur-md"
        style={{
          paddingTop: "calc(env(safe-area-inset-top) + 12px)",
          paddingBottom: "12px",
        }}
      >
        <div className="mx-auto flex w-full max-w-md items-center justify-between px-4 lg:max-w-none lg:px-8">
          <div className="flex items-center gap-2">
            {/* Back button */}
            <button
              onClick={handleBack}
              className={cn(
                "flex shrink-0 items-center justify-center rounded-full p-2",
                "text-fg/70 hover:bg-fg/10 hover:text-fg",
                interactive.transition.fast,
                interactive.active.scale,
              )}
              aria-label="Go back"
            >
              <ArrowLeft size={20} strokeWidth={2.5} />
            </button>

            <motion.h1
              initial={{ opacity: 0, y: 5 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.3, ease: "easeOut" }}
              className={cn(typography.h1.size, "font-bold tracking-tight text-fg")}
            >
              {config.title}
            </motion.h1>
          </div>

          {/* Sort button */}
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

        {/* Results count */}
        {!loading && cards.length > 0 && (
          <div className="mx-auto mt-3 max-w-md px-4 lg:max-w-none lg:px-8">
            <p className="text-xs text-fg/50">{cards.length} characters</p>
          </div>
        )}
      </header>

      {/* Main content */}
      <main
        className="flex-1 overflow-y-auto"
        style={{
          paddingBottom: "calc(env(safe-area-inset-bottom) + 24px)",
        }}
      >
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
            <h3 className="mb-2 text-lg font-semibold text-fg">Something went wrong</h3>
            <p className="mb-6 text-center text-sm text-fg/50">{error}</p>
            <button
              onClick={loadCards}
              className="flex items-center gap-2 rounded-xl border border-fg/20 bg-fg/10 px-5 py-2.5 text-sm font-medium text-fg transition-all hover:bg-fg/15 active:scale-95"
            >
              Try Again
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
            <h3 className="mb-2 text-lg font-semibold text-fg">No characters found</h3>
            <p className="text-sm text-fg/50">Check back later for new content</p>
          </div>
        )}
      </main>

      {/* Sort Menu */}
      <BottomMenu isOpen={showSortMenu} onClose={() => setShowSortMenu(false)} title="Sort By">
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
