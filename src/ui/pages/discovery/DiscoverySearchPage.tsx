import { useState, useEffect, useCallback, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { useNavigate, useSearchParams } from "react-router-dom";
import { Search, X, Loader2, TrendingUp, Clock, Sparkles } from "lucide-react";
import { cn, typography } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import { DiscoveryCard, DiscoveryGridSkeleton } from "./components";
import { Routes, useNavigationManager } from "../../navigation";
import {
  searchDiscoveryCards,
  type DiscoveryCard as DiscoveryCardType,
  type DiscoverySearchResponse,
} from "../../../core/discovery";

interface RecentSearch {
  query: string;
  timestamp: number;
}

const RECENT_SEARCHES_KEY = "discovery_recent_searches";
const MAX_RECENT_SEARCHES = 8;

function loadRecentSearches(): RecentSearch[] {
  try {
    const stored = sessionStorage.getItem(RECENT_SEARCHES_KEY);
    return stored ? JSON.parse(stored) : [];
  } catch {
    return [];
  }
}

function saveRecentSearch(query: string) {
  const trimmed = query.trim();
  if (!trimmed) return;

  const searches = loadRecentSearches().filter((s) => s.query !== trimmed);
  searches.unshift({ query: trimmed, timestamp: Date.now() });
  const limited = searches.slice(0, MAX_RECENT_SEARCHES);

  try {
    sessionStorage.setItem(RECENT_SEARCHES_KEY, JSON.stringify(limited));
  } catch {
    // Storage full or unavailable
  }
}

function clearRecentSearches() {
  try {
    sessionStorage.removeItem(RECENT_SEARCHES_KEY);
  } catch {
    // Storage unavailable
  }
}

export function DiscoverySearchPage() {
  const navigate = useNavigate();
  const { } = useNavigationManager();
  const { t } = useI18n();

  const TRENDING_SEARCHES = [
    t("discovery.search.trends.anime"),
    t("discovery.search.trends.fantasy"),
    t("discovery.search.trends.romance"),
    t("discovery.search.trends.villain"),
    t("discovery.search.trends.adventure"),
    t("discovery.search.trends.comedy"),
    t("discovery.search.trends.mystery"),
    t("discovery.search.trends.sciFi"),
  ];
  const [searchParams] = useSearchParams();
  const inputRef = useRef<HTMLInputElement>(null);

  const [query, setQuery] = useState(searchParams.get("q") || "");
  const [debouncedQuery, setDebouncedQuery] = useState(query);
  const [results, setResults] = useState<DiscoverySearchResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [recentSearches, setRecentSearches] = useState<RecentSearch[]>([]);

  // Load recent searches on mount
  useEffect(() => {
    setRecentSearches(loadRecentSearches());
    // Auto-focus search input
    inputRef.current?.focus();
  }, []);

  // Debounce search query
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedQuery(query);
    }, 400);

    return () => clearTimeout(timer);
  }, [query]);

  // Search when debounced query changes
  useEffect(() => {
    if (!debouncedQuery.trim()) {
      setResults(null);
      setError(null);
      return;
    }

    const search = async () => {
      setLoading(true);
      setError(null);
      setPage(1);
      setHasMore(true);

      try {
        const response = await searchDiscoveryCards(debouncedQuery, 1, 30);
        setResults(response);
        setHasMore(
          response.page !== undefined &&
            response.totalPages !== undefined &&
            response.page < response.totalPages,
        );

        // Save to recent searches
        saveRecentSearch(debouncedQuery);
        setRecentSearches(loadRecentSearches());

        // Update URL without triggering re-render
        const newParams = new URLSearchParams(window.location.search);
        newParams.set("q", debouncedQuery);
        window.history.replaceState({}, "", `${window.location.pathname}?${newParams.toString()}`);
      } catch (err) {
        console.error("Search failed:", err);
        setError(err instanceof Error ? err.message : t("discovery.errors.searchFailed"));
      } finally {
        setLoading(false);
      }
    };

    search();
  }, [debouncedQuery]);

  const loadMore = useCallback(async () => {
    if (!debouncedQuery.trim() || loadingMore || !hasMore) return;

    setLoadingMore(true);
    try {
      const nextPage = page + 1;
      const response = await searchDiscoveryCards(debouncedQuery, nextPage, 30);

      setResults((prev) =>
        prev
          ? {
              ...response,
              hits: [...prev.hits, ...response.hits],
            }
          : response,
      );

      setPage(nextPage);
      setHasMore(
        response.page !== undefined &&
          response.totalPages !== undefined &&
          response.page < response.totalPages,
      );
    } catch (err) {
      console.error("Failed to load more:", err);
    } finally {
      setLoadingMore(false);
    }
  }, [debouncedQuery, page, loadingMore, hasMore]);

  const handleCardClick = useCallback(
    (card: DiscoveryCardType) => {
      if (!card.path) return;
      const trimmedQuery = query.trim();
      const fromParams = new URLSearchParams(location.search);
      if (trimmedQuery) {
        fromParams.set("q", trimmedQuery);
      }
      const from = trimmedQuery
        ? `${Routes.discoverSearch}?${fromParams.toString()}`
        : location.pathname + location.search;
      const encodedFrom = encodeURIComponent(from);
      navigate(`/discover/card/${encodeURIComponent(card.path)}?from=${encodedFrom}`, {
        state: { from },
      });
    },
    [navigate, location.pathname, location.search, query],
  );



  const handleClearQuery = () => {
    setQuery("");
    setResults(null);
    inputRef.current?.focus();
  };

  const handleRecentSearchClick = (searchQuery: string) => {
    setQuery(searchQuery);
  };

  const handleClearRecent = () => {
    clearRecentSearches();
    setRecentSearches([]);
  };

  const handleTrendingClick = (term: string) => {
    setQuery(term);
  };

  const showEmptyState = !loading && !query.trim() && !results;
  const showResults = !loading && results && results.hits.length > 0;
  const showNoResults = !loading && results && results.hits.length === 0;

  return (
    <div className="flex h-full min-h-0 flex-col bg-surface">
      {/* Search bar */}
      <div className="shrink-0 border-b border-fg/10 bg-surface-el/95 px-4 py-3 lg:px-8">
        <div className="relative mx-auto max-w-md lg:max-w-none">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-fg/40" />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("discovery.search.placeholder")}
            className="w-full rounded-xl border border-fg/10 bg-fg/5 py-2.5 pl-10 pr-10 text-sm text-fg placeholder-fg/40 transition-all focus:border-fg/20 focus:bg-fg/[0.07] focus:outline-none"
          />
          {query && (
            <button
              onClick={handleClearQuery}
              className="absolute right-3 top-1/2 -translate-y-1/2 rounded-full p-1 text-fg/40 hover:bg-fg/10 hover:text-fg"
            >
              <X className="h-4 w-4" />
            </button>
          )}
        </div>
        {results && (
          <div className="mx-auto mt-2 max-w-md lg:max-w-none">
            <p className="text-xs text-fg/50">
              {results.totalHits !== undefined
                ? `${results.totalHits.toLocaleString()} ${t("discovery.search.resultsUnit")}`
                : `${results.hits.length} ${t("discovery.search.resultsUnit")}`}
              {results.processingTimeMs !== undefined && (
                <span className="ml-2 text-fg/30">({results.processingTimeMs}{t("discovery.search.timingUnit")})</span>
              )}
            </p>
          </div>
        )}
      </div>

      {/* Main Content */}
      <main
        className="flex-1 min-h-0 overflow-y-auto"
        style={{
          paddingBottom: "calc(env(safe-area-inset-bottom) + 24px)",
        }}
      >
        {/* Loading state */}
        {loading && (
          <div className="pt-4">
            <DiscoveryGridSkeleton cardCount={8} />
          </div>
        )}

        {/* Empty state - show recent & trending */}
        {showEmptyState && (
          <div className="space-y-6 px-4 py-6">
            {/* Recent Searches */}
            {recentSearches.length > 0 && (
              <section>
                <div className="mb-3 flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <Clock className="h-4 w-4 text-fg/50" />
                    <h3 className={cn(typography.body.size, "font-semibold text-fg")}>
                      {t("discovery.search.recentSearches")}
                    </h3>
                  </div>
                  <button
                    onClick={handleClearRecent}
                    className="text-xs text-fg/50 hover:text-fg"
                  >
                    {t("discovery.search.clearAll")}
                  </button>
                </div>
                <div className="flex flex-wrap gap-2">
                  {recentSearches.map((search) => (
                    <button
                      key={search.timestamp}
                      onClick={() => handleRecentSearchClick(search.query)}
                      className="flex items-center gap-1.5 rounded-full border border-fg/10 bg-fg/5 px-3 py-1.5 text-sm text-fg/70 transition-all hover:border-fg/20 hover:bg-fg/10 hover:text-fg active:scale-95"
                    >
                      <Clock className="h-3 w-3" />
                      {search.query}
                    </button>
                  ))}
                </div>
              </section>
            )}

            {/* Trending Searches */}
            <section>
              <div className="mb-3 flex items-center gap-2">
                <TrendingUp className="h-4 w-4 text-danger" />
                <h3 className={cn(typography.body.size, "font-semibold text-fg")}>
                  {t("discovery.search.trendingSearches")}
                </h3>
              </div>
              <div className="flex flex-wrap gap-2">
                {TRENDING_SEARCHES.map((term) => (
                  <button
                    key={term}
                    onClick={() => handleTrendingClick(term)}
                    className="flex items-center gap-1.5 rounded-full border border-danger/20 bg-danger/10 px-3 py-1.5 text-sm text-danger transition-all hover:border-danger/30 hover:bg-danger/20 active:scale-95"
                  >
                    <Sparkles className="h-3 w-3" />
                    {term}
                  </button>
                ))}
              </div>
            </section>

            {/* Tips */}
            <section className="rounded-xl border border-fg/10 bg-fg/5 p-4">
              <h3 className={cn(typography.body.size, "mb-2 font-semibold text-fg")}>
                {t("discovery.search.tips.title")}
              </h3>
              <ul className="space-y-1.5 text-xs text-fg/60">
                <li>• {t("discovery.search.tips.tip1")}</li>
                <li>• {t("discovery.search.tips.tip2")}</li>
                <li>• {t("discovery.search.tips.tip3")}</li>
              </ul>
            </section>
          </div>
        )}

        {/* Search Results */}
        <AnimatePresence mode="wait">
          {showResults && (
            <motion.div
              key="results"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="px-4 pt-4"
            >
              <div className="grid grid-cols-2 gap-3 lg:grid-cols-3 xl:grid-cols-4">
                {results.hits.map((card, index) => (
                  <DiscoveryCard
                    key={card.id}
                    card={card}
                    onClick={handleCardClick}
                    index={index}
                  />
                ))}
              </div>

              {/* Load more */}
              {hasMore && (
                <div className="flex justify-center py-6">
                  <button
                    onClick={loadMore}
                    disabled={loadingMore}
                    className="flex items-center gap-2 rounded-xl border border-fg/15 bg-fg/5 px-6 py-2.5 text-sm font-medium text-fg transition-all hover:border-fg/25 hover:bg-fg/10 active:scale-95 disabled:opacity-50"
                  >
                    {loadingMore ? (
                      <>
                        <Loader2 className="h-4 w-4 animate-spin" />
                        {t("discovery.search.loading")}
                      </>
                    ) : (
                      t("discovery.search.loadMore")
                    )}
                  </button>
                </div>
              )}
            </motion.div>
          )}

          {/* No results */}
          {showNoResults && (
            <motion.div
              key="no-results"
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0 }}
              className="flex flex-col items-center justify-center px-6 py-20"
            >
              <div className="mb-4 flex h-16 w-16 items-center justify-center rounded-2xl border border-fg/10 bg-fg/5">
                <Search className="h-8 w-8 text-fg/30" />
              </div>
              <h3 className="mb-2 text-lg font-semibold text-fg">{t("discovery.search.noResults")}</h3>
              <p className="mb-4 text-center text-sm text-fg/50">
                {t("discovery.search.noResultsFor")} "{query}"
              </p>
              <p className="text-xs text-fg/40">{t("discovery.search.noResultsHint")}</p>
            </motion.div>
          )}

          {/* Error state */}
          {error && (
            <motion.div
              key="error"
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0 }}
              className="flex flex-col items-center justify-center px-6 py-20"
            >
              <p className="text-sm text-danger">{error}</p>
            </motion.div>
          )}
        </AnimatePresence>
      </main>
    </div>
  );
}

export default DiscoverySearchPage;
