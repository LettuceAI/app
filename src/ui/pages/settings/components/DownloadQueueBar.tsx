import { useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { Download, X, Check, AlertTriangle, Loader, Cpu } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { cn, interactive } from "../../../design-tokens";
import { Routes } from "../../../navigation";
import {
  useDownloadQueue,
  type QueuedDownload,
} from "../../../../core/downloads/DownloadQueueContext";
import { useI18n, type TranslationKey } from "../../../../core/i18n/context";

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const value = bytes / Math.pow(1024, i);
  return `${value.toFixed(i > 1 ? 1 : 0)} ${units[i]}`;
}

function formatSpeed(bytesPerSec: number): string {
  return `${formatBytes(bytesPerSec)}/s`;
}

function extractShortName(modelId: string): string {
  const parts = modelId.split("/");
  return parts[parts.length - 1] || modelId;
}

function isMmprojFilename(filename: string): boolean {
  return filename.toLowerCase().includes("mmproj");
}

function queueSubtitle(
  item: QueuedDownload,
  t: (key: TranslationKey, params?: Record<string, string | number>) => string,
): string {
  if (item.queueKind === "kokoro") {
    return item.displayName || t("models.downloadQueue.kokoroAsset" as TranslationKey);
  }
  return extractShortName(item.modelId);
}

function isCreateableModelDownload(item: QueuedDownload): boolean {
  return (
    item.queueKind !== "kokoro" &&
    item.queueKind !== "whisper" &&
    !isMmprojFilename(item.filename)
  );
}

function pct(d: QueuedDownload): number {
  if (d.total === 0) return 0;
  return Math.min(100, Math.round((d.downloaded / d.total) * 100));
}

/**
 * Inline download cards that can be embedded in any view.
 * Shows active, completed, and failed downloads as cards in the content flow.
 *
 * @param showDivider - Whether to show a divider line below the cards (for search view)
 * @param dividerLabel - Optional label for the divider
 * @param compact - Use slightly tighter spacing for embedding in sidebars or tight layouts
 */
