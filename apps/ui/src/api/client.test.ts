import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiFailure, createApiClient, isApiError, isInsideShell, selectTransport } from "./client";
import type { GenerationEvent } from "./generated/bindings";
import { createMockTransport, mockAppStatus } from "./mock-transport";
import type { Transport } from "./transport";

describe("createApiClient", () => {
  it("returns the data of an ok outcome", async () => {
    const api = createApiClient(createMockTransport());
    await expect(api.call("appStatus")).resolves.toEqual(mockAppStatus);
  });

  it("rejects an error outcome with a typed ApiFailure carrying code and details", async () => {
    const api = createApiClient(
      createMockTransport({
        calls: {
          conversationOpen: () => ({
            status: "error",
            error: { code: "model_required", message: "embedding model missing", details: { type: "model", model: "embedding" } },
          }),
        },
      }),
    );
    const failure = await api.call("conversationOpen", { conversation_id: "c1" }).catch((error: unknown) => error);
    expect(failure).toBeInstanceOf(ApiFailure);
    expect(failure).toMatchObject({
      code: "model_required",
      message: "embedding model missing",
      details: { type: "model", model: "embedding" },
    });
  });

  it("turns an error outcome that is not an ApiError into a transport failure", async () => {
    const transport: Transport = {
      kind: "mock",
      call: async () => ({ status: "error", error: "unknown command app_status" }),
      stream: async () => ({ status: "error", error: { code: 7 } }),
      subscribe: async () => () => {},
    };
    const api = createApiClient(transport);
    await expect(api.call("appStatus")).rejects.toMatchObject({
      code: "transport",
      message: "unknown command app_status",
      details: null,
    });
    await expect(api.stream("jobWatch", [{ job_id: "j1" }], () => {})).rejects.toMatchObject({ code: "transport" });
  });

  it("turns a transport that throws into a transport ApiFailure keeping the cause", async () => {
    const broken = new Error("ipc closed");
    const transport: Transport = {
      kind: "mock",
      call: () => Promise.reject(broken),
      stream: () => Promise.reject(broken),
      subscribe: () => Promise.reject(broken),
    };
    const api = createApiClient(transport);
    for (const pending of [
      api.call("appStatus"),
      api.stream("jobWatch", [{ job_id: "j1" }], () => {}),
      api.subscribe(() => {}),
    ]) {
      const failure = await pending.catch((error: unknown) => error);
      expect(failure).toBeInstanceOf(ApiFailure);
      expect(failure).toMatchObject({ code: "transport", message: "ipc closed", details: null, cause: broken });
    }
  });

  it("turns a non-Error rejection into a transport ApiFailure", async () => {
    const transport: Transport = {
      kind: "mock",
      call: () => Promise.reject("gone"),
      stream: () => Promise.reject("gone"),
      subscribe: () => Promise.reject("gone"),
    };
    const failure = await createApiClient(transport).call("appStatus").catch((error: unknown) => error);
    expect(failure).toMatchObject({ code: "transport", message: "gone" });
  });

  it("reports a command the mock does not handle as unsupported", async () => {
    const failure = await createApiClient(createMockTransport())
      .call("charactersList", { cursor: null, limit: null })
      .catch((error: unknown) => error);
    expect(failure).toMatchObject({ code: "unsupported" });
  });

  it("delivers stream events in order, also after the call settles, until the signal aborts", async () => {
    let emitLater: ((event: GenerationEvent) => void) | undefined;
    const api = createApiClient(
      createMockTransport({
        streams: {
          conversationSend: ([request], emit) => {
            emit({ type: "started", turn_id: "t1" });
            emitLater = emit;
            return { status: "ok", data: { user_message_id: "m1", turn_id: request.conversation_id } };
          },
        },
      }),
    );
    const events: string[] = [];
    const controller = new AbortController();
    const accepted = await api.stream(
      "conversationSend",
      [{ conversation_id: "c1", text: "hi", client_operation_id: "op1" }],
      (event) => events.push(event.type),
      { signal: controller.signal },
    );
    expect(accepted).toEqual({ user_message_id: "m1", turn_id: "c1" });
    emitLater?.({ type: "delta", turn_id: "t1", text: "hi", reasoning: null });
    emitLater?.({ type: "completed", turn_id: "t1", message_id: "m2" });
    controller.abort();
    emitLater?.({ type: "cancelled", turn_id: "t1" });
    expect(events).toEqual(["started", "delta", "completed"]);
  });

  it("cancels a stream whose signal is already aborted without running the command", async () => {
    let ran = false;
    const api = createApiClient(
      createMockTransport({
        streams: {
          jobWatch: () => {
            ran = true;
            return { status: "error", error: { code: "internal", message: "unreachable", details: null } };
          },
        },
      }),
    );
    const controller = new AbortController();
    controller.abort();
    const failure = await api
      .stream("jobWatch", [{ job_id: "j1" }], () => {}, { signal: controller.signal })
      .catch((error: unknown) => error);
    expect(failure).toBeInstanceOf(ApiFailure);
    expect(failure).toMatchObject({ code: "cancelled" });
    expect(ran).toBe(false);
  });

  it("detaches a stream whose call fails", async () => {
    let emitLater: ((event: GenerationEvent) => void) | undefined;
    const api = createApiClient(
      createMockTransport({
        streams: {
          conversationSend: (_args, emit) => {
            emitLater = emit;
            return { status: "error", error: { code: "busy", message: "turn running", details: null } };
          },
        },
      }),
    );
    const events: string[] = [];
    await expect(
      api.stream("conversationSend", [{ conversation_id: "c1", text: "hi", client_operation_id: "op1" }], (event) =>
        events.push(event.type),
      ),
    ).rejects.toMatchObject({ code: "busy" });
    emitLater?.({ type: "started", turn_id: "t1" });
    expect(events).toEqual([]);
  });

  it("forwards app events to subscribers until they unsubscribe", async () => {
    const transport = createMockTransport();
    const api = createApiClient(transport);
    const seen: string[] = [];
    const unsubscribe = await api.subscribe((event) => seen.push(event.type));
    transport.emitAppEvent({ type: "generation_settled", conversation_id: "c1", turn_id: "t1" });
    unsubscribe();
    transport.emitAppEvent({ type: "generation_settled", conversation_id: "c1", turn_id: "t2" });
    expect(seen).toEqual(["generation_settled"]);
  });
});

