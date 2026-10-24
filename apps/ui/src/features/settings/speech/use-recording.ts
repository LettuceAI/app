import { useQueryClient } from "@tanstack/react-query";
import { queryKeys } from "@/api/query-keys";
import { useEffect, useRef, useState } from "react";
import { ApiFailure, type ApiClient } from "@/api/client";
import type { AssetRef, TranscribeOptions, TranscriptionView } from "@/api/generated/bindings";
import { watchJob } from "@/entities/job";

export function useRecording(api: ApiClient, deviceId: string | null) {
  const client = useQueryClient();
  const [phase, setPhase] = useState<"idle" | "starting" | "recording" | "stopping" | "stop_failed" | "transcribing">("idle");
  const [audio, setAudio] = useState<AssetRef | null>(null);
  const [result, setResult] = useState<TranscriptionView | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [level, setLevel] = useState(0);
  const [elapsed, setElapsed] = useState(0);
  const pendingRequest = useRef<{ key: string; id: string } | null>(null);
  const state = useRef({ capture: null as string | null, job: null as string | null, busy: false, live: true, epoch: 0, watch: null as AbortController | null });
  useEffect(() => {
    const owned = state.current;
    owned.live = true;
    return () => {
      owned.live = false;
      owned.epoch++;
      owned.watch?.abort();
      if (owned.capture) void api.call("dictationCancel", { capture_id: owned.capture }).catch(() => undefined);
      if (owned.job) void api.call("jobCancel", { job_id: owned.job }).catch(() => undefined);
      owned.capture = null;
      owned.job = null;
    };
  }, [api]);
  useEffect(() => {
    if (phase !== "recording") return;
    const since = Date.now();
    const timer = window.setInterval(() => setElapsed(Date.now() - since), 250);
    let live = true;
    let stop: (() => void) | undefined;
    void api.subscribe((event) => {
      if (live && event.type === "dictation_level" && event.capture_id === state.current.capture) setLevel(event.level / 10);
    }).then((unsubscribe) => { if (live) stop = unsubscribe; else unsubscribe(); }).catch((failure: unknown) => { if (live) setError(failure); });
    return () => { live = false; window.clearInterval(timer); stop?.(); };
  }, [api, phase]);
  const start = async () => {
    const owned = state.current;
    if (owned.busy || owned.capture) return;
    owned.busy = true;
    const epoch = ++owned.epoch;
    setError(null); setResult(null); setAudio(null); setLevel(0); setElapsed(0); setPhase("starting");
    try {
      if (owned.job) { await api.call("jobCancel", { job_id: owned.job }); owned.job = null; }
      pendingRequest.current = null;
      const started = await api.call("audioCaptureStart", { device_id: deviceId });
      if (!owned.live || owned.epoch !== epoch) { await api.call("dictationCancel", { capture_id: started.capture_id }); return; }
      owned.capture = started.capture_id;
      setPhase("recording");
    } catch (failure) { if (owned.live && owned.epoch === epoch) { setError(failure); setPhase("idle"); } }
    finally { owned.busy = false; }
  };
  const stop = async (): Promise<AssetRef | null> => {
    const owned = state.current;
    if (owned.busy || !owned.capture) return null;
    owned.busy = true; setPhase("stopping");
    try {
      const captured = await api.call("audioCaptureStop", { capture_id: owned.capture });
      owned.capture = null;
      if (!owned.live) return null;
      setAudio(captured); setError(null); setPhase("idle");
      return captured;
    } catch (failure) {
      const ended = failure instanceof ApiFailure && (failure.code === "conflict" || failure.code === "not_found" || failure.details?.type === "speech" && failure.details.failure.type === "no_audio_captured");
      if (ended) owned.capture = null;
      if (owned.live) { setError(failure); setPhase(ended ? "idle" : "stop_failed"); }
      return null;
    }
    finally { owned.busy = false; }
  };
  const transcribe = async (source: AssetRef, modelId: string, options: TranscribeOptions): Promise<TranscriptionView | null> => {
    const owned = state.current;
    if (owned.busy || owned.capture) return null;
    owned.busy = true;
    const epoch = ++owned.epoch;
    const controller = new AbortController(); owned.watch = controller;
    setPhase("transcribing"); setError(null);
    try {
      const key = JSON.stringify([source.asset_id, modelId, options]);
      if (owned.job && pendingRequest.current?.key !== key) { await api.call("jobCancel", { job_id: owned.job }); owned.job = null; }
      if (pendingRequest.current?.key !== key) pendingRequest.current = { key, id: crypto.randomUUID() };
      const accepted = await api.call("transcribeFile", { request_id: pendingRequest.current.id, source: { uri: source.url }, model_id: modelId, retain_audio: true, options });
      if (!owned.live || owned.epoch !== epoch) { await api.call("jobCancel", { job_id: accepted.job_id }); return null; }
      owned.job = accepted.job_id;
      const job = await watchJob(api, accepted.job_id, () => undefined, controller.signal);
      owned.job = null;
      pendingRequest.current = null;
      void client.invalidateQueries({ queryKey: queryKeys.speech.all });
      if (job.state !== "succeeded" || job.result?.type !== "transcription") throw new ApiFailure("unavailable", "transcription did not succeed", job.failure?.speech ? { type: "speech", failure: job.failure.speech } : null);
      if (owned.live && owned.epoch === epoch) { setResult(job.result.transcription); return job.result.transcription; }
      return null;
    } catch (failure) { if (owned.live && !controller.signal.aborted) setError(failure); return null; }
    finally { owned.watch = null; owned.busy = false; if (owned.live && owned.epoch === epoch) setPhase("idle"); }
  };
  const cancel = async () => {
    const owned = state.current;
    owned.epoch++;
    owned.watch?.abort();
    try {
      if (owned.capture) { await api.call("dictationCancel", { capture_id: owned.capture }); owned.capture = null; }
      if (owned.job) { await api.call("jobCancel", { job_id: owned.job }); owned.job = null; }
      if (owned.live) setPhase("idle");
    } catch (failure) { if (owned.live) setError(failure); }
  };
  return { phase, audio, result, error, level, elapsed, start, stop, transcribe, cancel, setAudio };
}
