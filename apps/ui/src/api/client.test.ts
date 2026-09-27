import { describe, expect, it } from "vitest";
import { ApiFailure, createApiClient, detectEnvironment, selectTransport } from "./client";
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
      api.stream("jobWatch", { job_id: "j1" }, () => {}),
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

  it("delivers stream events before the command settles", async () => {
    const api = createApiClient(
      createMockTransport({
        streams: {
          conversationSend: (request, emit) => {
            emit({ type: "started", turn_id: "t1" });
            emit({ type: "delta", turn_id: "t1", text: request.text, reasoning: null });
            return { status: "ok", data: { user_message_id: "m1", turn_id: "t1" } };
          },
        },
      }),
    );
    const events: string[] = [];
    const accepted = await api.stream(
      "conversationSend",
      { conversation_id: "c1", text: "hi", client_operation_id: "op1" },
      (event) => events.push(event.type),
    );
    expect(accepted).toEqual({ user_message_id: "m1", turn_id: "t1" });
    expect(events).toEqual(["started", "delta"]);
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

describe("selectTransport", () => {
  it("uses Tauri when the shell is present, even in development", async () => {
    await expect(selectTransport({ hasTauri: true, allowMock: true })).resolves.toMatchObject({ kind: "tauri" });
  });

  it("uses the mock in a plain browser during development", async () => {
    await expect(selectTransport({ hasTauri: false, allowMock: true })).resolves.toMatchObject({ kind: "mock" });
  });

  it("fails typed in a production build outside the shell", async () => {
    const failure = await selectTransport({ hasTauri: false, allowMock: false }).catch((error: unknown) => error);
    expect(failure).toBeInstanceOf(ApiFailure);
    expect(failure).toMatchObject({ code: "transport" });
  });

  it("detects the shell from window.__TAURI_INTERNALS__", () => {
    expect(detectEnvironment().hasTauri).toBe(false);
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
    try {
      expect(detectEnvironment().hasTauri).toBe(true);
    } finally {
      Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
    }
  });
});
