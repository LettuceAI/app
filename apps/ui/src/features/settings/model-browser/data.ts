import { useEffect, useState } from "react";
import { queryOptions } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type {
  HfCanonicalPlan,
  HfPlanGpuMode,
  HfRecommendationRequest,
  HfSort,
  ImageComponentRole,
  ProviderAccountView,
} from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

export function fitSettingsKey(
  request: Pick<HfRecommendationRequest, "gpu_policy" | "sidecars" | "plan">,
) {
  const sidecars = request.sidecars;
  return JSON.stringify({
    gpu_policy: request.gpu_policy ?? null,
    sidecars: sidecars?.mmproj || sidecars?.mtp || sidecars?.mtp_bundled ? sidecars : null,
    context_length: request.plan?.context_length ?? 8192,
    model_offload: request.plan?.model_offload ?? "auto",
    kv_placement: request.plan?.kv_placement ?? "auto",
    kv_type: request.plan?.kv_type ?? "q8_0",
  });
}

export const pageSize = 30;
export const browserKey = ["hf-browser"] as const;
export const bundleProfilesQuery = (api: ApiClient) =>
  queryOptions({
    queryKey: [...queryKeys.localImages.all, "bundle-profiles"],
    queryFn: () => api.call("hfImageBundleProfiles"),
  });
export const bundleSearchQuery = (
  api: ApiClient,
  profile: string,
  role: ImageComponentRole,
  query: string,
  sort: HfSort = "downloads",
  format: string | null = null,
) =>
  queryOptions({
    queryKey: [...queryKeys.localImages.all, "bundle-search", profile, role, query, sort, format],
    queryFn: async () => {
      const formats = format === null && role !== "vae" ? ["gguf", "safetensors"] : [format];
      const pages = await Promise.all(
        formats.map((format) =>
          api.call("hfImageBundleSearch", {
            profile_id: profile,
            role,
            query,
            sort,
            author: null,
            format,
          }),
        ),
      );
      const models = new Map(
        pages.flatMap((page) => page.models).map((model) => [model.model_id, model]),
      );
      return { models: [...models.values()] };
    },
  });
export const bundleFilesQuery = (
  api: ApiClient,
  profile: string,
  role: ImageComponentRole,
  repo: string,
) =>
  queryOptions({
    queryKey: [...queryKeys.localImages.all, "bundle-files", profile, role, repo],
    queryFn: () =>
      api.call("hfImageBundleFiles", {
        profile_id: profile,
        role,
        model_id: repo,
      }),
    enabled: Boolean(repo),
  });
export function hasSprout(account: Pick<ProviderAccountView, "config"> | undefined) {
  if (!account) return true;
  const sprout = account.config.sprout;
  return (
    typeof sprout === "object" &&
    sprout !== null &&
    "enabled" in sprout &&
    sprout.enabled === true &&
    "url" in sprout &&
    typeof sprout.url === "string" &&
    Boolean(sprout.url.trim())
  );
}
export function repoFromLink(value: string, repositoryBase: string) {
  try {
    const url = new URL(value.trim());
    if (
      url.hostname !== new URL(repositoryBase).hostname ||
      !["https:", "http:"].includes(url.protocol)
    )
      return null;
    const parts = url.pathname.split("/").filter(Boolean);
    return parts.length >= 2 ? `${parts[0]}/${parts[1]}` : null;
  } catch {
    return null;
  }
}
export function parameterBillions(repo: string) {
  const match = /(?:^|[-_])(\d+(?:\.\d+)?)b(?:[-_]|$)/i.exec(repo.split("/").at(-1) ?? repo);
  return match ? Number(match[1]) : null;
}
export function contextValue(value: string, max: number) {
  const input = value.trim().replaceAll(",", "");
  if (!/^\d+(?:\.\d+)?k?$/i.test(input)) return null;
  const amount = Number.parseFloat(input) * (/k$/i.test(input) ? 1024 : 1);
  return Math.min(max, Math.max(Math.min(2048, max), Math.round(amount / 256) * 256));
}
export const fitTone = (label: string) =>
  label === "excellent"
    ? "accent"
    : label === "good"
      ? "info"
      : label === "marginal"
        ? "warning"
        : "danger";

export function useBrowserDebounce<T>(value: T, delay = 180) {
  const [settled, setSettled] = useState(value);
  useEffect(() => {
    const timer = window.setTimeout(() => setSettled(value), delay);
    return () => window.clearTimeout(timer);
  }, [value, delay]);
  return settled;
}

export const repositoryUrl = (base: string, repo: string) =>
  new URL(repo, `${base.replace(/\/+$/, "")}/`).href;

export type PlacementKey =
  | "full"
  | "near_full"
  | "kv_spill"
  | "most_layers"
  | "half_layers"
  | "few_layers"
  | "cpu"
  | "shared";

const modeKeys: Record<HfPlanGpuMode, Exclude<PlacementKey, "shared">> = {
  full: "full",
  near_full: "near_full",
  kv_spill: "kv_spill",
  kv_heavy_spill: "kv_spill",
  ram_model_vram_ctx: "few_layers",
  ram_model_ram_ctx: "cpu",
  most_layers: "most_layers",
  half_layers: "half_layers",
  few_layers: "few_layers",
  cpu: "cpu",
  gpu_unavailable: "cpu",
};

export const placementKey = (plan: HfCanonicalPlan): PlacementKey =>
  plan.unified_memory ? "shared" : modeKeys[plan.gpu_mode];

export const runStatus = (label: string) =>
  label === "unrunnable" ? "no" : label === "excellent" || label === "good" ? "yes" : "borderline";

export function prettyPipeline(tag: string | null) {
  if (!tag) return null;
  const text = tag.replaceAll("-", " ");
  return text.charAt(0).toUpperCase() + text.slice(1);
}

export const contextTokens = (value: number) =>
  value >= 1024 ? `${Math.round(value / 1024)}K` : String(value);

export const separator = "·";
