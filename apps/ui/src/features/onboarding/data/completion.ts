import type { QueryClient } from "@tanstack/react-query";
import { appStatusQuery } from "@/api/app-status-query";
import type { ApiClient } from "@/api/client";
import { queryKeys } from "@/api/query-keys";

export interface Services {
  api: ApiClient;
  queryClient: QueryClient;
}

export async function completeOnboarding({ api, queryClient }: Services): Promise<void> {
  const status = await queryClient.ensureQueryData(appStatusQuery(api));
  await api.call("appUiStateUpdate", {
    patch: { onboarding: { completed: true }, lastSeenAppVersion: status.version },
  });
  await queryClient.invalidateQueries({ queryKey: queryKeys.app.status() });
}

const defaultMinSimilarityBasisPoints = 3200;

export async function applyManualMemory({ api }: Services): Promise<void> {
  const settings = await api.call("settingsGet");
  const memory = settings.global.dynamic_memory;
  if (!memory.enabled) return;
  await api.call("settingsUpdate", {
    patch: { section: "dynamic_memory", value: { ...memory, enabled: false } },
    expected_revision: settings.revision,
  });
}

export async function enableAutomaticMemory({ api }: Services, memoryModelId: string | null, enable: boolean): Promise<void> {
  let settings = await api.call("settingsGet");
  if (enable) {
    const memory = settings.global.dynamic_memory;
    settings = await api.call("settingsUpdate", {
      patch: {
        section: "dynamic_memory",
        value: {
          ...memory,
          enabled: true,
          min_similarity_basis_points: memory.min_similarity_basis_points ?? defaultMinSimilarityBasisPoints,
        },
      },
      expected_revision: settings.revision,
    });
  }
  if (memoryModelId && settings.dynamic_memory_model_profile_id !== memoryModelId) {
    await api.call("settingsUpdate", {
      patch: {
        section: "selections",
        default_model: null,
        default_prompt: null,
        dynamic_memory_model: { type: "set", id: memoryModelId },
        group_speaker_model: null,
      },
      expected_revision: settings.revision,
    });
  }
}

export async function hasEidosInstalled({ api }: Services): Promise<boolean> {
  const models = await api.call("embeddingStatus");
  return models.some((model) => model.family === "lettuce_eidos_v5");
}
