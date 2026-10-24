import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Button, ListRow, Progress } from "@lettuceai/crisp";
import { ApiFailure } from "@/api/client";
import type { JobView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { DownloadCard, DownloadJob, downloadName } from "@/entities/download";
import { terminalJobStates, watchJob } from "@/entities/job";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { useImageAction } from "./local-data";

function ImageOperationJob({ id, title, download = false, onUpdate, onDone }: { id: string; title?: string | undefined; download?: boolean; onUpdate?: ((job: JobView) => void) | undefined; onDone?: ((job: JobView) => void) | undefined }) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const client = useQueryClient();
  const action = useImageAction();
  const [job, setJob] = useState<JobView | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    const controller = new AbortController();
    void watchJob(api, id, (next) => { setJob(next); onUpdate?.(next); }, controller.signal).then(async (settled) => {
      onDone?.(settled);
      await Promise.all([queryKeys.localImages.all, queryKeys.models.all, queryKeys.loras.all, queryKeys.jobs.lists()].map((queryKey) => client.invalidateQueries({ queryKey })));
    }).catch((reason: unknown) => { if (!controller.signal.aborted) setError(reason); });
    return () => controller.abort();
  }, [api, client, id, attempt, onUpdate, onDone]);
  const active = job !== null && !terminalJobStates.has(job.state);
  const failed = job && ["failed", "interrupted"].includes(job.state);
  const cancel = () => action.mutate(() => api.call("jobCancel", { job_id: id }));
  const name = title ?? downloadName(job) ?? t("image.local.download");
  const isDownload = download || job?.kind === "artifact_install";
  return (
    <div className="space-y-2">
      {isDownload ? <DownloadCard job={job} id={id} name={name} /> : <>
        <ListRow title={title ?? t("image.local.download")} description={t(`image.local.task.${job?.state ?? "queued"}`)} trailing={active ? <Button variant="ghost" size="sm" loading={action.isPending} onClick={cancel}>{t("manage.cancel")}</Button> : undefined} />
        {active && <Progress label={t("image.local.task.running")} {...(job.progress.total ? { value: Math.min(100, job.progress.current / job.progress.total * 100) } : {})} />}
      </>}
      {error ? <FailureState error={error} size="sm" onRetry={() => { setError(null); setAttempt((current) => current + 1); }} /> : failed && !isDownload ? <FailureState error={new ApiFailure("unavailable", "image operation failed", null)} size="sm" /> : null}
      {action.isError && <FailureState error={action.error} size="sm" />}
    </div>
  );
}

export function ImageJob(props: { id: string; title?: string | undefined; download?: boolean; onUpdate?: ((job: JobView) => void) | undefined; onDone?: ((job: JobView) => void) | undefined }) {
  const api = useSettingsApi();
  const client = useQueryClient();
  if (!props.download) return <ImageOperationJob {...props} />;
  return <DownloadJob api={api} id={props.id} name={props.title} onUpdate={props.onUpdate} onDone={async (job) => {
    props.onDone?.(job);
    await Promise.all([queryKeys.localImages.all, queryKeys.models.all, queryKeys.loras.all, queryKeys.jobs.lists()].map((queryKey) => client.invalidateQueries({ queryKey })));
  }} />;
}
