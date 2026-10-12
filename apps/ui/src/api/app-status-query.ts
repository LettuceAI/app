import { queryOptions } from "@tanstack/react-query";
import type { ApiClient } from "./client";
import { queryKeys } from "./query-keys";

export function appStatusQuery(api: ApiClient) {
  return queryOptions({
    queryKey: queryKeys.app.status(),
    queryFn: () => api.call("appStatus"),
  });
}
