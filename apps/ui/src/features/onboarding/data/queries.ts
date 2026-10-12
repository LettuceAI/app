import { queryOptions } from "@tanstack/react-query";
import { useRouteContext } from "@tanstack/react-router";
import type { ApiClient } from "@/api/client";
import { queryKeys } from "@/api/query-keys";

export function useApi(): ApiClient {
  return useRouteContext({ from: "__root__", select: (context) => context.api });
}

export function catalogQuery(api: ApiClient) {
  return queryOptions({
    queryKey: queryKeys.providers.catalog(),
    queryFn: async () => (await api.call("providerCatalog")).providers,
  });
}

export function accountsQuery(api: ApiClient) {
  return queryOptions({
    queryKey: queryKeys.providers.accounts(),
    queryFn: () => api.call("providerAccountsList"),
  });
}

export function providerModelsQuery(api: ApiClient, accountId: string) {
  return queryOptions({
    queryKey: queryKeys.providers.models(accountId),
    queryFn: () => api.call("providerModels", { account_id: accountId }),
  });
}

export function embeddingStatusQuery(api: ApiClient) {
  return queryOptions({
    queryKey: queryKeys.embedding.status(),
    queryFn: () => api.call("embeddingStatus"),
  });
}
