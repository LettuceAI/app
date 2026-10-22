import { useQueries } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import { ApiFailure } from "@/api/client";
import type { SdCatalogProfile, SdRunnability, SdRuntimeRef } from "@/api/generated/bindings";
import { watchJob } from "@/entities/job";
import { useSettingsApi } from "../data";
import { localImageKeys } from "./local-data";
import type { FitState } from "./fit";

async function runFit(api: ApiClient, profile: string, variant: string, runtime: SdRuntimeRef, signal: AbortSignal): Promise<SdRunnability> {
  const accepted = await api.call("sdRunnability", { profile_id: profile, variant_id: variant, runtime_release: runtime.release, runtime_asset: runtime.asset, width: null, height: null, reference_image_count: null, loras: [], prompt: null, negative_prompt: null, sample_steps: null, cfg_scale: null, seed: null, sample_method: null, batch_count: null, full_execution: false, client_operation_id: crypto.randomUUID() });
  const settled = await watchJob(api, accepted.job_id, () => undefined, signal);
  if (settled.state === "succeeded" && settled.result?.type === "runnability") return settled.result.verdict;
  throw new ApiFailure("unavailable", "fit check failed", null);
}

/** Checks every variant that is not installed yet as soon as it is enabled; installed ones wait for `test`. */
export function useVariantFits({ profile, runtime, enabled, epoch, installed }: { profile: SdCatalogProfile; runtime: SdRuntimeRef | null; enabled: boolean; epoch: number; installed: ReadonlySet<string> }) {
  const api = useSettingsApi();
  const queries = useQueries({
    queries: profile.variants.map((variant) => ({
      queryKey: localImageKeys.fit(profile.id, variant.id, runtime?.release ?? "", runtime?.asset ?? "", epoch),
      queryFn: ({ signal }: { signal: AbortSignal }) => runFit(api, profile.id, variant.id, runtime!, signal),
      enabled: enabled && runtime !== null && !installed.has(variant.id),
      staleTime: Infinity,
      retry: false,
    })),
  });
  const fits = new Map<string, FitState>();
  profile.variants.forEach((variant, index) => {
    const query = queries[index]!;
    if (query.data) fits.set(variant.id, { state: "ready", verdict: query.data });
    else if (query.isError) fits.set(variant.id, { state: "error" });
    else if (query.isFetching) fits.set(variant.id, { state: "checking" });
  });
  const run = (variantId: string) => { void queries[profile.variants.findIndex((variant) => variant.id === variantId)]?.refetch(); };
  return { fits, run };
}
