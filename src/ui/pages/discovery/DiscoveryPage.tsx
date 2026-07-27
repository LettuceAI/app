import { useState, useEffect, useCallback } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import {
  Search,
  TrendingUp,
  Flame,
  Clock,
  RefreshCw,
  AlertCircle,
  Sparkles,
  Loader2,
} from "lucide-react";
import { cn } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import {
  DiscoveryCard,
  DiscoverySection,
  DiscoverySectionSkeleton,
  DiscoveryFeaturedSkeleton,
  DiscoveryGridSkeleton,
  InfiniteScrollSentinel,
} from "./components";
import { PageHeader } from "../../components/App";
import { useDiscoverySearch } from "./hooks/useDiscoverySearch";
import { useIsMobileViewport } from "./hooks/useIsMobileViewport";
import { useShowNsfwImages } from "./hooks/useDiscoveryNsfw";
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

export function DiscoveryPage() {
  const navigate = useNavigate();
  const { t } = useI18n();
  const [searchParams] = useSearchParams();
  const isMobileViewport = useIsMobileViewport();
  const showNsfw = useShowNsfwImages();

  const TABS: TabItem[] = [
    { id: "all", label: t("discovery.tabs.forYou"), icon: Sparkles },
    { id: "trending", label: t("discovery.tabs.trending"), icon: TrendingUp },
    { id: "popular", label: t("discovery.tabs.popular"), icon: Flame },
    { id: "newest", label: t("discovery.tabs.new"), icon: Clock },
  ];

  const [sections, setSections] = useState<DiscoverySections | null>(null);
  const [featuredCard, setFeaturedCard] = useState<DiscoveryCardType | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<TabType>("all");
  const [, setRefreshing] = useState(false);

  const search = useDiscoverySearch(isMobileViewport ? "" : searchParams.get("q") || "");
  const searching = !isMobileViewport && search.query.trim().length > 0;

  const loadSections = useCallback(
    async (isRefresh = false) => {
      try {
        if (isRefresh) {
          setRefreshing(true);
        } else {
          setLoading(true);
        }
        setError(null);

        const data = await fetchDiscoverySections();
        setSections(data);
        const featuredPool = data.trending.slice(0, 5);
        setFeaturedCard(
          featuredPool.length > 0
            ? featuredPool[Math.floor(Math.random() * featuredPool.length)]
            : null,
        );
      } catch (err) {
        console.error("Failed to load discovery sections:", err);
        setError(err instanceof Error ? err.message : t("discovery.errors.loadContent"));
      } finally {
        setLoading(false);
        setRefreshing(false);
      }
    },
    [t],
  );

  useEffect(() => {
    loadSections();
  }, [loadSections]);

  // Keep the inline query in the URL so back navigation restores results
  useEffect(() => {
    if (isMobileViewport) return;
    const params = new URLSearchParams(window.location.search);
    const trimmed = search.debouncedQuery.trim();
    if (trimmed) {
      params.set("q", trimmed);
    } else {
      params.delete("q");
    }
    const next = params.toString();
    window.history.replaceState({}, "", next ? `${window.location.pathname}?${next}` : window.location.pathname);
  }, [search.debouncedQuery, isMobileViewport]);

  const handleCardClick = useCallback(
    (card: DiscoveryCardType) => {
      if (!card.path) return;
      if (searching) {
        const from = `/discover?q=${encodeURIComponent(search.query.trim())}`;
        navigate(`/discover/card/${encodeURIComponent(card.path)}?from=${encodeURIComponent(from)}`, {
          state: { from },
        });
        return;
      }
      navigate(`/discover/card/${encodeURIComponent(card.path)}`);
    },
    [navigate, searching, search.query],
  );

  const handleTagClick = useCallback(
    (tag: string) => {
      if (isMobileViewport) {
        navigate(`/discover/search?q=${encodeURIComponent(tag)}`);
      } else {
        search.setQuery(tag);
      }
    },
    [isMobileViewport, navigate, search.setQuery],
  );

  const handleViewAll = useCallback(
    (section: "trending" | "popular" | "newest") => {
      navigate(`/discover/browse?section=${section}`);
    },
    [navigate],
  );

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

  const trendingCards = sections
    ? sections.trending.filter((card) => card.id !== featuredCard?.id)
    : [];

  const tabBar = TABS.map((tab) => {
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
  });

  const searchResultsSummary = search.results
    ? `${
        search.results.totalHits !== undefined
          ? search.results.totalHits.toLocaleString()
          : search.results.hits.length
      } ${t("discovery.search.resultsUnit")}${
        search.results.processingTimeMs !== undefined
          ? ` (${search.results.processingTimeMs}${t("discovery.search.timingUnit")})`
          : ""
      }`
    : undefined;

  return (
    <div className="flex h-full flex-col bg-surface lg:px-4">
      {/* Main content with bottom padding for safe area */}
      <main
        className="flex-1 overflow-y-auto mx-auto w-full lg:max-w-[1600px]"
        style={{
          paddingBottom: "calc(env(safe-area-inset-bottom) + 80px)",
        }}
      >
        {isMobileViewport ? (
          <div className="sticky top-0 z-20 bg-surface/95 backdrop-blur-md">
            <div className="px-4 py-3">
              <button
                onClick={() => navigate("/discover/search")}
                className="flex w-full items-center gap-3 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-left transition-all hover:border-fg/15 hover:bg-fg/[0.07] active:scale-[0.99]"
              >
                <Search className="h-4 w-4 text-fg/40" />
                <span className="text-sm text-fg/40">{t("discovery.searchPlaceholder")}</span>
              </button>
            </div>
            <div className="scrollbar-hide flex gap-2 overflow-x-auto px-4 pb-4">{tabBar}</div>
          </div>
        ) : (
          <PageHeader
            title={t("common.bottomNav.discover")}
            meta={searching ? searchResultsSummary : undefined}
            searchValue={search.query}
            onSearchChange={search.setQuery}
            searchPlaceholder={t("discovery.searchPlaceholder")}
            filters={searching ? undefined : tabBar}
          />
        )}

        {/* Inline search results (desktop) */}
        {searching && (
          <div className="pt-1">
            {search.loading && (
              <div className="lg:px-8">
                <DiscoveryGridSkeleton cardCount={8} />
              </div>
            )}

            {!search.loading && search.error && (
              <div className="flex flex-col items-center justify-center px-6 py-20">
                <p className="text-sm text-danger">{search.error}</p>
              </div>
            )}

            {!search.loading && !search.error && search.results && search.results.hits.length > 0 && (
              <div className="px-4 lg:px-8">
                <div className="grid grid-cols-2 gap-3 lg:grid-cols-3 xl:grid-cols-4">
                  {search.results.hits.map((card, index) => (
                    <DiscoveryCard
                      key={card.id}
                      card={card}
                      onClick={handleCardClick}
                      index={index}
                      showNsfw={showNsfw}
                      onTagClick={handleTagClick}
                    />
                  ))}
                </div>

                {search.hasMore && (
                  <>
                    <InfiniteScrollSentinel
                      onReach={search.loadMore}
                      disabled={search.loadingMore}
                    />
                    <div className="flex h-14 items-center justify-center">
                      {search.loadingMore && (
                        <Loader2 className="h-5 w-5 animate-spin text-fg/40" />
                      )}
                    </div>
                  </>
                )}
              </div>
            )}

            {!search.loading && !search.error && search.results && search.results.hits.length === 0 && (
              <div className="flex flex-col items-center justify-center px-6 py-20">
                <div className="mb-4 flex h-16 w-16 items-center justify-center rounded-2xl border border-fg/10 bg-fg/5">
                  <Search className="h-8 w-8 text-fg/30" />
                </div>
                <h3 className="mb-2 text-lg font-semibold text-fg">
                  {t("discovery.search.noResults")}
                </h3>
                <p className="mb-4 text-center text-sm text-fg/50">
                  {t("discovery.search.noResultsFor")} "{search.query}"
                </p>
                <p className="text-xs text-fg/40">{t("discovery.search.noResultsHint")}</p>
              </div>
            )}
          </div>
        )}

        {/* Error state */}
        {!searching && error && (
          <div className="flex flex-col items-center justify-center px-6 py-20">
            <div className="mb-4 flex h-16 w-16 items-center justify-center rounded-2xl border border-danger/30 bg-danger/10">
              <AlertCircle className="h-8 w-8 text-danger" />
            </div>
            <h3 className="mb-2 text-lg font-semibold text-fg">{t("discovery.errorTitle")}</h3>
            <p className="mb-6 text-center text-sm text-fg/50">{error}</p>
            <button
              onClick={() => loadSections()}
              className="flex items-center gap-2 rounded-xl border border-fg/20 bg-fg/10 px-5 py-2.5 text-sm font-medium text-fg transition-all hover:bg-fg/15 active:scale-95"
            >
              <RefreshCw className="h-4 w-4" />
              {t("common.buttons.retry")}
            </button>
          </div>
        )}

        {/* Loading state */}
        {!searching && loading && !error && (
          <div className="space-y-2">
            {activeTab === "all" ? (
              <>
                <DiscoveryFeaturedSkeleton />
                <DiscoverySectionSkeleton />
                <DiscoverySectionSkeleton />
                <DiscoverySectionSkeleton />
              </>
            ) : (
              <div className="px-0 lg:px-8">
                <DiscoveryGridSkeleton cardCount={12} />
              </div>
            )}
          </div>
        )}

        {/* Content */}
        {!searching && !loading && !error && sections && (
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
                      showNsfw={showNsfw}
                      onTagClick={handleTagClick}
                    />
                  </section>
                )}

                {/* Trending Section */}
                <DiscoverySection
                  title={t("discovery.sections.trendingNow")}
                  subtitle={t("discovery.sections.trendingSubtitle")}
                  cards={trendingCards.slice(0, 11)}
                  onCardClick={handleCardClick}
                  onViewAll={() => handleViewAll("trending")}
                  showNsfw={showNsfw}
                  onTagClick={handleTagClick}
                  icon={<TrendingUp className="h-4 w-4 text-fg" />}
                  accentColor="from-accent to-accent/80"
                />

                {/* Popular Section */}
                <DiscoverySection
                  title={t("discovery.sections.mostPopular")}
                  subtitle={t("discovery.sections.popularSubtitle")}
                  cards={sections.popular.slice(0, 12)}
                  onCardClick={handleCardClick}
                  onViewAll={() => handleViewAll("popular")}
                  showNsfw={showNsfw}
                  onTagClick={handleTagClick}
                  icon={<Flame className="h-4 w-4 text-fg" />}
                  accentColor="from-accent/80 to-accent/80"
                />

                {/* Newest Section */}
                <DiscoverySection
                  title={t("discovery.sections.freshArrivals")}
                  subtitle={t("discovery.sections.freshSubtitle")}
                  cards={sections.newest.slice(0, 12)}
                  onCardClick={handleCardClick}
                  onViewAll={() => handleViewAll("newest")}
                  showNsfw={showNsfw}
                  onTagClick={handleTagClick}
                  icon={<Clock className="h-4 w-4 text-fg" />}
                  accentColor="from-accent to-info/80"
                />
              </div>
            ) : (
              <div className="px-4 lg:px-8">
                {/* Grid view for filtered tabs */}
                <div className="grid grid-cols-2 gap-3 lg:grid-cols-3 xl:grid-cols-4">
                  {getDisplayCards().map((card, index) => (
                    <DiscoveryCard
                      key={card.id}
                      card={card}
                      onClick={handleCardClick}
                      index={index}
                      showNsfw={showNsfw}
                      onTagClick={handleTagClick}
                    />
                  ))}
                </div>

                {getDisplayCards().length === 0 && (
                  <div className="flex flex-col items-center justify-center py-20">
                    <p className="text-sm text-fg/50">{t("discovery.noCardsFound")}</p>
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
