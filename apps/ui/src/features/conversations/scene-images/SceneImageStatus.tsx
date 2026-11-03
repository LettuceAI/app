import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Alert, Button, Spinner } from "@lettuceai/crisp";
import { ApiFailure } from "@/api/client";
import { useApi } from "@/api/use-api";
import type { ImageProgress, JobView, TimelineMessage } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { terminalJobStates, watchJob } from "@/entities/job";

export function SceneImageStatus({ conversationId, message, onReview }: { conversationId: string; message: TimelineMessage; onReview: (message: TimelineMessage) => void }) {
  const api = useApi();
  const client = useQueryClient();
  const { t, i18n } = useTranslation("conversations");
  const { t: common } = useTranslation("common");
  const scene = message.scene_image;
  const [watched, setWatched] = useState<JobView | null>(null);
  const [imageProgress, setImageProgress] = useState<{ jobId: string; progress: ImageProgress } | null>(null);
  const [error, setError] = useState<ApiFailure | null>(null);
  const [retry, setRetry] = useState(0);
  const [busy, setBusy] = useState(false);
  const active = scene !== null && ["approved", "running", "awaiting_turn"].includes(scene.state);
  const jobId = scene?.job_id ?? null;
  useEffect(() => {
    if (!active || !jobId) return;
    const controller = new AbortController();
    void watchJob(api, jobId, (job) => {
      if (controller.signal.aborted) return;
      setWatched(job);
      if (terminalJobStates.has(job.state)) void client.invalidateQueries({ queryKey: queryKeys.conversations.messages(conversationId) });
    }, controller.signal, undefined, (progress) => {
      if (!controller.signal.aborted) setImageProgress({ jobId, progress });
    }).catch((error: unknown) => {
      if (!controller.signal.aborted) setError(ApiFailure.fromRejection(error));
    });
    return () => controller.abort();
  }, [api, client, conversationId, jobId, active, retry]);
  if (!scene || scene.state === "done" || scene.state === "dismissed") return null;
  const job = watched?.id === jobId ? watched : null;
  const run = async (cancel: boolean) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      if (cancel && jobId) await api.call("jobCancel", { job_id: jobId });
      else await api.call("messageSceneImageDismiss", { message_id: message.id });
      void client.invalidateQueries({ queryKey: queryKeys.conversations.messages(conversationId) });
    } catch (error) {
      setError(ApiFailure.fromRejection(error));
    } finally { setBusy(false); }
  };
  const review = scene.state === "pending" && scene.mode !== "auto";
  const failed = scene.state === "failed";
  const progress = job?.progress;
  const image = imageProgress?.jobId === jobId ? imageProgress.progress : null;
  const numbers = new Intl.NumberFormat(i18n.language);
  const detail = image?.step != null && image.total_steps ? t("sceneImage.progress", { current: numbers.format(image.step), total: numbers.format(image.total_steps) })
    : image?.queue_position != null ? t("sceneImage.queuePosition", { position: numbers.format(image.queue_position) })
    : progress?.total && progress.total > 0 ? t("sceneImage.progress", { current: numbers.format(progress.current), total: numbers.format(progress.total) }) : null;
  return <div className="mt-3 space-y-2">
    {error && <Alert tone="danger" title={common(`errors.code.${error.code}`)} action={<Button size="sm" variant="ghost" onClick={() => { setError(null); setRetry((value) => value + 1); }}>{common("actions.retry")}</Button>} />}
    {review || failed ? <Alert tone={failed ? "danger" : "info"} title={t(failed ? "sceneImage.failed" : "sceneImage.requested")} description={failed ? t(`sceneImage.error.${scene.failure ?? "failed"}`) : t("sceneImage.requestedHint")} action={<div className="flex flex-wrap gap-2">
      <Button size="sm" variant="secondary" disabled={busy} onClick={() => onReview(message)}>{t(failed ? "sceneImage.retry" : "sceneImage.review")}</Button>
      <Button size="sm" variant="ghost" disabled={busy} onClick={() => void run(false)}>{t("sceneImage.dismiss")}</Button>
    </div>} /> : <div className="flex flex-wrap items-center gap-2 text-sm text-fg-3" role="status">
      <Spinner size="sm" />
      <span>{t(scene.state === "awaiting_turn" || job?.state === "succeeded" ? "sceneImage.attaching" : image ? `sceneImage.phase.${image.phase}` : "sceneImage.generating")}{detail ? ` · ${detail}` : ""}</span>
      {jobId && (!job || !terminalJobStates.has(job.state)) && <Button size="sm" variant="ghost" disabled={busy} onClick={() => void run(true)}>{common("actions.cancel")}</Button>}
    </div>}
  </div>;
}
