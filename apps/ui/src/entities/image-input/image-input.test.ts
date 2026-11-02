import { describe, expect, it, vi } from "vitest";
import { ApiFailure, createApiClient } from "@/api/client";
import type { AssetRef, AvatarGenerateRequest, AvatarGenerationOptions, JobView } from "@/api/generated/bindings";
import { createMockTransport, type MockCallHandlers, type MockStreamHandlers } from "@/api/mock-transport";
import { AvatarSession, pickImage, pickImages } from ".";

const ok = <T>(data: T) => ({ status: "ok" as const, data });
const asset = (id: string): AssetRef => ({ asset_id: id, url: `asset://${id}`, preview_url: null });
const options: AvatarGenerationOptions = { enabled: true, model: { id: "model", name: "Image model", provider_kind: "openai", supports_editing: true } };
const subject = { subjectName: "Ada", subjectDescription: "Engineer" };
const job = (id: string, state: JobView["state"] = "succeeded"): JobView => ({
  id, kind: "image_generate", subject: { kind: "image_request", id }, subject_detail: null, state,
  progress: { current: 0, total: null, unit: null, label_code: null, bytes_per_second: null }, created_at: 1, updated_at: 1, failure: null,
  result: state === "succeeded" ? { type: "image_generation", images: [{ asset: asset(id), mime_type: "image/png", width: 512, height: 512, text: null }], rejected_outputs: 0 } : null,
});
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}
function setup(calls: MockCallHandlers = {}, streams: MockStreamHandlers = {}) {
  const generate = vi.fn((request: AvatarGenerateRequest) => ok({ job_id: request.request_id, model: options.model!, prompt: `rendered ${request.kind.type === "edit" ? request.kind.edit_request : request.kind.avatar_request}` }));
  const cancel = vi.fn(() => ok(null));
  const api = createApiClient(createMockTransport({ calls: { avatarGenerationOptions: () => ok(options), avatarGenerate: generate, jobCancel: cancel, ...calls }, streams: { jobWatch: ([{ job_id }]) => ok(job(job_id)), ...streams } }));
  return { api, generate, cancel };
}

describe("shared image input", () => {
  it("treats picker dismissal as a no-op", async () => {
    const ingest = vi.fn();
    const { api } = setup({ filesPickOpen: () => ok({ sources: [] }), assetsIngest: ingest });
    expect(await pickImage(api, "avatar")).toBeNull();
    expect(ingest).not.toHaveBeenCalled();
  });

  it("keeps each successfully attached image when a later upload fails", async () => {
    const attached = vi.fn();
    const ingest = vi.fn(({ source }: { source: { uri: string }; role: string }) => source.uri === "first.png" ? ok(asset("first")) : { status: "error" as const, error: { code: "invalid_input" as const, message: "bad image", details: null } });
    const { api } = setup({ filesPickOpen: () => ok({ sources: [{ uri: "first.png" }, { uri: "bad.png" }] }), assetsIngest: ingest });
    await expect(pickImages(api, { role: "attachment", multiple: true, onImage: attached })).rejects.toMatchObject({ code: "invalid_input" });
    expect(attached).toHaveBeenCalledExactlyOnceWith({ source: { uri: "first.png" }, asset: asset("first") });
    expect(ingest.mock.calls.map(([request]) => request.role)).toEqual(["attachment", "attachment"]);
  });

  it.each(["avatar", "background", "reference_image", "attachment"] as const)("ingests a single selected image with its %s role", async (role) => {
    const ingest = vi.fn(() => ok(asset(role)));
    const { api } = setup({ filesPickOpen: () => ok({ sources: [{ uri: "first.png" }, { uri: "ignored.png" }] }), assetsIngest: ingest });
    expect(await pickImage(api, role)).toEqual(asset(role));
    expect(ingest).toHaveBeenCalledExactlyOnceWith({ source: { uri: "first.png" }, role });
  });
});

