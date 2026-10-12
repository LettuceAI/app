import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ApiClient } from "@/api/client";
import type { JobEvent, JobView } from "@/api/generated/bindings";
import { finishedStages, migrationStages, useLegacyMigration } from "./migration";

function job(state: JobView["state"], label: string | null): JobView {
  return {
    id: "job",
    kind: "backup_restore",
    subject: { type: "none" } as unknown as JobView["subject"],
    subject_detail: null,
    state,
    progress: { current: 0, total: null, unit: null, label_code: label, bytes_per_second: null },
    created_at: 0,
    updated_at: 0,
    failure: null,
    result: null,
  };
}

describe("finishedStages", () => {
  it("follows the import job's stage labels", () => {
    expect(finishedStages(null)).toBe(0);
    expect(finishedStages(job("running", null))).toBe(0);
    expect(finishedStages(job("running", "decode"))).toBe(0);
    expect(finishedStages(job("running", "stage"))).toBe(0);
    expect(finishedStages(job("running", "write"))).toBe(1);
    expect(finishedStages(job("running", "verify"))).toBe(2);
    expect(finishedStages(job("running", "switch"))).toBe(3);
  });

  it("counts every stage once the job succeeded", () => {
    expect(finishedStages(job("succeeded", "switch"))).toBe(migrationStages.length);
  });
});

it("keeps the current stage when a delayed watch snapshot arrives", async () => {
  let receive!: (event: JobEvent) => void;
  let reply!: (snapshot: JobView) => void;
  const initial = new Promise<JobView>((resolve) => { reply = resolve; });
  const api = {
    call: vi.fn().mockResolvedValue({ job_id: "job" }),
    stream: vi.fn((_command: string, _args: unknown, onEvent: (event: JobEvent) => void) => {
      receive = onEvent;
      return initial;
    }),
  } as unknown as ApiClient;
  const done = vi.fn().mockResolvedValue(undefined);
  const { result } = renderHook(() => useLegacyMigration(api, done));
  let moving!: Promise<void>;
  await act(async () => { moving = result.current.start(); });
  await act(async () => {
    receive({ type: "progress", job: job("running", "write") });
    reply(job("running", "decode"));
  });
  expect(finishedStages(result.current.state.job)).toBe(1);
  expect(done).not.toHaveBeenCalled();
  await act(async () => {
    receive({ type: "completed", job: job("succeeded", "switch") });
    await moving;
  });
  expect(result.current.state.phase).toBe("done");
  expect(done).toHaveBeenCalledOnce();
});
