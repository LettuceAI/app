import { describe, expect, it } from "vitest";
import { ApiFailure } from "@/api/client";
import type { ImageFailureKind, JobFailureDto, JobView } from "@/api/generated/bindings";
import type { AvatarSessionState } from "./avatar-session";
import { avatarNotice } from "./avatar-notice";

const model = { id: "m", name: "Model", provider_kind: "openai", supports_editing: true };
const base: AvatarSessionState = { options: { enabled: true, model }, loadingOptions: false, phase: "idle", variants: [], selectedId: null, job: null, error: null, errorSource: null, pendingRequest: false };
const failure = (patch: Partial<JobFailureDto>): JobFailureDto => ({ entity_import: null, code: "unknown", retryable: true, reason: null, model: null, hugging_face: null, ollama: null, image: null, speech: null, ...patch });
const job = (state: JobView["state"], failed: JobFailureDto | null): JobView => ({
  id: "j", kind: "image_generate", subject: { kind: "image_request", id: "j" }, subject_detail: null, state,
  progress: { current: 0, total: null, unit: null, label_code: null, bytes_per_second: null }, created_at: 1, updated_at: 1, failure: failed, result: null,
});
const failed = (patch: Partial<JobFailureDto>): AvatarSessionState => ({ ...base, job: job("failed", failure(patch)), error: new ApiFailure("unavailable", "diagnostic", null), errorSource: "generation" });

describe("avatar notice", () => {
  it("is quiet when the model is usable", () => {
    expect(avatarNotice(base)).toBeNull();
  });

  it.each([
    [{ enabled: false, model: null }, "disabled"],
    [{ enabled: true, model: null }, "noModel"],
    [{ enabled: true, model: { ...model, supports_editing: false } }, "incompatible"],
  ] as const)("explains unusable options %j", (options, kind) => {
    expect(avatarNotice({ ...base, options })).toMatchObject({ kind, configure: true, retry: null });
  });

  it("offers a retry for options that failed to load", () => {
    expect(avatarNotice({ ...base, options: null, error: new ApiFailure("transport", "x", null), errorSource: "options" })).toMatchObject({ kind: "options", retry: "options" });
  });

  it("points stale and incompatible model failures to settings", () => {
    for (const error of [new ApiFailure("not_found", "deleted model", null), new ApiFailure("invalid_input", "unsupported model", { type: "invalid_field", field: "model_id" })]) {
      expect(avatarNotice({ ...base, error, errorSource: "generation" })).toMatchObject({ copy: "setup", configure: true });
    }
    expect(avatarNotice({ ...base, options: null, error: new ApiFailure("invalid_input", "unsupported model", null), errorSource: "options" })).toMatchObject({ configure: true });
  });

  it.each<[ImageFailureKind, string, boolean]>([
    ["model_file_missing", "setup", true],
    ["out_of_memory", "memory", false],
    ["no_image_returned", "empty", false],
    ["engine_timed_out", "timedOut", false],
  ])("maps the image failure %s to localized copy", (kind, copy, configure) => {
    expect(avatarNotice(failed({ image: { kind, message: "diagnostic" } }))).toMatchObject({ kind: "generation", copy, configure });
  });

  it("maps a safety refusal and respects non-retryable failures", () => {
    expect(avatarNotice(failed({ code: "safety_refusal", retryable: false }))).toMatchObject({ copy: "refused", retry: null, configure: false });
    expect(avatarNotice(failed({ code: "worker_failed", retryable: true }))).toMatchObject({ retry: "generation" });
  });

  it("treats a succeeded job with no image as empty", () => {
    expect(avatarNotice({ ...base, job: job("succeeded", null), error: new ApiFailure("unavailable", "x", null), errorSource: "generation" })).toMatchObject({ copy: "empty" });
  });

  it("asks for recovery when admission or observation is uncertain, and keeps cancellation retryable", () => {
    const uncertain = { ...base, pendingRequest: true, error: new ApiFailure("transport", "x", null), errorSource: "generation" as const };
    expect(avatarNotice(uncertain)).toMatchObject({ kind: "uncertain", retry: "generation", cancel: true });
    expect(avatarNotice({ ...uncertain, error: new ApiFailure("transport", "x", null), errorSource: "cancellation" })).toMatchObject({ kind: "cancellation", retry: "cancellation" });
  });

  it("notes a cancelled job", () => {
    expect(avatarNotice({ ...base, job: job("cancelled", null) })).toMatchObject({ kind: "cancelled", tone: "info" });
  });
});
