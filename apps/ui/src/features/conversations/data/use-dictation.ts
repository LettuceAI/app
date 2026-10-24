import { useQueryClient } from "@tanstack/react-query";
import { appStatusQuery } from "@/api/app-status-query";
import { useCallback, useEffect, useRef, useState } from "react";
import type { ApiClient } from "@/api/client";
import { ApiFailure } from "@/api/client";
import type { JobView, SpeechFailure } from "@/api/generated/bindings";
import { watchJob } from "@/entities/job/watch";

export type DictationPhase = "idle" | "recording" | "transcribing";

export type DictationErrorKey = "noModel" | "permission" | "noMicrophone" | "noAudio" | "busy" | "unsupported" | "failed";

const speechKeys: Partial<Record<SpeechFailure["type"], DictationErrorKey>> = {
  model_required: "noModel",
  microphone_permission_denied: "permission",
  no_microphone: "noMicrophone",
  no_audio_captured: "noAudio",
};

export function dictationErrorKey(error: ApiFailure): DictationErrorKey | null {
  if (error.details?.type === "speech") return speechKeys[error.details.failure.type] ?? null;
  if (error.code === "conflict" && error.details?.type === "dictation_active") return "busy";
  if (error.code === "unsupported") return "unsupported";
  return null;
}

function jobErrorKey(job: JobView): DictationErrorKey {
  const speech = job.failure?.speech;
  return (speech && speechKeys[speech.type]) || "failed";
}

export interface DictationHooks {
  onText: (text: string) => void;
  onError: (error: ApiFailure) => void;
  onJobError: (key: DictationErrorKey) => void;
}

export function useDictation(api: ApiClient, conversationId: string, hooks: DictationHooks) {
  const queryClient = useQueryClient();
  const [phase, setPhase] = useState<DictationPhase>("idle");
  const [elapsedMs, setElapsedMs] = useState(0);
  const [level, setLevel] = useState(0);
  const capture = useRef<string | null>(null);
  const starting = useRef(false);
  const controller = useRef<AbortController | null>(null);
  const transcribing = useRef<string | null>(null);
  const unmounted = useRef(false);
  const hooksRef = useRef(hooks);
  useEffect(() => {
    hooksRef.current = hooks;
  });

  useEffect(() => {
    if (phase !== "recording") return;
    const startedAt = Date.now();
    const tick = window.setInterval(() => setElapsedMs(Date.now() - startedAt), 250);
    return () => window.clearInterval(tick);
  }, [phase]);

  useEffect(() => {
    if (phase !== "recording") return;
    let live = true;
    let unsubscribe: (() => void) | null = null;
    void api
      .subscribe((event) => {
        if (event.type === "dictation_level" && event.capture_id === capture.current) setLevel(event.level / 1000);
      })
      .then((off) => {
        if (live) unsubscribe = off;
        else off();
      })
      .catch(() => undefined);
    return () => {
      live = false;
      unsubscribe?.();
      setLevel(0);
    };
  }, [api, phase]);

  useEffect(() => {
    unmounted.current = false;
    return () => {
      unmounted.current = true;
      controller.current?.abort();
      const jobId = transcribing.current;
      transcribing.current = null;
      if (jobId) void api.call("jobCancel", { job_id: jobId }).catch(() => undefined);
      const id = capture.current;
      capture.current = null;
      if (id) void api.call("dictationCancel", { capture_id: id }).catch(() => undefined);
    };
  }, [api]);

  const start = useCallback(async () => {
    if (phase !== "idle" || starting.current) return;
    starting.current = true;
    try {
      const query = appStatusQuery(api);
      let status = queryClient.getQueryData(query.queryKey);
      if (!status) {
        status = await queryClient.ensureQueryData(query);
        if (unmounted.current) return;
      }
      const microphone = status.ui_state.speechMicrophoneId;
      const started = await api.call("dictationStart", { conversation_id: conversationId, device_id: typeof microphone === "string" ? microphone : null });
      if (unmounted.current) {
        void api.call("dictationCancel", { capture_id: started.capture_id }).catch(() => undefined);
        return;
      }
      capture.current = started.capture_id;
      setElapsedMs(0);
      setPhase("recording");
    } catch (error) {
      hooksRef.current.onError(ApiFailure.fromRejection(error));
    } finally {
      starting.current = false;
    }
  }, [api, conversationId, phase, queryClient]);

  const cancel = useCallback(() => {
    const id = capture.current;
    capture.current = null;
    setPhase("idle");
    if (id) void api.call("dictationCancel", { capture_id: id }).catch(() => undefined);
  }, [api]);

  const stop = useCallback(async () => {
    const id = capture.current;
    if (!id || phase !== "recording") return;
    setPhase("transcribing");
    const watch = new AbortController();
    controller.current = watch;
    try {
      const accepted = await api.call("dictationStop", { capture_id: id, model_id: null });
      capture.current = null;
      transcribing.current = accepted.job_id;
      const job = await watchJob(api, accepted.job_id, () => undefined, watch.signal);
      if (job.state === "succeeded" && job.result?.type === "transcription") {
        const { text, raw_text } = job.result.transcription;
        const spoken = text.trim() || raw_text.trim();
        if (spoken) hooksRef.current.onText(spoken);
        else hooksRef.current.onJobError("noAudio");
      } else if (job.state === "failed") {
        hooksRef.current.onJobError(jobErrorKey(job));
      }
    } catch (error) {
      if (watch.signal.aborted) return;
      if (capture.current) {
        const left = capture.current;
        capture.current = null;
        void api.call("dictationCancel", { capture_id: left }).catch(() => undefined);
      }
      hooksRef.current.onError(ApiFailure.fromRejection(error));
    } finally {
      transcribing.current = null;
      if (controller.current === watch) controller.current = null;
      if (!watch.signal.aborted) setPhase("idle");
    }
  }, [api, phase]);

  return { phase, elapsedMs, level, start, stop, cancel };
}
