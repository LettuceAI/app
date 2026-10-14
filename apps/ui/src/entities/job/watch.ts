import type { ApiClient } from "@/api/client";
import type { JobView } from "@/api/generated/bindings";

export const terminalJobStates: ReadonlySet<JobView["state"]> = new Set(["succeeded", "failed", "cancelled", "interrupted"]);

export type DownloadPhase = "downloading" | "verifying" | "done" | "failed";

/** Where a download-and-verify install job is, from its state and stage label. */
export function downloadPhaseOf(job: JobView): DownloadPhase {
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

/**
 * Follows a job until it settles, reporting each view, and resolves with the settled one. `jobWatch` answers with the
 * job as it is when the watch attaches; every later change arrives as an event.
 */
export function watchJob(
  api: ApiClient,
  jobId: string,
  onJob: (job: JobView) => void,
  signal: AbortSignal,
  onText?: (text: string | null, reasoning: string | null) => void,
): Promise<JobView> {
  return new Promise((resolve, reject) => {
    const stream = new AbortController();
    let settled = false;
    const stop = () => {
      if (settled) return;
      settled = true;
      stream.abort();
      reject(new DOMException("The job watch was stopped", "AbortError"));
    };
    if (signal.aborted) {
      stop();
      return;
    }
    signal.addEventListener("abort", stop, { once: true });
    const apply = (job: JobView) => {
      if (settled) return;
      const terminal = terminalJobStates.has(job.state);
      if (terminal) {
        settled = true;
        signal.removeEventListener("abort", stop);
      }
      onJob(job);
      if (terminal) {
        stream.abort();
        resolve(job);
      }
    };
    api
      .stream(
        "jobWatch",
        [{ job_id: jobId }],
        (event) => {
          if ("job" in event) apply(event.job);
          else if (event.type === "text_delta" && !settled) onText?.(event.text, event.reasoning);
        },
        { signal: stream.signal },
      )
      .then(apply, (error: unknown) => {
        if (settled) return;
        settled = true;
        signal.removeEventListener("abort", stop);
        reject(error);
      });
  });
}
