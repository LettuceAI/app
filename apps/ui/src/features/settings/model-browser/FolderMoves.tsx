import type { useFolderMoves } from "./folder-move-data";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, ListGroup, ListRow, Progress } from "@lettuceai/crisp";
import type { JobView } from "@/api/generated/bindings";
import { ApiFailure } from "@/api/client";
import { terminalJobStates, watchJob } from "@/entities/job";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi, useSettingsAction } from "../data";

export function FolderMoves({ state }: { state: ReturnType<typeof useFolderMoves> }) {
  return (
    <>
      {state.error && <FailureState error={state.error} onRetry={state.retry} />}
      {state.ids.map((id) => (
        <FolderMove
          key={id}
          id={id}
          initial={state.jobs.get(id) ?? null}
          onUpdate={state.observe}
          onAdmit={state.admit}
        />
      ))}
    </>
  );
}

function FolderMove({
  id,
  initial,
  onUpdate,
  onAdmit,
}: {
  id: string;
  initial: JobView | null;
  onUpdate: (job: JobView) => void;
  onAdmit: (id: string) => void;
}) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const action = useSettingsAction();
  const [job, setJob] = useState(initial);
  const [error, setError] = useState<unknown>(null);
  const [attempt, setAttempt] = useState(0);
  const terminal = initial ? terminalJobStates.has(initial.state) : false;
  useEffect(() => {
    if (terminal) return;
    const controller = new AbortController();
    void watchJob(
      api,
      id,
      (next) => {
        setJob(next);
        onUpdate(next);
      },
      controller.signal,
    ).catch((error: unknown) => {
      if (!controller.signal.aborted) setError(error);
    });
    return () => controller.abort();
  }, [api, id, terminal, attempt, onUpdate]);
  const current = initial && (!job || initial.updated_at > job.updated_at) ? initial : job;
  const detail = current?.subject_detail;
  const failed = current && ["failed", "interrupted", "cancelled"].includes(current.state);
  const active = !current || !terminalJobStates.has(current.state);
  return (
    <ListGroup>
      <ListRow
        title={t("hf.move")}
        description={detail?.type === "models_folder_move" ? detail.to : t("hf.queued")}
        trailing={current ? t(`tools.job.${current.state}`) : t("hf.queued")}
      />
      {active && (
        <Progress
          label={t("hf.move")}
          {...(current?.progress.total
            ? {
                value: (current.progress.current / current.progress.total) * 100,
              }
            : {})}
        />
      )}
      {error !== null && (
        <FailureState
          error={error}
          size="sm"
          onRetry={() => {
            setError(null);
            setAttempt((attempt) => attempt + 1);
          }}
        />
      )}
      {failed && (
        <FailureState
          error={new ApiFailure("unavailable", "models folder move failed", null)}
          size="sm"
        />
      )}
      {failed && detail?.type === "models_folder_move" && (
        <ListRow
          title={t("hf.retryMove")}
          trailing={
            <Button
              variant="secondary"
              loading={action.isPending}
              onClick={() =>
                action.mutate(async () => {
                  const accepted = await api.call("localModelsDirSet", {
                    path: detail.to,
                    move_existing: true,
                    client_operation_id: crypto.randomUUID(),
                  });
                  onAdmit(accepted.job_id);
                })
              }
            >
              {t("hf.retryMove")}
            </Button>
          }
        />
      )}
      {action.isError && <FailureState error={action.error} size="sm" />}
    </ListGroup>
  );
}