describe("isApiError", () => {
  it("accepts only objects with a known code, a message and nullable details", () => {
    expect(isApiError({ code: "busy", message: "m", details: null })).toBe(true);
    expect(isApiError({ code: "model_required", message: "m", details: { type: "model", model: "emotion" } })).toBe(true);
    expect(isApiError("busy")).toBe(false);
    expect(isApiError(null)).toBe(false);
    expect(isApiError({ code: "toString", message: "m", details: null })).toBe(false);
    expect(isApiError({ code: "busy", details: null })).toBe(false);
    expect(isApiError({ code: "busy", message: "m", details: "x" })).toBe(false);
  });
});

describe("selectTransport", () => {
  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it("uses Tauri when the shell is present", async () => {
    await expect(selectTransport(true)).resolves.toMatchObject({ kind: "tauri" });
  });

  it("uses the mock in a plain browser during development", async () => {
    await expect(selectTransport(false)).resolves.toMatchObject({ kind: "mock" });
  });

  it("fails typed in a production build outside the shell", async () => {
    vi.stubEnv("DEV", false);
    const failure = await selectTransport(false).catch((error: unknown) => error);
    expect(failure).toBeInstanceOf(ApiFailure);
    expect(failure).toMatchObject({ code: "transport" });
  });

  it("detects the shell from window.__TAURI_INTERNALS__", () => {
    expect(isInsideShell()).toBe(false);
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    try {
      expect(isInsideShell()).toBe(true);
    } finally {
      Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
    }
  });
});