export function InlineDownloadCards({
  showDivider = false,
  dividerLabel,
  compact = false,
}: {
  showDivider?: boolean;
  dividerLabel?: string;
  compact?: boolean;
}) {
  const navigate = useNavigate();
  const { t } = useI18n();
  const { queue, cancelItem, dismissItem } = useDownloadQueue();

  const createModel = useCallback(
    (item: QueuedDownload) => {
      if (isMmprojFilename(item.filename)) return;
      if (!item.resultPath) return;
      const displayName = extractShortName(item.modelId).replace(/-GGUF$/i, "");
      const params = new URLSearchParams();
      params.set("hfModelPath", item.resultPath);
      params.set("hfModelName", item.filename);
      params.set("hfDisplayName", displayName);
      navigate(`${Routes.settingsModelsNew}?${params.toString()}`);
      dismissItem(item.id);
    },
    [navigate, dismissItem],
  );

  const activeDownloads = queue.filter((d) => d.status === "downloading" || d.status === "queued");
  const completedDownloads = queue.filter((d) => d.status === "complete");
  const failedDownloads = queue.filter((d) => d.status === "error" || d.status === "cancelled");

  if (queue.length === 0) return null;

  const px = compact ? "px-3" : "px-4";
  const py = compact ? "py-2" : "py-3";

  return (
    <div className="space-y-2 pb-1">
      <AnimatePresence mode="popLayout">
        {/* Active downloads */}
        {activeDownloads.map((item) => (
          <motion.div
            key={`dl-${item.id}`}
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            transition={{ duration: 0.15 }}
            className="overflow-hidden"
          >
            <div
              className={cn(
                "rounded-xl border border-accent/20 bg-accent/5",
                px,
                py,
                interactive.transition.fast,
              )}
            >
              <div className="flex items-center gap-3">
                {item.status === "downloading" ? (
                  <Download
                    size={compact ? 13 : 14}
                    className="shrink-0 text-accent/70 animate-pulse"
                  />
                ) : (
                  <Loader size={compact ? 13 : 14} className="shrink-0 text-fg/30 animate-spin" />
                )}
                <div className="min-w-0 flex-1">
                  <p className="truncate text-[12px] font-medium text-fg/80">{item.filename}</p>
                  {!compact && <p className="truncate text-[10px] text-fg/40">{queueSubtitle(item, t)}</p>}
                </div>
                <button
                  onClick={() => cancelItem(item.id)}
                  className={cn(
                    "flex items-center justify-center rounded-lg p-1.5 text-fg/25",
                    interactive.transition.fast,
                    "hover:bg-fg/10 hover:text-danger/70 active:scale-90",
                  )}
                  title={t("models.downloadQueue.cancel" as TranslationKey)}
                >
                  <X size={13} />
                </button>
              </div>
              {item.status === "downloading" && (
                <div className="mt-2 flex items-center gap-2">
                  <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-fg/10">
                    <div
                      className="h-full rounded-full bg-accent transition-all duration-300"
                      style={{ width: `${pct(item)}%` }}
                    />
                  </div>
                  <span className="shrink-0 text-[10px] tabular-nums text-fg/40">
                    {pct(item)}%
                    {item.total > 0 && (
                      <span className="ml-1 text-fg/25">
                        {formatBytes(item.downloaded)}/{formatBytes(item.total)}
                      </span>
                    )}
                    {item.speedBytesPerSec > 0 && (
                      <span className="ml-1 text-fg/25">
                        · {formatSpeed(item.speedBytesPerSec)}
                      </span>
                    )}
                  </span>
                </div>
              )}
              {item.status === "queued" && (
                <p className="mt-1.5 text-[10px] text-fg/30">
                  {t("models.downloadQueue.waiting" as TranslationKey)}
                </p>
              )}
            </div>
          </motion.div>
        ))}

        {/* Completed downloads */}
        {completedDownloads.map((item) => (
          <motion.div
            key={`dl-${item.id}`}
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            transition={{ duration: 0.15 }}
            className="overflow-hidden"
          >
            <div className={cn("rounded-xl border border-emerald-400/20 bg-emerald-500/5", px, py)}>
              <div className="flex items-center gap-3">
                <Check size={compact ? 13 : 14} className="shrink-0 text-emerald-400" />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-[12px] font-medium text-fg/80">{item.filename}</p>
                  <p className="text-[10px] text-emerald-400/60">
                    {t("models.downloadQueue.completeBytes" as TranslationKey, {
                      size: formatBytes(item.total),
                    })}
                  </p>
                </div>
                <div className="flex shrink-0 items-center gap-1.5">
                  {isCreateableModelDownload(item) && (
                    <button
                      onClick={() => createModel(item)}
                      className={cn(
                        "flex items-center gap-1.5 rounded-lg border border-emerald-400/30 bg-emerald-500/15 text-[11px] font-semibold text-emerald-300",
                        compact ? "px-2 py-1" : "px-3 py-1.5",
                        interactive.transition.fast,
                        "hover:bg-emerald-500/25 active:scale-95",
                      )}
                    >
                      <Cpu size={compact ? 10 : 11} />
                      Create
                    </button>
                  )}
                  <button
                    onClick={() => dismissItem(item.id)}
                    className={cn(
                      "flex items-center justify-center rounded-lg p-1.5 text-fg/20",
                      interactive.transition.fast,
                      "hover:bg-fg/10 hover:text-fg/50 active:scale-90",
                    )}
                    title="Dismiss"
                  >
                    <X size={12} />
                  </button>
                </div>
              </div>
            </div>
          </motion.div>
        ))}

        {/* Failed downloads */}
        {failedDownloads.map((item) => (
          <motion.div
            key={`dl-${item.id}`}
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            transition={{ duration: 0.15 }}
            className="overflow-hidden"
          >
            <div
              className={cn(
                "rounded-xl border border-danger/15 bg-danger/5",
                px,
                compact ? "py-2" : "py-2.5",
              )}
            >
              <div className="flex items-center gap-3">
                <AlertTriangle size={13} className="shrink-0 text-danger/60" />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-[11px] font-medium text-fg/60">{item.filename}</p>
                  <p className="truncate text-[10px] text-danger/50">
                    {item.status === "cancelled" ? "Cancelled" : item.error || "Download failed"}
                  </p>
                </div>
                <button
                  onClick={() => dismissItem(item.id)}
                  className={cn(
                    "flex items-center justify-center rounded-lg p-1.5 text-fg/20",
                    interactive.transition.fast,
                    "hover:bg-fg/10 hover:text-fg/50 active:scale-90",
                  )}
                  title="Dismiss"
                >
                  <X size={12} />
                </button>
              </div>
            </div>
          </motion.div>
        ))}
      </AnimatePresence>

      {/* Optional divider */}
      {showDivider && (
        <div className="flex items-center gap-2 pt-1 pb-0.5">
          <div className="h-px flex-1 bg-fg/5" />
          {dividerLabel && (
            <span className="text-[9px] font-medium uppercase tracking-widest text-fg/25">
              {dividerLabel}
            </span>
          )}
          <div className="h-px flex-1 bg-fg/5" />
        </div>
      )}
    </div>
  );
}
