import { useCallback, useEffect, useRef, useState } from "react";
import type { ApiClient } from "@/api/client";
import type { JobView } from "@/api/generated/bindings";
import { downloadPhaseOf, terminalJobStates, watchJob, type DownloadPhase } from "@/entities/job";

export type InstallPhase = "idle" | DownloadPhase;

export interface InstallState {
  phase: InstallPhase;
  job: JobView | null;
  error: unknown;
}

const idle: InstallState = { phase: "idle", job: null, error: null };

export function useEmbeddingInstall(api: ApiClient) {
  const [state, setState] = useState<InstallState>(idle);
  const abort = useRef<AbortController | null>(null);

  useEffect(() => () => abort.current?.abort(), []);

  const start = useCallback(async () => {
    abort.current?.abort();
    const controller = new AbortController();
    abort.current = controller;
    setState({ phase: "downloading", job: null, error: null });
    const apply = (job: JobView) => {
      if (controller.signal.aborted) return;
      setState({ phase: downloadPhaseOf(job), job, error: null });
      if (terminalJobStates.has(job.state)) controller.abort();
    };
    try {
      const accepted = await api.call("embeddingInstall", { family: "lettuce_eidos_v5", enable_dynamic_memory: true });
      await watchJob(api, accepted.job_id, apply, controller.signal);
    } catch (error) {
      if (!controller.signal.aborted) setState({ phase: "failed", job: null, error });
    }
  }, [api]);

  const reset = useCallback(() => {
    abort.current?.abort();
    setState(idle);
  }, []);

  return { state, start, reset };
}
