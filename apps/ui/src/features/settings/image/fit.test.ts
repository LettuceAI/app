import { describe, expect, it } from "vitest";
import type { SdCatalogVariant, SdFitEstimate, SdFitPlacement, SdRunnability } from "@/api/generated/bindings";
import { fitKind, fitMemory, pickVariant, variantName, type FitState } from "./fit";

const placement = (patch: Partial<SdFitPlacement> = {}): SdFitPlacement => ({ component: "dit", params_bytes: 100, compute_reserve_bytes: 10, targets: ["GPU"], cpu: false, split: false, ...patch });
const estimate = (patch: Partial<SdFitEstimate> = {}): SdFitEstimate => ({ model_bytes: 100, available_ram_bytes: 1000, plan_mode: "concurrent", devices: [], placements: [placement()], ...patch });
const verdict = (status: SdRunnability["status"], value: SdFitEstimate | null = estimate()): SdRunnability => ({ status, method: "estimate", exact: false, scope: "model", placement_policy: "auto", elapsed_ms: null, reason: "diagnostic", estimate: value });
const variant = (id: string, bytes: number, recommended = false): SdCatalogVariant => ({ id, label: id, description: "", download_bytes: bytes, installed: false, recommended, smaller: false });
const ready = (kind: "gpu" | "cpu"): FitState => ({ state: "ready", verdict: verdict("estimated_runnable", kind === "cpu" ? estimate({ placements: [placement({ cpu: true, targets: ["CPU"] })] }) : estimate()) });

describe("fitKind", () => {
  it("reads a catalog estimate that places a component on the CPU as CPU offload", () => {
    expect(fitKind(verdict("estimated_runnable", estimate({ placements: [placement(), placement({ component: "vae", cpu: true, targets: ["CPU"] })] })))).toBe("cpu");
    expect(fitKind(verdict("estimated_runnable", estimate({ plan_mode: "default_backend" })))).toBe("cpu");
    expect(fitKind(verdict("cpu_fallback"))).toBe("cpu");
  });

  it("tells a GPU fit from a phased and a split one", () => {
    expect(fitKind(verdict("estimated_runnable"))).toBe("gpu");
    expect(fitKind(verdict("estimated_runnable", estimate({ plan_mode: "time_share" })))).toBe("phased");
    expect(fitKind(verdict("estimated_runnable", estimate({ placements: [placement({ split: true })] })))).toBe("split");
    expect(fitKind(verdict("estimated_runnable", null))).toBe("estimated");
  });

  it("maps the probe outcomes", () => {
    expect(fitKind(verdict("passed"))).toBe("verified");
    expect(fitKind(verdict("failed"))).toBe("failed");
    expect(fitKind(verdict("inconclusive", null))).toBe("inconclusive");
    expect(fitKind(verdict("incompatible_runtime", null))).toBe("update");
  });
});

describe("fitMemory", () => {
  it("splits weights between the GPU and RAM and totals the GPU need and budget", () => {
    const value = estimate({ devices: [{ id: 0, name: "a", description: "", total_bytes: 900, free_bytes: 800, budget_bytes: 700 }], placements: [placement({ params_bytes: 50, compute_reserve_bytes: 5 }), placement({ component: "vae", params_bytes: 30, compute_reserve_bytes: 3 }), placement({ component: "conditioner", params_bytes: 20, compute_reserve_bytes: 2, cpu: true })] });
    expect(fitMemory(value)).toEqual({ gpuWeights: 80, gpuNeed: 88, ramWeights: 20, gpuBudget: 700 });
  });
});

describe("variantName", () => {
  it("drops the recommendation suffix the catalog puts in labels", () => {
    expect(variantName("Q4 K (recommended)")).toBe("Q4 K");
    expect(variantName("Q3 K (smaller)")).toBe("Q3 K");
    expect(variantName("Q8 (high quality)")).toBe("Q8 (high quality)");
  });
});

describe("pickVariant", () => {
  const variants = [variant("q3", 5), variant("q4", 6, true), variant("q8", 9)];

  it("keeps the catalog recommendation when it fits the GPU, even if a larger one fits too", () => {
    const fits = new Map<string, FitState>([["q3", ready("gpu")], ["q4", ready("gpu")], ["q8", ready("gpu")]]);
    expect(pickVariant(variants, fits)).toEqual({ id: "q4", fits: true });
  });

  it("steps down to the largest smaller variant that fits", () => {
    const fits = new Map<string, FitState>([["q3", ready("gpu")], ["q4", ready("cpu")], ["q8", ready("cpu")]]);
    expect(pickVariant(variants, fits)).toEqual({ id: "q3", fits: true });
  });

  it("falls back to the smallest variant, flagged as not fitting, when none fits the GPU", () => {
    const fits = new Map<string, FitState>([["q3", ready("cpu")], ["q4", ready("cpu")], ["q8", ready("cpu")]]);
    expect(pickVariant(variants, fits)).toEqual({ id: "q3", fits: false });
  });

  it("waits while a candidate is still being checked and gives no pick when none could be checked", () => {
    expect(pickVariant(variants, new Map<string, FitState>([["q3", ready("gpu")], ["q4", { state: "checking" }]]))).toBeNull();
    expect(pickVariant(variants, new Map<string, FitState>([["q3", { state: "error" }], ["q4", { state: "error" }]]))).toBeNull();
  });
});
