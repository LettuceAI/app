import { useCallback, useEffect, useRef, useState } from "react";
import type { ApiClient } from "@/api/client";
import type { JobView } from "@/api/generated/bindings";
import { terminalJobStates, watchJob } from "@/entities/job";

export const migrationStages = ["read", "write", "verify", "switch"] as const;
export type MigrationStage = (typeof migrationStages)[number];

const stageOfLabel: Record<string, number> = { decode: 0, stage: 0, write: 1, verify: 2, switch: 3 };

/** How many stages of the move are finished, from the import job's stage label. */
export function finishedStages(job: JobView | null): number {
  if (!job) return 0;
  if (job.state === "succeeded") return migrationStages.length;
  const label = job.progress.label_code;
  return label === null ? 0 : (stageOfLabel[label] ?? 0);
}

export interface MigrationState {
  phase: "idle" | "running" | "done" | "failed";
  job: JobView | null;
  error: unknown;
}

const idle: MigrationState = { phase: "idle", job: null, error: null };

export function useLegacyMigration(api: ApiClient, onDone: () => Promise<void>) {
  const [state, setState] = useState<MigrationState>(idle);
  const abort = useRef<AbortController | null>(null);
  const running = useRef(false);
  const done = useRef(onDone);
  useEffect(() => {
    done.current = onDone;
  });
  useEffect(() => () => abort.current?.abort(), []);

  const start = useCallback(async () => {
    if (running.current) return;
    running.current = true;
    abort.current?.abort();
    const controller = new AbortController();
    abort.current = controller;
    setState({ phase: "running", job: null, error: null });
    try {
      const accepted = await api.call("legacyDatabaseImport", { client_operation_id: crypto.randomUUID() });
      const finished = await watchJob(
        api,
        accepted.job_id,
        (job) => {
          if (!terminalJobStates.has(job.state)) {
            setState((current) => finishedStages(job) < finishedStages(current.job) ? current : { phase: "running", job, error: null });
          }
        },
        controller.signal,
      );
      if (controller.signal.aborted) return;
      if (finished.state !== "succeeded") {
        setState({ phase: "failed", job: finished, error: finished.failure });
        return;
      }
      setState({ phase: "done", job: finished, error: null });
      await done.current();
    } catch (error) {
      if (!controller.signal.aborted) setState((current) => ({ phase: "failed", job: current.job, error }));
    } finally {
      running.current = false;
    }
  }, [api]);

  return { state, start };
}
