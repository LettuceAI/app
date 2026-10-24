import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { DownloadItem } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { JobView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { useDownloadProgress } from "@/entities/download";
import { downloadPhaseOf, terminalJobStates, watchJob } from "@/entities/job";
import { FailureState } from "@/shared/ui/FailureState";
import { modelName, useSpeechAction } from "./data";

export function SpeechDownload({ api, id, modelId, onUpdate }: { api: ApiClient; id: string; modelId: string; onUpdate: (job: JobView) => void }) {
  const { t } = useTranslation("common");
  const client = useQueryClient();
  const [job, setJob] = useState<JobView | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [attempt, setAttempt] = useState(0);
  const action = useSpeechAction();
  useEffect(() => {
    const controller = new AbortController();
    void watchJob(api, id, (next) => { setJob(next); onUpdate(next); }, controller.signal).then(() => client.invalidateQueries({ queryKey: queryKeys.speech.all })).catch((failure: unknown) => { if (!controller.signal.aborted) setError(failure); });
    return () => controller.abort();
  }, [api, client, id, attempt, onUpdate]);
  const progress = useDownloadProgress(job, id);
  const active = job !== null && !terminalJobStates.has(job.state);
  return <div className="space-y-2"><DownloadItem name={modelName(modelId)} status={!job || job.state === "queued" || job.state === "claimed" ? "queued" : downloadPhaseOf(job)} {...(progress.fraction === undefined ? {} : { progress: progress.fraction })} {...(progress.size === undefined ? {} : { size: progress.size })} {...(progress.speed === undefined ? {} : { speed: progress.speed })} {...(progress.eta === undefined ? {} : { eta: progress.eta })} error={t("errors.code.unavailable")} {...(active && !action.isPending && job.state !== "cancellation_requested" && job.state !== "cleaning_up" ? { onCancel: () => action.mutate(() => api.call("jobCancel", { job_id: id })) } : {})} />{error !== null && <FailureState error={error} size="sm" onRetry={() => { setError(null); setAttempt((value) => value + 1); }} />}{action.isError && <FailureState error={action.error} size="sm" />}</div>;
}