describe("avatar session", () => {
  it("generates, branches edits from the selected variant, and starts new images without a source", async () => {
    const { api, generate } = setup();
    const session = new AvatarSession(api, { ...subject, currentAvatar: asset("original") });
    await session.loadOptions();
    await session.generate("harbor");
    const generated = session.getSnapshot().variants[1]!;
    await session.edit("add a scarf");
    expect(generate.mock.calls[1]![0]).toMatchObject({ source_asset_id: generated.asset.asset_id, kind: { type: "edit", current_avatar_prompt: generated.prompt, edit_request: "add a scarf" } });
    session.select(generated.id);
    await session.edit("change the sky");
    expect(session.getSnapshot().variants.at(-1)?.parentId).toBe(generated.id);
    await session.generate("forest");
    expect(generate.mock.calls[3]![0]).toMatchObject({ source_asset_id: null, kind: { type: "generation" } });
    expect(session.getSnapshot().variants).toHaveLength(5);
    session.select(session.getSnapshot().variants[0]!.id);
    expect(session.selectedAsset()?.asset_id).toBe("original");
    session.dispose();
  });

  it("allows uploaded and existing avatars to be edited without an invented previous prompt", async () => {
    const { api, generate } = setup({ filesPickOpen: () => ok({ sources: [{ uri: "avatar.png" }] }), assetsIngest: () => ok(asset("upload")) });
    const session = new AvatarSession(api, subject);
    await session.loadOptions();
    await session.upload();
    await session.edit("blue jacket");
    expect(generate.mock.calls[0]![0]).toMatchObject({ source_asset_id: "upload", kind: { current_avatar_prompt: "", edit_request: "blue jacket" } });
    expect(session.getSnapshot().variants[0]?.origin).toBe("upload");
  });

  it.each([{ enabled: false, model: null }, { enabled: true, model: null }, { enabled: true, model: { ...options.model!, supports_editing: false } }])("blocks generation when model options are unusable: %j", async (unusable) => {
    const { api, generate } = setup({ avatarGenerationOptions: () => ok(unusable) });
    const session = new AvatarSession(api, subject);
    await session.loadOptions();
    await session.generate("portrait");
    expect(generate).not.toHaveBeenCalled();
    expect(session.getSnapshot().error).toBeInstanceOf(ApiFailure);
  });

  it("retries uncertain admission with the same request id and prevents overlapping operations", async () => {
    const requests: AvatarGenerateRequest[] = [];
    const { api } = setup({ avatarGenerate: (request) => {
      requests.push(request);
      if (requests.length === 1) throw new Error("lost reply");
      return ok({ job_id: "accepted", model: options.model!, prompt: "rendered" });
    } });
    const session = new AvatarSession(api, subject);
    await session.loadOptions();
    await session.generate("portrait");
    expect(session.getSnapshot().error?.code).toBe("transport");
    await Promise.all([session.retry(), session.generate("should not start")]);
    expect(requests).toHaveLength(2);
    expect(requests[1]).toEqual(requests[0]);
    expect(session.getSnapshot().variants).toHaveLength(1);
  });

  it("reattaches to an accepted job after a watch failure without admitting another generation", async () => {
    let watches = 0;
    const { api, generate } = setup({}, { jobWatch: ([{ job_id }]) => {
      if (++watches === 1) throw new Error("stream disconnected");
      return ok(job(job_id));
    } });
    const session = new AvatarSession(api, subject);
    await session.loadOptions();
    await session.generate("portrait");
    await session.retry();
    expect(generate).toHaveBeenCalledTimes(1);
    expect(watches).toBe(2);
    expect(session.selectedAsset()).not.toBeNull();
  });

  it("does not allow another generation or Use while admission is uncertain", async () => {
    const { api, generate } = setup({ avatarGenerate: () => { throw new Error("lost reply"); } });
    const session = new AvatarSession(api, { ...subject, currentAvatar: asset("original") });
    await session.loadOptions();
    await session.generate("portrait");
    expect(session.getSnapshot().pendingRequest).toBe(true);
    await session.generate("another portrait");
    expect(generate).not.toHaveBeenCalled();
    expect(session.selectedAsset()).toBeNull();
    session.dispose();
  });

  it("unblocks the session after a definite admission rejection", async () => {
    const { api } = setup({ avatarGenerate: () => ({ status: "error", error: { code: "invalid_input", message: "invalid model", details: null } }) });
    const session = new AvatarSession(api, { ...subject, currentAvatar: asset("original") });
    await session.loadOptions();
    await session.generate("portrait");
    expect(session.getSnapshot().pendingRequest).toBe(false);
    expect(session.selectedAsset()?.asset_id).toBe("original");
  });

  it.each(["internal", "unavailable"] as const)("recovers uncertain %s admission without changing the request id", async (code) => {
    const requests: AvatarGenerateRequest[] = [];
    const { api } = setup({ avatarGenerate: (request) => {
      requests.push(request);
      return requests.length === 1 ? { status: "error", error: { code, message: "admission interrupted", details: null } } : ok({ job_id: "recovered", model: options.model!, prompt: "rendered" });
    } });
    const session = new AvatarSession(api, subject);
    await session.loadOptions();
    await session.generate("portrait");
    expect(session.getSnapshot().pendingRequest).toBe(true);
    await session.retry();
    expect(requests[1]).toEqual(requests[0]);
    expect(session.selectedAsset()?.asset_id).toBe("recovered");
  });

  it("clears a failed cancellation notice once the job settles without applying its output", async () => {
    const completion = deferred<ReturnType<typeof ok<JobView>>>();
    let watching = false;
    const { api } = setup({ jobCancel: () => { throw new Error("cancel disconnected"); } }, { jobWatch: () => { watching = true; return completion.promise; } });
    const session = new AvatarSession(api, { ...subject, currentAvatar: asset("original") });
    await session.loadOptions();
    const pending = session.generate("portrait");
    await vi.waitFor(() => expect(watching).toBe(true));
    await session.cancel();
    expect(session.getSnapshot().errorSource).toBe("cancellation");
    completion.resolve(ok(job("late-success")));
    await pending;
    expect(session.getSnapshot().error).toBeNull();
    expect(session.selectedAsset()?.asset_id).toBe("original");
  });

  it("cancels a known job when closing after a stream disconnect", async () => {
    const { api, cancel } = setup({}, { jobWatch: () => { throw new Error("stream lost"); } });
    const session = new AvatarSession(api, subject);
    await session.loadOptions();
    await session.generate("portrait");
    const id = session.getSnapshot().phase;
    expect(id).toBe("idle");
    expect(session.getSnapshot().pendingRequest).toBe(true);
    session.dispose();
    expect(cancel).toHaveBeenCalledTimes(1);
  });

  it("keeps watching after cancellation fails, permits cancelling again, and ignores late success", async () => {
    const completion = deferred<ReturnType<typeof ok<JobView>>>();
    let cancellations = 0;
    let watching = false;
    const { api } = setup({ jobCancel: () => {
      if (++cancellations === 1) throw new Error("cancel disconnected");
      return ok(null);
    } }, { jobWatch: () => { watching = true; return completion.promise; } });
    const session = new AvatarSession(api, { ...subject, currentAvatar: asset("original") });
    await session.loadOptions();
    const pending = session.generate("portrait");
    await vi.waitFor(() => expect(watching).toBe(true));
    await session.cancel();
    expect(session.getSnapshot().errorSource).toBe("cancellation");
    await session.cancel();
    completion.resolve(ok(job("late-success")));
    await pending;
    expect(cancellations).toBe(2);
    expect(session.selectedAsset()?.asset_id).toBe("original");
    expect(session.getSnapshot().variants).toHaveLength(1);
  });

  it("retries a terminal failure as a new generation and retains earlier variants", async () => {
    let watches = 0;
    const { api, generate } = setup({}, { jobWatch: ([{ job_id }]) => ok(job(job_id, ++watches === 1 ? "failed" : "succeeded")) });
    const session = new AvatarSession(api, { ...subject, currentAvatar: asset("original") });
    await session.loadOptions();
    await session.generate("portrait");
    expect(session.getSnapshot().error?.code).toBe("unavailable");
    await session.retry();
    expect(generate.mock.calls[0]![0].request_id).not.toBe(generate.mock.calls[1]![0].request_id);
    expect(session.getSnapshot().variants).toHaveLength(2);
  });

  it("cancels a job whose admission completes after the session closes and ignores its result", async () => {
    const admitted = deferred<ReturnType<typeof ok<{ job_id: string; model: NonNullable<AvatarGenerationOptions["model"]>; prompt: string }>>>();
    const { api, cancel } = setup({ avatarGenerate: () => admitted.promise });
    const session = new AvatarSession(api, subject);
    await session.loadOptions();
    const pending = session.generate("portrait");
    session.dispose();
    admitted.resolve(ok({ job_id: "late", model: options.model!, prompt: "rendered" }));
    await pending;
    expect(cancel).toHaveBeenCalledWith({ job_id: "late" });
    expect(session.getSnapshot().variants).toHaveLength(0);
  });

  it("cancels during admission and retains the previous avatar even if the job already succeeded", async () => {
    const admitted = deferred<{ status: "ok"; data: { job_id: string; model: NonNullable<AvatarGenerationOptions["model"]>; prompt: string } }>();
    const { api, cancel } = setup({ avatarGenerate: () => admitted.promise });
    const session = new AvatarSession(api, { ...subject, currentAvatar: asset("original") });
    await session.loadOptions();
    const pending = session.generate("portrait");
    await session.cancel();
    expect(session.getSnapshot().phase).toBe("cancelling");
    admitted.resolve(ok({ job_id: "late", model: options.model!, prompt: "rendered" }));
    await pending;
    expect(cancel).toHaveBeenCalledWith({ job_id: "late" });
    expect(session.selectedAsset()?.asset_id).toBe("original");
  });
});
