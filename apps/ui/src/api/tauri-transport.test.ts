import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiFailure, createApiClient } from "./client";
import type { AppEvent, JobEvent, JobView } from "./generated/bindings";
import { createTauriTransport } from "./tauri-transport";

afterEach(() => {
  clearMocks();
});

function rejectWith(value: unknown) {
  mockIPC(() => {
    throw value;
  });
  return createApiClient(createTauriTransport()).call("appStatus").catch((error: unknown) => error);
}

const job: JobView = {
  id: "j1",
  kind: "maintenance",
  subject: { kind: "maintenance", id: "m1" },
  state: "running",
  progress: { current: 0, total: null, unit: null, label_code: null, bytes_per_second: null },
  created_at: 0,
  updated_at: 0,
  failure: null,
  result: null,
};

type ChannelHandle = { id: number };

function deliver(channel: ChannelHandle, index: number, message: JobEvent) {
  const internals = (window as unknown as { __TAURI_INTERNALS__: { runCallback: (id: number, data: unknown) => void } })
    .__TAURI_INTERNALS__;
  internals.runCallback(channel.id, { index, message });
}

describe("tauri transport rejections", () => {
  it("keeps a backend ApiError's code and details", async () => {
    const failure = await rejectWith({ code: "busy", message: "db locked", details: null });
    expect(failure).toBeInstanceOf(ApiFailure);
    expect(failure).toMatchObject({ code: "busy", message: "db locked", details: null });
  });

  it("turns an IPC string rejection into a transport failure", async () => {
    const failure = await rejectWith("command app_status not allowed by ACL");
    expect(failure).toBeInstanceOf(ApiFailure);
    expect(failure).toMatchObject({
      code: "transport",
      message: "command app_status not allowed by ACL",
      details: null,
      cause: "command app_status not allowed by ACL",
    });
  });

  it("turns an object without a known code into a transport failure", async () => {
    const failure = await rejectWith({ code: "made_up", message: "x", details: null });
    expect(failure).toMatchObject({ code: "transport" });
  });

  it("turns a thrown Error into a transport failure", async () => {
    const broken = new Error("ipc closed");
    const failure = await rejectWith(broken);
    expect(failure).toMatchObject({ code: "transport", message: "ipc closed", cause: broken });
  });
});

describe("tauri transport streams", () => {
  it("delivers events after the call settles until the signal aborts", async () => {
    let channel: ChannelHandle | undefined;
    mockIPC((command, payload) => {
      expect(command).toBe("job_watch");
      expect(payload).toMatchObject({ request: { job_id: "j1" } });
      channel = (payload as { onEvent: ChannelHandle }).onEvent;
      deliver(channel, 0, { type: "progress", job });
      return job;
    });
    const seen: string[] = [];
    const controller = new AbortController();
    const api = createApiClient(createTauriTransport());
    const view = await api.stream("jobWatch", [{ job_id: "j1" }], (event) => seen.push(event.type), {
      signal: controller.signal,
    });
    expect(view).toEqual(job);
    if (!channel) throw new Error("the command never received its channel");
    deliver(channel, 1, { type: "progress", job });
    controller.abort();
    deliver(channel, 2, { type: "completed", job: { ...job, state: "succeeded" } });
    expect(seen).toEqual(["progress", "progress"]);
  });

  it("cancels without sending the command when the signal is already aborted", async () => {
    const sent: string[] = [];
    mockIPC((command) => {
      sent.push(command);
      return job;
    });
    const controller = new AbortController();
    controller.abort();
    const failure = await createApiClient(createTauriTransport())
      .stream("jobWatch", [{ job_id: "j1" }], () => {}, { signal: controller.signal })
      .catch((error: unknown) => error);
    expect(failure).toBeInstanceOf(ApiFailure);
    expect(failure).toMatchObject({ code: "cancelled" });
    expect(sent).toEqual([]);
  });

  it("detaches the handler and removes its abort listener when the call fails", async () => {
    let channel: ChannelHandle | undefined;
    mockIPC((_command, payload) => {
      channel = (payload as { onEvent: ChannelHandle }).onEvent;
      throw { code: "not_found", message: "no such job", details: null };
    });
    const controller = new AbortController();
    const removeListener = vi.spyOn(controller.signal, "removeEventListener");
    const seen: string[] = [];
    await expect(
      createApiClient(createTauriTransport()).stream("jobWatch", [{ job_id: "j1" }], (event) => seen.push(event.type), {
        signal: controller.signal,
      }),
    ).rejects.toMatchObject({ code: "not_found" });
    if (!channel) throw new Error("the command never received its channel");
    deliver(channel, 0, { type: "progress", job });
    expect(seen).toEqual([]);
    expect(removeListener).toHaveBeenCalledWith("abort", expect.any(Function));
  });

  it("keeps the abort listener after a successful call so late events can still be detached", async () => {
    mockIPC(() => job);
    const controller = new AbortController();
    const removeListener = vi.spyOn(controller.signal, "removeEventListener");
    await createApiClient(createTauriTransport()).stream("jobWatch", [{ job_id: "j1" }], () => {}, {
      signal: controller.signal,
    });
    expect(removeListener).not.toHaveBeenCalled();
  });

  it("forwards app events to subscribers", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    const seen: AppEvent[] = [];
    const unsubscribe = await createApiClient(createTauriTransport()).subscribe((event) => seen.push(event));
    const event: AppEvent = { type: "job_updated", job };
    await emit("app-event", event);
    unsubscribe();
    expect(seen).toEqual([event]);
  });
});
