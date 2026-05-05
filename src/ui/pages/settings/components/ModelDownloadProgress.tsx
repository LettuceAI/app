import { motion } from "framer-motion";
import { CheckCircle, Download, Loader2, XCircle } from "lucide-react";
import type { DownloadProgress } from "../hooks/useModelDownload";
import { useI18n } from "../../../../core/i18n/context";

export type ModelDownloadPhase = "idle" | "downloading" | "verifying" | "passed" | "failed";

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 Bytes";
  const k = 1024;
  const sizes = ["Bytes", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return Math.round((bytes / Math.pow(k, i)) * 100) / 100 + " " + sizes[i];
}

export function ModelDownloadProgress({
  progress,
  phase,
  statusText,
}: {
  progress: DownloadProgress;
  phase: ModelDownloadPhase;
  statusText: string;
}) {
  const { t } = useI18n();
  const progressPercent =
    phase === "passed" || phase === "verifying"
      ? 100
      : progress.total > 0
        ? (progress.downloaded / progress.total) * 100
        : 0;

  return (
    <div className="rounded-xl border border-fg/10 bg-fg/5 p-6">
      <div className="mb-4 flex justify-center">
        {phase === "idle" || phase === "downloading" ? (
          <div className="flex h-16 w-16 items-center justify-center rounded-full border border-info/20 bg-info/10">
            <Download className="h-8 w-8 text-info" />
          </div>
        ) : phase === "verifying" ? (
          <div className="flex h-16 w-16 items-center justify-center rounded-full border border-warning/20 bg-warning/10">
            <Loader2 className="h-8 w-8 text-warning animate-spin" />
          </div>
        ) : phase === "passed" ? (
          <div className="flex h-16 w-16 items-center justify-center rounded-full border border-accent/20 bg-accent/10">
            <CheckCircle className="h-8 w-8 text-accent" />
          </div>
        ) : (
          <div className="flex h-16 w-16 items-center justify-center rounded-full border border-danger/20 bg-danger/10">
            <XCircle className="h-8 w-8 text-danger/80" />
          </div>
        )}
      </div>

      <div className="space-y-6">
        {progress.status.toLowerCase().includes("downloading") && progress.totalFiles > 0 && (
          <div className="text-center">
            <div className="text-sm font-medium text-fg/80">
              {t("hfBrowser.fileOfTotal", {
                current: progress.currentFileIndex,
                total: progress.totalFiles,
              })}
            </div>
            {progress.currentFileName && (
              <div className="mt-1 text-xs text-fg/50 font-mono">{progress.currentFileName}</div>
            )}
          </div>
        )}

        <div className="space-y-2">
          <div className="flex justify-between text-sm">
            <span className="text-fg/70">{t("hfBrowser.progress")}</span>
            <span className="font-medium text-fg">{Math.round(progressPercent)}%</span>
          </div>
          <div className="h-2 w-full overflow-hidden rounded-full bg-fg/10">
            <motion.div
              className="h-full bg-accent"
              initial={{ width: 0 }}
              animate={{
                width:
                  progress.total > 0 ? `${(progress.downloaded / progress.total) * 100}%` : "0%",
              }}
              transition={{ duration: 0.3 }}
            />
          </div>
          <div className="flex justify-between text-xs text-fg/50">
            <span>{formatBytes(progress.downloaded)}</span>
            <span>{formatBytes(progress.total)}</span>
          </div>
        </div>

        <div className="text-center text-sm text-fg/60">{statusText}</div>
      </div>
    </div>
  );
}
