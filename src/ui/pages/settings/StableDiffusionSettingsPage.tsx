import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Check,
  ChevronRight,
  Cpu,
  Download,
  HardDrive,
  Loader2,
  RefreshCw,
  Settings2,
  Trash2,
} from "lucide-react";

import {
  useDownloadQueueOptional,
  type QueuedDownload,
} from "../../../core/downloads/DownloadQueueContext";
import { useI18n, type TranslationKey } from "../../../core/i18n/context";
import { openExternalUrl } from "../../../core/utils/openExternal";
import { getPlatform } from "../../../core/utils/platform";
import { confirmBottomMenu } from "../../components/ConfirmBottomMenu";
import { BottomMenu } from "../../components/BottomMenu";
import { toast } from "../../components/toast";
import { cn } from "../../design-tokens";
import { InlineDownloadCards } from "./components/DownloadQueueBar";
import { GuidedTour, useGuidedTour } from "../../components/GuidedTour";

type RuntimeAsset = {
  name: string;
  backend: string;
  bytes: number;
  dependencies: { name: string; bytes: number }[];
};

type RuntimeRelease = {
  tag: string;
  name: string;
  publishedAt: string | null;
  prerelease: boolean;
  assets: RuntimeAsset[];
};

type CatalogVariant = {
  id: string;
  label: string;
  description: string;
  downloadBytes: number;
  installed: boolean;
  recommended: boolean;
  smaller: boolean;
};

type CatalogProfile = {
  id: string;
  displayName: string;
  family: string;
  description: string;
  license: string;
  sourceUrl: string;
  supportsTextToImage: boolean;
  supportsImageEdit: boolean;
  supportsLora: boolean;
  maxReferenceImages: number | null;
  requiresReferenceImage: boolean;
  recommendedForScenes: boolean;
  defaultWidth: number;
  defaultHeight: number;
  defaultSteps: number;
  defaultCfg: number;
  minimumRuntimeBuild: number | null;
  variants: CatalogVariant[];
};

type RuntimeCatalog = {
  runtimeSupported: boolean;
  unsupportedReason: string | null;
  runtimeReleases: RuntimeRelease[];
  profiles: CatalogProfile[];
};

type InstalledModel = {
  profileId: string;
  variantId: string;
  displayName: string;
  runtimeRelease: string | null;
  runtimeAsset: string | null;
  runtimeBackend: string | null;
  componentBytesOnDisk: number;
  modelId: string | null;
  supportsTextToImage: boolean;
  supportsImageEdit: boolean;
  recommendedForScenes: boolean;
  requiresReferenceImage: boolean;
};

type RunnabilityPlacement = {
  component: string;
  paramsBytes: number;
  computeReserveBytes: number;
  targets: string[];
  cpu: boolean;
  split: boolean;
};

type RunnabilityEstimate = {
  modelBytes: number;
  availableRamBytes: number | null;
  planMode: string;
  deviceSource: string;
  devices: ComputeDevice[];
  placements: RunnabilityPlacement[];
};

type ComputeDevice = {
  id: number;
  name: string;
  description: string;
  totalBytes: number;
  freeBytes: number;
  budgetBytes: number;
};

type ComputePolicy = {
  multiGpuEnabled: boolean;
  gpuDeviceIds: number[];
  singleGpuDeviceId: number | null;
  deviceBudgetsGib: Record<string, number>;
  splitMode: "layer" | "row";
};

type ComputeMode = "auto" | "single" | "multi";

function computePolicyMode(policy: ComputePolicy): ComputeMode {
  if (policy.multiGpuEnabled) return "multi";
  if (policy.singleGpuDeviceId !== null) return "single";
  return "auto";
}

type ComputePolicyInfo = {
  runtimeRelease: string;
  runtimeAsset: string;
  backend: string;
  supportsRowSplit: boolean;
  policy: ComputePolicy;
  devices: ComputeDevice[];
};

type Runnability = {
  status: string;
  method: string;
  exact: boolean;
  scope: string;
  placementPolicy: string;
  elapsedMs: number | null;
  reason: string;
  estimate?: RunnabilityEstimate | null;
};

type FitTone = "good" | "ok" | "warn" | "muted";

const FIT_TEXT: Record<FitTone, string> = {
  good: "text-success",
  ok: "text-fg/70",
  warn: "text-warning",
  muted: "text-fg/40",
};

const FIT_DOT: Record<FitTone, string> = {
  good: "bg-success",
  ok: "bg-fg/40",
  warn: "bg-warning",
  muted: "bg-fg/25",
};

function classifyFit(
  fit: Runnability | "loading" | undefined,
): { tone: FitTone; key: string } | null {
  if (fit === undefined) return null;
  if (fit === "loading") return { tone: "muted", key: "fitChecking" };
  switch (fit.status) {
    case "passed":
      return { tone: "good", key: "fitVerified" };
    case "failed":
      return { tone: "warn", key: "fitFailed" };
    case "inconclusive":
      return { tone: "muted", key: "fitInconclusive" };
    case "estimatedRunnable": {
      const estimate = fit.estimate;
      const offloads = Boolean(
        estimate &&
        (estimate.planMode === "defaultBackend" ||
          estimate.placements.some((placement) => placement.cpu)),
      );
      if (offloads) return { tone: "warn", key: "fitCpuOffload" };
      if (estimate?.planMode === "timeShare") return { tone: "ok", key: "fitPhased" };
      return { tone: "good", key: "fitFitsGpu" };
    }
    default:
      return null;
  }
}

function fitOffloadsToCpu(fit: Runnability | "loading" | undefined): boolean {
  if (!fit || fit === "loading" || fit.status !== "estimatedRunnable") return false;
  const estimate = fit.estimate;
  return Boolean(
    estimate &&
    (estimate.planMode === "defaultBackend" ||
      estimate.placements.some((placement) => placement.cpu)),
  );
}

type InstalledRuntime = {
  release: string;
  asset: string;
  backend: string;
  sizeBytes: number;
  active: boolean;
};

type RuntimeInventory = {
  installed: InstalledRuntime[];
  active: { release: string; asset: string } | null;
};

type UpscalerInventory = {
  models: string[];
  hiresUpscalerNames: string[];
  recommendedFilename: string;
  recommendedBytes: number;
  recommendedInstalled: boolean;
};

type GpuDevice = {
  index: number;
  name: string;
  description: string;
  backend: string;
  memoryTotal: number;
  memoryFree: number;
  deviceType: string;
};

type RuntimeInstall = {
  items: QueuedDownload[];
  active: boolean;
};

const focusRing =
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/55 focus-visible:ring-offset-2 focus-visible:ring-offset-bg";

function formatBytes(bytes: number): string {
  const gib = bytes / 1024 ** 3;
  const mib = bytes / 1024 ** 2;
  const value = gib >= 1 ? gib : mib;
  return `${new Intl.NumberFormat(undefined, {
    maximumFractionDigits: gib >= 1 ? 1 : 0,
  }).format(value)} ${gib >= 1 ? "GiB" : "MiB"}`;
}

function totalAssetBytes(asset: RuntimeAsset): number {
  return asset.bytes + asset.dependencies.reduce((total, item) => total + item.bytes, 0);
}

function runtimeBuildNumber(release: string): number | null {
  const match = /^master-(\d+)-/.exec(release);
  if (!match) return null;
  const build = Number.parseInt(match[1], 10);
  return Number.isFinite(build) ? build : null;
}

function runtimeSupportsProfile(profile: CatalogProfile, release: string): boolean {
  if (profile.minimumRuntimeBuild === null) return true;
  const build = runtimeBuildNumber(release);
  return build !== null && build >= profile.minimumRuntimeBuild;
}

function usableGpuDevices(devices: GpuDevice[]): GpuDevice[] {
  return devices.filter(
    (device) =>
      device.deviceType.toLowerCase() !== "integratedgpu" &&
      device.deviceType.toLowerCase() !== "cpu",
  );
}

function recommendAsset(
  os: ReturnType<typeof getPlatform>["os"],
  devices: GpuDevice[],
  assets: RuntimeAsset[],
): RuntimeAsset | null {
  if (assets.length === 0) return null;
  const discrete = usableGpuDevices(devices);
  const hardware = discrete
    .map((device) => `${device.name} ${device.description} ${device.backend}`.toLowerCase())
    .join(" ");
  const isAmd = /\bamd\b|radeon|rocm/.test(hardware);
  const isNvidia = /nvidia|cuda/.test(hardware);

  let priorities: string[];
  if (os === "linux") {
    priorities = isAmd
      ? ["rocm", "vulkan", "cpu"]
      : discrete.length > 0
        ? ["vulkan", isNvidia ? "cuda" : "rocm", "cpu"]
        : ["cpu", "vulkan"];
  } else if (os === "macos") {
    priorities = ["metal", "cpu"];
  } else if (isAmd) {
    priorities = ["rocm", "vulkan", "cpu"];
  } else if (isNvidia) {
    priorities = ["cuda", "vulkan", "cpu"];
  } else if (discrete.length > 0) {
    priorities = ["vulkan", "cpu"];
  } else {
    priorities = ["cpu", "vulkan", "cuda", "rocm", "metal"];
  }

  for (const backend of priorities) {
    const match = assets.find((asset) => asset.backend.toLowerCase() === backend);
    if (match) return match;
  }
  return assets[0];
}

function runtimeInstallGroups(queue: QueuedDownload[]): RuntimeInstall[] {
  const groups = new Map<string, QueuedDownload[]>();
  for (const item of queue) {
    if (item.queueKind !== "sdcpp" || item.installKind !== "runtime") continue;
    const id = item.installId ?? item.id;
    groups.set(id, [...(groups.get(id) ?? []), item]);
  }
  return Array.from(groups.values(), (items) => ({
    items,
    active: items.some((item) => item.status === "queued" || item.status === "downloading"),
  }));
}

function Skeleton({ className }: { className?: string }) {
  return (
    <div
      aria-hidden="true"
      className={cn("animate-pulse rounded-md bg-fg/8 motion-reduce:animate-none", className)}
    />
  );
}

type ModelInstall = {
  installId: string;
  profileId: string;
  variantId: string;
  items: QueuedDownload[];
  active: boolean;
};

