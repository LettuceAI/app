import { useCallback } from "react";
import type { ApiClient } from "@/api/client";
import { useJobInstall } from "@/entities/job";

export type { InstallPhase, InstallState } from "@/entities/job";

export function useEmbeddingInstall(api: ApiClient, enableDynamicMemory = true) {
  const accept = useCallback(() => api.call("embeddingInstall", { family: "lettuce_eidos_v5", enable_dynamic_memory: enableDynamicMemory }), [api, enableDynamicMemory]);
  return useJobInstall(api, accept);
}
