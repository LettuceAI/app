import { useCallback, useMemo, useState } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { ConversationView, MessageWindow, TimelineMessage } from "@/api/generated/bindings";
import { flattenPages } from "./messages";
import { timelineQuery, windowQuery } from "./queries";

interface Extension {
  anchorId: string;
  base: MessageWindow;
  older: TimelineMessage[];
  olderCursor: string | null;
  newer: TimelineMessage[];
  newerCursor: string | null;
}

export interface Timeline {
  view: ConversationView | undefined;
  messages: TimelineMessage[];
  pending: boolean;
  error: unknown;
  windowError: unknown;
  refetch: () => void;
  hasOlder: boolean;
  loadingOlder: boolean;
  loadOlder: () => void;
  hasNewer: boolean;
  loadNewer: () => void;
  windowed: boolean;
  jumpTo: (messageId: string) => void;
  jumpToLatest: () => void;
}

/** The messages on screen: the newest pages, or a window around a search hit that is not among them. */
export function useTimeline(api: ApiClient, conversationId: string): Timeline {
  const main = useInfiniteQuery(timelineQuery(api, conversationId));
  const [anchorId, setAnchorId] = useState<string | null>(null);
  const around = useQuery(windowQuery(api, conversationId, anchorId));
  const [extension, setExtension] = useState<Extension | null>(null);
  const [loadingMore, setLoadingMore] = useState(false);

  const view = main.data?.pages[0]?.view ?? undefined;

  if (anchorId === null && extension !== null) setExtension(null);
  if (anchorId !== null && around.data) {
    if (extension === null || extension.anchorId !== anchorId) {
      setExtension({ anchorId, base: around.data, older: [], olderCursor: around.data.before_cursor, newer: [], newerCursor: around.data.after_cursor });
    } else if (extension.base !== around.data) {
      setExtension({ ...extension, base: around.data });
    }
  }

  const mainMessages = useMemo(() => (main.data ? flattenPages(main.data.pages) : []), [main.data]);

  const windowMessages = useMemo(() => {
    if (anchorId === null || !extension || extension.anchorId !== anchorId) return null;
    const seen = new Set<string>();
    return [...extension.older, ...extension.base.items, ...extension.newer].filter((message) => {
      if (seen.has(message.id)) return false;
      seen.add(message.id);
      return true;
    });
  }, [anchorId, extension]);

  const loadWindowEdge = useCallback(
    async (direction: "older" | "newer") => {
      if (!extension || loadingMore) return;
      const cursor = direction === "older" ? extension.olderCursor : extension.newerCursor;
      if (cursor === null) return;
      setLoadingMore(true);
      try {
        const page = await api.call("conversationMessages", {
          conversation_id: conversationId,
          before_cursor: direction === "older" ? cursor : null,
          after_cursor: direction === "newer" ? cursor : null,
          limit: null,
        });
        setExtension((current) => {
          if (!current || current.anchorId !== extension.anchorId) return current;
          return direction === "older"
            ? { ...current, older: [...page.items, ...current.older], olderCursor: page.next_cursor }
            : { ...current, newer: [...current.newer, ...page.items], newerCursor: page.next_cursor };
        });
      } finally {
        setLoadingMore(false);
      }
    },
    [api, conversationId, extension, loadingMore],
  );

  const windowed = windowMessages !== null;
  const loadOlder = useCallback(() => {
    if (windowed) void loadWindowEdge("older");
    else if (main.hasNextPage && !main.isFetching) void main.fetchNextPage();
  }, [windowed, loadWindowEdge, main]);

  return {
    view,
    messages: windowMessages ?? mainMessages,
    pending: main.isPending,
    error: main.error,
    windowError: anchorId !== null ? around.error : null,
    refetch: () => void main.refetch(),
    hasOlder: windowed ? extension?.olderCursor != null : (main.hasNextPage ?? false),
    loadingOlder: windowed ? loadingMore : main.isFetchingNextPage,
    loadOlder,
    hasNewer: windowed && extension?.newerCursor != null,
    loadNewer: () => void loadWindowEdge("newer"),
    windowed,
    jumpTo: setAnchorId,
    jumpToLatest: () => setAnchorId(null),
  };
}
