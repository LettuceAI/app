import { useCallback, useEffect, useState } from "react";
import {
  Brain,
  Check,
  ChevronDown,
  Cpu,
  FolderInput,
  FolderOpen,
  Gauge,
  HardDrive,
  KeyRound,
  Layers,
  ListOrdered,
  Loader2,
  MemoryStick,
  Pin,
  RotateCcw,
  Scale,
  Sparkles,
  X,
  type LucideIcon,
} from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";

import { BottomMenu, MenuButton, MenuButtonGroup, MenuDivider } from "../../components/BottomMenu";
import { GuidedTour, useGuidedTour } from "../../components/GuidedTour";
import { HfTokenMenu } from "./components/HfTokenMenu";
import { CivitaiTokenMenu, type CivitaiAuthStatus } from "./components/CivitaiTokenMenu";
import { Switch } from "../../components/Switch";
import type { AuthStatus } from "./components/imageBundle/types";

import {
  addOrUpdateModel,
  readSettings,
  saveAdvancedModelSettings,
  saveAdvancedSettings,
} from "../../../core/storage/repo";
import { useI18n } from "../../../core/i18n/context";
import { cn, interactive } from "../../design-tokens";
import { toast } from "../../components/toast";
import { NumberInput } from "../../components/NumberInput";
import {
  ADVANCED_LLAMA_N_PEN_RANGE,
  ADVANCED_LLAMA_REPEAT_PENALTY_RANGE,
} from "../../components/AdvancedModelSettingsForm";

type RuntimeDefaults = {
  llamaDefaultContextLength: number | null;
  llamaDefaultKvCacheType: "auto" | "f16" | "q8_0" | "q4_0";
  llamaMultiGpuEnabled: boolean | null;
  llamaGpuDeviceIds: number[] | null;
  llamaGpuDistributionMode: "balanced" | "proportional" | "priority" | "manual" | null;
  llamaKvPlacement: "auto" | "split" | "systemRam" | "pin" | null;
  llamaMainGpu: number | null;
  llamaPriorityVramLimitBytes: number | null;
  llamaBatchSize: number | null;
  llamaUbatchSize: number | null;
  llamaRepeatPenalty: number | null;
  llamaNPenRange: number | null;
};

type LlamaGpuDevice = {
  index: number;
  name: string;
  description: string;
  backend: string;
  memoryTotal: number;
  memoryFree: number;
  deviceType: string;
};

type ModelDirKind = "llm";

type LlmModelsDirInfo = {
  path: string;
  defaultPath: string;
  isCustom: boolean;
  modelCount: number;
};

type SetLlmModelsDirResult = {
  path: string;
  movedEntries: number;
  rewiredModels: number;
};

function SectionHeading({ label }: { label: string }) {
  return (
    <h3 className="px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
      {label}
    </h3>
  );
}

function SettingRow({
  icon,
  iconClassName,
  title,
  description,
  children,
}: {
  icon: React.ReactNode;
  iconClassName: string;
  title: string;
  description: string;
  children: React.ReactNode;
}) {
  return (
    <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-3">
          <div className={cn("rounded-lg border p-1.5", iconClassName)}>{icon}</div>
          <div className="min-w-0">
            <span className="text-sm font-medium text-fg">{title}</span>
            <p className="text-[11px] text-fg/45">{description}</p>
          </div>
        </div>
        <div className="shrink-0">{children}</div>
      </div>
    </div>
  );
}

const controlClassName =
  "rounded-xl border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg transition hover:bg-surface-el/30 focus:border-fg/25 focus:outline-none";

function SelectTrigger({
  label,
  onClick,
  className,
}: {
  label: string;
  onClick: () => void;
  className?: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        controlClassName,
        "flex shrink-0 items-center justify-between gap-2 text-left",
        className,
      )}
    >
      <span className="truncate">{label}</span>
      <ChevronDown className="h-4 w-4 shrink-0 text-fg/40" />
    </button>
  );
}

