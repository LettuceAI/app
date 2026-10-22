import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Button, DownloadItem, ListRow, Progress } from "@lettuceai/crisp";
import { ApiFailure } from "@/api/client";
import type { JobView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { downloadPhaseOf, downloadProgressOf, terminalJobStates, watchJob } from "@/entities/job";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { useImageAction } from "./local-data";

export function ImageJob({ id, title, download = false, onUpdate, onDone }: { id: string; title?: string | undefined; download?: boolean; onUpdate?: (job: JobView) => void; onDone?: (job: JobView) => void }) {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation("common");
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
  const { fraction, size, speed, eta } = downloadProgressOf(job);
  const detail = job?.subject_detail;
  const name = title ?? (detail?.type === "sd_runtime_install" ? `${detail.release} · ${detail.backend}` : detail?.type === "sd_upscaler_install" ? detail.filename : detail?.type === "sd_model_install" || detail?.type === "image_bundle" || detail?.type === "model_download" ? detail.display_name : t("image.local.download"));
  const isDownload = download || job?.kind === "artifact_install";
  const downloadStatus = !job || job.state === "queued" || job.state === "claimed" ? "queued" : downloadPhaseOf(job);
  return (
    <div className="space-y-2">
      {isDownload ? <DownloadItem name={name} status={downloadStatus} {...(fraction !== undefined ? { progress: fraction } : {})} {...(size !== undefined ? { size } : {})} {...(speed !== undefined ? { speed } : {})} {...(eta !== undefined ? { eta } : {})} error={job?.state === "cancelled" ? t("tools.job.cancelled") : common("errors.code.unavailable")} {...(active && !action.isPending && job.state !== "cancellation_requested" && job.state !== "cleaning_up" ? { onCancel: cancel } : {})} /> : <>
        <ListRow title={title ?? t("image.local.download")} description={job ? t(`tools.job.${job.state}`) : t("tools.job.queued")} trailing={active ? <Button variant="ghost" size="sm" loading={action.isPending} onClick={cancel}>{t("manage.cancel")}</Button> : undefined} />
        {active && <Progress label={t("tools.job.running")} {...(job.progress.total ? { value: Math.min(100, job.progress.current / job.progress.total * 100) } : {})} />}
      </>}
      {error ? <FailureState error={error} size="sm" onRetry={() => { setError(null); setAttempt((current) => current + 1); }} /> : failed && !isDownload ? <FailureState error={new ApiFailure("unavailable", "image operation failed", null)} size="sm" /> : null}
      {action.isError && <FailureState error={action.error} size="sm" />}
    </div>
  );
}
