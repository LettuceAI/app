import { useTranslation } from "react-i18next";
import { ArrowDownToLine, X } from "lucide-react";
import { IconButton, icon, ListGroup, Progress } from "@lettuceai/crisp";
import { useDownloads, downloadName, DownloadCard } from "@/entities/download";
import { downloadProgressOf } from "@/entities/job";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsAction } from "../data";
import { useByteSize } from "../image/local-data";

export function BrowserDownloads({
  accountId,
  repo,
  destination,
  variant = "rows",
}: {
  accountId?: string | undefined;
  repo?: string | undefined;
  destination?: string | undefined;
  variant?: "rows" | "cards";
}) {
  const { t } = useTranslation("settings");
  const downloads = useDownloads();
  const size = useByteSize();
  const action = useSettingsAction();
  const jobs = downloads.downloads.filter((job) => {
    const detail = job.subject_detail;
    return accountId
      ? detail?.type === "model_pull" && detail.provider_account_id === accountId
      : detail?.type === "model_download" && (!repo || detail.repo === repo);
  });
  const where = accountId ? (destination ?? t("hf.onServer", { name: accountId })) : t("hf.local");
  return (
    <>
      {downloads.error !== null && (
        <FailureState error={downloads.error} onRetry={downloads.retry} size="sm" />
      )}
      {jobs.length > 0 && variant === "cards" && (
        <div className="space-y-2">
          {jobs.map((job) => (
            <DownloadCard
              key={job.id}
              job={job}
              detail={
                job.subject_detail?.type === "model_download"
                  ? `${job.subject_detail.repo} · ${where}`
                  : where
              }
            />
          ))}
        </div>
      )}
      {jobs.length > 0 && variant === "rows" && (
        <ListGroup label={t("hf.downloadsGroup", { destination: where })}>
          {jobs.map((job) => {
            const progress = downloadProgressOf(job);
            const name = downloadName(job) ?? t("hf.download");
            return (
              <div key={job.id} className="relative flex items-center gap-3 px-4 py-2.5">
                <ArrowDownToLine size={icon.md} className="shrink-0 text-accent" />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-sm text-fg">{name}</span>
                  <span className="block truncate text-xs tabular-nums text-fg-3">
                    {progress.fraction === undefined
                      ? t("hf.queued")
                      : [
                          t("hf.downloadProgress", {
                            percent: Math.round(progress.fraction * 100),
                            done: size(job.progress.current),
                            total: size(progress.size!),
                          }),
                          progress.eta === undefined
                            ? null
                            : progress.eta < 90
                              ? t("hf.eta", { seconds: Math.ceil(progress.eta) })
                              : t("hf.etaMinutes", { minutes: Math.round(progress.eta / 60) }),
                        ]
                          .filter(Boolean)
                          .join(" · ")}
                  </span>
                </span>
                {downloads.cancel && (
                  <IconButton
                    size="sm"
                    label={t("hf.cancel", { name })}
                    disabled={downloads.cancelling.has(job.id) || action.isPending}
                    onClick={() => action.mutate(() => downloads.cancel!(job.id))}
                  >
                    <X size={icon.sm} />
                  </IconButton>
                )}
                <Progress
                  size="sm"
                  className="absolute inset-x-4 bottom-0 h-0.5 w-auto"
                  {...(progress.fraction === undefined ? {} : { value: progress.fraction * 100 })}
                  label={name}
                />
              </div>
            );
          })}
        </ListGroup>
      )}
      {action.isError && <FailureState error={action.error} size="sm" />}
    </>
  );
}
