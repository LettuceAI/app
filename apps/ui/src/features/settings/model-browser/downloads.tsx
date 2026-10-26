import { useTranslation } from "react-i18next";
import { IconButton, icon, ListGroup, ListRow, Progress } from "@lettuceai/crisp";
import { X } from "lucide-react";
import { useDownloads, downloadName } from "@/entities/download";
import { downloadProgressOf } from "@/entities/job";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsAction } from "../data";
import { useByteSize } from "../image/local-data";

export function BrowserDownloads({
  accountId,
  repo,
  destination,
}: {
  accountId?: string | undefined;
  repo?: string | undefined;
  destination?: string | undefined;
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
  return (
    <>
      {downloads.error !== null && (
        <FailureState error={downloads.error} onRetry={downloads.retry} size="sm" />
      )}
      {jobs.length > 0 && (
        <ListGroup
          label={t("hf.downloadsGroup", {
            destination: accountId
              ? (destination ?? t("hf.onServer", { name: accountId }))
              : t("hf.local"),
          })}
        >
          {jobs.map((job) => {
            const progress = downloadProgressOf(job);
            const name = downloadName(job) ?? t("hf.download");
            return (
              <div key={job.id}>
                <ListRow
                  title={name}
                  description={
                    progress.fraction === undefined
                      ? t("hf.queued")
                      : [
                          t("hf.downloadProgress", {
                            percent: Math.round(progress.fraction * 100),
                            done: size(job.progress.current),
                            total: size(progress.size!),
                          }),
                          progress.eta === undefined
                            ? null
                            : t("hf.eta", { seconds: Math.ceil(progress.eta) }),
                        ]
                          .filter(Boolean)
                          .join(" · ")
                  }
                  trailing={
                    downloads.cancel && (
                      <IconButton
                        size="sm"
                        label={t("hf.cancel", { name })}
                        disabled={downloads.cancelling.has(job.id) || action.isPending}
                        onClick={() => action.mutate(() => downloads.cancel!(job.id))}
                      >
                        <X size={icon.sm} />
                      </IconButton>
                    )
                  }
                />
                <Progress
                  size="sm"
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
