import { useCallback, useEffect, useState } from "react";
import { useI18n } from "../../../../core/i18n/context";
import {
  searchDiscoveryCards,
  type DiscoverySearchResponse,
} from "../../../../core/discovery";

export interface RecentSearch {
  query: string;
  timestamp: number;
}

const RECENT_SEARCHES_KEY = "discovery_recent_searches";
const MAX_RECENT_SEARCHES = 8;

export function loadRecentSearches(): RecentSearch[] {
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

export function clearRecentSearches() {
  try {
    sessionStorage.removeItem(RECENT_SEARCHES_KEY);
  } catch {
    // Storage unavailable
  }
}

export function useDiscoverySearch(initialQuery = "") {
  const { t } = useI18n();
  const [query, setQuery] = useState(initialQuery);
  const [debouncedQuery, setDebouncedQuery] = useState(initialQuery);
  const [results, setResults] = useState<DiscoverySearchResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [recentSearches, setRecentSearches] = useState<RecentSearch[]>([]);

  useEffect(() => {
    setRecentSearches(loadRecentSearches());
  }, []);

  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedQuery(query);
    }, 400);

    return () => clearTimeout(timer);
  }, [query]);

  useEffect(() => {
    if (!debouncedQuery.trim()) {
      setResults(null);
      setError(null);
      setLoading(false);
      return;
    }

    let cancelled = false;

    const search = async () => {
      setLoading(true);
      setError(null);
      setPage(1);
      setHasMore(true);

      try {
        const response = await searchDiscoveryCards(debouncedQuery, 1, 30);
        if (cancelled) return;
        setResults(response);
        setHasMore(
          response.page !== undefined &&
            response.totalPages !== undefined &&
            response.page < response.totalPages,
        );

        saveRecentSearch(debouncedQuery);
        setRecentSearches(loadRecentSearches());
      } catch (err) {
        if (cancelled) return;
        console.error("Search failed:", err);
        setError(err instanceof Error ? err.message : t("discovery.errors.searchFailed"));
      } finally {
        if (!cancelled) setLoading(false);
      }
    };

    void search();

    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
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

  const clear = useCallback(() => {
    setQuery("");
    setResults(null);
    setError(null);
  }, []);

  const clearRecent = useCallback(() => {
    clearRecentSearches();
    setRecentSearches([]);
  }, []);

  return {
    query,
    setQuery,
    debouncedQuery,
    results,
    loading,
    loadingMore,
    error,
    hasMore,
    loadMore,
    clear,
    recentSearches,
    clearRecent,
  };
}
