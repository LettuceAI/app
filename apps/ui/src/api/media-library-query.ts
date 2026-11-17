import { infiniteQueryOptions } from "@tanstack/react-query";
import type { ApiClient } from "./client";
import { queryKeys } from "./query-keys";

export const libraryImagePageSize = 32;

export function mediaQuery(api: ApiClient, kind: "image" | "audio", query: string) {
  const limit = kind === "image" ? libraryImagePageSize : 24;
  return infiniteQueryOptions({
    queryKey: [...queryKeys.media.library(), "metadata", kind, query, limit] as const,
    initialPageParam: null as string | null,
    queryFn: ({ pageParam }) =>
      api.call("mediaLibraryList", {
        cursor: pageParam,
        limit,
        kind,
        query: query || null,
        include_references: false,
      }),
    getNextPageParam: (last) => last.next_cursor ?? undefined,
  });
}
