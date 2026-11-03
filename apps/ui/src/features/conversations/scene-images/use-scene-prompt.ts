import { useCallback, useEffect, useRef, useState } from "react";
import { ApiFailure, type ApiClient } from "@/api/client";
import { watchJob } from "@/entities/job";

interface Attempt {
  operationId: string;
  jobId: string | null;
  terminal: boolean;
}

export function useScenePrompt(api: ApiClient, messageId: string) {
  const [state, setState] = useState({ writing: false, text: "", thinking: false, error: null as ApiFailure | null, reason: null as string | null });
  const current = useRef<{ controller: AbortController; attempt: Attempt } | null>(null);
  const attempt = useRef<Attempt | null>(null);
  const cancel = useCallback(() => {
    const run = current.current;
    current.current = null;
    run?.controller.abort();
    const pending = run?.attempt ?? attempt.current;
    if (pending?.jobId && !pending.terminal) void api.call("jobCancel", { job_id: pending.jobId }).catch(() => undefined);
  }, [api]);
  useEffect(() => cancel, [cancel]);

  const start = async () => {
    if (current.current) return null;
    const request = attempt.current && !attempt.current.terminal ? attempt.current : { operationId: crypto.randomUUID(), jobId: null, terminal: false };
    attempt.current = request;
    const run = { controller: new AbortController(), attempt: request };
    current.current = run;
    setState({ writing: true, text: "", thinking: false, error: null, reason: null });
    try {
      let jobId = request.jobId;
      if (!jobId) {
        const accepted = await api.call("messageScenePromptGenerate", { message_id: messageId, client_operation_id: request.operationId });
        jobId = accepted.job_id;
        run.attempt = { ...request, jobId };
        attempt.current = run.attempt;
      }
      if (current.current !== run) {
        void api.call("jobCancel", { job_id: jobId }).catch(() => undefined);
        return null;
      }
      let streamed = "";
      const job = await watchJob(api, jobId, () => undefined, run.controller.signal, (text, reasoning) => {
        if (current.current !== run) return;
        streamed += text ?? "";
        setState((previous) => ({ ...previous, text: streamed, thinking: !streamed && Boolean(reasoning) }));
      });
      if (current.current !== run) return null;
      attempt.current = { ...run.attempt, terminal: true };
      const text = job.state === "succeeded" && job.result?.type === "generated_text" ? job.result.text.trim() : "";
      const reason = text ? null : job.state === "cancelled" ? "cancelled" : job.failure?.reason ?? "scene_prompt_no_reply";
      setState({ writing: false, text, thinking: false, error: null, reason });
      return text || null;
    } catch (error) {
      if (current.current === run) setState((previous) => ({ ...previous, writing: false, error: ApiFailure.fromRejection(error) }));
      return null;
    } finally {
      if (current.current === run) current.current = null;
    }
  };
  return { ...state, start, cancel };
}