export function LocalRuntimeDefaultsPage() {
  const { t } = useI18n();
  const [defaults, setDefaults] = useState<RuntimeDefaults | null>(null);
  const [modelsDir, setModelsDir] = useState<LlmModelsDirInfo | null>(null);
  const [gpuDevices, setGpuDevices] = useState<LlamaGpuDevice[]>([]);
  const [pending, setPending] = useState<{ kind: ModelDirKind; dir: string } | null>(null);
  const [movingDir, setMovingDir] = useState(false);
  const [openMenu, setOpenMenu] = useState<"distribution" | "kvCache" | "pinnedGpu" | null>(null);
  const [reconfigureKind, setReconfigureKind] = useState<"multi" | "single" | null>(null);
  const [reconfigureCount, setReconfigureCount] = useState(0);
  const [reconfiguring, setReconfiguring] = useState(false);
  const [hfAuth, setHfAuth] = useState<AuthStatus | null>(null);
  const [hfMenuOpen, setHfMenuOpen] = useState(false);
  const [civitaiAuth, setCivitaiAuth] = useState<CivitaiAuthStatus | null>(null);
  const [civitaiMenuOpen, setCivitaiMenuOpen] = useState(false);
  const { shouldShow: showTour, dismiss: dismissTour } = useGuidedTour("runtimeDefaults");

  const refreshModelsDir = useCallback(async () => {
    try {
      setModelsDir(await invoke<LlmModelsDirInfo>("hf_get_llm_models_dir"));
    } catch (err) {
      console.error("Failed to load LLM models dir:", err);
    }
  }, []);

  const applyModelsDir = useCallback(
    async (kind: ModelDirKind, newDir: string, moveExisting: boolean) => {
      void kind;
      setMovingDir(true);
      try {
        const result = await invoke<SetLlmModelsDirResult>("hf_set_llm_models_dir", {
          newDir,
          moveExisting,
        });
        await refreshModelsDir();
        if (moveExisting && result.movedEntries > 0) {
          toast.success(
            t("runtimeDefaults.folderChanged"),
            t("runtimeDefaults.folderChangedMoved", { count: result.movedEntries }),
          );
        } else {
          toast.success(t("runtimeDefaults.folderChanged"));
        }
      } catch (err) {
        toast.error(
          t("runtimeDefaults.folderChangeFailed"),
          err instanceof Error ? err.message : String(err),
        );
      } finally {
        setMovingDir(false);
        setPending(null);
      }
    },
    [refreshModelsDir, t],
  );

  const pickModelsFolder = useCallback(
    async (kind: ModelDirKind) => {
      const info = modelsDir;
      const selection = await open({ directory: true, multiple: false });
      if (typeof selection !== "string") return;
      if (info && selection === info.path) return;
      if (info && info.modelCount > 0) {
        setPending({ kind, dir: selection });
      } else {
        void applyModelsDir(kind, selection, false);
      }
    },
    [modelsDir, applyModelsDir],
  );

  const resetModelsFolder = useCallback(
    (kind: ModelDirKind) => {
      const info = modelsDir;
      if (!info) return;
      if (info.modelCount > 0) {
        setPending({ kind, dir: info.defaultPath });
      } else {
        void applyModelsDir(kind, info.defaultPath, false);
      }
    },
    [modelsDir, applyModelsDir],
  );

  const pendingInfo = pending ? modelsDir : null;

  useEffect(() => {
    void refreshModelsDir();
    readSettings()
      .then((settings) => {
        const advanced = settings.advancedSettings ?? {};
        const advancedModel = settings.advancedModelSettings ?? {};
        setDefaults({
          llamaDefaultContextLength: advanced.llamaDefaultContextLength ?? null,
          llamaDefaultKvCacheType: advanced.llamaDefaultKvCacheType ?? "auto",
          llamaMultiGpuEnabled: advancedModel.llamaMultiGpuEnabled ?? null,
          llamaGpuDeviceIds: advancedModel.llamaGpuDeviceIds ?? null,
          llamaGpuDistributionMode: advancedModel.llamaGpuDistributionMode ?? null,
          llamaKvPlacement: advancedModel.llamaKvPlacement ?? null,
          llamaMainGpu: advancedModel.llamaMainGpu ?? null,
          llamaPriorityVramLimitBytes: advancedModel.llamaPriorityVramLimitBytes ?? null,
          llamaBatchSize: advancedModel.llamaBatchSize ?? null,
          llamaUbatchSize: advancedModel.llamaUbatchSize ?? null,
          llamaRepeatPenalty: advancedModel.llamaRepeatPenalty ?? null,
          llamaNPenRange: advancedModel.llamaNPenRange ?? null,
        });
      })
      .catch(() => {});
    invoke<LlamaGpuDevice[]>("llamacpp_backend_devices")
      .then(setGpuDevices)
      .catch(() => setGpuDevices([]));
    invoke<AuthStatus>("hf_auth_status")
      .then(setHfAuth)
      .catch(() => {});
    invoke<CivitaiAuthStatus>("civitai_auth_status")
      .then(setCivitaiAuth)
      .catch(() => {});
  }, [refreshModelsDir]);

  const removeHfToken = useCallback(async () => {
    try {
      await invoke("hf_auth_clear");
      setHfAuth({ saved: false, valid: false, username: null, errorKind: "missingToken" });
    } catch (err) {
      toast.error(
        t("runtimeDefaults.saveFailed"),
        err instanceof Error ? err.message : String(err),
      );
    }
  }, [t]);

  const removeCivitaiToken = useCallback(async () => {
    try {
      await invoke("civitai_auth_clear");
      setCivitaiAuth({ saved: false, valid: false, errorKind: "missingToken" });
    } catch (err) {
      toast.error(
        t("runtimeDefaults.saveFailed"),
        err instanceof Error ? err.message : String(err),
      );
    }
  }, [t]);

  const persistDefaults = useCallback(
    async (next: RuntimeDefaults) => {
      setDefaults(next);
      try {
        const settings = await readSettings();
        await saveAdvancedSettings({
          ...(settings.advancedSettings ?? {}),
          llamaDefaultContextLength: next.llamaDefaultContextLength ?? undefined,
          llamaDefaultKvCacheType:
            next.llamaDefaultKvCacheType === "auto" ? undefined : next.llamaDefaultKvCacheType,
        });
        await saveAdvancedModelSettings({
          ...(settings.advancedModelSettings ?? {}),
          llamaMultiGpuEnabled: next.llamaMultiGpuEnabled === true ? true : undefined,
          llamaGpuDeviceIds:
            next.llamaGpuDeviceIds && next.llamaGpuDeviceIds.length > 0
              ? next.llamaGpuDeviceIds
              : undefined,
          llamaGpuDistributionMode: next.llamaGpuDistributionMode ?? undefined,
          llamaKvPlacement: next.llamaKvPlacement ?? undefined,
          llamaMainGpu: next.llamaMainGpu ?? undefined,
          llamaPriorityVramLimitBytes: next.llamaPriorityVramLimitBytes ?? undefined,
          llamaBatchSize: next.llamaBatchSize ?? undefined,
          llamaUbatchSize: next.llamaUbatchSize ?? undefined,
          llamaRepeatPenalty: next.llamaRepeatPenalty ?? undefined,
          llamaNPenRange: next.llamaNPenRange ?? undefined,
        });
      } catch (err) {
        toast.error(
          t("runtimeDefaults.saveFailed"),
          err instanceof Error ? err.message : String(err),
        );
      }
    },
    [t],
  );

  const renderFolderRow = (kind: ModelDirKind, info: LlmModelsDirInfo | null, title: string) => (
    <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-3">
          <div className="rounded-lg border border-info/30 bg-info/10 p-1.5">
            <HardDrive className="h-4 w-4 text-info/80" />
          </div>
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <span className="text-sm font-medium text-fg">{title}</span>
              <span className="rounded border border-fg/15 bg-fg/10 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-fg/55">
                {info?.isCustom
                  ? t("runtimeDefaults.modelsFolderCustomBadge")
                  : t("runtimeDefaults.modelsFolderDefaultBadge")}
              </span>
            </div>
            <p className="truncate font-mono text-[11px] text-fg/45" title={info?.path}>
              {info?.path ?? ""}
            </p>
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          {info?.isCustom && (
            <button
              type="button"
              onClick={() => resetModelsFolder(kind)}
              disabled={movingDir}
              title={t("runtimeDefaults.modelsFolderReset")}
              className="rounded-lg p-2 text-fg/40 transition-colors hover:bg-fg/10 hover:text-fg/70 disabled:opacity-50"
            >
              <RotateCcw className="h-4 w-4" />
            </button>
          )}
          <button
            type="button"
            onClick={() => void pickModelsFolder(kind)}
            disabled={movingDir}
            className="inline-flex items-center gap-2 rounded-xl border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm font-medium text-fg/85 transition hover:bg-surface-el/30 disabled:opacity-50"
          >
            {movingDir ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <FolderOpen className="h-4 w-4 text-fg/45" />
            )}
            {t("runtimeDefaults.modelsFolderChange")}
          </button>
        </div>
      </div>
    </div>
  );

  if (!defaults) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-fg/10 border-t-fg/60" />
      </div>
    );
  }

  const selectedGpuIds = defaults.llamaGpuDeviceIds ?? [];
  const eligibleGpuDevices = gpuDevices.filter((device) => device.deviceType !== "IntegratedGpu");
  const multiGpuAvailable = eligibleGpuDevices.length >= 2;
  const selectedEligibleDevices = eligibleGpuDevices.filter((device) =>
    selectedGpuIds.includes(device.index),
  );
  const distributionOptions: {
    value: NonNullable<RuntimeDefaults["llamaGpuDistributionMode"]>;
    label: string;
    description: string;
    icon: LucideIcon;
  }[] = [
    {
      value: "balanced",
      label: t("runtimeDefaults.llamaDistBalanced"),
      description: t("runtimeDefaults.llamaDistBalancedDesc"),
      icon: Scale,
    },
    {
      value: "proportional",
      label: t("runtimeDefaults.llamaDistProportional"),
      description: t("runtimeDefaults.llamaDistProportionalDesc"),
      icon: Gauge,
    },
    {
      value: "priority",
      label: t("runtimeDefaults.llamaDistPriority"),
      description: t("runtimeDefaults.llamaDistPriorityDesc"),
      icon: ListOrdered,
    },
  ];
  const kvPlacementOptions: {
    value: NonNullable<RuntimeDefaults["llamaKvPlacement"]>;
    label: string;
    description: string;
    icon: LucideIcon;
  }[] = [
    {
      value: "auto",
      label: t("runtimeDefaults.llamaKvAuto"),
      description: t("runtimeDefaults.llamaKvAutoDesc"),
      icon: Sparkles,
    },
    {
      value: "split",
      label: t("runtimeDefaults.llamaKvSplit"),
      description: t("runtimeDefaults.llamaKvSplitDesc"),
      icon: Layers,
    },
    {
      value: "systemRam",
      label: t("runtimeDefaults.llamaKvSystemRam"),
      description: t("runtimeDefaults.llamaKvSystemRamDesc"),
      icon: MemoryStick,
    },
    {
      value: "pin",
      label: t("runtimeDefaults.llamaKvPin"),
      description: t("runtimeDefaults.llamaKvPinDesc"),
      icon: Pin,
    },
  ];
  const currentDistribution = defaults?.llamaGpuDistributionMode ?? "balanced";
  const currentKvPlacement = defaults?.llamaKvPlacement ?? "auto";
  const distributionLabel =
    distributionOptions.find((opt) => opt.value === currentDistribution)?.label ?? "";
  const kvPlacementLabel =
    kvPlacementOptions.find((opt) => opt.value === currentKvPlacement)?.label ?? "";
  const pinnedGpuIndex = defaults?.llamaMainGpu ?? selectedEligibleDevices[0]?.index ?? 0;
  const pinnedGpuDevice = selectedEligibleDevices.find((device) => device.index === pinnedGpuIndex);
  const pinnedGpuLabel = pinnedGpuDevice
    ? pinnedGpuDevice.description || pinnedGpuDevice.name || `GPU ${pinnedGpuDevice.index}`
    : "";
  const reconfigureGpuDevice =
    gpuDevices.find((device) => device.index === pinnedGpuIndex) ?? pinnedGpuDevice;
  const reconfigureGpuLabel = reconfigureGpuDevice
    ? reconfigureGpuDevice.description ||
      reconfigureGpuDevice.name ||
      `GPU ${reconfigureGpuDevice.index}`
    : `GPU ${pinnedGpuIndex}`;

  const openReconfigureMenu = async (kind: "multi" | "single") => {
    try {
      const settings = await readSettings();
      const count = settings.models.filter((model) => model.providerId === "llamacpp").length;
      if (count === 0) return;
      setReconfigureCount(count);
      setReconfigureKind(kind);
    } catch {}
  };

  const runReconfigure = async () => {
    if (!reconfigureKind || reconfiguring) return;
    setReconfiguring(true);
    try {
      const settings = await readSettings();
      const localModels = settings.models.filter((model) => model.providerId === "llamacpp");
      for (const model of localModels) {
        await addOrUpdateModel({
          ...model,
          advancedModelSettings: {
            ...(model.advancedModelSettings ?? {}),
            llamaGpuLayers: null,
            llamaMultiGpuEnabled: null,
            llamaSingleGpuDeviceId: reconfigureKind === "single" ? pinnedGpuIndex : null,
          },
        });
      }
      toast.success(
        t("runtimeDefaults.reconfigureDone"),
        t("runtimeDefaults.reconfigureDoneBody", { count: localModels.length }),
      );
      setReconfigureKind(null);
    } catch (err) {
      toast.error(
        t("runtimeDefaults.saveFailed"),
        err instanceof Error ? err.message : String(err),
      );
    } finally {
      setReconfiguring(false);
    }
  };
  const toggleGpuDevice = (index: number) => {
    const nextIds = selectedGpuIds.includes(index)
      ? selectedGpuIds.filter((id) => id !== index)
      : [...selectedGpuIds, index].sort((a, b) => a - b);
    void persistDefaults({
      ...defaults,
      llamaGpuDeviceIds: nextIds.length > 0 ? nextIds : null,
    });
  };

  return (
    <div className="flex min-h-screen flex-col">
      <main className="flex-1 px-4 pb-24 pt-4">
        <div className="mx-auto w-full max-w-5xl space-y-6">
          <div className="space-y-4" data-tour-id="runtime-defaults-storage">
            <SectionHeading label={t("runtimeDefaults.storageSection")} />
            <p className="px-1 text-xs text-fg/50">{t("runtimeDefaults.storageDescription")}</p>

            <div className="space-y-3">
              {renderFolderRow("llm", modelsDir, t("runtimeDefaults.modelsFolderTitle"))}
            </div>
          </div>

          <div className="space-y-4">
            <SectionHeading label={t("hfBrowser.bundle.hfAccess")} />
            <p className="px-1 text-xs text-fg/50">{t("hfAuth.manageBody")}</p>

            <SettingRow
              icon={<KeyRound className="h-4 w-4 text-info/80" />}
              iconClassName="border-info/30 bg-info/10"
              title={t("hfBrowser.bundle.tokenPlaceholder")}
              description={
                hfAuth === null
                  ? "…"
                  : hfAuth.valid && hfAuth.username
                    ? t("hfAuth.statusConnected", { username: hfAuth.username })
                    : hfAuth.saved
                      ? t("hfAuth.statusInvalid")
                      : t("hfAuth.statusMissing")
              }
            >
              <div className="flex shrink-0 items-center gap-1.5">
                {hfAuth?.saved && (
                  <button
                    type="button"
                    onClick={() => void removeHfToken()}
                    title={t("hfAuth.remove")}
                    className="rounded-lg p-2 text-fg/40 transition-colors hover:bg-fg/10 hover:text-fg/70"
                  >
                    <X className="h-4 w-4" />
                  </button>
                )}
                <button
                  type="button"
                  onClick={() => setHfMenuOpen(true)}
                  className="inline-flex items-center gap-2 rounded-xl border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm font-medium text-fg/85 transition hover:bg-surface-el/30"
                >
                  <KeyRound className="h-4 w-4 text-fg/45" />
                  {hfAuth?.saved ? t("hfAuth.replace") : t("hfAuth.add")}
                </button>
              </div>
            </SettingRow>
          </div>

          <div data-tour-id="runtime-defaults-civitai" className="space-y-4">
            <SectionHeading label={t("loraLibrary.tokenTitle")} />
            <p className="px-1 text-xs text-fg/50">{t("loraLibrary.tokenBody")}</p>

            <SettingRow
              icon={<KeyRound className="h-4 w-4 text-info/80" />}
              iconClassName="border-info/30 bg-info/10"
              title={t("loraLibrary.tokenPlaceholder")}
              description={
                civitaiAuth === null
                  ? "…"
                  : civitaiAuth.valid
                    ? t("loraLibrary.tokenStatusSaved")
                    : civitaiAuth.saved
                      ? civitaiAuth.errorKind === "unverified"
                        ? t("loraLibrary.tokenUnverified")
                        : t("loraLibrary.tokenStatusInvalid")
                      : t("loraLibrary.tokenStatusMissing")
              }
            >
              <div className="flex shrink-0 items-center gap-1.5">
                {civitaiAuth?.saved && (
                  <button
                    type="button"
                    onClick={() => void removeCivitaiToken()}
                    title={t("loraLibrary.tokenRemove")}
                    className="rounded-lg p-2 text-fg/40 transition-colors hover:bg-fg/10 hover:text-fg/70"
                  >
                    <X className="h-4 w-4" />
                  </button>
                )}
                <button
                  type="button"
                  onClick={() => setCivitaiMenuOpen(true)}
                  className="inline-flex items-center gap-2 rounded-xl border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm font-medium text-fg/85 transition hover:bg-surface-el/30"
                >
                  <KeyRound className="h-4 w-4 text-fg/45" />
                  {civitaiAuth?.saved ? t("loraLibrary.tokenReplace") : t("loraLibrary.tokenAdd")}
                </button>
              </div>
            </SettingRow>
          </div>

          <div className="space-y-4" data-tour-id="runtime-defaults-llama">
            <SectionHeading label={t("runtimeDefaults.llamaSection")} />
            <p className="px-1 text-xs text-fg/50">{t("runtimeDefaults.llamaDescription")}</p>

            <SettingRow
              icon={<Brain className="h-4 w-4 text-warning/80" />}
              iconClassName="border-warning/30 bg-warning/10"
              title={t("runtimeDefaults.llamaContextTitle")}
              description={t("runtimeDefaults.llamaContextDescription")}
            >
              <div className="w-28">
                <NumberInput
                  min={512}
                  max={1048576}
                  step={1024}
                  value={defaults.llamaDefaultContextLength}
                  onChange={(next) =>
                    void persistDefaults({
                      ...defaults,
                      llamaDefaultContextLength: next === null ? null : Math.trunc(next),
                    })
                  }
                  placeholder="8192"
                  className={cn(controlClassName, "w-full text-center")}
                />
              </div>
            </SettingRow>

            <SettingRow
              icon={<Gauge className="h-4 w-4 text-warning/80" />}
              iconClassName="border-warning/30 bg-warning/10"
              title={t("runtimeDefaults.llamaBatchTitle")}
              description={t("runtimeDefaults.llamaBatchDescription")}
            >
              <div className="w-28">
                <NumberInput
                  min={1}
                  max={8192}
                  step={128}
                  value={defaults.llamaBatchSize}
                  onChange={(next) =>
                    void persistDefaults({
                      ...defaults,
                      llamaBatchSize: next === null ? null : Math.trunc(next),
                    })
                  }
                  placeholder={t("common.labels.auto")}
                  className={cn(controlClassName, "w-full text-center")}
                />
              </div>
            </SettingRow>

            <SettingRow
              icon={<ListOrdered className="h-4 w-4 text-warning/80" />}
              iconClassName="border-warning/30 bg-warning/10"
              title={t("runtimeDefaults.llamaUbatchTitle")}
              description={t("runtimeDefaults.llamaUbatchDescription")}
            >
              <div className="w-28">
                <NumberInput
                  min={1}
                  max={8192}
                  step={128}
                  value={defaults.llamaUbatchSize}
                  onChange={(next) =>
                    void persistDefaults({
                      ...defaults,
                      llamaUbatchSize: next === null ? null : Math.trunc(next),
                    })
                  }
                  placeholder={t("common.labels.auto")}
                  className={cn(controlClassName, "w-full text-center")}
                />
              </div>
            </SettingRow>

            <SettingRow
              icon={<Layers className="h-4 w-4 text-warning/80" />}
              iconClassName="border-warning/30 bg-warning/10"
              title={t("runtimeDefaults.llamaKvTitle")}
              description={t("runtimeDefaults.llamaKvDescription")}
            >
              <select
                value={defaults.llamaDefaultKvCacheType}
                onChange={(event) =>
                  void persistDefaults({
                    ...defaults,
                    llamaDefaultKvCacheType: event.target
                      .value as RuntimeDefaults["llamaDefaultKvCacheType"],
                  })
                }
                className={controlClassName}
              >
                <option value="auto">{t("common.labels.auto")}</option>
                <option value="f16">F16</option>
                <option value="q8_0">Q8_0</option>
                <option value="q4_0">Q4_0</option>
              </select>
            </SettingRow>

            <SettingRow
              icon={<Sparkles className="h-4 w-4 text-warning/80" />}
              iconClassName="border-warning/30 bg-warning/10"
              title={t("editModel.ollamaParams.repeatPenalty")}
              description={t("editModel.ollamaParams.repeatPenaltyDescription")}
            >
              <div className="w-28">
                <NumberInput
                  min={ADVANCED_LLAMA_REPEAT_PENALTY_RANGE.min}
                  max={ADVANCED_LLAMA_REPEAT_PENALTY_RANGE.max}
                  step={0.01}
                  value={defaults.llamaRepeatPenalty}
                  onChange={(next) =>
                    void persistDefaults({
                      ...defaults,
                      llamaRepeatPenalty: next,
                    })
                  }
                  placeholder="1.00"
                  className={cn(controlClassName, "w-full text-center")}
                />
              </div>
            </SettingRow>

            <SettingRow
              icon={<ListOrdered className="h-4 w-4 text-warning/80" />}
              iconClassName="border-warning/30 bg-warning/10"
              title={t("editModel.llamaSampler.penaltyRange")}
              description={t("editModel.llamaSampler.penaltyRangeDescription")}
            >
              <div className="w-28">
                <NumberInput
                  min={ADVANCED_LLAMA_N_PEN_RANGE.min}
                  max={ADVANCED_LLAMA_N_PEN_RANGE.max}
                  step={1}
                  value={defaults.llamaNPenRange}
                  onChange={(next) =>
                    void persistDefaults({
                      ...defaults,
                      llamaNPenRange: next,
                    })
                  }
                  placeholder="-1"
                  className={cn(controlClassName, "w-full text-center")}
                />
              </div>
            </SettingRow>

            <div data-tour-id="runtime-defaults-multigpu">
            <SettingRow
              icon={<Cpu className="h-4 w-4 text-warning/80" />}
              iconClassName="border-warning/30 bg-warning/10"
              title={t("runtimeDefaults.llamaMultiGpuTitle")}
              description={t("runtimeDefaults.llamaMultiGpuDescription")}
            >
              <Switch
                checked={defaults.llamaMultiGpuEnabled === true && multiGpuAvailable}
                disabled={!multiGpuAvailable}
                onChange={(next) => {
                  void persistDefaults({
                    ...defaults,
                    llamaMultiGpuEnabled: next ? true : null,
                  });
                  void openReconfigureMenu(next ? "multi" : "single");
                }}
                aria-label={t("runtimeDefaults.llamaMultiGpuTitle")}
              />
            </SettingRow>
            </div>

            {!multiGpuAvailable && (
              <p className="-mt-1 px-1 text-[11px] text-fg/45">
                {t("runtimeDefaults.llamaMultiGpuRequiresTwo")}
              </p>
            )}

            {defaults.llamaMultiGpuEnabled === true && multiGpuAvailable && (
              <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
                <div className="flex items-start gap-3">
                  <div className="flex min-w-0 items-center gap-3">
                    <div className="rounded-lg border border-warning/30 bg-warning/10 p-1.5">
                      <Cpu className="h-4 w-4 text-warning/80" />
                    </div>
                    <div className="min-w-0">
                      <span className="text-sm font-medium text-fg">
                        {t("runtimeDefaults.llamaGpuDevicesTitle")}
                      </span>
                      <p className="text-[11px] text-fg/45">
                        {t("runtimeDefaults.llamaGpuDevicesDescription")}
                      </p>
                    </div>
                  </div>
                </div>

                <div className="mt-3 space-y-2">
                  {eligibleGpuDevices.length === 0 ? (
                    <p className="rounded-lg border border-dashed border-fg/10 bg-fg/[0.03] px-3 py-4 text-center text-[13px] text-fg/45">
                      {t("runtimeDefaults.llamaGpuNone")}
                    </p>
                  ) : (
                    eligibleGpuDevices.map((device) => {
                      const checked = selectedGpuIds.includes(device.index);
                      return (
                        <button
                          key={device.index}
                          type="button"
                          role="checkbox"
                          aria-checked={checked}
                          onClick={() => toggleGpuDevice(device.index)}
                          className={cn(
                            "flex w-full items-center justify-between gap-3 rounded-lg border px-3 py-2 text-left",
                            interactive.transition.default,
                            checked
                              ? "border-accent/40 bg-accent/10"
                              : "border-fg/10 bg-fg/[0.04] hover:border-fg/20 hover:bg-fg/[0.07]",
                          )}
                        >
                          <div className="min-w-0">
                            <span className="block truncate text-sm font-medium text-fg/85">
                              {device.description || device.name || `GPU ${device.index}`}
                            </span>
                            <span className="block font-mono text-[11px] text-fg/40">
                              #{device.index} · {device.backend} ·{" "}
                              {t("runtimeDefaults.llamaGpuMemory", {
                                free: (device.memoryFree / 1024 ** 3).toFixed(1),
                                total: (device.memoryTotal / 1024 ** 3).toFixed(1),
                              })}
                            </span>
                          </div>
                          <span
                            className={cn(
                              "flex h-5 w-5 shrink-0 items-center justify-center rounded-full border",
                              checked ? "border-accent bg-accent text-black" : "border-fg/15",
                            )}
                          >
                            {checked && <Check className="h-3.5 w-3.5" />}
                          </span>
                        </button>
                      );
                    })
                  )}
                </div>

                {selectedGpuIds.length === 1 && (
                  <p className="mt-3 text-[11px] text-warning/80">
                    {t("runtimeDefaults.llamaGpuMinTwo")}
                  </p>
                )}

                <div className="mt-4 space-y-3 border-t border-fg/10 pt-4">
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <span className="block text-sm font-medium text-fg">
                        {t("runtimeDefaults.llamaDistributionTitle")}
                      </span>
                      <p className="text-[11px] text-fg/45">
                        {t("runtimeDefaults.llamaDistributionDescription")}
                      </p>
                    </div>
                    <SelectTrigger
                      label={distributionLabel}
                      onClick={() => setOpenMenu("distribution")}
                      className="w-56"
                    />
                  </div>

                  {defaults.llamaGpuDistributionMode === "priority" && (
                    <div className="flex items-start justify-between gap-3">
                      <div className="min-w-0">
                        <span className="block text-sm font-medium text-fg">
                          {t("runtimeDefaults.llamaPriorityVramTitle")}
                        </span>
                        <p className="text-[11px] text-fg/45">
                          {t("runtimeDefaults.llamaPriorityVramDescription")}
                        </p>
                      </div>
                      <div className="w-28 shrink-0">
                        <NumberInput
                          min={0}
                          max={1024}
                          step={0.5}
                          value={
                            defaults.llamaPriorityVramLimitBytes != null
                              ? Number(
                                  (defaults.llamaPriorityVramLimitBytes / 1024 ** 3).toFixed(2),
                                )
                              : null
                          }
                          onChange={(next) =>
                            void persistDefaults({
                              ...defaults,
                              llamaPriorityVramLimitBytes:
                                next === null || next <= 0 ? null : Math.round(next * 1024 ** 3),
                            })
                          }
                          placeholder={t("common.labels.auto")}
                          className={controlClassName}
                        />
                      </div>
                    </div>
                  )}

                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <span className="block text-sm font-medium text-fg">
                        {t("runtimeDefaults.llamaKvPlacementTitle")}
                      </span>
                      <p className="text-[11px] text-fg/45">
                        {t("runtimeDefaults.llamaKvPlacementDescription")}
                      </p>
                    </div>
                    <SelectTrigger
                      label={kvPlacementLabel}
                      onClick={() => setOpenMenu("kvCache")}
                      className="w-56"
                    />
                  </div>

                  {defaults.llamaKvPlacement === "pin" && selectedEligibleDevices.length > 0 && (
                    <div className="flex items-start justify-between gap-3">
                      <span className="text-sm font-medium text-fg">
                        {t("runtimeDefaults.llamaPinnedGpu")}
                      </span>
                      <SelectTrigger
                        label={pinnedGpuLabel}
                        onClick={() => setOpenMenu("pinnedGpu")}
                        className="w-64"
                      />
                    </div>
                  )}

                  <p className="text-[11px] text-fg/40">
                    {t("runtimeDefaults.llamaManualPerModelNote")}
                  </p>
                </div>
              </div>
            )}
          </div>
        </div>
      </main>

      <BottomMenu
        isOpen={pending !== null}
        onClose={() => {
          if (!movingDir) setPending(null);
        }}
        title={t("runtimeDefaults.moveMenuTitle")}
      >
        <MenuButtonGroup>
          <MenuButton
            icon={movingDir ? <Loader2 className="h-5 w-5 animate-spin" /> : FolderInput}
            title={movingDir ? t("runtimeDefaults.moveWorking") : t("runtimeDefaults.moveConfirm")}
            description={
              pendingInfo
                ? `${t("runtimeDefaults.moveCount", { count: pendingInfo.modelCount })} ${t("runtimeDefaults.moveConfirmDescription")}`
                : t("runtimeDefaults.moveConfirmDescription")
            }
            color="#34d399"
            disabled={movingDir}
            onClick={() => {
              if (pending) void applyModelsDir(pending.kind, pending.dir, true);
            }}
          />
          <MenuButton
            icon={FolderOpen}
            title={t("runtimeDefaults.moveKeep")}
            description={t("runtimeDefaults.moveKeepDescription")}
            disabled={movingDir}
            onClick={() => {
              if (pending) void applyModelsDir(pending.kind, pending.dir, false);
            }}
          />
          <MenuDivider />
          <MenuButton
            icon={X}
            title={t("common.buttons.cancel")}
            disabled={movingDir}
            onClick={() => setPending(null)}
          />
        </MenuButtonGroup>
      </BottomMenu>

      <BottomMenu
        isOpen={openMenu === "distribution"}
        onClose={() => setOpenMenu(null)}
        title={t("runtimeDefaults.llamaDistributionTitle")}
      >
        <MenuButtonGroup>
          {distributionOptions.map((option) => (
            <MenuButton
              key={option.value}
              icon={option.icon}
              title={option.label}
              description={option.description}
              rightElement={
                currentDistribution === option.value ? (
                  <Check className="h-4 w-4 text-accent" />
                ) : undefined
              }
              onClick={() => {
                if (defaults) {
                  void persistDefaults({ ...defaults, llamaGpuDistributionMode: option.value });
                }
                setOpenMenu(null);
              }}
            />
          ))}
        </MenuButtonGroup>
      </BottomMenu>

      <BottomMenu
        isOpen={openMenu === "kvCache"}
        onClose={() => setOpenMenu(null)}
        title={t("runtimeDefaults.llamaKvPlacementTitle")}
      >
        <MenuButtonGroup>
          {kvPlacementOptions.map((option) => (
            <MenuButton
              key={option.value}
              icon={option.icon}
              title={option.label}
              description={option.description}
              rightElement={
                currentKvPlacement === option.value ? (
                  <Check className="h-4 w-4 text-accent" />
                ) : undefined
              }
              onClick={() => {
                if (defaults) {
                  void persistDefaults({ ...defaults, llamaKvPlacement: option.value });
                }
                setOpenMenu(null);
              }}
            />
          ))}
        </MenuButtonGroup>
      </BottomMenu>

      <BottomMenu
        isOpen={openMenu === "pinnedGpu"}
        onClose={() => setOpenMenu(null)}
        title={t("runtimeDefaults.llamaPinnedGpu")}
      >
        <MenuButtonGroup>
          {selectedEligibleDevices.map((device) => (
            <MenuButton
              key={device.index}
              icon={Cpu}
              title={device.description || device.name || `GPU ${device.index}`}
              description={t("runtimeDefaults.llamaGpuMemory", {
                free: (device.memoryFree / 1024 ** 3).toFixed(1),
                total: (device.memoryTotal / 1024 ** 3).toFixed(1),
              })}
              rightElement={
                pinnedGpuIndex === device.index ? (
                  <Check className="h-4 w-4 text-accent" />
                ) : undefined
              }
              onClick={() => {
                if (defaults) {
                  void persistDefaults({ ...defaults, llamaMainGpu: device.index });
                }
                setOpenMenu(null);
              }}
            />
          ))}
        </MenuButtonGroup>
      </BottomMenu>

      <BottomMenu
        isOpen={reconfigureKind !== null}
        onClose={() => {
          if (!reconfiguring) setReconfigureKind(null);
        }}
        title={t("runtimeDefaults.reconfigureTitle")}
      >
        <p className="mb-5 text-[12.5px] leading-relaxed text-fg/55">
          {reconfigureKind === "single"
            ? t("runtimeDefaults.reconfigureBodySingle", { gpu: reconfigureGpuLabel })
            : t("runtimeDefaults.reconfigureBodyMulti")}
        </p>
        <div className="space-y-2">
          <button
            type="button"
            onClick={() => void runReconfigure()}
            disabled={reconfiguring}
            className={cn(
              "flex w-full items-center justify-center gap-2 rounded-xl border border-accent/40 bg-accent/15 px-4 py-3 text-sm font-medium text-accent transition",
              reconfiguring ? "cursor-not-allowed opacity-70" : "hover:bg-accent/25 active:scale-[0.99]",
            )}
          >
            {reconfiguring && <Loader2 className="h-4 w-4 animate-spin" />}
            {reconfiguring
              ? t("runtimeDefaults.reconfigureRunning")
              : t("runtimeDefaults.reconfigureConfirm", { count: reconfigureCount })}
          </button>
          <button
            type="button"
            onClick={() => setReconfigureKind(null)}
            disabled={reconfiguring}
            className="w-full rounded-xl border border-fg/10 bg-fg/4 px-4 py-3 text-sm font-medium text-fg/70 transition hover:border-fg/20 hover:text-fg disabled:opacity-50"
          >
            {t("runtimeDefaults.reconfigureSkip")}
          </button>
        </div>
      </BottomMenu>

      <HfTokenMenu
        isOpen={hfMenuOpen}
        onClose={() => setHfMenuOpen(false)}
        onSaved={setHfAuth}
      />

      <CivitaiTokenMenu
        isOpen={civitaiMenuOpen}
        onClose={() => setCivitaiMenuOpen(false)}
        onSaved={setCivitaiAuth}
      />

      {showTour && <GuidedTour tour="runtimeDefaults" onDismiss={dismissTour} />}
    </div>
  );
}
