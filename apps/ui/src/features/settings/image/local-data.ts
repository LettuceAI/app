import { queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { fileSize, toast } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { CivitaiAuthStatus, JobPage, JobSubjectDetail, JobView, LlamaDevice, SdRuntimeAsset } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { useSettingsApi } from "../data";

export type ImageInstallDetail = Extract<JobSubjectDetail, { type: "sd_runtime_install" | "sd_upscaler_install" | "sd_model_install" }>;

export const localImageKeys = {
  inventory: [...queryKeys.localImages.all, "inventory"],
  installed: [...queryKeys.localImages.all, "installed"],
  catalog: [...queryKeys.localImages.all, "catalog"],
  releases: [...queryKeys.localImages.all, "releases"],
  upscaler: [...queryKeys.localImages.all, "upscaler"],
  disk: [...queryKeys.localImages.all, "disk"],
  fit: (profile: string, variant: string, release: string, asset: string, epoch: number) => ["local-image-fit", profile, variant, release, asset, epoch],
  policy: (release: string, asset: string) => [...queryKeys.localImages.all, "policy", release, asset],
};

export function inventoryQuery(api: ApiClient) {
  return queryOptions({ queryKey: localImageKeys.inventory, queryFn: () => api.call("sdRuntimeInventory") });
}

export function installedQuery(api: ApiClient) {
  return queryOptions({ queryKey: localImageKeys.installed, queryFn: () => api.call("sdModelsInstalled") });
}

export function useImageAction() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (run: () => Promise<unknown>) => run(),
    onSuccess: async () => {
      await Promise.all([queryKeys.localImages.all, queryKeys.models.all, queryKeys.loras.all, queryKeys.jobs.lists()].map((queryKey) => client.invalidateQueries({ queryKey })));
    },
  });
}

export function useByteSize() {
  const { i18n } = useTranslation("settings");
  return (bytes: number) => fileSize(bytes, i18n.language);
}

export function useCleanupWarning() {
  const { t } = useTranslation("settings");
  return (left: string[]) => { if (left.length > 0) toast.warning(t("image.local.cleanup", { count: left.length })); };
}

export function useCopyText() {
  const { t } = useTranslation("settings");
  return (value: string) => {
    void navigator.clipboard.writeText(value).then(() => toast.success(t("image.local.copied")), () => toast.error(t("image.local.copyFailed")));
  };
}

export function recommendedAsset(platform: string, devices: LlamaDevice[], assets: SdRuntimeAsset[]) {
  const discrete = devices.filter((device) => !["integratedgpu", "cpu"].includes(device.device_type.toLowerCase()));
  const hardware = discrete.map((device) => `${device.name} ${device.description} ${device.backend}`.toLowerCase()).join(" ");
  const amd = /\bamd\b|radeon|rocm/.test(hardware);
  const nvidia = /nvidia|cuda/.test(hardware);
  const priorities = platform === "linux" ? amd ? ["rocm", "vulkan", "cpu"] : discrete.length ? ["vulkan", nvidia ? "cuda" : "rocm", "cpu"] : ["cpu", "vulkan"]
    : platform === "macos" ? ["metal", "cpu"] : amd ? ["rocm", "vulkan", "cpu"] : nvidia ? ["cuda", "vulkan", "cpu"] : discrete.length ? ["vulkan", "cpu"] : ["cpu", "vulkan", "cuda", "rocm", "metal"];
  return priorities.map((backend) => assets.find((asset) => asset.backend.toLowerCase() === backend)).find(Boolean) ?? assets[0] ?? null;
}

export async function activeImageDownloads(api: ApiClient): Promise<JobView[]> {
  const jobs: JobView[] = [];
  let cursor: string | null = null;
  do {
    const page: JobPage = await api.call("jobsList", { kinds: ["artifact_install"], states: ["queued", "claimed", "running", "cancellation_requested", "cleaning_up"], subject: null, cursor, limit: 100 });
    jobs.push(...page.items);
    cursor = page.next_cursor;
  } while (cursor);
  return jobs;
}

const civitaiImageHost = "image.civitai.com";
const civitaiSizedSegment = /\/(?:original=true|width=\d+)[^/]*\//;
export const civitaiWidths = { card: 450, hero: 1024 } as const;
export const loraDownloadKey = (versionId: number, fileId: number) => `${versionId}/${fileId}`;
export const civitaiModelUrl = (id: number) => `https://civitai.com/models/${id}`;

export function civitaiImage(url: string, width: number) {
  if (width <= 0) return url;
  try {
    if (new URL(url).hostname !== civitaiImageHost) return url;
  } catch {
    return url;
  }
  return url.replace(civitaiSizedSegment, `/anim=false,width=${width}/`);
}

export function usePureMode() {
  const api = useSettingsApi();
  return useQuery({ queryKey: queryKeys.settings.detail(), queryFn: () => api.call("settingsGet"), select: (settings) => settings.global.pure_mode }).data;
}

export function useCivitaiAuth() {
  const api = useSettingsApi();
  return useQuery({ queryKey: queryKeys.civitai.auth(), queryFn: () => api.call("civitaiAuthStatus") });
}

export const civitaiTokenNeeded = (status: CivitaiAuthStatus | undefined) => status !== undefined && (!status.saved || status.error_kind === "invalid_or_expired");
