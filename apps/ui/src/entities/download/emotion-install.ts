import { useCallback } from "react";
import { queryOptions, useQueryClient, type QueryClient } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import { queryKeys } from "@/api/query-keys";
import { useJobInstall } from "@/entities/job";

export function emotionStatusQuery(api: ApiClient) {
  return queryOptions({ queryKey: queryKeys.companions.emotion(), queryFn: () => api.call("companionEmotionStatus") });
}

export async function installEmotionModel(api: ApiClient, client: QueryClient) {
  const accepted = await api.call("companionEmotionInstall");
  void client.invalidateQueries({ queryKey: queryKeys.downloads.list() });
  return accepted;
}

export async function refreshEmotionModel(client: QueryClient): Promise<void> {
  await Promise.all([queryKeys.companions.all, queryKeys.downloads.list(), queryKeys.conversations.all].map((queryKey) => client.invalidateQueries({ queryKey })));
}

export function useEmotionInstall(api: ApiClient) {
  const client = useQueryClient();
  const accept = useCallback(() => installEmotionModel(api, client), [api, client]);
  return useJobInstall(api, accept);
}