function modelInstallGroups(queue: QueuedDownload[]): ModelInstall[] {
  const groups = new Map<string, QueuedDownload[]>();
  for (const item of queue) {
    if (item.queueKind !== "sdcpp") continue;
    if (item.downloadRole === "runtime" || item.downloadRole === "runtime_dependency") continue;
    const id = item.installId ?? item.id;
    if (!id.startsWith("sdcpp:")) continue;
    groups.set(id, [...(groups.get(id) ?? []), item]);
  }
  return Array.from(groups.entries(), ([installId, items]) => {
    const [, profileId = "", variantId = ""] = installId.split(":");
    return {
      installId,
      profileId,
      variantId,
      items,
      active: items.some((item) => item.status === "queued" || item.status === "downloading"),
    };
  });
}

function selectedVariant(
  profile: CatalogProfile,
  selection: Record<string, string>,
): CatalogVariant | null {
  const chosen = profile.variants.find((variant) => variant.id === selection[profile.id]);
  if (chosen) return chosen;
  return profile.variants.find((variant) => variant.recommended) ?? profile.variants[0] ?? null;
}

type FitDetailsSelection = {
  modelName: string;
  variantName: string;
  fit: Runnability;
};

function RunnabilityDetailsMenu({
  selection,
  onClose,
}: {
  selection: FitDetailsSelection | null;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const estimate = selection?.fit.estimate ?? null;
  const cpuOffloadBytes =
    estimate?.placements
      .filter((placement) => placement.cpu)
      .reduce((total, placement) => total + placement.paramsBytes, 0) ?? 0;
  const gpuPlacedBytes =
    estimate?.placements
      .filter((placement) => !placement.cpu)
      .reduce((total, placement) => total + placement.paramsBytes, 0) ?? 0;
  const planLabel =
    estimate?.planMode === "concurrent"
      ? t("imageGeneration.local.modelLibrary.planConcurrent")
      : estimate?.planMode === "timeShare"
        ? t("imageGeneration.local.modelLibrary.planTimeShare")
        : t("imageGeneration.local.modelLibrary.planDefaultBackend");

  return (
    <BottomMenu
      isOpen={selection !== null}
      onClose={onClose}
      title={selection ? `${selection.modelName} · ${selection.variantName}` : undefined}
    >
      {selection && estimate ? (
        <div className="space-y-5 pb-2">
          <section>
            <h4 className="text-xs font-semibold text-fg/80">
              {t("imageGeneration.local.modelLibrary.memorySummary")}
            </h4>
            <dl className="mt-2 divide-y divide-fg/8 border-y border-fg/8 text-xs">
              <div className="flex items-baseline justify-between gap-4 py-2">
                <dt className="text-fg/45">
                  {t("imageGeneration.local.modelLibrary.modelWeights")}
                </dt>
                <dd className="font-mono tabular-nums text-fg/75">
                  {formatBytes(estimate.modelBytes)}
                </dd>
              </div>
              <div className="flex items-baseline justify-between gap-4 py-2">
                <dt className="text-fg/45">
                  {t("imageGeneration.local.modelLibrary.gpuPlacedWeights")}
                </dt>
                <dd className="font-mono tabular-nums text-fg/75">{formatBytes(gpuPlacedBytes)}</dd>
              </div>
              <div className="flex items-baseline justify-between gap-4 py-2">
                <dt className="text-fg/45">
                  {t("imageGeneration.local.modelLibrary.cpuOffloadedWeights")}
                </dt>
                <dd
                  className={cn(
                    "font-mono tabular-nums",
                    cpuOffloadBytes > 0 ? "text-warning" : "text-fg/75",
                  )}
                >
                  {cpuOffloadBytes > 0 ? formatBytes(cpuOffloadBytes) : t("common.labels.none")}
                </dd>
              </div>
              <div className="flex items-baseline justify-between gap-4 py-2">
                <dt className="text-fg/45">
                  {t("imageGeneration.local.modelLibrary.availableRam")}
                </dt>
                <dd className="font-mono tabular-nums text-fg/75">
                  {estimate.availableRamBytes ? formatBytes(estimate.availableRamBytes) : "—"}
                </dd>
              </div>
              <div className="flex items-baseline justify-between gap-4 py-2">
                <dt className="text-fg/45">{t("imageGeneration.local.modelLibrary.strategy")}</dt>
                <dd className="text-right text-fg/75">{planLabel}</dd>
              </div>
            </dl>
          </section>

          <section>
            <h4 className="text-xs font-semibold text-fg/80">
              {t("imageGeneration.local.modelLibrary.deviceMemory")}
            </h4>
            {estimate.devices.length > 0 ? (
              <div className="mt-2 divide-y divide-fg/8 border-y border-fg/8">
                {estimate.devices.map((device) => (
                  <div key={device.name} className="flex items-center justify-between gap-4 py-2.5">
                    <div className="min-w-0">
                      <div className="truncate text-xs font-medium text-fg/75">
                        {device.description || device.name}
                      </div>
                      <div className="mt-0.5 font-mono text-[10px] text-fg/35">{device.name}</div>
                    </div>
                    <dl className="grid shrink-0 grid-cols-2 gap-x-5 text-right text-[11px]">
                      <div>
                        <dt className="text-fg/35">
                          {t("imageGeneration.local.modelLibrary.freeMemory")}
                        </dt>
                        <dd className="mt-0.5 font-mono tabular-nums text-fg/65">
                          {formatBytes(device.freeBytes)}
                        </dd>
                      </div>
                      <div>
                        <dt className="text-fg/35">
                          {t("imageGeneration.local.modelLibrary.usableBudget")}
                        </dt>
                        <dd className="mt-0.5 font-mono tabular-nums text-fg/75">
                          {formatBytes(device.budgetBytes)}
                        </dd>
                      </div>
                    </dl>
                  </div>
                ))}
              </div>
            ) : (
              <p className="mt-2 text-xs text-fg/45">
                {t("imageGeneration.local.modelLibrary.noGpuPlacement")}
              </p>
            )}
          </section>

          <section>
            <h4 className="text-xs font-semibold text-fg/80">
              {t("imageGeneration.local.modelLibrary.componentPlacement")}
            </h4>
            <div className="mt-2 divide-y divide-fg/8 border-y border-fg/8">
              {estimate.placements.map((placement) => (
                <div key={placement.component} className="py-2.5">
                  <div className="flex items-baseline justify-between gap-4">
                    <span className="text-xs font-medium text-fg/80">{placement.component}</span>
                    <span className={cn("text-xs", placement.cpu ? "text-warning" : "text-fg/70")}>
                      {placement.targets.join(" + ")}
                      {placement.split ? ` · ${t("imageGeneration.local.modelLibrary.split")}` : ""}
                    </span>
                  </div>
                  <dl className="mt-2 grid grid-cols-3 gap-3 text-[11px]">
                    <div>
                      <dt className="text-fg/35">
                        {t("imageGeneration.local.modelLibrary.weights")}
                      </dt>
                      <dd className="mt-0.5 font-mono tabular-nums text-fg/65">
                        {formatBytes(placement.paramsBytes)}
                      </dd>
                    </div>
                    <div>
                      <dt className="text-fg/35">
                        {t("imageGeneration.local.modelLibrary.computeReserve")}
                      </dt>
                      <dd className="mt-0.5 font-mono tabular-nums text-fg/65">
                        {formatBytes(placement.computeReserveBytes)}
                      </dd>
                    </div>
                    <div>
                      <dt className="text-fg/35">
                        {t("imageGeneration.local.modelLibrary.singleGpuNeed")}
                      </dt>
                      <dd className="mt-0.5 font-mono tabular-nums text-fg/75">
                        {formatBytes(placement.paramsBytes + placement.computeReserveBytes)}
                      </dd>
                    </div>
                  </dl>
                </div>
              ))}
            </div>
          </section>

          <p className="text-[11px] leading-5 text-fg/38">{selection.fit.reason}</p>
        </div>
      ) : null}
    </BottomMenu>
  );
}

function ComputePolicyMenu({
  info,
  isOpen,
  saving,
  onClose,
  onSave,
}: {
  info: ComputePolicyInfo | null;
  isOpen: boolean;
  saving: boolean;
  onClose: () => void;
  onSave: (policy: ComputePolicy) => void;
}) {
  const { t } = useI18n();
  const [draft, setDraft] = useState<ComputePolicy | null>(null);

  useEffect(() => {
    if (!isOpen || !info) return;
    setDraft({
      ...info.policy,
      gpuDeviceIds: [...info.policy.gpuDeviceIds],
      deviceBudgetsGib: { ...info.policy.deviceBudgetsGib },
    });
  }, [info, isOpen]);

  const setMode = (mode: ComputeMode) => {
    if (!info) return;
    setDraft((current) => {
      if (!current) return current;
      const available = info.devices.map((device) => device.id);
      const selected = current.gpuDeviceIds.filter((id) => available.includes(id));
      const gpuDeviceIds =
        mode === "multi" ? (selected.length >= 2 ? selected : available.slice(0, 2)) : selected;
      return {
        ...current,
        multiGpuEnabled: mode === "multi",
        gpuDeviceIds,
        singleGpuDeviceId:
          mode === "single"
            ? (current.singleGpuDeviceId ?? selected[0] ?? available[0] ?? null)
            : null,
      };
    });
  };

  const toggleDevice = (id: number) => {
    setDraft((current) => {
      if (!current || computePolicyMode(current) === "auto") return current;
      if (computePolicyMode(current) === "single") {
        return {
          ...current,
          singleGpuDeviceId: id,
        };
      }
      return {
        ...current,
        gpuDeviceIds: current.gpuDeviceIds.includes(id)
          ? current.gpuDeviceIds.filter((deviceId) => deviceId !== id)
          : [...current.gpuDeviceIds, id].sort((left, right) => left - right),
      };
    });
  };

  const setBudget = (id: number, value: string) => {
    setDraft((current) => {
      if (!current) return current;
      const budgets = { ...current.deviceBudgetsGib };
      if (value === "") delete budgets[id];
      else budgets[id] = Number(value);
      return { ...current, deviceBudgetsGib: budgets };
    });
  };

  const mode = draft ? computePolicyMode(draft) : "auto";
  const valid = Boolean(
    draft &&
    (mode === "auto" ||
      (mode === "single" && draft.singleGpuDeviceId !== null) ||
      (mode === "multi" && draft.gpuDeviceIds.length >= 2)) &&
    Object.values(draft.deviceBudgetsGib).every(
      (budget) => Number.isFinite(budget) && budget > 0,
    ) &&
    Object.entries(draft.deviceBudgetsGib).every(([id, budget]) => {
      const device = info?.devices.find((candidate) => candidate.id === Number(id));
      return Boolean(device && budget <= device.totalBytes / 1024 ** 3);
    }),
  );

  return (
    <BottomMenu
      isOpen={isOpen}
      onClose={saving ? () => undefined : onClose}
      title={t("imageGeneration.local.engineManager.computePolicyTitle")}
    >
      {info && draft ? (
        <div className="space-y-6 pb-2">
          <p className="text-sm leading-6 text-fg/50">
            {t("imageGeneration.local.engineManager.computePolicyDescription")}
          </p>

          <section>
            <h4 className="text-xs font-semibold text-fg/75">
              {t("imageGeneration.local.engineManager.computeMode")}
            </h4>
            <div className="mt-2 divide-y divide-fg/8 border-y border-fg/8">
              {(
                [
                  ["auto", "computeModeAuto", "computeModeAutoDescription"],
                  ["single", "computeModeSingle", "computeModeSingleDescription"],
                  ["multi", "computeModeMulti", "computeModeMultiDescription"],
                ] as const
              ).map(([mode, titleKey, descriptionKey]) => {
                const disabled = mode === "multi" && info.devices.length < 2;
                return (
                  <button
                    key={mode}
                    type="button"
                    onClick={() => setMode(mode)}
                    disabled={disabled}
                    aria-pressed={computePolicyMode(draft) === mode}
                    className={cn(
                      "flex w-full items-start gap-3 py-3 text-left transition-colors disabled:cursor-not-allowed disabled:opacity-35",
                      !disabled && "hover:bg-fg/[0.025]",
                      focusRing,
                    )}
                  >
                    <span
                      aria-hidden="true"
                      className={cn(
                        "mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full border",
                        computePolicyMode(draft) === mode ? "border-accent" : "border-fg/25",
                      )}
                    >
                      {computePolicyMode(draft) === mode ? (
                        <span className="h-2 w-2 rounded-full bg-accent" />
                      ) : null}
                    </span>
                    <span className="min-w-0">
                      <span className="block text-sm font-medium text-fg/80">
                        {t(`imageGeneration.local.engineManager.${titleKey}` as TranslationKey)}
                      </span>
                      <span className="mt-0.5 block text-xs leading-5 text-fg/45">
                        {t(
                          `imageGeneration.local.engineManager.${descriptionKey}` as TranslationKey,
                        )}
                      </span>
                    </span>
                  </button>
                );
              })}
            </div>
          </section>

          <section>
            <div className="flex items-baseline justify-between gap-3">
              <h4 className="text-xs font-semibold text-fg/75">
                {t("imageGeneration.local.engineManager.eligibleGpus")}
              </h4>
              <span className="text-[11px] text-fg/35">{info.backend.toUpperCase()}</span>
            </div>
            {info.devices.length > 0 ? (
              <div className="mt-2 divide-y divide-fg/8 border-y border-fg/8">
                {info.devices.map((device) => {
                  const selected =
                    mode === "auto" ||
                    (mode === "single"
                      ? draft.singleGpuDeviceId === device.id
                      : draft.gpuDeviceIds.includes(device.id));
                  const budget = draft.deviceBudgetsGib[device.id];
                  return (
                    <div key={device.id} className="py-3">
                      <button
                        type="button"
                        disabled={mode === "auto"}
                        onClick={() => toggleDevice(device.id)}
                        aria-pressed={selected}
                        className={cn(
                          "flex w-full items-start gap-3 text-left disabled:cursor-default",
                          focusRing,
                        )}
                      >
                        <span
                          aria-hidden="true"
                          className={cn(
                            "mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center border",
                            mode === "single" ? "rounded-full" : "rounded",
                            selected ? "border-accent bg-accent text-bg" : "border-fg/25",
                          )}
                        >
                          {selected ? <Check className="h-3 w-3" /> : null}
                        </span>
                        <span className="min-w-0 flex-1">
                          <span className="block truncate text-sm font-medium text-fg/80">
                            {device.description || device.name}
                          </span>
                          <span className="mt-0.5 block font-mono text-[10px] text-fg/35">
                            #{device.id} · {device.name} · {formatBytes(device.freeBytes)} /{" "}
                            {formatBytes(device.totalBytes)}
                          </span>
                        </span>
                      </button>
                      {selected ? (
                        <label className="mt-3 flex items-center justify-between gap-4 pl-7">
                          <span className="min-w-0">
                            <span className="block text-xs font-medium text-fg/60">
                              {t("imageGeneration.local.engineManager.vramBudget")}
                            </span>
                            <span className="mt-0.5 block text-[11px] text-fg/35">
                              {t("imageGeneration.local.engineManager.vramBudgetHint", {
                                available: formatBytes(device.freeBytes),
                              })}
                            </span>
                          </span>
                          <span className="flex shrink-0 items-center gap-2">
                            <input
                              type="number"
                              inputMode="decimal"
                              min="0.1"
                              max={device.totalBytes / 1024 ** 3}
                              step="0.1"
                              value={budget ?? ""}
                              onChange={(event) => setBudget(device.id, event.target.value)}
                              placeholder={t("imageGeneration.local.engineManager.vramBudgetAuto")}
                              aria-label={t("imageGeneration.local.engineManager.vramBudgetFor", {
                                device: device.description || device.name,
                              })}
                              className={cn(
                                "h-9 w-24 rounded-lg border border-fg/12 bg-surface px-2 text-right text-sm tabular-nums text-fg placeholder:text-fg/30",
                                focusRing,
                              )}
                            />
                            <span className="text-xs text-fg/40">GiB</span>
                          </span>
                        </label>
                      ) : null}
                    </div>
                  );
                })}
              </div>
            ) : (
              <p className="mt-2 border-y border-fg/8 py-3 text-xs text-fg/45">
                {t("imageGeneration.local.engineManager.noEligibleGpus")}
              </p>
            )}
          </section>

          <section>
            <h4 className="text-xs font-semibold text-fg/75">
              {t("imageGeneration.local.engineManager.splitMode")}
            </h4>
            <div className="mt-2 grid grid-cols-2 gap-2">
              {(
                [
                  ["layer", "splitLayer", "splitLayerDescription"],
                  ["row", "splitRow", "splitRowDescription"],
                ] as const
              ).map(([mode, titleKey, descriptionKey]) => {
                const disabled = mode === "row" && !info.supportsRowSplit;
                return (
                  <button
                    key={mode}
                    type="button"
                    disabled={disabled}
                    onClick={() =>
                      setDraft((current) => current && { ...current, splitMode: mode })
                    }
                    aria-pressed={draft.splitMode === mode}
                    className={cn(
                      "rounded-lg border px-3 py-3 text-left transition-colors disabled:cursor-not-allowed disabled:opacity-35",
                      draft.splitMode === mode
                        ? "border-accent/50 bg-accent/8"
                        : "border-fg/10 hover:bg-fg/[0.025]",
                      focusRing,
                    )}
                  >
                    <span className="block text-xs font-semibold text-fg/75">
                      {t(`imageGeneration.local.engineManager.${titleKey}` as TranslationKey)}
                    </span>
                    <span className="mt-1 block text-[11px] leading-4 text-fg/40">
                      {t(`imageGeneration.local.engineManager.${descriptionKey}` as TranslationKey)}
                    </span>
                  </button>
                );
              })}
            </div>
            {!info.supportsRowSplit ? (
              <p className="mt-2 text-[11px] text-fg/35">
                {t("imageGeneration.local.engineManager.rowCudaOnly")}
              </p>
            ) : null}
          </section>

          <button
            type="button"
            onClick={() => onSave(draft)}
            disabled={!valid || saving}
            className={cn(
              "inline-flex h-10 w-full items-center justify-center gap-2 rounded-lg bg-accent px-4 text-sm font-semibold text-bg transition-[filter] hover:brightness-110 disabled:opacity-50",
              focusRing,
            )}
          >
            {saving ? (
              <Loader2 className="h-4 w-4 animate-spin motion-reduce:animate-none" />
            ) : null}
            {t("imageGeneration.local.engineManager.computePolicySave")}
          </button>
        </div>
      ) : null}
    </BottomMenu>
  );
}

export function StableDiffusionSettingsPage() {
  const { t } = useI18n();
  const platform = useMemo(() => getPlatform(), []);
  const { shouldShow: showEngineTour, dismiss: dismissEngineTour } = useGuidedTour("sdEngine");
  const downloadQueue = useDownloadQueueOptional();
  const [inventory, setInventory] = useState<RuntimeInventory | null>(null);
  const [upscalerInventory, setUpscalerInventory] = useState<UpscalerInventory | null>(null);
  const [upscalerBusy, setUpscalerBusy] = useState(false);
  const [upscalerError, setUpscalerError] = useState<string | null>(null);
  const [catalog, setCatalog] = useState<RuntimeCatalog | null>(null);
  const [gpuDevices, setGpuDevices] = useState<GpuDevice[]>([]);
  const [inventoryLoading, setInventoryLoading] = useState(platform.type === "desktop");
  const [catalogLoading, setCatalogLoading] = useState(platform.type === "desktop");
  const [inventoryError, setInventoryError] = useState<string | null>(null);
  const [catalogError, setCatalogError] = useState<string | null>(null);
  const [selectedRelease, setSelectedRelease] = useState<string | null>(null);
  const [selectedAsset, setSelectedAsset] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [switchingKey, setSwitchingKey] = useState<string | null>(null);
  const [deletingKey, setDeletingKey] = useState<string | null>(null);
  const [installedModels, setInstalledModels] = useState<InstalledModel[]>([]);
  const [selectedVariants, setSelectedVariants] = useState<Record<string, string>>({});
  const [familyOverrides, setFamilyOverrides] = useState<Record<string, boolean>>({});
  const [installingModelKey, setInstallingModelKey] = useState<string | null>(null);
  const [uninstallingModelKey, setUninstallingModelKey] = useState<string | null>(null);
  const [runnability, setRunnability] = useState<Record<string, Runnability | "loading">>({});
  const runnabilityRevision = useRef(0);
  const [modelHardwareRefreshing, setModelHardwareRefreshing] = useState(false);
  const [fitDetails, setFitDetails] = useState<FitDetailsSelection | null>(null);
  const [computePolicyInfo, setComputePolicyInfo] = useState<ComputePolicyInfo | null>(null);
  const [computePolicyLoading, setComputePolicyLoading] = useState(false);
  const [computePolicyOpen, setComputePolicyOpen] = useState(false);
  const [computePolicySaving, setComputePolicySaving] = useState(false);
  const [computePolicyError, setComputePolicyError] = useState<string | null>(null);

  const installs = useMemo(
    () => runtimeInstallGroups(downloadQueue?.queue ?? []),
    [downloadQueue?.queue],
  );
  const hasActiveInstall = installs.some((install) => install.active);
  const visibleInstall =
    installs.find((install) => install.active) ?? installs[installs.length - 1] ?? null;
  const visibleInstallItemIds = useMemo(
    () => new Set(visibleInstall?.items.map((item) => item.id) ?? []),
    [visibleInstall],
  );

  const modelInstalls = useMemo(
    () => modelInstallGroups(downloadQueue?.queue ?? []),
    [downloadQueue?.queue],
  );
  const hasActiveModelInstall = modelInstalls.some((install) => install.active);
  const activeModelKeys = useMemo(
    () =>
      new Set(
        modelInstalls
          .filter((install) => install.active)
          .map((install) => `${install.profileId}:${install.variantId}`),
      ),
    [modelInstalls],
  );
  const modelInstallItemIds = useMemo(
    () =>
      new Set(
        modelInstalls.flatMap((install) =>
          install.active ? install.items.map((item) => item.id) : [],
        ),
      ),
    [modelInstalls],
  );
  const installedModelKeys = useMemo(
    () => new Set(installedModels.map((model) => `${model.profileId}:${model.variantId}`)),
    [installedModels],
  );
  const installedProfileIds = useMemo(
    () => new Set(installedModels.map((model) => model.profileId)),
    [installedModels],
  );

  const modelFamilies = useMemo(() => {
    if (!catalog) return [];
    const order: string[] = [];
    const grouped = new Map<string, CatalogProfile[]>();
    for (const profile of catalog.profiles) {
      const existing = grouped.get(profile.family);
      if (existing) {
        existing.push(profile);
      } else {
        grouped.set(profile.family, [profile]);
        order.push(profile.family);
      }
    }
    return order.map((name) => ({ name, profiles: grouped.get(name) ?? [] }));
  }, [catalog]);

  const loadInventory = useCallback(async () => {
    if (platform.type === "mobile") return;
    setInventoryLoading(true);
    setInventoryError(null);
    try {
      setInventory(await invoke<RuntimeInventory>("sdcpp_runtime_inventory"));
    } catch (error) {
      setInventoryError(error instanceof Error ? error.message : String(error));
    } finally {
      setInventoryLoading(false);
    }
  }, [platform.type]);

  const loadInstalledModels = useCallback(async () => {
    if (platform.type === "mobile") return;
    const result = await invoke<InstalledModel[]>("sdcpp_installed").catch(() => null);
    if (result) setInstalledModels(result);
  }, [platform.type]);

  const loadUpscalerInventory = useCallback(async () => {
    if (platform.type === "mobile") return;
    const result = await invoke<UpscalerInventory>("sdcpp_upscaler_inventory").catch(() => null);
    setUpscalerInventory(result);
  }, [platform.type]);

  const handleInstallUpscaler = useCallback(async () => {
    setUpscalerBusy(true);
    setUpscalerError(null);
    try {
      setUpscalerInventory(await invoke<UpscalerInventory>("sdcpp_install_upscaler"));
    } catch (error) {
      setUpscalerError(error instanceof Error ? error.message : String(error));
    } finally {
      setUpscalerBusy(false);
    }
  }, []);

  const handleRemoveUpscaler = useCallback(async () => {
    if (!upscalerInventory?.recommendedInstalled) return;
    setUpscalerBusy(true);
    setUpscalerError(null);
    try {
      setUpscalerInventory(
        await invoke<UpscalerInventory>("sdcpp_remove_upscaler", {
          filename: upscalerInventory.recommendedFilename,
        }),
      );
    } catch (error) {
      setUpscalerError(error instanceof Error ? error.message : String(error));
    } finally {
      setUpscalerBusy(false);
    }
  }, [upscalerInventory]);

  const loadCatalog = useCallback(async () => {
    if (platform.type === "mobile") return;
    setCatalogLoading(true);
    setCatalogError(null);
    const [catalogResult, devicesResult] = await Promise.allSettled([
      invoke<RuntimeCatalog>("sdcpp_catalog"),
      invoke<GpuDevice[]>("llamacpp_backend_devices"),
    ]);
    if (catalogResult.status === "fulfilled") {
      setCatalog(catalogResult.value);
      setSelectedRelease((current) =>
        catalogResult.value.runtimeReleases.some((release) => release.tag === current)
          ? current
          : (catalogResult.value.runtimeReleases[0]?.tag ?? null),
      );
    } else {
      setCatalogError(
        catalogResult.reason instanceof Error
          ? catalogResult.reason.message
          : String(catalogResult.reason),
      );
    }
    if (devicesResult.status === "fulfilled") setGpuDevices(devicesResult.value);
    setCatalogLoading(false);
  }, [platform.type]);

  useEffect(() => {
    void loadInventory();
    void loadCatalog();
    void loadInstalledModels();
    void loadUpscalerInventory();
  }, [loadCatalog, loadInventory, loadInstalledModels, loadUpscalerInventory]);

  useEffect(() => {
    if (!hasActiveInstall) void loadInventory();
  }, [hasActiveInstall, loadInventory]);

  useEffect(() => {
    if (!hasActiveModelInstall) void loadInstalledModels();
  }, [hasActiveModelInstall, loadInstalledModels]);

  const release =
    catalog?.runtimeReleases.find((candidate) => candidate.tag === selectedRelease) ?? null;
  const recommendedAsset = release ? recommendAsset(platform.os, gpuDevices, release.assets) : null;
  const asset =
    release?.assets.find((candidate) => candidate.name === selectedAsset) ?? recommendedAsset;
  const assetInstalled = Boolean(
    asset &&
    inventory?.installed.some((item) => item.release === release?.tag && item.asset === asset.name),
  );
  const discreteGpu = usableGpuDevices(gpuDevices)[0] ?? null;

  const hasInstalledEngine = (inventory?.installed.length ?? 0) > 0;

  useEffect(() => {
    if (!downloadQueue || inventoryLoading || hasInstalledEngine) return;
    const legacyInstallIds = new Set(
      downloadQueue.queue
        .filter(
          (item) =>
            item.queueKind === "sdcpp" &&
            item.installKind !== "runtime" &&
            (item.downloadRole === "runtime" || item.downloadRole === "runtime_dependency"),
        )
        .map((item) => item.installId)
        .filter((installId): installId is string => Boolean(installId)),
    );
    const activeLegacyItems = downloadQueue.queue.filter(
      (item) =>
        item.installId !== null &&
        legacyInstallIds.has(item.installId) &&
        (item.status === "queued" || item.status === "downloading"),
    );
    for (const item of activeLegacyItems) void downloadQueue.cancelItem(item.id);
  }, [downloadQueue, hasInstalledEngine, inventoryLoading]);

  const installedEnginePairing = useMemo<{ release: string; asset: string } | null>(() => {
    if (inventory?.active) return inventory.active;
    const installedRuntime = inventory?.installed[0];
    return installedRuntime
      ? { release: installedRuntime.release, asset: installedRuntime.asset }
      : null;
  }, [inventory]);
  const runtimePairing = installedEnginePairing;

  const loadComputePolicy = useCallback(async (pairing: { release: string; asset: string }) => {
    setComputePolicyLoading(true);
    setComputePolicyError(null);
    try {
      const info = await invoke<ComputePolicyInfo>("sdcpp_compute_policy", {
        request: {
          runtimeRelease: pairing.release,
          runtimeAsset: pairing.asset,
        },
      });
      setComputePolicyInfo(info);
    } catch (error) {
      setComputePolicyInfo(null);
      setComputePolicyError(error instanceof Error ? error.message : String(error));
    } finally {
      setComputePolicyLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!installedEnginePairing) {
      setComputePolicyInfo(null);
      setComputePolicyError(null);
      setComputePolicyOpen(false);
      return;
    }
    void loadComputePolicy(installedEnginePairing);
  }, [installedEnginePairing, loadComputePolicy]);

  const saveComputePolicy = async (policy: ComputePolicy) => {
    if (!installedEnginePairing) return;
    setComputePolicySaving(true);
    try {
      const info = await invoke<ComputePolicyInfo>("sdcpp_compute_policy_save", {
        request: {
          runtimeRelease: installedEnginePairing.release,
          runtimeAsset: installedEnginePairing.asset,
          policy,
        },
      });
      setComputePolicyInfo(info);
      runnabilityRevision.current += 1;
      setRunnability({});
      setFitDetails(null);
      setComputePolicyOpen(false);
      toast.success(t("imageGeneration.local.engineManager.computePolicySaved"));
    } catch (error) {
      toast.error(
        t("imageGeneration.local.engineManager.computePolicySaveFailed"),
        error instanceof Error ? error.message : String(error),
      );
    } finally {
      setComputePolicySaving(false);
    }
  };

  const computePolicySummary = useMemo(() => {
    if (!computePolicyInfo) return null;
    const policy = computePolicyInfo.policy;
    const mode = computePolicyMode(policy);
    if (mode === "auto") {
      return t("imageGeneration.local.engineManager.computeSummaryAuto", {
        count: computePolicyInfo.devices.length,
      });
    }
    if (mode === "single") {
      const selected = computePolicyInfo.devices.find(
        (device) => device.id === policy.singleGpuDeviceId,
      );
      return t("imageGeneration.local.engineManager.computeSummarySingle", {
        device: selected?.description || selected?.name || "—",
      });
    }
    return t("imageGeneration.local.engineManager.computeSummaryMulti", {
      count: policy.gpuDeviceIds.length,
    });
  }, [computePolicyInfo, t]);

  const fitKey = useCallback(
    (profileId: string, variantId: string) =>
      installedEnginePairing
        ? `${profileId}:${variantId}@${installedEnginePairing.release}:${installedEnginePairing.asset}`
        : null,
    [installedEnginePairing],
  );

  const checkRunnability = useCallback(
    async (
      profileId: string,
      variantId: string,
      pairing: { release: string; asset: string },
      revision = runnabilityRevision.current,
    ) => {
      const key = `${profileId}:${variantId}@${pairing.release}:${pairing.asset}`;
      setRunnability((current) => ({ ...current, [key]: "loading" }));
      const result = await invoke<Runnability>("sdcpp_runnability", {
        request: {
          profileId,
          variantId,
          runtimeRelease: pairing.release,
          runtimeAsset: pairing.asset,
        },
      }).catch(() => null);
      if (revision !== runnabilityRevision.current) return;
      setRunnability((current) => {
        const next = { ...current };
        if (result) next[key] = result;
        else delete next[key];
        return next;
      });
    },
    [],
  );

  const refreshModelHardware = useCallback(async () => {
    if (modelHardwareRefreshing) return;
    const revision = runnabilityRevision.current + 1;
    runnabilityRevision.current = revision;
    setModelHardwareRefreshing(true);
    setFitDetails(null);
    try {
      const devices = await invoke<GpuDevice[]>("llamacpp_backend_devices");
      setGpuDevices(devices);

      if (!installedEnginePairing || !catalog) {
        setRunnability({});
        return;
      }

      await loadComputePolicy(installedEnginePairing);
      const checks = catalog.profiles.flatMap((profile) => {
        const variant = selectedVariant(profile, selectedVariants);
        if (!variant || installedModelKeys.has(`${profile.id}:${variant.id}`)) return [];
        if (!runtimeSupportsProfile(profile, installedEnginePairing.release)) return [];
        return [checkRunnability(profile.id, variant.id, installedEnginePairing, revision)];
      });
      await Promise.all(checks);
    } catch (error) {
      toast.error(
        t("imageGeneration.local.modelLibrary.refreshFailed"),
        error instanceof Error ? error.message : String(error),
      );
    } finally {
      setModelHardwareRefreshing(false);
    }
  }, [
    catalog,
    checkRunnability,
    installedEnginePairing,
    installedModelKeys,
    loadComputePolicy,
    modelHardwareRefreshing,
    selectedVariants,
    t,
  ]);

  useEffect(() => {
    if (!installedEnginePairing || !catalog) return;
    for (const profile of catalog.profiles) {
      const variant = selectedVariant(profile, selectedVariants);
      if (!variant) continue;
      if (installedModelKeys.has(`${profile.id}:${variant.id}`)) continue;
      if (!runtimeSupportsProfile(profile, installedEnginePairing.release)) continue;
      const key = `${profile.id}:${variant.id}@${installedEnginePairing.release}:${installedEnginePairing.asset}`;
      if (runnability[key] !== undefined) continue;
      void checkRunnability(profile.id, variant.id, installedEnginePairing);
    }
  }, [
    catalog,
    selectedVariants,
    installedEnginePairing,
    installedModelKeys,
    runnability,
    checkRunnability,
  ]);

  useEffect(() => {
    setSelectedAsset(recommendedAsset?.name ?? null);
  }, [recommendedAsset?.name, release?.tag]);

  const install = async () => {
    if (!release || !asset) return;
    setInstalling(true);
    try {
      await invoke<string[]>("sdcpp_runtime_install", {
        request: {
          runtimeRelease: release.tag,
          runtimeAsset: asset.name,
        },
      });
      toast.success(
        t("imageGeneration.local.engineManager.queued"),
        `${release.tag} · ${asset.backend.toUpperCase()}`,
      );
    } catch (error) {
      toast.error(
        t("imageGeneration.local.engineManager.installFailed"),
        error instanceof Error ? error.message : String(error),
      );
    } finally {
      setInstalling(false);
    }
  };

  const switchRuntime = async (runtime: InstalledRuntime) => {
    const key = `${runtime.release}:${runtime.asset}`;
    setSwitchingKey(key);
    try {
      await invoke("sdcpp_runtime_switch", {
        request: { runtimeRelease: runtime.release, runtimeAsset: runtime.asset },
      });
      await loadInventory();
    } catch (error) {
      toast.error(
        t("imageGeneration.local.engineManager.switchFailed"),
        error instanceof Error ? error.message : String(error),
      );
    } finally {
      setSwitchingKey(null);
    }
  };

  const deleteRuntime = async (runtime: InstalledRuntime) => {
    const confirmed = await confirmBottomMenu({
      title: t("imageGeneration.local.engineManager.deleteConfirmTitle", {
        version: runtime.release,
      }),
      message: runtime.active
        ? t("imageGeneration.local.engineManager.deleteActiveConfirm")
        : t("imageGeneration.local.engineManager.deleteConfirm"),
      confirmLabel: t("common.buttons.delete"),
      destructive: true,
    });
    if (!confirmed) return;
    const key = `${runtime.release}:${runtime.asset}`;
    setDeletingKey(key);
    try {
      await invoke("sdcpp_runtime_delete", {
        request: { runtimeRelease: runtime.release, runtimeAsset: runtime.asset },
      });
      await loadInventory();
    } catch (error) {
      toast.error(
        t("imageGeneration.local.engineManager.deleteFailed"),
        error instanceof Error ? error.message : String(error),
      );
    } finally {
      setDeletingKey(null);
    }
  };

  const installModel = async (
    profile: CatalogProfile,
    variant: CatalogVariant,
    fit: Runnability | "loading" | undefined,
  ) => {
    if (!runtimePairing) {
      toast.error(
        t("imageGeneration.local.modelLibrary.installFailed"),
        t("imageGeneration.local.modelLibrary.noRuntime"),
      );
      return;
    }
    if (!runtimeSupportsProfile(profile, runtimePairing.release)) {
      toast.error(
        t("imageGeneration.local.modelLibrary.installFailed"),
        t("imageGeneration.local.modelLibrary.engineUpdateRequired", {
          build: profile.minimumRuntimeBuild ?? "—",
        }),
      );
      return;
    }
    if (fitOffloadsToCpu(fit)) {
      const confirmed = await confirmBottomMenu({
        title: t("imageGeneration.local.modelLibrary.offloadConfirmTitle", {
          model: profile.displayName,
        }),
        message: t("imageGeneration.local.modelLibrary.offloadConfirm"),
        confirmLabel: t("imageGeneration.local.modelLibrary.download"),
      });
      if (!confirmed) return;
    }
    const key = `${profile.id}:${variant.id}`;
    setInstallingModelKey(key);
    try {
      await invoke<string[]>("sdcpp_install", {
        request: {
          profileId: profile.id,
          variantId: variant.id,
          runtimeRelease: runtimePairing.release,
          runtimeAsset: runtimePairing.asset,
        },
      });
      toast.success(
        t("imageGeneration.local.modelLibrary.queued"),
        `${profile.displayName} · ${variant.label}`,
      );
    } catch (error) {
      toast.error(
        t("imageGeneration.local.modelLibrary.installFailed"),
        error instanceof Error ? error.message : String(error),
      );
    } finally {
      setInstallingModelKey(null);
    }
  };

  const uninstallModel = async (profile: CatalogProfile, variant: CatalogVariant) => {
    const confirmed = await confirmBottomMenu({
      title: t("imageGeneration.local.modelLibrary.uninstallConfirmTitle", {
        model: profile.displayName,
      }),
      message: t("imageGeneration.local.modelLibrary.uninstallConfirm"),
      confirmLabel: t("common.buttons.delete"),
      destructive: true,
    });
    if (!confirmed) return;
    const key = `${profile.id}:${variant.id}`;
    setUninstallingModelKey(key);
    try {
      await invoke("sdcpp_uninstall", { profileId: profile.id, variantId: variant.id });
      await loadInstalledModels();
    } catch (error) {
      toast.error(
        t("imageGeneration.local.modelLibrary.uninstallFailed"),
        error instanceof Error ? error.message : String(error),
      );
    } finally {
      setUninstallingModelKey(null);
    }
  };

  if (platform.type === "mobile") {
    return (
      <main className="flex min-h-screen items-center justify-center px-6 text-center">
        <div>
          <h1 className="text-lg font-semibold text-fg">
            {t("imageGeneration.local.hub.desktopOnlyTitle")}
          </h1>
          <p className="mt-2 text-sm text-fg/50">
            {t("imageGeneration.local.hub.desktopOnlyBody")}
          </p>
        </div>
      </main>
    );
  }

  return (
    <main className="min-h-screen px-4 pb-24 pt-5">
      <div className="mx-auto w-full max-w-6xl space-y-6">
        <header className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <h1 className="text-lg font-semibold text-fg">
              {t("imageGeneration.local.engineManager.title")}
            </h1>
            <p className="mt-1 max-w-2xl text-sm leading-6 text-fg/50">
              {t("imageGeneration.local.engineManager.description")}
            </p>
          </div>
          <button
            type="button"
            onClick={() => {
              void loadInventory();
              void loadCatalog();
            }}
            disabled={inventoryLoading || catalogLoading}
            className={cn(
              "inline-flex h-9 shrink-0 items-center gap-2 rounded-lg border border-fg/12 px-3 text-sm font-medium text-fg/70 transition-colors hover:bg-fg/5 disabled:opacity-50",
              focusRing,
            )}
          >
            <RefreshCw
              aria-hidden="true"
              className={cn(
                "h-4 w-4",
                (inventoryLoading || catalogLoading) && "animate-spin motion-reduce:animate-none",
              )}
            />
            {t("common.buttons.refresh")}
          </button>
        </header>

        {inventoryError ? (
          <div
            aria-live="polite"
            className="rounded-lg border border-danger/25 bg-danger/5 px-4 py-3 text-sm text-danger/80"
          >
            <div className="font-medium">
              {t("imageGeneration.local.engineManager.inventoryFailed")}
            </div>
            <div className="mt-1 break-words text-xs">{inventoryError}</div>
          </div>
        ) : null}

        <div className="grid gap-6 lg:grid-cols-2 lg:items-start">
          <div className="space-y-6">
            {downloadQueue && visibleInstall ? (
              <section aria-live="polite" className="space-y-3">
                <h2 className="text-sm font-semibold text-fg">
                  {t("imageGeneration.local.engineManager.downloads")}
                </h2>
                <InlineDownloadCards filter={(item) => visibleInstallItemIds.has(item.id)} />
              </section>
            ) : null}

            <section data-tour-id="sd-installed">
              <h2 className="mb-3 text-sm font-semibold text-fg">
                {t("imageGeneration.local.engineManager.installedTitle")}
              </h2>
              {inventoryLoading && !inventory ? (
                <div
                  role="status"
                  aria-busy="true"
                  className="divide-y divide-fg/8 overflow-hidden rounded-lg border border-fg/10"
                >
                  <span className="sr-only">{t("common.labels.loading")}</span>
                  {[0, 1].map((row) => (
                    <div
                      key={row}
                      className="flex items-center justify-between gap-4 bg-fg/[0.025] px-4 py-4"
                    >
                      <div className="min-w-0 space-y-2">
                        <Skeleton className="h-4 w-32" />
                        <Skeleton className="h-3 w-52" />
                      </div>
                      <Skeleton className="h-8 w-28" />
                    </div>
                  ))}
                </div>
              ) : inventory && inventory.installed.length > 0 ? (
                <div className="space-y-3">
                  <div className="divide-y divide-fg/8 overflow-hidden rounded-lg border border-fg/10">
                    {inventory.installed.map((runtime) => {
                      const key = `${runtime.release}:${runtime.asset}`;
                      const switching = switchingKey === key;
                      const deleting = deletingKey === key;
                      return (
                        <div
                          key={key}
                          className="flex flex-wrap items-center justify-between gap-4 bg-fg/[0.025] px-4 py-4"
                        >
                          <div className="min-w-0">
                            <div className="flex flex-wrap items-center gap-2">
                              <span className="font-mono text-sm font-semibold text-fg">
                                {runtime.release}
                              </span>
                              {runtime.active ? (
                                <span className="inline-flex items-center gap-1 text-xs font-medium text-success">
                                  <Check aria-hidden="true" className="h-3.5 w-3.5" />
                                  {t("imageGeneration.local.engineManager.active")}
                                </span>
                              ) : null}
                            </div>
                            <p className="mt-1 break-all text-xs text-fg/45">
                              {runtime.backend.toUpperCase()} · {formatBytes(runtime.sizeBytes)} ·{" "}
                              {runtime.asset}
                            </p>
                          </div>
                          <div className="flex items-center gap-2">
                            {!runtime.active ? (
                              <button
                                type="button"
                                onClick={() => void switchRuntime(runtime)}
                                disabled={switching || deleting}
                                className={cn(
                                  "inline-flex h-8 items-center gap-2 rounded-lg border border-fg/12 px-3 text-xs font-medium text-fg/70 transition-colors hover:bg-fg/5 disabled:opacity-50",
                                  focusRing,
                                )}
                              >
                                {switching ? (
                                  <Loader2
                                    aria-hidden="true"
                                    className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none"
                                  />
                                ) : null}
                                {t("imageGeneration.local.engineManager.useVersion")}
                              </button>
                            ) : null}
                            <button
                              type="button"
                              onClick={() => void deleteRuntime(runtime)}
                              disabled={switching || deleting}
                              aria-label={t("imageGeneration.local.engineManager.deleteVersion", {
                                version: runtime.release,
                              })}
                              className={cn(
                                "rounded-lg p-2 text-fg/42 transition-colors hover:bg-danger/8 hover:text-danger disabled:opacity-50",
                                focusRing,
                              )}
                            >
                              {deleting ? (
                                <Loader2
                                  aria-hidden="true"
                                  className="h-4 w-4 animate-spin motion-reduce:animate-none"
                                />
                              ) : (
                                <Trash2 aria-hidden="true" className="h-4 w-4" />
                              )}
                            </button>
                          </div>
                        </div>
                      );
                    })}
                  </div>
                  {installedEnginePairing ? (
                    <div className="flex flex-wrap items-center justify-between gap-4 rounded-lg border border-fg/10 bg-fg/[0.025] px-4 py-3.5">
                      <div className="flex min-w-0 items-start gap-3">
                        <Settings2
                          aria-hidden="true"
                          className="mt-0.5 h-4 w-4 shrink-0 text-fg/40"
                        />
                        <div className="min-w-0">
                          <h3 className="text-sm font-medium text-fg/80">
                            {t("imageGeneration.local.engineManager.computePolicyTitle")}
                          </h3>
                          {computePolicyLoading ? (
                            <p className="mt-1 text-xs text-fg/35">
                              {t("imageGeneration.local.engineManager.computePolicyLoading")}
                            </p>
                          ) : computePolicyError ? (
                            <p className="mt-1 break-words text-xs text-danger/70">
                              {computePolicyError}
                            </p>
                          ) : computePolicyInfo ? (
                            <p className="mt-1 text-xs text-fg/45">
                              {computePolicySummary}
                              <span className="mx-1.5 text-fg/25">·</span>
                              {computePolicyInfo.policy.splitMode === "row"
                                ? t("imageGeneration.local.engineManager.splitRow")
                                : t("imageGeneration.local.engineManager.splitLayer")}
                              <span className="mx-1.5 text-fg/25">·</span>
                              {computePolicyInfo.backend.toUpperCase()}
                            </p>
                          ) : null}
                        </div>
                      </div>
                      <button
                        type="button"
                        onClick={() => {
                          if (computePolicyInfo) setComputePolicyOpen(true);
                          else void loadComputePolicy(installedEnginePairing);
                        }}
                        disabled={computePolicyLoading}
                        className={cn(
                          "inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg border border-fg/12 px-3 text-xs font-medium text-fg/70 transition-colors hover:bg-fg/5 disabled:opacity-50",
                          focusRing,
                        )}
                      >
                        {computePolicyLoading ? (
                          <Loader2 className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none" />
                        ) : null}
                        {computePolicyError
                          ? t("common.buttons.retry")
                          : t("imageGeneration.local.engineManager.computePolicyConfigure")}
                      </button>
                    </div>
                  ) : null}
                </div>
              ) : (
                <div className="rounded-lg border border-fg/10 bg-fg/[0.025] px-4 py-5">
                  <div className="flex items-start gap-3">
                    <HardDrive aria-hidden="true" className="mt-0.5 h-5 w-5 shrink-0 text-fg/40" />
                    <div>
                      <h3 className="text-sm font-semibold text-fg">
                        {t("imageGeneration.local.engineManager.notInstalled")}
                      </h3>
                      <p className="mt-1 text-sm leading-6 text-fg/50">
                        {t("imageGeneration.local.engineManager.notInstalledBody")}
                      </p>
                    </div>
                  </div>
                </div>
              )}
            </section>

            <section data-tour-id="sd-upscaler" className="border-t border-fg/8 pt-5">
              <h2 className="mb-3 text-sm font-semibold text-fg">
                {t("imageGeneration.local.upscaler.title")}
              </h2>
              <div className="rounded-lg border border-fg/10 bg-fg/[0.025] px-4 py-4">
                <div className="flex flex-wrap items-center justify-between gap-4">
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="font-mono text-sm font-semibold text-fg">
                        {upscalerInventory?.recommendedFilename ?? "RealESRGAN_x4plus_anime_6B"}
                      </span>
                      {upscalerInventory?.recommendedInstalled ? (
                        <span className="inline-flex items-center gap-1 text-xs font-medium text-success">
                          <Check aria-hidden="true" className="h-3.5 w-3.5" />
                          {t("imageGeneration.local.upscaler.installed")}
                        </span>
                      ) : null}
                    </div>
                    <p className="mt-1 text-xs leading-5 text-fg/45">
                      {t("imageGeneration.local.upscaler.description", {
                        size: formatBytes(upscalerInventory?.recommendedBytes ?? 17938799),
                      })}
                    </p>
                  </div>
                  {upscalerInventory?.recommendedInstalled ? (
                    <button
                      type="button"
                      onClick={() => void handleRemoveUpscaler()}
                      disabled={upscalerBusy}
                      className={cn(
                        "inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg border border-fg/12 px-3 text-xs font-medium text-fg/70 transition-colors hover:bg-fg/5 disabled:opacity-50",
                        focusRing,
                      )}
                    >
                      {upscalerBusy ? (
                        <Loader2 className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none" />
                      ) : null}
                      {t("common.buttons.remove")}
                    </button>
                  ) : (
                    <button
                      type="button"
                      onClick={() => void handleInstallUpscaler()}
                      disabled={upscalerBusy}
                      className={cn(
                        "inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg border border-fg/12 px-3 text-xs font-medium text-fg/70 transition-colors hover:bg-fg/5 disabled:opacity-50",
                        focusRing,
                      )}
                    >
                      {upscalerBusy ? (
                        <Loader2 className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none" />
                      ) : null}
                      {t("imageGeneration.local.upscaler.install")}
                    </button>
                  )}
                </div>
                {upscalerError ? (
                  <p className="mt-2 break-words text-xs text-danger/70">{upscalerError}</p>
                ) : null}
              </div>
            </section>

            <section data-tour-id="sd-install" className="border-t border-fg/8 pt-5">
              <h2 className="text-sm font-semibold text-fg">
                {inventory?.installed.length
                  ? t("imageGeneration.local.engineManager.addVersion")
                  : t("imageGeneration.local.engineManager.installTitle")}
              </h2>
              {catalogError ? (
                <div
                  aria-live="polite"
                  className="mt-3 rounded-lg border border-danger/25 bg-danger/5 px-4 py-3 text-sm text-danger/80"
                >
                  <div className="font-medium">
                    {t("imageGeneration.local.engineManager.catalogFailed")}
                  </div>
                  <div className="mt-1 break-words text-xs">{catalogError}</div>
                </div>
              ) : catalogLoading ? (
                <div
                  role="status"
                  aria-busy="true"
                  className="mt-3 overflow-hidden rounded-lg border border-fg/10 bg-fg/[0.02]"
                >
                  <span className="sr-only">
                    {t("imageGeneration.local.engineLoadingVariants")}
                  </span>
                  <div className="grid items-end gap-3 px-4 py-4 md:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)_auto]">
                    <label className="block">
                      <span className="mb-1.5 block text-xs font-medium text-fg/60">
                        {t("imageGeneration.local.engineManager.version")}
                      </span>
                      <div className="flex h-10 w-full items-center rounded-lg border border-fg/12 bg-surface px-3 text-sm text-fg/35">
                        {t("common.labels.loading")}
                      </div>
                    </label>
                    <label className="block">
                      <span className="mb-1.5 block text-xs font-medium text-fg/60">
                        {t("imageGeneration.local.engineManager.variant")}
                      </span>
                      <div className="flex h-10 w-full items-center rounded-lg border border-fg/12 bg-surface px-3 text-sm text-fg/35">
                        {t("common.labels.loading")}
                      </div>
                    </label>
                    <Skeleton className="h-10 md:w-36" />
                  </div>
                  <div className="flex items-center gap-2.5 border-t border-fg/8 px-4 py-3">
                    <Skeleton className="h-4 w-4 shrink-0 rounded" />
                    <Skeleton className="h-3 w-full max-w-md" />
                  </div>
                </div>
              ) : catalog && !catalog.runtimeSupported ? (
                <p className="mt-3 text-sm text-warning">
                  {catalog.unsupportedReason || t("imageGeneration.local.hub.unsupportedTitle")}
                </p>
              ) : release && asset && recommendedAsset ? (
                <div className="mt-3 overflow-hidden rounded-lg border border-fg/10 bg-fg/[0.02]">
                  <div className="grid items-end gap-3 px-4 py-4 md:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)_auto]">
                    <label className="block">
                      <span className="mb-1.5 block text-xs font-medium text-fg/60">
                        {t("imageGeneration.local.engineManager.version")}
                      </span>
                      <select
                        name="sdcpp-engine-version"
                        autoComplete="off"
                        value={release.tag}
                        onChange={(event) => setSelectedRelease(event.target.value)}
                        className={cn(
                          "h-10 w-full rounded-lg border border-fg/12 bg-surface px-3 text-sm text-fg",
                          focusRing,
                        )}
                      >
                        {catalog?.runtimeReleases.map((candidate) => (
                          <option key={candidate.tag} value={candidate.tag}>
                            {candidate.tag}
                            {candidate.prerelease ? " (pre-release)" : ""}
                          </option>
                        ))}
                      </select>
                    </label>

                    <label className="block">
                      <span className="mb-1.5 block text-xs font-medium text-fg/60">
                        {t("imageGeneration.local.engineManager.variant")}
                      </span>
                      <select
                        name="sdcpp-engine-variant"
                        autoComplete="off"
                        value={asset.name}
                        onChange={(event) => setSelectedAsset(event.target.value)}
                        className={cn(
                          "h-10 w-full rounded-lg border border-fg/12 bg-surface px-3 text-sm text-fg",
                          focusRing,
                        )}
                      >
                        {release.assets.map((candidate) => (
                          <option key={candidate.name} value={candidate.name}>
                            {candidate.backend.toUpperCase()} ·{" "}
                            {formatBytes(totalAssetBytes(candidate))}
                            {candidate.name === recommendedAsset.name
                              ? ` · ${t("imageGeneration.local.engineManager.recommended")}`
                              : ""}
                          </option>
                        ))}
                      </select>
                    </label>
                    <button
                      type="button"
                      onClick={() => void install()}
                      disabled={installing || hasActiveInstall || assetInstalled}
                      className={cn(
                        "inline-flex h-10 items-center justify-center gap-2 rounded-lg px-4 text-sm font-semibold transition-[filter] md:min-w-36",
                        assetInstalled
                          ? "cursor-default border border-fg/12 bg-fg/5 text-fg/55"
                          : "bg-accent text-bg hover:brightness-110 disabled:opacity-50",
                        focusRing,
                      )}
                    >
                      {assetInstalled ? (
                        <Check aria-hidden="true" className="h-4 w-4 text-success" />
                      ) : installing || hasActiveInstall ? (
                        <Loader2
                          aria-hidden="true"
                          className="h-4 w-4 animate-spin motion-reduce:animate-none"
                        />
                      ) : (
                        <Download aria-hidden="true" className="h-4 w-4" />
                      )}
                      {assetInstalled
                        ? t("imageGeneration.local.engineManager.alreadyInstalled")
                        : t("imageGeneration.local.engineManager.install")}
                    </button>
                  </div>

                  <div className="flex flex-col gap-2 border-t border-fg/8 px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
                    <div className="flex min-w-0 items-start gap-2.5">
                      <Cpu aria-hidden="true" className="mt-0.5 h-4 w-4 shrink-0 text-fg/40" />
                      <p className="min-w-0 text-xs leading-5 text-fg/50">
                        <span className="font-medium text-fg/75">
                          {asset.backend.toUpperCase()}
                        </span>
                        <span className="mx-1.5 text-fg/25">·</span>
                        {asset.name === recommendedAsset.name
                          ? discreteGpu
                            ? t("imageGeneration.local.engineManager.detectedGpu", {
                                device: discreteGpu.description || discreteGpu.name,
                              })
                            : t("imageGeneration.local.engineManager.noDiscreteGpu")
                          : t("imageGeneration.local.engineManager.manualVariant", {
                              backend: recommendedAsset.backend.toUpperCase(),
                            })}
                      </p>
                    </div>
                    <div className="flex shrink-0 items-center gap-2 pl-6 text-xs text-fg/38 sm:pl-0">
                      <span className="max-w-72 truncate" title={asset.name}>
                        {asset.name}
                      </span>
                      <span aria-hidden="true">·</span>
                      <span className="tabular-nums">{formatBytes(totalAssetBytes(asset))}</span>
                    </div>
                  </div>
                </div>
              ) : null}
            </section>
          </div>

          <div className="space-y-6">
            <section data-tour-id="sd-models" className="border-t border-fg/8 pt-5 lg:border-t-0 lg:pt-0">
              <div className="flex items-start justify-between gap-4">
                <div className="min-w-0">
                  <h2 className="text-sm font-semibold text-fg">
                    {t("imageGeneration.local.modelLibrary.title")}
                  </h2>
                  <p className="mt-1 max-w-2xl text-sm leading-6 text-fg/50">
                    {t("imageGeneration.local.modelLibrary.description")}
                  </p>
                </div>
                <button
                  type="button"
                  onClick={() => void refreshModelHardware()}
                  disabled={modelHardwareRefreshing || catalogLoading}
                  title={t("imageGeneration.local.modelLibrary.refreshHardwareDescription")}
                  className={cn(
                    "inline-flex h-8 shrink-0 items-center gap-2 rounded-lg border border-fg/12 px-3 text-xs font-medium text-fg/60 transition-colors hover:bg-fg/5 hover:text-fg/80 disabled:opacity-50",
                    focusRing,
                  )}
                >
                  <RefreshCw
                    aria-hidden="true"
                    className={cn(
                      "h-3.5 w-3.5",
                      modelHardwareRefreshing && "animate-spin motion-reduce:animate-none",
                    )}
                  />
                  {modelHardwareRefreshing
                    ? t("imageGeneration.local.modelLibrary.refreshingHardware")
                    : t("imageGeneration.local.modelLibrary.refreshHardware")}
                </button>
              </div>

              {downloadQueue && modelInstallItemIds.size > 0 ? (
                <div className="mt-3">
                  <InlineDownloadCards filter={(item) => modelInstallItemIds.has(item.id)} />
                </div>
              ) : null}

              {catalogError ? (
                <div
                  aria-live="polite"
                  className="mt-3 rounded-lg border border-danger/25 bg-danger/5 px-4 py-3 text-sm text-danger/80"
                >
                  <div className="font-medium">
                    {t("imageGeneration.local.engineManager.catalogFailed")}
                  </div>
                  <div className="mt-1 break-words text-xs">{catalogError}</div>
                </div>
              ) : catalogLoading ? (
                <div role="status" aria-busy="true" className="mt-3 space-y-3">
                  <span className="sr-only">{t("common.labels.loading")}</span>
                  {[0, 1].map((row) => (
                    <div
                      key={row}
                      className="flex items-center justify-between gap-4 rounded-lg border border-fg/10 bg-fg/[0.02] px-4 py-4"
                    >
                      <div className="min-w-0 space-y-2">
                        <Skeleton className="h-4 w-40" />
                        <Skeleton className="h-3 w-64" />
                      </div>
                      <Skeleton className="h-10 w-32" />
                    </div>
                  ))}
                </div>
              ) : catalog && !catalog.runtimeSupported ? (
                <p className="mt-3 text-sm text-warning">
                  {catalog.unsupportedReason || t("imageGeneration.local.hub.unsupportedTitle")}
                </p>
              ) : catalog && catalog.profiles.length > 0 ? (
                <div className="mt-3 space-y-3">
                  {!hasInstalledEngine ? (
                    <p className="text-xs leading-5 text-fg/45">
                      {t("imageGeneration.local.modelLibrary.engineNeededForFit")}
                    </p>
                  ) : null}
                  {modelFamilies.map((family, familyIndex) => {
                    const installedCount = family.profiles.filter((profile) =>
                      installedProfileIds.has(profile.id),
                    ).length;
                    const open =
                      familyOverrides[family.name] ?? (familyIndex === 0 || installedCount > 0);
                    return (
                      <div
                        key={family.name}
                        className="overflow-hidden rounded-lg border border-fg/10 bg-fg/[0.02]"
                      >
                        <button
                          type="button"
                          onClick={() =>
                            setFamilyOverrides((current) => ({ ...current, [family.name]: !open }))
                          }
                          aria-expanded={open}
                          className={cn(
                            "flex w-full items-center justify-between gap-3 px-4 py-3 text-left transition-colors hover:bg-fg/[0.025]",
                            focusRing,
                          )}
                        >
                          <span className="flex min-w-0 items-center gap-2">
                            <ChevronRight
                              aria-hidden="true"
                              className={cn(
                                "h-4 w-4 shrink-0 text-fg/40 transition-transform",
                                open && "rotate-90",
                              )}
                            />
                            <span className="truncate text-sm font-semibold text-fg">
                              {family.name}
                            </span>
                            <span className="shrink-0 text-xs text-fg/40">
                              {family.profiles.length === 1
                                ? t("imageGeneration.local.modelLibrary.familyModelCountOne")
                                : t("imageGeneration.local.modelLibrary.familyModelCount", {
                                    count: family.profiles.length,
                                  })}
                            </span>
                          </span>
                          {installedCount > 0 ? (
                            <span className="shrink-0 text-xs font-medium text-success">
                              {t("imageGeneration.local.modelLibrary.familyInstalledCount", {
                                count: installedCount,
                              })}
                            </span>
                          ) : null}
                        </button>
                        {open ? (
                          <div className="divide-y divide-fg/8 border-t border-fg/8">
                            {family.profiles.map((profile) => {
                              const variant = selectedVariant(profile, selectedVariants);
                              if (!variant) return null;
                              const key = `${profile.id}:${variant.id}`;
                              const installed = installedModelKeys.has(key);
                              const installing =
                                installingModelKey === key || activeModelKeys.has(key);
                              const uninstalling = uninstallingModelKey === key;
                              const runKey = fitKey(profile.id, variant.id);
                              const fit = runKey ? runnability[runKey] : undefined;
                              const requiresEngineUpdate = Boolean(
                                runtimePairing &&
                                !runtimeSupportsProfile(profile, runtimePairing.release),
                              );
                              const fitInfo = requiresEngineUpdate
                                ? ({ tone: "warn", key: "engineUpdateRequired" } as const)
                                : installed
                                  ? null
                                  : classifyFit(fit);
                              const resolvedFit = fit && fit !== "loading" ? fit : null;
                              const estimate = resolvedFit?.estimate ?? null;
                              return (
                                <div key={profile.id}>
                                  <div className="px-4 py-4">
                                    <div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
                                      <h3 className="text-sm font-semibold text-fg">
                                        {profile.displayName}
                                      </h3>
                                      {fitInfo ? (
                                        <span
                                          title={fit && fit !== "loading" ? fit.reason : undefined}
                                          className={cn(
                                            "inline-flex items-center gap-1.5 text-xs font-medium",
                                            FIT_TEXT[fitInfo.tone],
                                          )}
                                        >
                                          <span
                                            aria-hidden="true"
                                            className={cn(
                                              "h-1.5 w-1.5 rounded-full",
                                              FIT_DOT[fitInfo.tone],
                                            )}
                                          />
                                          {fitInfo.key === "engineUpdateRequired"
                                            ? t(
                                                "imageGeneration.local.modelLibrary.engineUpdateRequired",
                                                {
                                                  build: profile.minimumRuntimeBuild ?? "—",
                                                },
                                              )
                                            : t(
                                                `imageGeneration.local.modelLibrary.${fitInfo.key}` as TranslationKey,
                                              )}
                                        </span>
                                      ) : null}
                                    </div>
                                    <p className="mt-1 text-xs leading-5 text-fg/50">
                                      {profile.description}
                                    </p>
                                    {profile.requiresReferenceImage ? (
                                      <p className="mt-1 text-xs leading-5 text-fg/40">
                                        {t("imageGeneration.local.modelLibrary.needsReference")}
                                      </p>
                                    ) : null}
                                    <div className="mt-3 flex flex-col gap-3 sm:flex-row sm:items-end">
                                      <label className="block sm:flex-1">
                                        <span className="mb-1.5 block text-xs font-medium text-fg/60">
                                          {t("imageGeneration.local.modelLibrary.variant")}
                                        </span>
                                        <select
                                          name={`sdcpp-model-${profile.id}`}
                                          autoComplete="off"
                                          value={variant.id}
                                          onChange={(event) =>
                                            setSelectedVariants((current) => ({
                                              ...current,
                                              [profile.id]: event.target.value,
                                            }))
                                          }
                                          className={cn(
                                            "h-10 w-full rounded-lg border border-fg/12 bg-surface px-3 text-sm text-fg",
                                            focusRing,
                                          )}
                                        >
                                          {profile.variants.map((candidate) => (
                                            <option key={candidate.id} value={candidate.id}>
                                              {candidate.label} ·{" "}
                                              {formatBytes(candidate.downloadBytes)}
                                              {installedModelKeys.has(
                                                `${profile.id}:${candidate.id}`,
                                              )
                                                ? ` · ${t("imageGeneration.local.modelLibrary.installedShort")}`
                                                : ""}
                                            </option>
                                          ))}
                                        </select>
                                      </label>
                                      <div className="flex items-center gap-2">
                                        {installed ? (
                                          <>
                                            <span className="inline-flex h-10 items-center gap-2 rounded-lg border border-fg/12 bg-fg/5 px-4 text-sm font-semibold text-fg/55">
                                              <Check
                                                aria-hidden="true"
                                                className="h-4 w-4 text-success"
                                              />
                                              {t(
                                                "imageGeneration.local.modelLibrary.installedShort",
                                              )}
                                            </span>
                                            <button
                                              type="button"
                                              onClick={() => void uninstallModel(profile, variant)}
                                              disabled={uninstalling}
                                              aria-label={t(
                                                "imageGeneration.local.modelLibrary.uninstallModel",
                                                {
                                                  model: profile.displayName,
                                                },
                                              )}
                                              className={cn(
                                                "rounded-lg p-2 text-fg/42 transition-colors hover:bg-danger/8 hover:text-danger disabled:opacity-50",
                                                focusRing,
                                              )}
                                            >
                                              {uninstalling ? (
                                                <Loader2
                                                  aria-hidden="true"
                                                  className="h-4 w-4 animate-spin motion-reduce:animate-none"
                                                />
                                              ) : (
                                                <Trash2 aria-hidden="true" className="h-4 w-4" />
                                              )}
                                            </button>
                                          </>
                                        ) : (
                                          <button
                                            type="button"
                                            onClick={() => void installModel(profile, variant, fit)}
                                            disabled={
                                              installing || !runtimePairing || requiresEngineUpdate
                                            }
                                            title={
                                              !runtimePairing
                                                ? t("imageGeneration.local.modelLibrary.noRuntime")
                                                : requiresEngineUpdate
                                                  ? t(
                                                      "imageGeneration.local.modelLibrary.engineUpdateRequired",
                                                      {
                                                        build: profile.minimumRuntimeBuild ?? "—",
                                                      },
                                                    )
                                                  : undefined
                                            }
                                            className={cn(
                                              "inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-accent px-4 text-sm font-semibold text-bg transition-[filter] hover:brightness-110 disabled:opacity-50 sm:min-w-36",
                                              focusRing,
                                            )}
                                          >
                                            {installing ? (
                                              <Loader2
                                                aria-hidden="true"
                                                className="h-4 w-4 animate-spin motion-reduce:animate-none"
                                              />
                                            ) : (
                                              <Download aria-hidden="true" className="h-4 w-4" />
                                            )}
                                            {installing
                                              ? t("imageGeneration.local.modelLibrary.installing")
                                              : requiresEngineUpdate
                                                ? t(
                                                    "imageGeneration.local.modelLibrary.updateEngineFirst",
                                                  )
                                                : runtimePairing
                                                  ? t("imageGeneration.local.modelLibrary.download")
                                                  : t(
                                                      "imageGeneration.local.modelLibrary.engineRequired",
                                                    )}
                                          </button>
                                        )}
                                      </div>
                                    </div>
                                  </div>

                                  <div className="flex min-h-11 flex-wrap items-center justify-between gap-x-4 gap-y-2 border-t border-fg/8 px-4 py-2 text-xs text-fg/45">
                                    <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                                      <span>{profile.license}</span>
                                      {profile.sourceUrl ? (
                                        <>
                                          <span aria-hidden="true">·</span>
                                          <button
                                            type="button"
                                            onClick={() => void openExternalUrl(profile.sourceUrl)}
                                            className={cn(
                                              "underline-offset-2 transition-colors hover:text-fg/70 hover:underline",
                                              focusRing,
                                            )}
                                          >
                                            {t("imageGeneration.local.modelLibrary.modelCard")}
                                          </button>
                                        </>
                                      ) : null}
                                    </div>
                                    {estimate && resolvedFit ? (
                                      <button
                                        type="button"
                                        onClick={() =>
                                          setFitDetails({
                                            modelName: profile.displayName,
                                            variantName: variant.label,
                                            fit: resolvedFit,
                                          })
                                        }
                                        className={cn(
                                          "inline-flex h-7 items-center gap-1.5 rounded-md px-2 font-medium text-fg/55 transition-colors hover:bg-fg/5 hover:text-fg/80",
                                          focusRing,
                                        )}
                                      >
                                        {t("imageGeneration.local.modelLibrary.more")}
                                        <ChevronRight aria-hidden="true" className="h-3.5 w-3.5" />
                                      </button>
                                    ) : null}
                                  </div>
                                </div>
                              );
                            })}
                          </div>
                        ) : null}
                      </div>
                    );
                  })}
                </div>
              ) : null}
            </section>
          </div>
        </div>
      </div>
      <RunnabilityDetailsMenu selection={fitDetails} onClose={() => setFitDetails(null)} />
      <ComputePolicyMenu
        info={computePolicyInfo}
        isOpen={computePolicyOpen}
        saving={computePolicySaving}
        onClose={() => setComputePolicyOpen(false)}
        onSave={(policy) => void saveComputePolicy(policy)}
      />

      {showEngineTour && <GuidedTour tour="sdEngine" onDismiss={dismissEngineTour} />}
    </main>
  );
}
