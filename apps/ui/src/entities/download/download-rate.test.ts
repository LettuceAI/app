import { describe, expect, it } from "vitest";
import type { JobView } from "@/api/generated/bindings";
import { downloadProgressOf } from "@/entities/job";
import { DownloadRateTracker } from "./download-rate";

const job = (current: number, patch: Partial<JobView> = {}): JobView => ({ id: "whisper", kind: "artifact_install", subject: { kind: "artifact_install", id: "model" }, subject_detail: { type: "whisper_model_install", model_id: "large-v3-turbo-q5_0", filename: "ggml-large-v3-turbo-q5_0.bin" }, state: "running", progress: { current, total: 1000, unit: "bytes", bytes_per_second: null, label_code: "download" }, created_at: 7, updated_at: 7, failure: null, result: null, ...patch });

describe("shared download measurements", () => {
  it("measures resumed byte progress despite a fixed backend timestamp", () => {
    const tracker = new DownloadRateTracker();
    expect(downloadProgressOf(tracker.observe(job(500), 1000)).speed).toBeUndefined();
    const measured = tracker.observe(job(600), 2000);
    expect(downloadProgressOf(measured)).toEqual({ fraction: 0.6, size: 1000, speed: 100, eta: 4 });
  });

  it("keeps a measurement across duplicate events without resetting the time baseline", () => {
    const tracker = new DownloadRateTracker();
    tracker.observe(job(100), 1000);
    tracker.observe(job(200), 2000);
    expect(downloadProgressOf(tracker.observe(job(200), 2500)).speed).toBe(100);
    expect(downloadProgressOf(tracker.observe(job(400), 4000)).speed).toBe(100);
    expect(downloadProgressOf(tracker.observe(job(500), 4000)).speed).toBe(100);
    expect(downloadProgressOf(tracker.observe(job(600), 6000)).speed).toBe(100);
  });

  it("resets after a restart or a changed download size", () => {
    const tracker = new DownloadRateTracker();
    tracker.observe(job(500), 1000);
    tracker.observe(job(600), 2000);
    expect(downloadProgressOf(tracker.observe(job(0), 3000)).speed).toBeUndefined();
    expect(downloadProgressOf(tracker.observe(job(100), 4000)).speed).toBe(100);
    const changed = job(200);
    changed.progress.total = 2000;
    expect(downloadProgressOf(tracker.observe(changed, 5000)).speed).toBeUndefined();
  });

  it("drops speed and ETA while queued, verifying, cancelling or finished", () => {
    for (const patch of [{ state: "queued" as const }, { state: "cancellation_requested" as const }, { state: "succeeded" as const }, { progress: { ...job(1000).progress, label_code: "verify" } }]) {
      const tracker = new DownloadRateTracker();
      tracker.observe(job(100), 1000);
      tracker.observe(job(200), 2000);
      expect(downloadProgressOf(tracker.observe(job(200, patch), 3000)).speed).toBeUndefined();
      expect(tracker.has("whisper")).toBe(false);
      expect(downloadProgressOf(tracker.observe(job(300), 4000)).speed).toBeUndefined();
    }
  });
});
