import { useCallback, useEffect, useState } from "react";
import {
  Brain,
  Check,
  Cpu,
  FolderInput,
  FolderOpen,
  HardDrive,
  Layers,
  Loader2,
  RotateCcw,
  X,
} from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";

import { BottomMenu, MenuButton, MenuButtonGroup, MenuDivider } from "../../components/BottomMenu";
import { Switch } from "../../components/Switch";

import {
  readSettings,
  saveAdvancedModelSettings,
  saveAdvancedSettings,
} from "../../../core/storage/repo";
import { useI18n } from "../../../core/i18n/context";
import { cn, interactive } from "../../design-tokens";
import { toast } from "../../components/toast";
import { NumberInput } from "../../components/NumberInput";

type RuntimeDefaults = {
  llamaDefaultContextLength: number | null;
  llamaDefaultKvCacheType: "auto" | "f16" | "q8_0" | "q4_0";
  llamaMultiGpuEnabled: boolean | null;
  llamaGpuDeviceIds: number[] | null;
  llamaGpuDistributionMode: "balanced" | "proportional" | "priority" | "manual" | null;
  llamaKvPlacement: "auto" | "split" | "systemRam" | "pin" | null;
  llamaMainGpu: number | null;
  llamaPriorityVramLimitBytes: number | null;
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

export function LocalRuntimeDefaultsPage() {
  const { t } = useI18n();
  const [defaults, setDefaults] = useState<RuntimeDefaults | null>(null);
  const [modelsDir, setModelsDir] = useState<LlmModelsDirInfo | null>(null);
  const [gpuDevices, setGpuDevices] = useState<LlamaGpuDevice[]>([]);
  const [pending, setPending] = useState<{ kind: ModelDirKind; dir: string } | null>(null);
  const [movingDir, setMovingDir] = useState(false);

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
        });
      })
      .catch(() => {});
    invoke<LlamaGpuDevice[]>("llamacpp_backend_devices")
      .then(setGpuDevices)
      .catch(() => setGpuDevices([]));
  }, [refreshModelsDir]);

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
  const eligibleGpuDevices = gpuDevices.filter(
    (device) => device.deviceType !== "IntegratedGpu",
  );
  const multiGpuAvailable = eligibleGpuDevices.length >= 2;
  const selectedEligibleDevices = eligibleGpuDevices.filter((device) =>
    selectedGpuIds.includes(device.index),
  );
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
          <div className="space-y-4">
            <SectionHeading label={t("runtimeDefaults.storageSection")} />
            <p className="px-1 text-xs text-fg/50">{t("runtimeDefaults.storageDescription")}</p>

            <div className="space-y-3">
              {renderFolderRow("llm", modelsDir, t("runtimeDefaults.modelsFolderTitle"))}
            </div>
          </div>

          <div className="space-y-4">
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
              icon={<Cpu className="h-4 w-4 text-warning/80" />}
              iconClassName="border-warning/30 bg-warning/10"
              title={t("runtimeDefaults.llamaMultiGpuTitle")}
              description={t("runtimeDefaults.llamaMultiGpuDescription")}
            >
              <Switch
                checked={defaults.llamaMultiGpuEnabled === true && multiGpuAvailable}
                disabled={!multiGpuAvailable}
                onChange={(next) =>
                  void persistDefaults({
                    ...defaults,
                    llamaMultiGpuEnabled: next ? true : null,
                  })
                }
                aria-label={t("runtimeDefaults.llamaMultiGpuTitle")}
              />
            </SettingRow>

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
                    <select
                      value={defaults.llamaGpuDistributionMode ?? "balanced"}
                      onChange={(event) =>
                        void persistDefaults({
                          ...defaults,
                          llamaGpuDistributionMode: event.target
                            .value as RuntimeDefaults["llamaGpuDistributionMode"],
                        })
                      }
                      className={cn(controlClassName, "shrink-0")}
                    >
                      <option value="balanced">
                        {t("runtimeDefaults.llamaDistBalanced")}
                      </option>
                      <option value="proportional">
                        {t("runtimeDefaults.llamaDistProportional")}
                      </option>
                      <option value="priority">
                        {t("runtimeDefaults.llamaDistPriority")}
                      </option>
                    </select>
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
                                next === null || next <= 0
                                  ? null
                                  : Math.round(next * 1024 ** 3),
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
                    <select
                      value={defaults.llamaKvPlacement ?? "auto"}
                      onChange={(event) =>
                        void persistDefaults({
                          ...defaults,
                          llamaKvPlacement: event.target
                            .value as RuntimeDefaults["llamaKvPlacement"],
                        })
                      }
                      className={cn(controlClassName, "shrink-0")}
                    >
                      <option value="auto">{t("runtimeDefaults.llamaKvAuto")}</option>
                      <option value="split">{t("runtimeDefaults.llamaKvSplit")}</option>
                      <option value="systemRam">{t("runtimeDefaults.llamaKvSystemRam")}</option>
                      <option value="pin">{t("runtimeDefaults.llamaKvPin")}</option>
                    </select>
                  </div>

                  {defaults.llamaKvPlacement === "pin" && selectedEligibleDevices.length > 0 && (
                    <div className="flex items-start justify-between gap-3">
                      <span className="text-sm font-medium text-fg">
                        {t("runtimeDefaults.llamaPinnedGpu")}
                      </span>
                      <select
                        value={String(
                          defaults.llamaMainGpu ?? selectedEligibleDevices[0]?.index ?? 0,
                        )}
                        onChange={(event) =>
                          void persistDefaults({
                            ...defaults,
                            llamaMainGpu: Number(event.target.value),
                          })
                        }
                        className={cn(controlClassName, "shrink-0")}
                      >
                        {selectedEligibleDevices.map((device) => (
                          <option key={device.index} value={String(device.index)}>
                            {device.description || device.name || `GPU ${device.index}`}
                          </option>
                        ))}
                      </select>
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
    </div>
  );
}
