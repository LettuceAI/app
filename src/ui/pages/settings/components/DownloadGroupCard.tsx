import { useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { Download, X, Check, AlertTriangle, Loader, Cpu, Layers } from "lucide-react";
import { cn, interactive } from "../../../design-tokens";
import { Routes } from "../../../navigation";
import {
  useDownloadQueue,
  isCreateableModelDownload,
  isMmprojDownloadFilename,
  isMtpDownloadFilename,
  type DownloadGroup,
  type QueuedDownload,
} from "../../../../core/downloads/DownloadQueueContext";
import { useI18n, type TranslationKey } from "../../../../core/i18n/context";

function formatBytes(bytes: number, units: string[]): string {
  if (bytes === 0) return `0 ${units[0]}`;
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const value = bytes / Math.pow(1024, i);
  return `${value.toFixed(i > 1 ? 1 : 0)} ${units[i]}`;
}

function pct(d: QueuedDownload): number {
  if (d.total === 0) return 0;
  return Math.min(100, Math.round((d.downloaded / d.total) * 100));
}

function extractShortName(modelId: string): string {
  const parts = modelId.split("/");
  return parts[parts.length - 1] || modelId;
}

function isActive(item: QueuedDownload): boolean {
  return item.status === "downloading" || item.status === "queued";
}

function roleLabel(
  item: QueuedDownload,
  isModelRow: boolean,
  t: (key: TranslationKey, params?: Record<string, string | number>) => string,
): string | null {
  if (isModelRow) return null;
  if (item.downloadRole === "mmproj" || isMmprojDownloadFilename(item.filename))
    return t("installedModels.downloads.roleVision");
  if (item.downloadRole === "mtp" || isMtpDownloadFilename(item.filename))
    return t("installedModels.downloads.roleMtp");
  return null;
}

export function DownloadGroupCard({
  group,
  compact = false,
  onClick,
}: {
  group: DownloadGroup;
  compact?: boolean;
  onClick?: () => void;
}) {
  const navigate = useNavigate();
  const { t } = useI18n();
  const sizeUnits = [
    t("common.units.bytes"),
    t("common.units.kb"),
    t("common.units.mb"),
    t("common.units.gb"),
    t("common.units.tb"),
  ];
  const { cancelItem, dismissItem } = useDownloadQueue();

  const model = group.model;
  const anyActive = group.items.some(isActive);
  const allComplete = group.items.every((item) => item.status === "complete");
  const anyFailed = group.items.some(
    (item) => item.status === "error" || item.status === "cancelled",
  );

  const ordered = model
    ? [model, ...group.items.filter((item) => item.id !== model.id)]
    : group.items;

  const canCreate =
    !!model &&
    model.status === "complete" &&
    !!model.resultPath &&
    isCreateableModelDownload(model) &&
    !anyActive;

  const handleCreate = useCallback(() => {
    if (!model?.resultPath) return;
    const displayName =
      model.displayName || extractShortName(model.modelId).replace(/-GGUF$/i, "");
    const params = new URLSearchParams();
    params.set("hfModelPath", model.resultPath);
    params.set("hfModelName", model.filename);
    params.set("hfDisplayName", displayName);
    const mmproj = group.items.find(
      (item) =>
        item.id !== model.id &&
        item.status === "complete" &&
        item.resultPath &&
        (item.downloadRole === "mmproj" || isMmprojDownloadFilename(item.filename)),
    );
    if (mmproj?.resultPath) params.set("hfMmprojPath", mmproj.resultPath);
    const mtp = group.items.find(
      (item) =>
        item.id !== model.id &&
        item.status === "complete" &&
        item.resultPath &&
        (item.downloadRole === "mtp" || isMtpDownloadFilename(item.filename)),
    );
    if (mtp?.resultPath) params.set("hfMtpPath", mtp.resultPath);
    navigate(`${Routes.settingsModelsNew}?${params.toString()}`);
    for (const item of group.items) {
      if (!isActive(item)) void dismissItem(item.id);
    }
  }, [model, group.items, navigate, dismissItem]);

  const handleDismissAll = useCallback(() => {
    for (const item of group.items) {
      if (isActive(item)) void cancelItem(item.id);
      else void dismissItem(item.id);
    }
  }, [group.items, cancelItem, dismissItem]);

  const px = compact ? "px-3" : "px-4";
  const py = compact ? "py-2.5" : "py-3";
  const title =
    model?.displayName || extractShortName((model ?? group.items[0]).modelId).replace(/-GGUF$/i, "");

  return (
    <div
      onClick={onClick}
      className={cn(
        "rounded-xl border",
        anyActive
          ? "border-accent/20 bg-accent/5"
          : allComplete
            ? "border-emerald-400/20 bg-emerald-500/5"
            : anyFailed
              ? "border-danger/15 bg-danger/5"
              : "border-fg/10 bg-fg/5",
        px,
        py,
        onClick && "cursor-pointer",
        interactive.transition.fast,
      )}
    >
      <div className="flex items-center gap-3">
        <Layers
          size={compact ? 13 : 14}
          className={cn(
            "shrink-0",
            anyActive ? "text-accent/70" : allComplete ? "text-emerald-400" : "text-fg/40",
          )}
        />
        <div className="min-w-0 flex-1">
          <p className="truncate text-[12px] font-semibold text-fg/85">{title}</p>
          <p className="truncate text-[10px] text-fg/40">
            {t("installedModels.downloads.fileCount", { count: group.items.length })}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          {canCreate && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                handleCreate();
              }}
              className={cn(
                "flex items-center gap-1.5 rounded-lg border border-emerald-400/30 bg-emerald-500/15 text-[11px] font-semibold text-emerald-300",
                compact ? "px-2 py-1" : "px-3 py-1.5",
                interactive.transition.fast,
                "hover:bg-emerald-500/25 active:scale-95",
              )}
            >
              <Cpu size={compact ? 10 : 11} />
              {t("installedModels.downloads.create")}
            </button>
          )}
          <button
            onClick={(e) => {
              e.stopPropagation();
              handleDismissAll();
            }}
            className={cn(
              "flex items-center justify-center rounded-lg p-1.5 text-fg/25",
              interactive.transition.fast,
              anyActive
                ? "hover:bg-fg/10 hover:text-danger/70 active:scale-90"
                : "hover:bg-fg/10 hover:text-fg/50 active:scale-90",
            )}
            title={
              anyActive
                ? t("installedModels.downloads.cancelDownload")
                : t("installedModels.downloads.dismiss")
            }
          >
            <X size={13} />
          </button>
        </div>
      </div>

      <div className={cn("mt-2.5 space-y-2 border-t border-fg/5", compact ? "pt-2" : "pt-2.5")}>
        {ordered.map((item) => {
          const isModelRow = model?.id === item.id;
          const label = roleLabel(item, isModelRow, t);
          return (
            <div key={item.id}>
              <div className="flex items-center gap-2.5">
                {item.status === "downloading" && item.phase === "verifying" ? (
                  <Loader size={12} className="shrink-0 text-accent/70 animate-spin" />
                ) : item.status === "downloading" ? (
                  <Download size={12} className="shrink-0 text-accent/70 animate-pulse" />
                ) : item.status === "queued" ? (
                  <Loader size={12} className="shrink-0 text-fg/30 animate-spin" />
                ) : item.status === "complete" ? (
                  <Check size={12} className="shrink-0 text-emerald-400" />
                ) : (
                  <AlertTriangle size={12} className="shrink-0 text-danger/60" />
                )}
                <p className="min-w-0 flex-1 truncate text-[11px] font-medium text-fg/75">
                  {item.filename}
                </p>
                {label && (
                  <span className="shrink-0 text-[9px] font-semibold uppercase tracking-widest text-fg/30">
                    {label}
                  </span>
                )}
                {item.status === "complete" && (
                  <span className="shrink-0 text-[10px] tabular-nums text-emerald-400/60">
                    {formatBytes(item.total, sizeUnits)}
                  </span>
                )}
                {isActive(item) && (
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      void cancelItem(item.id);
                    }}
                    className={cn(
                      "flex items-center justify-center rounded-lg p-1 text-fg/25",
                      interactive.transition.fast,
                      "hover:bg-fg/10 hover:text-danger/70 active:scale-90",
                    )}
                    title={t("installedModels.downloads.cancelDownload")}
                  >
                    <X size={11} />
                  </button>
                )}
              </div>
              {item.status === "downloading" && item.phase === "verifying" && (
                <p className="mt-1 pl-[22px] text-[10px] text-accent/60">
                  {t("onboarding.common.verifying")}
                </p>
              )}
              {item.status === "downloading" && item.phase !== "verifying" && (
                <div className="mt-1.5 flex items-center gap-2 pl-[22px]">
                  <div className="h-1 flex-1 overflow-hidden rounded-full bg-fg/10">
                    <div
                      className="h-full rounded-full bg-accent transition-all duration-300"
                      style={{ width: `${pct(item)}%` }}
                    />
                  </div>
                  <span className="shrink-0 text-[10px] tabular-nums text-fg/40">
                    {pct(item)}%
                    {item.total > 0 && (
                      <span className="ml-1 text-fg/25">
                        {formatBytes(item.downloaded, sizeUnits)}/
                        {formatBytes(item.total, sizeUnits)}
                      </span>
                    )}
                    {item.speedBytesPerSec > 0 && (
                      <span className="ml-1 text-fg/25">
                        · {formatBytes(item.speedBytesPerSec, sizeUnits)}/s
                      </span>
                    )}
                  </span>
                </div>
              )}
              {item.status === "queued" && (
                <p className="mt-1 pl-[22px] text-[10px] text-fg/30">
                  {t("installedModels.downloads.waiting")}
                </p>
              )}
              {(item.status === "error" || item.status === "cancelled") && (
                <p className="mt-1 truncate pl-[22px] text-[10px] text-danger/50">
                  {item.status === "cancelled"
                    ? t("installedModels.downloads.cancelled")
                    : item.error || t("installedModels.downloads.failed")}
                </p>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
