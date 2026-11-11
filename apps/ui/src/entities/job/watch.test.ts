import { describe, expect, it } from "vitest";
import type { JobView } from "@/api/generated/bindings";
import { downloadPhaseOf } from "./watch";

const job = (progress: Partial<JobView["progress"]>, state: JobView["state"] = "running"): JobView => ({
  id: "job",
  kind: "artifact_install",
  subject: { kind: "artifact_install", id: "artifact" },
  subject_detail: null,
  state,
  progress: { current: 0, total: null, unit: null, label_code: null, bytes_per_second: null, ...progress },
  created_at: 1,
  updated_at: 1,
  failure: null,
  result: null,
});

describe("downloadPhaseOf", () => {
  it("keeps a multi-file install downloading while bytes are still arriving in a later stage", () => {
    expect(downloadPhaseOf(job({ label_code: "install", unit: "bytes", current: 5_000_000, total: 170_000_000 }))).toBe("downloading");
  });

  it("verifies once every byte is in, or when the backend says so", () => {
    expect(downloadPhaseOf(job({ label_code: "install", unit: "bytes", current: 170_000_000, total: 170_000_000 }))).toBe("verifying");
    expect(downloadPhaseOf(job({ label_code: "verify", unit: "bytes", current: 170_000_000, total: 170_000_000 }))).toBe("verifying");
    expect(downloadPhaseOf(job({ label_code: "install" }))).toBe("verifying");
  });

  it("reads the download stage and terminal states", () => {
    expect(downloadPhaseOf(job({ label_code: "download", unit: "bytes", current: 0, total: 10 }))).toBe("downloading");
    expect(downloadPhaseOf(job({ label_code: null }))).toBe("downloading");
    expect(downloadPhaseOf(job({}, "succeeded"))).toBe("done");
    expect(downloadPhaseOf(job({}, "failed"))).toBe("failed");
  });
});
