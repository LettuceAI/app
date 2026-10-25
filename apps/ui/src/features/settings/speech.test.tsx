import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { toast } from "@lettuceai/crisp";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AsrCorrectionView, AsrVocabularyView, AsrVoiceExampleView, JobView, WhisperInstalledModel, WhisperInstalledModels } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers, type MockStreamHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));
afterEach(() => { toast.dismiss(); cleanup(); vi.restoreAllMocks(); });
const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const fail = () => ({ status: "error" as const, error: { code: "unavailable" as const, message: "private diagnostic", details: null } });
const audio = { asset_id: "audio1", url: "asset://audio1" };
const models: WhisperInstalledModel[] = [{ id: "base", filename: "ggml-base.bin", size_bytes: 150000000, english_only: false, quantized: false, dictation: true, loaded: false }];
const word: AsrVocabularyView = { id: "word1", term: "LettuceAI", category: "Names", language: "en", scope: "project", priority: 250, use_count: 12, created_at: 1, updated_at: 1 };
const correction: AsrCorrectionView = { id: "correction1", wrong: "let us", correct: "LettuceAI", language: "en", scope: "character", confidence: 0.85, use_count: 9, accepted_count: 4, rejected_count: 1, seen_count: 7, last_seen_at: 9, user_approved: true, created_at: 1, updated_at: 1 };
const example: AsrVoiceExampleView = { id: "example1", audio, expected_text: "Hello LettuceAI", whisper_output: "Hello let us", language: "en", scope: "conversation", vocabulary_term_id: "word1", correction_id: "correction1", created_at: 1, updated_at: 1 };
const job = (patch: Partial<JobView> = {}): JobView => ({ id: "job1", kind: "speech_transcribe", subject: { kind: "speech_request", id: "request1" }, subject_detail: null, state: "succeeded", progress: { current: 0, total: null, unit: "items", label_code: null, bytes_per_second: null }, created_at: 1, updated_at: 2, failure: null, result: { type: "transcription", transcription: { request_id: "request1", audio: null, model_id: "base", raw_text: "Hello let us", text: "Hello LettuceAI", detected_language: "en", segments: [{ start_ms: 0, end_ms: 2000, text: "Hello let us" }], applied_corrections: [{ correction_id: "correction1", wrong: "let us", correct: "LettuceAI", matched_text: "let us" }] } }, ...patch });
async function mount(options: { calls?: MockCallHandlers; streams?: MockStreamHandlers; tab?: string; phone?: boolean; gpuSupported?: boolean; mic?: string; path?: string } = {}) {
  let ui_state = { onboarding: { completed: true }, ...(options.mic ? { speechMicrophoneId: options.mic } : {}) };
  const transport = createMockTransport({ calls: {
    appStatus: () => ok({ ...mockAppStatus, platform: options.phone === false ? "other" : "android", ui_state }),
    appUiStateUpdate: (request) => { ui_state = { ...ui_state, ...request.patch }; return ok({ state: ui_state }); },
    whisperModelsList: () => ok({ models, gpu_supported: options.gpuSupported ?? false }),
    whisperCatalog: () => ok({ models: ["base", "small"].map((id) => ({ ...models[0]!, id, filename: `ggml-${id}.bin`, recommended: true, recommended_for_mobile: true, recommended_for_desktop: true, installed: id === "base" })) }),
    asrVocabularyList: () => ok([word]), asrCorrectionsList: () => ok([correction]), asrIgnoredSuggestionsList: () => ok([]), asrVoiceExamplesList: () => ok([example]), asrPromptPreview: () => ok("LettuceAI."),
    microphoneDevices: () => ok({ supported: true, devices: [{ id: "USB mic", label: "USB mic", selectable: true }] }),
    audioCaptureStart: () => ok({ capture_id: "capture1" }), audioCaptureStop: () => ok(audio), dictationCancel: () => ok(null), transcribeFile: () => ok({ job_id: "job1" }), jobCancel: () => ok(null),
    jobsList: () => ok({ items: [], next_cursor: null }),
    ...options.calls,
  }, streams: { jobWatch: () => ok(job()), ...options.streams } });
  const history = createMemoryHistory({ initialEntries: ["/settings", options.path ?? `/settings/speech${options.tab ? `?speechTab=${options.tab}` : ""}`] });
  const runtime = await createAppRuntime({ transport, history, i18n: await createTestI18n("en") });
  await act(async () => { render(<App runtime={runtime} />); });
  await screen.findByRole("tab", { name: "Model" });
  return { runtime, history, transport };
}
const drawer = () => within(screen.getAllByRole("dialog").at(-1)!);
const click = async (name: string | RegExp) => fireEvent.click(await screen.findByRole("button", { name }));

