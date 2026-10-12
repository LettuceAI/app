import { useCallback, useEffect, useRef, useState } from "react";
import type { ApiClient } from "@/api/client";
import type { JobView } from "@/api/generated/bindings";

export type InstallPhase = "idle" | "downloading" | "verifying" | "done" | "failed";

export interface InstallState {
  phase: InstallPhase;
  job: JobView | null;
  error: unknown;
}

const idle: InstallState = { phase: "idle", job: null, error: null };

const terminalStates: ReadonlySet<JobView["state"]> = new Set(["succeeded", "failed", "cancelled", "interrupted"]);

export function phaseOf(job: JobView): InstallPhase {
  switch (job.state) {
    case "succeeded":
      return "done";
    case "failed":
    case "cancelled":
    case "interrupted":
      return "failed";
    default: {
      const label = job.progress.label_code;
      return label === null || label === "download" ? "downloading" : "verifying";
    }
  }
}

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
      setState({ phase: phaseOf(job), job, error: null });
      if (terminalStates.has(job.state)) controller.abort();
    };
    try {
      const accepted = await api.call("embeddingInstall", { family: "lettuce_eidos_v5", enable_dynamic_memory: true });
      const finished = await api.stream(
        "jobWatch",
        [{ job_id: accepted.job_id }],
        (event) => {
          if ("job" in event) apply(event.job);
        },
        { signal: controller.signal },
      );
      apply(finished);
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
