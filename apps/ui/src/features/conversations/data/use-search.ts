import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { SearchHit, SearchHitPage } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

const debounceMs = 250;

async function allHits(api: ApiClient, conversationId: string, query: string): Promise<SearchHit[]> {
  const hits: SearchHit[] = [];
  let cursor: string | null = null;
  do {
    const page: SearchHitPage = await api.call("conversationSearch", { conversation_id: conversationId, query, cursor, limit: null });
    hits.push(...page.items);
    cursor = page.next_cursor;
  } while (cursor !== null);
  return hits;
}

export interface Search {
  open: boolean;
  text: string;
  query: string;
  hits: SearchHit[];
  index: number;
  current: SearchHit | null;
  start: () => void;
  stop: () => void;
  setText: (text: string) => void;
  step: (direction: -1 | 1) => void;
}

/** The header search: debounced hits for the typed text, newest hit first in line, stepped with up and down. */
export function useSearch(api: ApiClient, conversationId: string): Search {
  const [open, setOpen] = useState(false);
  const [text, setText] = useState("");
  const [query, setQuery] = useState("");
  const [picked, setPicked] = useState<{ query: string; index: number } | null>(null);

  useEffect(() => {
    const timer = window.setTimeout(() => setQuery(text.trim()), debounceMs);
    return () => window.clearTimeout(timer);
  }, [text]);

  const result = useQuery({
    queryKey: [...queryKeys.conversations.detail(conversationId), "search", query] as const,
    queryFn: () => allHits(api, conversationId, query),
    enabled: open && query.length > 0,
  });

  const hits = open && query.length > 0 ? (result.data ?? []) : [];
  const index = picked && picked.query === query ? Math.min(picked.index, hits.length - 1) : hits.length - 1;
  const current = hits[index] ?? null;

  return {
    open,
    text,
    query: open ? query : "",
    hits,
    index,
    current,
    start: () => setOpen(true),
    stop: () => {
      setOpen(false);
      setText("");
      setQuery("");
      setPicked(null);
    },
    setText,
    step: (direction) => {
      if (hits.length === 0) return;
      setPicked({ query, index: (index + direction + hits.length) % hits.length });
    },
  };
}
