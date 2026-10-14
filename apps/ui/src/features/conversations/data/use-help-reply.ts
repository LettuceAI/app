import { useCallback, useEffect, useRef, useState } from "react";
import type { ApiClient } from "@/api/client";
import { ApiFailure } from "@/api/client";
import type { HelpMeReplyMode } from "@/api/generated/bindings";
import { watchJob } from "@/entities/job/watch";

export interface HelpReplyState {
  status: "idle" | "writing" | "ready" | "failed";
  text: string;
  thinking: boolean;
  error: string | null;
}

const idle: HelpReplyState = { status: "idle", text: "", thinking: false, error: null };

export function useHelpReply(api: ApiClient, conversationId: string, onError: (failure: ApiFailure) => void) {
  const [state, setState] = useState<HelpReplyState>(idle);
  const current = useRef<{ controller: AbortController; jobId: string | null } | null>(null);
  const errorRef = useRef(onError);
  useEffect(() => {
    errorRef.current = onError;
  });

  const cancel = useCallback(() => {
    const run = current.current;
    current.current = null;
    if (!run) return;
    run.controller.abort();
    if (run.jobId) void api.call("jobCancel", { job_id: run.jobId }).catch(() => undefined);
  }, [api]);

  useEffect(() => cancel, [cancel]);

  const reset = useCallback(() => {
    cancel();
    setState(idle);
  }, [cancel]);

  const start = useCallback(
    async (mode: HelpMeReplyMode, draft: string, swapPlaces: boolean) => {
      cancel();
      const run: { controller: AbortController; jobId: string | null } = { controller: new AbortController(), jobId: null };
      current.current = run;
      setState({ status: "writing", text: "", thinking: false, error: null });
      try {
        const accepted = await api.call("conversationHelpMeReply", {
          conversation_id: conversationId,
          mode,
          current_draft: mode === "enrich" && draft.trim() ? draft : null,
          swap_places: swapPlaces,
          client_operation_id: crypto.randomUUID(),
        });
        if (current.current !== run) {
          void api.call("jobCancel", { job_id: accepted.job_id }).catch(() => undefined);
          return;
        }
        run.jobId = accepted.job_id;
        let streamed = "";
        const job = await watchJob(
          api,
          accepted.job_id,
          () => undefined,
          run.controller.signal,
          (text, reasoning) => {
            if (current.current !== run) return;
            if (text) {
              streamed += text;
              setState({ status: "writing", text: streamed, thinking: false, error: null });
            } else if (reasoning && !streamed) {
              setState((previous) => (previous.thinking ? previous : { ...previous, thinking: true }));
            }
          },
        );
        if (current.current !== run) return;
        current.current = null;
        if (job.state === "succeeded") {
          const final = job.result?.type === "generated_text" ? job.result.text.trim() : "";
          const text = final || streamed.trim();
          setState(text ? { status: "ready", text, thinking: false, error: null } : { status: "failed", text: "", thinking: false, error: "help_me_reply_no_reply" });
        } else if (job.state === "failed") {
          setState({ status: "failed", text: "", thinking: false, error: job.failure?.reason ?? "failed" });
        } else {
          setState(idle);
        }
      } catch (error) {
        if (current.current !== run) return;
        current.current = null;
        setState(idle);
        errorRef.current(ApiFailure.fromRejection(error));
      }
    },
    [api, conversationId, cancel],
  );

  return { state, start, cancel, reset };
}
