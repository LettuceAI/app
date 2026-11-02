import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRootRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApiClient } from "@/api/client";
import { createMockTransport, type MockCallHandlers } from "@/api/mock-transport";
import type { AsrCorrectionView, AsrSuggestionView, JobView, TranscriptionView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { createTestI18n } from "@/shared/testing/i18n";
import { ChatComposer, type ComposerHandle } from "./ChatComposer";

const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const fail = () => ({ status: "error" as const, error: { code: "unavailable" as const, message: "private diagnostic", details: null } });
const suggestion: AsrSuggestionView = { wrong: "Jon", correct: "John", language: "en", scope: "conversation", confidence: 0.85, accepted_count: 1, rejected_count: 0, seen_count: 2 };
const correction: AsrCorrectionView = { ...suggestion, id: "correction", use_count: 0, last_seen_at: 1, user_approved: true, created_at: 1, updated_at: 1 };
const transcript: TranscriptionView = { request_id: "request", audio: null, model_id: "base", raw_text: "Meat Jon at noon.", text: "Meet Jon at noon.", detected_language: "en", segments: [], applied_corrections: [] };
const job = (transcription: TranscriptionView): JobView => ({ id: "job", kind: "speech_transcribe", subject: { kind: "speech_request", id: "request" }, subject_detail: null, state: "succeeded", progress: { current: 1, total: 1, unit: "items", label_code: null, bytes_per_second: null }, created_at: 1, updated_at: 2, failure: null, result: { type: "transcription", transcription } });

async function mount(options: { calls?: MockCallHandlers; transcription?: TranscriptionView; send?: () => Promise<boolean> } = {}) {
  const suggestions = vi.fn<NonNullable<MockCallHandlers["asrSuggestions"]>>(() => ok([suggestion]));
  const approve = vi.fn<NonNullable<MockCallHandlers["asrSuggestionApprove"]>>(() => ok(correction));
  const ignore = vi.fn<NonNullable<MockCallHandlers["asrSuggestionIgnore"]>>(() => ok({ ...suggestion, id: "ignored", ignored_count: 1, last_ignored_at: 2, created_at: 1, updated_at: 2 }));
  const api = createApiClient(createMockTransport({ calls: {
    dictationStart: () => ok({ capture_id: "capture" }),
    dictationStop: () => ok({ job_id: "job" }),
    dictationCancel: () => ok(null),
    jobCancel: () => ok(null),
    asrSuggestions: suggestions,
    asrSuggestionApprove: approve,
    asrSuggestionIgnore: ignore,
    ...options.calls,
  }, streams: { jobWatch: () => ok(job(options.transcription ?? transcript)) } }));
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const handle = { current: null as ComposerHandle | null };
  const send = vi.fn(options.send ?? (() => Promise.resolve(true)));
  const router = createRouter({ history: createMemoryHistory({ initialEntries: ["/"] }), routeTree: createRootRoute({ component: () => (
    <ChatComposer api={api} conversationId="chat" handle={handle} name="Mira" sending={false} disabled={false} canContinue={false} onSend={send} onStop={() => undefined} onContinue={() => undefined} onAttach={() => undefined} />
  ) }) });
  const view = render(<I18nextProvider i18n={await createTestI18n()}><QueryClientProvider client={client}><RouterProvider router={router} /></QueryClientProvider></I18nextProvider>);
  return { ...view, handle, client, suggestions, approve, ignore, send, box: await screen.findByRole("textbox") as HTMLTextAreaElement };
}

async function dictate(expected = transcript.text) {
  fireEvent.click(screen.getByRole("button", { name: "Record voice" }));
  fireEvent.click(await screen.findByRole("button", { name: "Stop and transcribe" }));
  await waitFor(() => expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe(expected));
  return screen.getByRole("textbox") as HTMLTextAreaElement;
}

function edit(box: HTMLTextAreaElement, value = "Meet John at noon.") {
  box.focus();
  fireEvent.change(box, { target: { value } });
}

afterEach(() => { cleanup(); vi.restoreAllMocks(); });

describe("dictation correction learning", () => {
  it("only checks edited dictated text and leaves focus in the composer", async () => {
    const { box: initial, suggestions } = await mount();
    edit(initial, "A manually typed message");
    expect(suggestions).not.toHaveBeenCalled();
    const box = await dictate();
    expect(suggestions).not.toHaveBeenCalled();
    edit(box);
    expect(await screen.findByText("Learn correction?")).toBeTruthy();
    expect(suggestions).toHaveBeenCalledOnce();
    expect(suggestions).toHaveBeenCalledWith({ before: transcript.raw_text, after: "Meet John at noon.", language: "en", scope: "conversation" });
    expect(screen.getByRole("textbox")).toBe(box);
    expect(document.activeElement).toBe(box);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("coalesces typing, clears a reverted edit and respects an empty backend result", async () => {
    const check = vi.fn<NonNullable<MockCallHandlers["asrSuggestions"]>>(() => ok([]));
    await mount({ calls: { asrSuggestions: check } });
    const box = await dictate();
    edit(box, "Meet Jo at noon.");
    edit(box, "Meet Joh at noon.");
    edit(box);
    await waitFor(() => expect(check).toHaveBeenCalledOnce());
    expect(check.mock.calls[0]?.[0].after).toBe("Meet John at noon.");
    expect(screen.queryByText("Learn correction?")).toBeNull();
    edit(box, transcript.text);
    expect(screen.queryByText("Learn correction?")).toBeNull();
  });

  it("saves an approved correction, refreshes the library and suppresses it for this dictation", async () => {
    const { approve, client, suggestions } = await mount();
    const invalidate = vi.spyOn(client, "invalidateQueries");
    const box = await dictate();
    edit(box);
    fireEvent.click(await screen.findByRole("button", { name: "Learn" }));
    await waitFor(() => expect(approve).toHaveBeenCalledOnce());
    expect(approve.mock.calls[0]?.[0]).toEqual({ client_operation_id: expect.any(String), suggestion });
    await waitFor(() => expect(screen.queryByText("Learn correction?")).toBeNull());
    expect(invalidate).toHaveBeenCalledWith({ queryKey: queryKeys.speech.all });
    expect(box.value).toBe("Meet John at noon.");
    edit(box, "Meet John at noon. Please.");
    await waitFor(() => expect(suggestions).toHaveBeenCalledTimes(2));
    expect(screen.queryByText("Learn correction?")).toBeNull();
  });

  it("persists ignored suggestions without approving them", async () => {
    const { ignore, approve } = await mount();
    const box = await dictate();
    edit(box);
    fireEvent.click(await screen.findByRole("button", { name: "Ignore" }));
    await waitFor(() => expect(ignore).toHaveBeenCalledWith({ client_operation_id: expect.any(String), suggestion }));
    await waitFor(() => expect(screen.queryByText("Learn correction?")).toBeNull());
    expect(approve).not.toHaveBeenCalled();
    expect(box.value).toBe("Meet John at noon.");
  });

  it("keeps the correction and reuses the operation id after a failed save", async () => {
    const approve = vi.fn<NonNullable<MockCallHandlers["asrSuggestionApprove"]>>().mockResolvedValueOnce(fail()).mockResolvedValueOnce(ok(correction));
    await mount({ calls: { asrSuggestionApprove: approve } });
    const box = await dictate();
    edit(box);
    fireEvent.click(await screen.findByRole("button", { name: "Learn" }));
    await screen.findByText("It isn't available right now.");
    expect(screen.queryByText("private diagnostic")).toBeNull();
    expect(box.value).toBe("Meet John at noon.");
    fireEvent.click(screen.getByRole("button", { name: "Learn" }));
    await waitFor(() => expect(approve).toHaveBeenCalledTimes(2));
    expect(approve.mock.calls[0]?.[0]).toEqual(approve.mock.calls[1]?.[0]);
    await waitFor(() => expect(screen.queryByText("Learn correction?")).toBeNull());
  });

  it("discards an out-of-order suggestion reply", async () => {
    let finishFirst!: (value: ReturnType<typeof ok<AsrSuggestionView[]>>) => void;
    const check = vi.fn<NonNullable<MockCallHandlers["asrSuggestions"]>>().mockImplementationOnce(() => new Promise((resolve) => { finishFirst = resolve; })).mockResolvedValueOnce(ok([{ ...suggestion, correct: "Joan" }]));
    await mount({ calls: { asrSuggestions: check } });
    const box = await dictate();
    edit(box);
    await waitFor(() => expect(check).toHaveBeenCalledOnce());
    edit(box, "Meet Joan at noon.");
    expect(await screen.findByText("Joan")).toBeTruthy();
    await act(async () => finishFirst(ok([suggestion])));
    expect(screen.queryByText("John")).toBeNull();
    expect(screen.getByText("Joan")).toBeTruthy();
  });

  it("does not learn from text replaced by another composer tool", async () => {
    const { handle, suggestions } = await mount();
    const box = await dictate();
    edit(box);
    await screen.findByText("Learn correction?");
    act(() => handle.current?.replace("A generated reply"));
    expect(screen.queryByText("Learn correction?")).toBeNull();
    edit(box, "An edited generated reply");
    fireEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(box.value).toBe(""));
    expect(suggestions).toHaveBeenCalledOnce();
  });

  it("keeps learning available when sending fails, then clears it after a successful send", async () => {
    const send = vi.fn<() => Promise<boolean>>().mockResolvedValue(true).mockResolvedValueOnce(false);
    const { suggestions } = await mount({ send });
    const box = await dictate();
    edit(box);
    await screen.findByText("Learn correction?");
    fireEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(box.value).toBe("Meet John at noon."));
    await screen.findByText("Learn correction?");
    fireEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(box.value).toBe(""));
    expect(screen.queryByText("Learn correction?")).toBeNull();
    const previous = suggestions.mock.calls.length;
    edit(box, "Another message");
    fireEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(box.value).toBe(""));
    expect(suggestions).toHaveBeenCalledTimes(previous);
  });

  it("can retry a failed suggestion lookup without changing the draft", async () => {
    const check = vi.fn<NonNullable<MockCallHandlers["asrSuggestions"]>>().mockResolvedValueOnce(fail()).mockResolvedValueOnce(ok([suggestion]));
    await mount({ calls: { asrSuggestions: check } });
    const box = await dictate();
    edit(box);
    await screen.findByText("It isn't available right now.");
    expect(screen.queryByText("private diagnostic")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    await screen.findByText("Learn correction?");
    expect(box.value).toBe("Meet John at noon.");
    expect(check).toHaveBeenCalledTimes(2);
  });

  it("blocks duplicate saves while a correction is being written", async () => {
    let finish!: (value: ReturnType<typeof ok<AsrCorrectionView>>) => void;
    const approve = vi.fn<NonNullable<MockCallHandlers["asrSuggestionApprove"]>>(() => new Promise((resolve) => { finish = resolve; }));
    await mount({ calls: { asrSuggestionApprove: approve } });
    const box = await dictate();
    edit(box);
    const button = await screen.findByRole("button", { name: "Learn" });
    fireEvent.click(button);
    fireEvent.click(button);
    expect(approve).toHaveBeenCalledOnce();
    expect(button.hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "Ignore" }).hasAttribute("disabled")).toBe(true);
    await act(async () => finish(ok(correction)));
    await waitFor(() => expect(screen.queryByText("Learn correction?")).toBeNull());
  });

  it("discards the previous dictation's pending reply when recording again", async () => {
    let finish!: (value: ReturnType<typeof ok<AsrSuggestionView[]>>) => void;
    const check = vi.fn<NonNullable<MockCallHandlers["asrSuggestions"]>>().mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; })).mockResolvedValueOnce(ok([suggestion]));
    await mount({ calls: { asrSuggestions: check } });
    const first = await dictate();
    edit(first);
    await waitFor(() => expect(check).toHaveBeenCalledOnce());
    const second = await dictate();
    await act(async () => finish(ok([suggestion])));
    expect(screen.queryByText("Learn correction?")).toBeNull();
    edit(second);
    await screen.findByText("Learn correction?");
    expect(check).toHaveBeenCalledTimes(2);
  });

  it("drops pending suggestions when leaving the chat", async () => {
    let finish!: (value: ReturnType<typeof ok<AsrSuggestionView[]>>) => void;
    const check = vi.fn<NonNullable<MockCallHandlers["asrSuggestions"]>>(() => new Promise((resolve) => { finish = resolve; }));
    const { unmount, approve, ignore } = await mount({ calls: { asrSuggestions: check } });
    const box = await dictate();
    edit(box);
    await waitFor(() => expect(check).toHaveBeenCalledOnce());
    unmount();
    await act(async () => finish(ok([suggestion])));
    expect(screen.queryByText("Learn correction?")).toBeNull();
    expect(approve).not.toHaveBeenCalled();
    expect(ignore).not.toHaveBeenCalled();
  });
});
