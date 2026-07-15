import { useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { Download, Check, AlertTriangle, Loader, ChevronRight, Cpu, X } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { cn, interactive } from "../../../design-tokens";
import { Routes } from "../../../navigation";
import {
  useDownloadQueueOptional,
  isCreateableModelDownload,
  groupQueueDownloads,
  type QueuedDownload,
} from "../../../../core/downloads/DownloadQueueContext";
import { DownloadGroupCard } from "./DownloadGroupCard";
import { useI18n } from "../../../../core/i18n/context";

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

function goToModel(navigate: ReturnType<typeof useNavigate>, modelId: string) {
  const params = new URLSearchParams();
  params.set("model", modelId);
  navigate(`${Routes.settingsModelsBrowse}?${params.toString()}`);
}

/**
 * Compact download indicator shown on the Models list page.
 * Shows active downloads with progress and completed downloads with a "Create" action.
 * Only renders when there are items in the download queue.
 */
export function ModelsDownloadIndicator() {
  const ctx = useDownloadQueueOptional();
  const navigate = useNavigate();
  const { t } = useI18n();
  const sizeUnits = [
    t("common.units.bytes"),
    t("common.units.kb"),
    t("common.units.mb"),
    t("common.units.gb"),
    t("common.units.tb"),
  ];

  const createModel = useCallback(
    (item: QueuedDownload) => {
      if (!item.resultPath || !ctx || !isCreateableModelDownload(item)) return;
      const displayName = extractShortName(item.modelId).replace(/-GGUF$/i, "");
      const params = new URLSearchParams();
      params.set("hfModelPath", item.resultPath);
      params.set("hfModelName", item.filename);
      params.set("hfDisplayName", displayName);
      navigate(`${Routes.settingsModelsNew}?${params.toString()}`);
      ctx.dismissItem(item.id);
    },
    [navigate, ctx],
  );

  if (!ctx || !ctx.hasItems) return null;

  const { queue, cancelItem, dismissItem } = ctx;
  const visibleQueue = queue.filter((d) => d.queueKind !== "kokoro");
  const { groups, singles } = groupQueueDownloads(visibleQueue);
  const activeItems = singles.filter((d) => d.status === "downloading" || d.status === "queued");
  const completedItems = singles.filter((d) => d.status === "complete");
  const failedItems = singles.filter((d) => d.status === "error" || d.status === "cancelled");
  const activeCount = visibleQueue.filter(
    (d) => d.status === "downloading" || d.status === "queued",
  ).length;

  if (visibleQueue.length === 0) return null;

  return (
    <div className="space-y-2">
      {/* Section header */}
      <div className="flex items-center gap-2 px-1">
        <Download size={12} className="text-accent/60" />
        <span className="text-[10px] font-semibold uppercase tracking-[0.15em] text-fg/40">
          {t("installedModels.downloads.heading")}
          {activeCount > 0 && (
            <span className="ml-1.5 text-accent/70">
              {t("installedModels.downloads.activeCount", { count: activeCount })}
            </span>
          )}
        </span>
        <div className="h-px flex-1 bg-fg/5" />
      </div>

      <AnimatePresence mode="popLayout">
        {groups.map((group) => (
          <motion.div
            key={`grp-${group.installId}`}
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            transition={{ duration: 0.15 }}
            className="overflow-hidden"
          >
            <DownloadGroupCard
              group={group}
              onClick={() => goToModel(navigate, (group.model ?? group.items[0]).modelId)}
            />
          </motion.div>
        ))}

        {/* Active downloads */}
        {activeItems.map((item) => (
          <motion.div
            key={item.id}
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            transition={{ duration: 0.15 }}
            className="overflow-hidden"
          >
            <div
              onClick={() => goToModel(navigate, item.modelId)}
              className={cn(
                "rounded-xl border border-accent/15 bg-accent/4 px-4 py-3 cursor-pointer",
                interactive.transition.fast,
                "hover:border-accent/25 hover:bg-accent/[0.07] active:scale-[0.99]",
              )}
            >
              <div className="flex items-center gap-3">
                {item.status === "downloading" && item.phase === "verifying" ? (
                  <Loader size={14} className="shrink-0 text-accent/70 animate-spin" />
                ) : item.status === "downloading" ? (
                  <Download size={14} className="shrink-0 text-accent/70 animate-pulse" />
                ) : (
                  <Loader size={14} className="shrink-0 text-fg/30 animate-spin" />
                )}
                <div className="min-w-0 flex-1">
                  <p className="truncate text-[12px] font-medium text-fg/80">{item.filename}</p>
                  <p className="truncate text-[10px] text-fg/40">
                    {extractShortName(item.modelId)}
                  </p>
                </div>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    cancelItem(item.id);
                  }}
                  className={cn(
                    "flex items-center justify-center rounded-lg p-1.5 text-fg/25",
                    interactive.transition.fast,
                    "hover:bg-fg/10 hover:text-danger/70 active:scale-90",
                  )}
                  title={t("installedModels.downloads.cancelDownload")}
                >
                  <X size={13} />
                </button>
              </div>

              {/* Progress bar */}
              {item.status === "downloading" && item.phase === "verifying" && (
                <p className="mt-1.5 text-[10px] text-accent/60">
                  {t("onboarding.common.verifying")}
                </p>
              )}
              {item.status === "downloading" && item.phase !== "verifying" && (
                <div className="mt-2 flex items-center gap-2">
                  <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-fg/10">
                    <motion.div
                      className="h-full rounded-full bg-accent"
                      initial={{ width: 0 }}
                      animate={{ width: `${pct(item)}%` }}
                      transition={{ duration: 0.3 }}
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
                  </span>
                </div>
              )}

              {item.status === "queued" && (
                <p className="mt-1.5 text-[10px] text-fg/30">
                  {t("installedModels.downloads.waiting")}
                </p>
              )}
            </div>
          </motion.div>
        ))}

        {/* Completed downloads */}
        {completedItems.map((item) => (
          <motion.div
            key={item.id}
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            transition={{ duration: 0.15 }}
            className="overflow-hidden"
          >
            <div
              onClick={() => goToModel(navigate, item.modelId)}
              className={cn(
                "rounded-xl border border-emerald-400/20 bg-emerald-500/4 px-4 py-3 cursor-pointer",
                interactive.transition.fast,
                "hover:border-emerald-400/30 hover:bg-emerald-500/[0.07] active:scale-[0.99]",
              )}
            >
              <div className="flex items-center gap-3">
                <Check size={14} className="shrink-0 text-emerald-400" />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-[12px] font-medium text-fg/80">{item.filename}</p>
                  <p className="text-[10px] text-emerald-400/60">
                    {formatBytes(item.total, sizeUnits)} ({t("installedModels.downloads.readyToCreate")})
                  </p>
                </div>
                <div className="flex shrink-0 items-center gap-1.5">
                  {isCreateableModelDownload(item) && (
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        createModel(item);
                      }}
                      className={cn(
                        "flex items-center gap-1.5 rounded-lg border border-emerald-400/30 bg-emerald-500/15 px-3 py-1.5 text-[11px] font-semibold text-emerald-300",
                        interactive.transition.fast,
                        "hover:bg-emerald-500/25 active:scale-95",
                      )}
                    >
                      <Cpu size={11} />
                      {t("installedModels.downloads.create")}
                    </button>
                  )}
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      dismissItem(item.id);
                    }}
                    className={cn(
                      "flex items-center justify-center rounded-lg p-1.5 text-fg/20",
                      interactive.transition.fast,
                      "hover:bg-fg/10 hover:text-fg/50 active:scale-90",
                    )}
                    title={t("installedModels.downloads.dismiss")}
                  >
                    <X size={12} />
                  </button>
                </div>
              </div>
            </div>
          </motion.div>
        ))}

        {/* Failed downloads (compact) */}
        {failedItems.map((item) => (
          <motion.div
            key={item.id}
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            transition={{ duration: 0.15 }}
            className="overflow-hidden"
          >
            <div
              onClick={() => goToModel(navigate, item.modelId)}
              className={cn(
                "rounded-xl border border-danger/15 bg-danger/4 px-4 py-2.5 cursor-pointer",
                interactive.transition.fast,
                "hover:border-danger/25 hover:bg-danger/[0.07] active:scale-[0.99]",
              )}
            >
              <div className="flex items-center gap-3">
                <AlertTriangle size={13} className="shrink-0 text-danger/60" />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-[11px] font-medium text-fg/60">{item.filename}</p>
                  <p className="truncate text-[10px] text-danger/50">
                    {item.status === "cancelled"
                      ? t("installedModels.downloads.cancelled")
                      : item.error || t("installedModels.downloads.failed")}
                  </p>
                </div>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    dismissItem(item.id);
                  }}
                  className={cn(
                    "flex items-center justify-center rounded-lg p-1.5 text-fg/20",
                    interactive.transition.fast,
                    "hover:bg-fg/10 hover:text-fg/50 active:scale-90",
                  )}
                  title={t("installedModels.downloads.dismiss")}
                >
                  <X size={12} />
                </button>
              </div>
            </div>
          </motion.div>
        ))}
      </AnimatePresence>

      {/* Browse more link */}
      {activeCount > 0 && (
        <button
          onClick={() => navigate(Routes.settingsModelsBrowse)}
          className={cn(
            "flex w-full items-center justify-center gap-1.5 rounded-lg py-1.5 text-[11px] text-accent/60",
            interactive.transition.fast,
            "hover:text-accent/80 hover:bg-fg/3",
          )}
        >
          {t("installedModels.downloads.viewInBrowser")}
          <ChevronRight size={12} />
        </button>
      )}
    </div>
  );
}
