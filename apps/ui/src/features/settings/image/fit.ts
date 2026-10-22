import type { SdCatalogVariant, SdFitEstimate, SdRunnability } from "@/api/generated/bindings";

export type FitKind = "gpu" | "phased" | "split" | "cpu" | "estimated" | "verified" | "failed" | "inconclusive" | "update";

const runsFromRam = (estimate: SdFitEstimate) => estimate.plan_mode === "default_backend" || estimate.placements.some((placement) => placement.cpu);

export function fitKind(verdict: SdRunnability): FitKind {
  switch (verdict.status) {
    case "passed":
      return "verified";
    case "failed":
      return "failed";
    case "incompatible_runtime":
      return "update";
    case "inconclusive":
    case "not_installed":
      return "inconclusive";
    case "estimated_runnable":
    case "cpu_fallback": {
      const estimate = verdict.estimate;
      if (verdict.status === "cpu_fallback" || (estimate && runsFromRam(estimate))) return "cpu";
      if (!estimate) return "estimated";
      if (estimate.placements.some((placement) => placement.split)) return "split";
      return estimate.plan_mode === "time_share" ? "phased" : "gpu";
    }
  }
}

export const fitsGpu = (kind: FitKind) => kind === "gpu" || kind === "phased" || kind === "split" || kind === "estimated" || kind === "verified";

export function fitMemory(estimate: SdFitEstimate) {
  const sum = (values: number[]) => values.reduce((total, value) => total + value, 0);
  const onGpu = estimate.placements.filter((placement) => !placement.cpu);
  return {
    gpuWeights: sum(onGpu.map((placement) => placement.params_bytes)),
    gpuNeed: sum(onGpu.map((placement) => placement.params_bytes + placement.compute_reserve_bytes)),
    ramWeights: sum(estimate.placements.filter((placement) => placement.cpu).map((placement) => placement.params_bytes)),
    gpuBudget: sum(estimate.devices.map((device) => device.budget_bytes)),
  };
}

export type FitState = { state: "checking" } | { state: "error" } | { state: "ready"; verdict: SdRunnability };

export function variantName(label: string) {
  return label.replace(/\s*\((?:recommended|smaller)\)\s*$/i, "");
}

/**
 * The variant to suggest on this computer: the catalog's recommendation when it fits the GPU, otherwise the largest
 * smaller variant that does, otherwise the smallest. Null while any candidate is still being checked, or when none
 * could be.
 */
export function pickVariant(variants: readonly SdCatalogVariant[], fits: ReadonlyMap<string, FitState>): { id: string; fits: boolean } | null {
  const catalogPick = variants.find((variant) => variant.recommended) ?? variants[0];
  if (!catalogPick) return null;
  const candidates = variants.filter((variant) => variant.download_bytes <= catalogPick.download_bytes).toSorted((a, b) => b.download_bytes - a.download_bytes);
  const states = candidates.map((variant) => fits.get(variant.id));
  if (states.some((state) => state === undefined || state.state === "checking")) return null;
  if (states.every((state) => state?.state !== "ready")) return null;
  const fitting = candidates.find((variant) => {
    const state = fits.get(variant.id);
    return state?.state === "ready" && fitsGpu(fitKind(state.verdict));
  });
  return fitting ? { id: fitting.id, fits: true } : { id: candidates.at(-1)!.id, fits: false };
}
