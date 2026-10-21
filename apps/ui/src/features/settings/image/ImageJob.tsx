import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Button, DownloadItem, ListRow, Progress } from "@lettuceai/crisp";
import { ApiFailure } from "@/api/client";
import type { JobView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { terminalJobStates, watchJob } from "@/entities/job";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { useImageAction } from "./local-data";

export function ImageJob({ id, title, onDone }: { id: string; title?: string | undefined; onDone?: (job: JobView) => void }) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const client = useQueryClient();
  const action = useImageAction();
  const [job, setJob] = useState<JobView | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    const controller = new AbortController();
    void watchJob(api, id, setJob, controller.signal).then(async (settled) => {
      onDone?.(settled);
      await Promise.all([queryKeys.localImages.all, queryKeys.models.all, queryKeys.loras.all].map((queryKey) => client.invalidateQueries({ queryKey })));
    }).catch((reason: unknown) => { if (!controller.signal.aborted) setError(reason); });
    return () => controller.abort();
  }, [api, client, id, attempt, onDone]);
  const active = job !== null && !terminalJobStates.has(job.state);
  const failed = job && ["failed", "interrupted"].includes(job.state);
  const cancel = () => action.mutate(() => api.call("jobCancel", { job_id: id }));
  return (
    <div className="space-y-2">
      {job?.kind === "artifact_install" ? <DownloadItem name={title ?? t("image.local.download")} detail={t(`tools.job.${job.state}`)} status={job.state === "succeeded" ? "done" : terminalJobStates.has(job.state) ? "failed" : job.state === "queued" || job.state === "claimed" ? "queued" : job.progress.label_code && job.progress.label_code !== "download" ? "verifying" : "downloading"} {...(job.progress.total && job.progress.unit === "bytes" ? { size: job.progress.total, progress: Math.min(1, job.progress.current / job.progress.total) } : {})} {...(job.progress.bytes_per_second !== null ? { speed: job.progress.bytes_per_second } : {})} {...(active && !action.isPending ? { onCancel: cancel } : {})} /> : <>
        <ListRow title={title ?? t("image.local.download")} description={job ? t(`tools.job.${job.state}`) : t("tools.job.queued")} trailing={active ? <Button variant="ghost" size="sm" loading={action.isPending} onClick={cancel}>{t("manage.cancel")}</Button> : undefined} />
        {active && <Progress label={t("tools.job.running")} {...(job.progress.total ? { value: Math.min(100, job.progress.current / job.progress.total * 100) } : {})} />}
      </>}
      {error ? <FailureState error={error} size="sm" onRetry={() => { setError(null); setAttempt((current) => current + 1); }} /> : failed ? <FailureState error={new ApiFailure("unavailable", "image operation failed", null)} size="sm" /> : null}
      {action.isError && <FailureState error={action.error} size="sm" />}
    </div>
  );
}
