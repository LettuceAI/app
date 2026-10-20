import { useCallback, useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { History, RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Alert, Button, icon, Progress, toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { JobAccepted, JobProgressDto, MemoryStatusView } from "@/api/generated/bindings";
import { watchJob } from "@/entities/job";
import { ModelChoiceSheet } from "@/features/characters";
import { FailureState } from "@/shared/ui/FailureState";
import { useMemoryError } from "./errors";
import { createOperationIds } from "./operation-ids";
import { refreshMemory } from "./use-memory-write";

export interface StatusCardProps {
  api: ApiClient;
  conversationId: string;
  status: MemoryStatusView;
}

export function StatusCard({ api, conversationId, status }: StatusCardProps) {
  const { t } = useTranslation("conversations");
  const queryClient = useQueryClient();
  const errorText = useMemoryError();
  const [ids] = useState(createOperationIds);
  const [accepted, setAccepted] = useState<string | null>(null);
  const [progress, setProgress] = useState<JobProgressDto | null>(null);
  const [picking, setPicking] = useState(false);
  const [busy, setBusy] = useState(false);
  const [watchAttempt, setWatchAttempt] = useState(0);
  const [watchError, setWatchError] = useState<{ jobId: string; error: unknown } | null>(null);
  const finished = useRef(new Set<string>());
  const running = status.latest_cycle_status === "queued" || status.latest_cycle_status === "processing";
  const acceptedSettled = accepted !== null && accepted === status.latest_job_id && !running;
  const watched = acceptedSettled ? null : accepted ?? (running ? status.latest_job_id : null);
  const refresh = useCallback(() => refreshMemory(queryClient, api, conversationId), [queryClient, api, conversationId]);

  useEffect(() => {
    if (watched === null || finished.current.has(watched)) return;
    const controller = new AbortController();
    setProgress(null);
    setWatchError(null);
    void watchJob(api, watched, (job) => setProgress(job.progress), controller.signal)
      .then(
        async () => {
          if (controller.signal.aborted) return;
          finished.current.add(watched);
          await refresh();
          if (controller.signal.aborted) return;
          setAccepted(null);
          setProgress(null);
        },
        (error: unknown) => {
          if (controller.signal.aborted) return;
          setWatchError({ jobId: watched, error });
          void refresh();
        },
      );
    return () => controller.abort();
  }, [api, watched, refresh, watchAttempt]);

  const perform = async (key: string, task: (operationId: string) => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    try {
      await task(ids.idFor(key));
      ids.settle(key);
      await refresh();
    } catch (error) {
      if (isApiFailure(error) && error.code === "conflict") ids.settle(key);
      toast.error(errorText(error));
      await refresh();
    } finally {
      setBusy(false);
    }
  };

  const start = (key: string, request: (operationId: string) => Promise<JobAccepted>) =>
    perform(key, async (operationId) => {
      setAccepted((await request(operationId)).job_id);
    });
  const trigger = () => start("trigger", (operationId) => api.call("memoryTrigger", { conversation_id: conversationId, client_operation_id: operationId }));
  const retry = (model: string | null) =>
    start(`retry:${model ?? ""}`, (operationId) =>
      api.call("memoryRetry", { conversation_id: conversationId, model_profile_id: model, client_operation_id: operationId }),
    );
  const skip = () =>
    perform("skip", async (operationId) => {
      await api.call("memorySkip", { conversation_id: conversationId, client_operation_id: operationId });
      toast.info(t("memory.status.skipped", { count: status.interval }));
    });
  const dismiss = () =>
    perform("dismiss", async (operationId) => {
      await api.call("memoryErrorDismiss", { conversation_id: conversationId, client_operation_id: operationId });
    });
  const stop = async () => {
    if (watched === null) return;
    try {
      await api.call("jobCancel", { job_id: watched });
      await refresh();
    } catch (error) {
      toast.error(errorText(error));
    }
  };

  const approvals = status.pending_approval_count ?? 0;
  const modelSheet = (
    <ModelChoiceSheet
      api={api}
      open={picking}
      value={null}
      onChange={(id) => void retry(id)}
      onClose={() => setPicking(false)}
    />
  );

  if (watched !== null || running) {
    const value = progress?.total ? Math.min(100, (progress.current / progress.total) * 100) : undefined;
    return (
      <div className="space-y-3 rounded-xl border border-line bg-surface-1 p-4">
        <div className="flex items-center justify-between gap-3">
          <div className="min-w-0">
            <p className="text-base font-medium text-fg">{t("memory.status.runningTitle")}</p>
            <p className="text-sm text-fg-3">{t("memory.status.runningBody")}</p>
          </div>
          {watched !== null && (
            <Button size="sm" variant="secondary" onClick={() => void stop()}>
              {t("memory.status.stop")}
            </Button>
          )}
        </div>
        <Progress {...(value === undefined ? {} : { value })} label={t("memory.status.runningTitle")} />
        {watchError?.jobId === watched && <FailureState error={watchError.error} onRetry={() => setWatchAttempt((current) => current + 1)} size="sm" />}
      </div>
    );
  }

  if (approvals > 0) {
    return (
      <Alert
        tone="info"
        title={t("memory.status.approvalTitle")}
        description={t("memory.status.approvalBody", { count: approvals })}
        action={
          <div className="flex gap-2">
            <Button size="sm" disabled={busy} onClick={() => void trigger()}>
              {t("memory.status.update")}
            </Button>
            <Button size="sm" variant="ghost" disabled={busy} onClick={() => void skip()}>
              {t("memory.status.notNow")}
            </Button>
          </div>
        }
      />
    );
  }

  if (status.failure !== null) {
    return (
      <>
        <Alert
          tone="danger"
          title={t("memory.status.failedTitle")}
          description={t(`memory.failure.${status.failure}`)}
          onDismiss={() => void dismiss()}
          action={
            <div className="flex flex-wrap gap-2">
              <Button size="sm" disabled={busy} onClick={() => void retry(null)}>
                {t("memory.status.tryAgain")}
              </Button>
              <Button size="sm" variant="secondary" disabled={busy} onClick={() => setPicking(true)}>
                {t("memory.status.otherModel")}
              </Button>
            </div>
          }
        />
        {modelSheet}
      </>
    );
  }

  if (status.paused_reason === "lease_lost") {
    return (
      <Alert
        tone="warning"
        title={t("memory.status.pausedTitle")}
        description={t("memory.status.pausedBody")}
        action={
          <Button size="sm" disabled={busy} onClick={() => void retry(null)}>
            {t("memory.status.tryAgain")}
          </Button>
        }
      />
    );
  }

  const mode = t(`memory.status.modes.${status.run_mode}`);
  const manual = status.run_mode === "manual";
  const percent = status.interval > 0 ? Math.min(100, (status.messages_since_last_cycle / status.interval) * 100) : 0;
  return (
    <div className="rounded-xl border border-line bg-surface-1 p-4">
      <div className="flex items-center gap-3">
        <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-line text-accent">
          <History size={icon.md} />
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-base font-medium text-fg">
            {manual ? t("memory.status.since", { count: status.messages_since_last_cycle }) : t("memory.status.nextIn", { count: status.messages_until_next_cycle })}
          </p>
          <p className="text-sm text-fg-3">
            {manual ? t("memory.status.manualOnly", { mode }) : t("memory.status.every", { mode, count: status.interval })}
          </p>
        </div>
      </div>
      <div className="mt-3 flex items-center gap-3">
        <Progress value={percent} size="sm" className="flex-1" label={t("memory.status.progress", { current: status.messages_since_last_cycle, total: status.interval })} />
        <span className="shrink-0 text-xs tabular-nums text-fg-3">{t("memory.status.progress", { current: status.messages_since_last_cycle, total: status.interval })}</span>
        <Button size="sm" variant="ghost" className="-mr-2 shrink-0" disabled={busy} leading={<RefreshCw size={icon.sm} />} onClick={() => void trigger()}>
          {t("memory.status.updateNow")}
        </Button>
      </div>
    </div>
  );
}
