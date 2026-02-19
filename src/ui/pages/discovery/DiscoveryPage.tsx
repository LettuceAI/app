import { useState, useEffect, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import {
  Search,
  ArrowLeft,
  TrendingUp,
  Flame,
  Clock,
  RefreshCw,
  AlertCircle,
  Sparkles,
  Settings,
} from "lucide-react";
import { cn, typography, interactive } from "../../design-tokens";
import { Routes, useNavigationManager } from "../../navigation";
import { DiscoveryCard, DiscoverySection, DiscoverySectionSkeleton } from "./components";
import {
  fetchDiscoverySections,
  type DiscoveryCard as DiscoveryCardType,
  type DiscoverySections,
} from "../../../core/discovery";

type TabType = "all" | "trending" | "popular" | "newest";

interface TabItem {
  id: TabType;
  label: string;
  icon: typeof TrendingUp;
}

const TABS: TabItem[] = [
  { id: "all", label: "For You", icon: Sparkles },
  { id: "trending", label: "Trending", icon: TrendingUp },
  { id: "popular", label: "Popular", icon: Flame },
  { id: "newest", label: "New", icon: Clock },
];

export function DiscoveryPage() {
  const navigate = useNavigate();
  const { backOrReplace } = useNavigationManager();

  const [sections, setSections] = useState<DiscoverySections | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<TabType>("all");
  const [refreshing, setRefreshing] = useState(false);

  const loadSections = useCallback(async (isRefresh = false) => {
    try {
      if (isRefresh) {
        setRefreshing(true);
      } else {
        setLoading(true);
      }
      setError(null);

      const data = await fetchDiscoverySections();
      setSections(data);
    } catch (err) {
      console.error("Failed to load discovery sections:", err);
      setError(err instanceof Error ? err.message : "Failed to load content");
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    loadSections();
  }, [loadSections]);

  const handleCardClick = useCallback(
    (card: DiscoveryCardType) => {
      if (card.path) {
        navigate(`/discover/card/${encodeURIComponent(card.path)}`);
      }
    },
    [navigate],
  );

  const handleViewAll = useCallback(
    (section: "trending" | "popular" | "newest") => {
      navigate(`/discover/browse?section=${section}`);
    },
    [navigate],
  );

  const handleSearchClick = () => {
    navigate("/discover/search");
  };

  const handleBack = () => {
    backOrReplace(Routes.chat);
  };

  const handleRefresh = () => {
    loadSections(true);
  };

  // Get cards for the current tab
  const getDisplayCards = (): DiscoveryCardType[] => {
    if (!sections) return [];

    switch (activeTab) {
      case "trending":
        return sections.trending;
      case "popular":
        return sections.popular;
      case "newest":
        return sections.newest;
      default:
        return [];
    }
  };

  // Get featured card (first trending card)
  const featuredCard = sections?.trending[0];

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
            <h1 className={cn(typography.h1.size, "font-bold tracking-tight text-fg")}>
              Discover
            </h1>
          </div>

          <div className="flex items-center gap-1">
            <button
              onClick={handleRefresh}
              disabled={refreshing}
              className={cn(
                "flex items-center justify-center rounded-full p-2",
                "text-fg/70 hover:bg-fg/10 hover:text-fg",
                interactive.transition.fast,
                interactive.active.scale,
                refreshing && "animate-spin",
              )}
              aria-label="Refresh"
            >
              <RefreshCw size={20} strokeWidth={2.5} />
            </button>

            <button
              onClick={handleSearchClick}
              className={cn(
                "flex items-center justify-center rounded-full p-2",
                "text-fg/70 hover:bg-fg/10 hover:text-fg",
                interactive.transition.fast,
                interactive.active.scale,
              )}
              aria-label="Search"
            >
              <Search size={20} strokeWidth={2.5} />
            </button>

            <button
              onClick={() => navigate("/settings")}
              className={cn(
                "flex items-center justify-center rounded-full p-2",
                "text-fg/70 hover:bg-fg/10 hover:text-fg",
                interactive.transition.fast,
                interactive.active.scale,
              )}
              aria-label="Settings"
            >
              <Settings size={20} strokeWidth={2.5} />
            </button>
          </div>
        </div>
      </header>

      {/* Main content with bottom padding for safe area */}
      <main
        className="flex-1 overflow-y-auto"
        style={{
          paddingBottom: "calc(env(safe-area-inset-bottom) + 80px)",
        }}
      >
        {/* Search bar shortcut */}
        <div className="px-4 py-3 lg:px-8">
          <button
            onClick={handleSearchClick}
            className="flex w-full items-center gap-3 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-left transition-all hover:border-fg/15 hover:bg-fg/[0.07] active:scale-[0.99]"
          >
            <Search className="h-4 w-4 text-fg/40" />
            <span className="text-sm text-fg/40">Search characters...</span>
          </button>
        </div>

        {/* Tab bar */}
        <div className="scrollbar-hide flex gap-2 overflow-x-auto px-4 pb-4 lg:px-8">
          {TABS.map((tab) => {
            const isActive = activeTab === tab.id;
            const Icon = tab.icon;

            return (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className={cn(
                  "flex shrink-0 items-center gap-1.5 rounded-full px-4 py-2 text-sm font-medium transition-all",
                  isActive
                    ? "bg-fg text-surface shadow-lg shadow-fg/20"
                    : "border border-fg/10 bg-fg/5 text-fg/70 hover:bg-fg/10 hover:text-fg",
                )}
              >
                <Icon className="h-3.5 w-3.5" />
                {tab.label}
              </button>
            );
          })}
        </div>

        {/* Error state */}
        {error && (
          <div className="flex flex-col items-center justify-center px-6 py-20">
            <div className="mb-4 flex h-16 w-16 items-center justify-center rounded-2xl border border-danger/30 bg-danger/10">
              <AlertCircle className="h-8 w-8 text-danger" />
            </div>
            <h3 className="mb-2 text-lg font-semibold text-fg">Something went wrong</h3>
            <p className="mb-6 text-center text-sm text-fg/50">{error}</p>
            <button
              onClick={() => loadSections()}
              className="flex items-center gap-2 rounded-xl border border-fg/20 bg-fg/10 px-5 py-2.5 text-sm font-medium text-fg transition-all hover:bg-fg/15 active:scale-95"
            >
              <RefreshCw className="h-4 w-4" />
              Try Again
            </button>
          </div>
        )}

        {/* Loading state */}
        {loading && !error && (
          <div className="space-y-2">
            <DiscoverySectionSkeleton />
            <DiscoverySectionSkeleton />
            <DiscoverySectionSkeleton />
          </div>
        )}

        {/* Content */}
        {!loading && !error && sections && (
          <>
            {activeTab === "all" ? (
              <div className="space-y-2">
                {/* Featured card */}
                {featuredCard && (
                  <section className="px-4 lg:px-8">
                    <DiscoveryCard
                      card={featuredCard}
                      onClick={handleCardClick}
                      variant="featured"
                    />
                  </section>
                )}

                {/* Trending Section */}
                <DiscoverySection
                  title="Trending Now"
                  subtitle="Hot this week"
                  cards={sections.trending.slice(1, 12)}
                  onCardClick={handleCardClick}
                  onViewAll={() => handleViewAll("trending")}
                  icon={<TrendingUp className="h-4 w-4 text-fg" />}
                  accentColor="from-accent to-accent/80"
                />

                {/* Popular Section */}
                <DiscoverySection
                  title="Most Popular"
                  subtitle="Community favorites"
                  cards={sections.popular.slice(0, 12)}
                  onCardClick={handleCardClick}
                  onViewAll={() => handleViewAll("popular")}
                  icon={<Flame className="h-4 w-4 text-fg" />}
                  accentColor="from-accent/80 to-accent/80"
                />

                {/* Newest Section */}
                <DiscoverySection
                  title="Fresh Arrivals"
                  subtitle="Just added"
                  cards={sections.newest.slice(0, 12)}
                  onCardClick={handleCardClick}
                  onViewAll={() => handleViewAll("newest")}
                  icon={<Clock className="h-4 w-4 text-fg" />}
                  accentColor="from-accent to-info/80"
                />
              </div>
            ) : (
              <div className="px-4">
                {/* Grid view for filtered tabs */}
                <div className="grid grid-cols-2 gap-3 lg:grid-cols-3 xl:grid-cols-4">
                  {getDisplayCards().map((card, index) => (
                    <DiscoveryCard
                      key={card.id}
                      card={card}
                      onClick={handleCardClick}
                      index={index}
                    />
                  ))}
                </div>

                {getDisplayCards().length === 0 && (
                  <div className="flex flex-col items-center justify-center py-20">
                    <p className="text-sm text-fg/50">No cards found</p>
                  </div>
                )}
              </div>
            )}
          </>
        )}
      </main>
    </div>
  );
}

export default DiscoveryPage;