describe("speech recognition settings", () => {
  it("locks CPU on without a GPU backend and forces CPU for preload and test transcription", async () => {
    const preload = vi.fn<NonNullable<MockCallHandlers["whisperPreload"]>>(() => ok(null));
    const transcribe = vi.fn<NonNullable<MockCallHandlers["transcribeFile"]>>(() => ok({ job_id: "job1" }));
    await mount({ phone: false, calls: { whisperPreload: preload, transcribeFile: transcribe } });
    const cpu = await screen.findByRole("switch", { name: "Use CPU" });
    expect(cpu.getAttribute("aria-checked")).toBe("true");
    expect(cpu.hasAttribute("disabled")).toBe(true);
    expect(screen.queryByRole("switch", { name: "Use GPU" })).toBeNull();
    await click("Load");
    await waitFor(() => expect(preload).toHaveBeenCalledWith({ model_id: "base", run: expect.objectContaining({ use_gpu: false, force_cpu: true }) }));
    await click("Record");
    await click("Stop");
    expect(await screen.findByText("Hello LettuceAI")).toBeTruthy();
    expect(transcribe.mock.calls[0]?.[0].options?.run).toMatchObject({ use_gpu: false, force_cpu: true });
  });

  it("uses Whisper GPU support even when the general build label is normal", async () => {
    const preload = vi.fn<NonNullable<MockCallHandlers["whisperPreload"]>>(() => ok(null));
    await mount({ gpuSupported: true, calls: { whisperPreload: preload } });
    expect((await screen.findByRole("switch", { name: "Use GPU" })).getAttribute("aria-checked")).toBe("true");
    expect(screen.queryByRole("switch", { name: "Use CPU" })).toBeNull();
    await click("Load");
    await waitFor(() => expect(preload).toHaveBeenCalledWith({ model_id: "base", run: expect.objectContaining({ use_gpu: true, force_cpu: false }) }));
  });

  it.each([false, true])("uses the declared GPU capability (%s) when transcribing a saved example", async (gpuSupported) => {
    const transcribe = vi.fn<NonNullable<MockCallHandlers["transcribeFile"]>>(() => ok({ job_id: "job1" }));
    await mount({ tab: "examples", gpuSupported, calls: { transcribeFile: transcribe } });
    await click("Edit");
    await screen.findByRole("dialog");
    fireEvent.click(drawer().getByRole("button", { name: "Transcribe" }));
    await waitFor(() => expect(transcribe).toHaveBeenCalled());
    expect(transcribe.mock.calls[0]?.[0].options?.run).toMatchObject({ use_gpu: gpuSupported, force_cpu: !gpuSupported });
  });

  it("loads legacy scopes and priority and uses the backend prompt preview", async () => {
    const list = vi.fn<NonNullable<MockCallHandlers["asrVocabularyList"]>>(() => ok([word]));
    const preview = vi.fn(() => ok("Backend-selected prompt."));
    await mount({ tab: "vocabulary", phone: false, calls: { asrVocabularyList: list, asrPromptPreview: preview } });
    await click(/^LettuceAI/);
    await screen.findByRole("dialog");
    expect(drawer().getByRole("textbox", { name: "Priority" }).getAttribute("value")).toBe("250");
    expect(drawer().getByText("Project")).toBeTruthy();
    await click("Cancel");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await click(/^What Whisper reads first/);
    expect(await screen.findByText("Backend-selected prompt.")).toBeTruthy();
    expect(list.mock.calls[0]?.[0]).toMatchObject({ scopes: ["global", "project", "character", "conversation"] });
  });

  it("loads an explicitly entered legacy scope rather than hiding its records", async () => {
    const list = vi.fn<NonNullable<MockCallHandlers["asrVocabularyList"]>>((request) => ok(request.scopes?.includes("team") ? [{ ...word, scope: "team" }] : []));
    await mount({ tab: "vocabulary", calls: { asrVocabularyList: list } });
    fireEvent.click(await screen.findByRole("combobox", { name: "Where" }));
    fireEvent.click(await screen.findByRole("option", { name: "Other scope" }));
    const field = screen.getByRole("textbox", { name: "Scope name" });
    fireEvent.change(field, { target: { value: "team" } });
    fireEvent.blur(field);
    expect(await screen.findByRole("button", { name: /^LettuceAI/ })).toBeTruthy();
    expect(list).toHaveBeenLastCalledWith(expect.objectContaining({ scopes: ["team"] }));
  });

  it("keeps failed word drafts and reuses their operation id when retrying", async () => {
    let failing = true;
    const save = vi.fn<NonNullable<MockCallHandlers["asrVocabularySave"]>>((request) => failing ? fail() : ok({ ...word, ...request, id: word.id, category: request.category, scope: request.scope ?? "global", priority: request.priority ?? 0, use_count: request.use_count ?? 0 }));
    await mount({ tab: "vocabulary", calls: { asrVocabularySave: save } });
    await click(/^LettuceAI/);
    await screen.findByRole("dialog");
    fireEvent.change(drawer().getByRole("textbox", { name: /Word or phrase/ }), { target: { value: "Lettuce AI" } });
    await click("Save");
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(await drawer().findByText(/available right now/)).toBeTruthy();
    expect((drawer().getByRole("textbox", { name: /Word or phrase/ }) as HTMLInputElement).value).toBe("Lettuce AI");
    expect(screen.queryByText("private diagnostic")).toBeNull();
    failing = false;
    await click("Save");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(save.mock.calls[1]?.[0]).toEqual(save.mock.calls[0]?.[0]);
    expect(save.mock.calls[0]?.[0]).toMatchObject({ id: "word1", use_count: 12, priority: 250, scope: "project" });
  });

  it("closes the editor on Back without unmounting the selected tab", async () => {
    const { history } = await mount({ tab: "vocabulary" });
    await click("Add word");
    expect(await screen.findByRole("dialog")).toBeTruthy();
    await act(async () => { history.back(); });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(screen.getByRole("tab", { name: "Words", selected: true })).toBeTruthy();
  });

  it("records natively, transcribes retained audio with runtime options and shows actual results", async () => {
    const start = vi.fn<NonNullable<MockCallHandlers["audioCaptureStart"]>>(() => ok({ capture_id: "capture1" }));
    const stop = vi.fn<NonNullable<MockCallHandlers["audioCaptureStop"]>>(() => ok(audio));
    const transcribe = vi.fn<NonNullable<MockCallHandlers["transcribeFile"]>>(() => ok({ job_id: "job1" }));
    await mount({ mic: "USB mic", gpuSupported: true, calls: { audioCaptureStart: start, audioCaptureStop: stop, transcribeFile: transcribe } });
    fireEvent.click(await screen.findByRole("switch", { name: "Use GPU" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Extra words" }), { target: { value: "LettuceAI" } });
    await click("Record");
    await click("Stop");
    expect(await screen.findByText("Hello LettuceAI")).toBeTruthy();
    expect(screen.getByText("Hello let us")).toBeTruthy();
    expect(start.mock.calls[0]?.[0]).toEqual({ device_id: "USB mic" });
    expect(stop.mock.calls[0]?.[0]).toEqual({ capture_id: "capture1" });
    expect(transcribe.mock.calls[0]?.[0]).toMatchObject({ retain_audio: true, source: { uri: audio.url }, options: { run: { use_gpu: false, force_cpu: true }, scopes: ["conversation", "global"], initial_prompt: "LettuceAI" } });
    expect(document.querySelector("audio")?.getAttribute("src")).toBe(audio.url);
    await click("Save as example");
    await screen.findByRole("dialog");
    expect((drawer().getByRole("textbox", { name: "Whisper heard" }) as HTMLTextAreaElement).value).toBe("Hello let us");
    expect(drawer().getByRole("button", { name: "Remove recording" })).toBeTruthy();
  });

  it("rejoins the same transcription after its watch fails instead of admitting duplicate work", async () => {
    let failed = false;
    const transcribe = vi.fn<NonNullable<MockCallHandlers["transcribeFile"]>>(() => ok({ job_id: "job1" }));
    await mount({ calls: { transcribeFile: transcribe }, streams: { jobWatch: () => { if (!failed) { failed = true; return fail(); } return ok(job()); } } });
    await click("Record");
    await click("Stop");
    await click("Transcribe recording");
    expect(await screen.findByText("Hello LettuceAI")).toBeTruthy();
    expect(transcribe).toHaveBeenCalledTimes(2);
    expect(transcribe.mock.calls[0]?.[0].request_id).toBe(transcribe.mock.calls[1]?.[0].request_id);
  });

  it("cancels a capture that starts after its page has closed", async () => {
    let resolve!: (value: ReturnType<typeof ok<{ capture_id: string }>>) => void;
    const cancel = vi.fn(() => ok(null));
    const { runtime } = await mount({ calls: { audioCaptureStart: () => new Promise((done) => { resolve = done; }), dictationCancel: cancel } });
    await click("Record");
    await act(async () => { await runtime.router.navigate({ to: "/settings" }); });
    await act(async () => { resolve(ok({ capture_id: "late" })); });
    await waitFor(() => expect(cancel).toHaveBeenCalledWith({ capture_id: "late" }));
  });

  it("preserves correction evidence when editing", async () => {
    const save = vi.fn<NonNullable<MockCallHandlers["asrCorrectionSave"]>>((request) => ok({ ...correction, wrong: request.wrong, correct: request.correct }));
    await mount({ tab: "corrections", calls: { asrCorrectionSave: save } });
    await click(/^let us/);
    await screen.findByRole("dialog");
    fireEvent.change(drawer().getByRole("textbox", { name: /Replace with/ }), { target: { value: "Lettuce AI" } });
    await click("Save");
    await waitFor(() => expect(save).toHaveBeenCalled());
    expect(save.mock.calls[0]?.[0]).toMatchObject({ id: correction.id, confidence: 0.85, use_count: 9, accepted_count: 4, rejected_count: 1, seen_count: 7, last_seen_at: 9, scope: "character", user_approved: true });
  });

  it("plays saved examples and keeps their linked learning records when editing", async () => {
    const save = vi.fn<NonNullable<MockCallHandlers["asrVoiceExampleSave"]>>((request) => ok({ ...example, expected_text: request.expected_text }));
    await mount({ tab: "examples", calls: { asrVoiceExampleSave: save } });
    await click("Edit");
    await screen.findByRole("dialog");
    fireEvent.change(drawer().getByRole("textbox", { name: /It should say/ }), { target: { value: "Welcome to LettuceAI" } });
    await click("Save");
    await waitFor(() => expect(save).toHaveBeenCalled());
    expect(save.mock.calls[0]?.[0]).toMatchObject({ audio_asset_id: "audio1", vocabulary_term_id: "word1", correction_id: "correction1" });
  });

  it("imports and exports the complete learning bundle through native dialogs", async () => {
    const imported = vi.fn(() => ok({ vocabulary_count: 2, correction_count: 1, ignored_suggestion_count: 0, voice_example_count: 3 }));
    const exported = vi.fn<NonNullable<MockCallHandlers["asrLearningExport"]>>(() => ok(null));
    await mount({ calls: { filesPickOpen: () => ok({ sources: [{ uri: "/learning.json" }] }), filesPickSave: () => ok({ target: { uri: "/export.json" } }), asrLearningImport: imported, asrLearningExport: exported } });
    await click("More");
    fireEvent.click(await screen.findByRole("menuitem", { name: "Import words and corrections" }));
    await waitFor(() => expect(imported).toHaveBeenCalled());
    await click("More");
    fireEvent.click(await screen.findByRole("menuitem", { name: "Export words and corrections" }));
    await waitFor(() => expect(exported).toHaveBeenCalled());
    expect(exported.mock.calls[0]?.[0]).toMatchObject({ target: { uri: "/export.json" }, filter: { scopes: ["global", "project", "character", "conversation"] } });
  });

  it("shares measured speed and ETA between the speech card and shell across navigation", async () => {
    const clock = performance.now.bind(performance);
    let offset = 0;
    vi.spyOn(performance, "now").mockImplementation(() => clock() + offset);
    const running = job({ kind: "artifact_install", state: "running", subject_detail: { type: "whisper_model_install", model_id: "small", filename: "ggml-small.bin" }, result: null, progress: { current: 100, total: 1000, unit: "bytes", bytes_per_second: null, label_code: "download" } });
    const { transport, runtime } = await mount({ phone: false, calls: { jobsList: () => ok({ items: [running], next_cursor: null }) }, streams: { jobWatch: () => ok(running) } });
    await click(/^Downloads/);
    const panel = within(await screen.findByRole("dialog"));
    expect(panel.queryByText(/\/s/)).toBeNull();
    offset += 1000;
    const advanced = { ...running, progress: { ...running.progress, current: 200 } };
    await act(async () => { transport.emitAppEvent({ type: "job_updated", job: advanced }); });
    const speed = await panel.findByText(/\/s/);
    expect(speed.textContent).toContain("left");
    expect(screen.getAllByText(speed.textContent!)).toHaveLength(2);
    await act(async () => { transport.emitAppEvent({ type: "job_updated", job: advanced }); });
    expect(screen.getAllByText(speed.textContent!)).toHaveLength(2);
    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await act(async () => { await runtime.router.navigate({ to: "/settings/$section", params: { section: "about" } }); });
    await click(/^Downloads/);
    expect(await within(await screen.findByRole("dialog")).findByText(/\/s/)).toBeTruthy();
  });

  it("restores download locks from jobs and uses real ETA in the shared card", async () => {
    const running = job({ kind: "artifact_install", state: "running", subject_detail: { type: "whisper_model_install", model_id: "small", filename: "ggml-small.bin" }, result: null, progress: { current: 100, total: 1000, unit: "bytes", bytes_per_second: 100, label_code: "download" } });
    const download = vi.fn(() => ok({ job_id: running.id }));
    await mount({ calls: { jobsList: () => ok({ items: [running], next_cursor: null }), whisperDownload: download }, streams: { jobWatch: () => ok(running) } });
    await click(/^base/);
    await screen.findByRole("dialog");
    expect(await drawer().findByText("Downloading")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Get" })).toBeNull();
    expect(await screen.findByText(/9s left/)).toBeTruthy();
    expect(download).not.toHaveBeenCalled();
  });

  it("shows a model skeleton while the installed inventory is loading", async () => {
    let release!: (value: ReturnType<typeof ok<WhisperInstalledModels>>) => void;
    await mount({ calls: { whisperModelsList: () => new Promise((resolve) => { release = resolve; }) } });
    expect(screen.getByRole("status", { name: "Loading speech recognition settings" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /^base/ })).toBeNull();
    await act(async () => { release(ok({ models, gpu_supported: false })); });
    expect(await screen.findByRole("button", { name: /^base/ })).toBeTruthy();
  });

  it("retries a failed model list", async () => {
    let failing = true;
    await mount({ calls: { whisperModelsList: () => failing ? fail() : ok({ models, gpu_supported: false }) } });
    expect(await screen.findByText(/available right now/)).toBeTruthy();
    failing = false;
    await click("Try again");
    expect(await screen.findByRole("button", { name: /^base/ })).toBeTruthy();
  });
});
