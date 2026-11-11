import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { toast } from "@lettuceai/crisp";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AudioProviderView, JobResultDto_Deserialize, JobView, KokoroInventory, UserVoiceView } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers, type MockStreamHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";
import { parseBlend, parseSpeed, serializeBlend, serializeSpeed } from "./voices/data";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));
afterEach(() => { toast.dismiss(); cleanup(); vi.restoreAllMocks(); });
const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const provider = (id: string, label: string, configuration: AudioProviderView["configuration"]): AudioProviderView => ({ id, label, configuration, has_api_key: true, revision: 1 });
const openai = provider("openai", "My OpenAI", { type: "open_ai_compatible", base_url: null, request_path: null });
const eleven = provider("eleven", "My ElevenLabs", { type: "elevenlabs" });
const kokoro = provider("kokoro", "Local Kokoro", { type: "kokoro", variant: "fp16" });
const voice = (patch: Partial<UserVoiceView>): UserVoiceView => ({ id: "voice1", provider_id: "openai", name: "Narrator", model_id: "tts-1", voice_id: "alloy", prompt: "A calm voice", revision: 3, ...patch });
const artifact = { byte_size: 1000 };
const installedInventory: KokoroInventory = { variant: "fp16", variant_allowed_on_platform: true, model: artifact, config: artifact, tokenizer: artifact, tokenizer_config: artifact, installed_voices: [{ id: "af_heart", artifact }], selected_voice_installed: null };
const variants = [
  { id: "int8", label: "Int8", filename: "model_int8.onnx", size_mb: 92, mobile_supported: true },
  { id: "fp16", label: "Fp16", filename: "model_fp16.onnx", size_mb: 163, mobile_supported: true },
  { id: "fp32", label: "Fp32", filename: "model.onnx", size_mb: 326, mobile_supported: false },
];
const available = [
  { id: "af_heart", installed: true, source_revision: "1", byte_size: 500000 },
  { id: "af_bella", installed: false, source_revision: "1", byte_size: 600000 },
  { id: "bf_emma", installed: false, source_revision: "1", byte_size: 700000 },
];
const job = (result: JobResultDto_Deserialize | null): JobView => ({ id: "job1", kind: "speech_synthesize" as JobView["kind"], subject: { kind: "speech_request", id: "request1" }, subject_detail: null, state: "succeeded", progress: { current: 0, total: null, unit: "items", label_code: null, bytes_per_second: null }, created_at: 1, updated_at: 2, failure: null, result });

async function mount(options: { path?: string; calls?: MockCallHandlers; streams?: MockStreamHandlers; phone?: boolean } = {}) {
  const transport = createMockTransport({
    calls: {
      appStatus: () => ok({ ...mockAppStatus, platform: options.phone ? "android" : "other", ui_state: { onboarding: { completed: true } } }),
      jobsList: () => ok({ items: [], next_cursor: null }),
      audioProvidersList: () => ok([]),
      userVoicesList: () => ok([]),
      ttsCacheStats: () => ok({ count: 0, size_bytes: 0 }),
      providerAccountsList: () => ok([]),
      providerCatalog: () => ok({ providers: [] }),
      modelsList: () => ok({ models: [], default_model_id: null, revision: 1 }),
      kokoroVariants: () => ok(variants),
      kokoroInventory: () => ok(installedInventory),
      kokoroVoicesAvailable: () => ok(available),
      ...options.calls,
    },
    streams: { jobWatch: () => ok(job(null)), ...options.streams },
  });
  const history = createMemoryHistory({ initialEntries: ["/settings", options.path ?? "/settings/voices"] });
  const runtime = await createAppRuntime({ transport, history, i18n: await createTestI18n("en") });
  await act(async () => { render(<App runtime={runtime} />); });
  return { history };
}
const drawer = () => within(screen.getAllByRole("dialog").at(-1)!);
const click = async (name: string | RegExp) => fireEvent.click(await screen.findByRole("button", { name }));

describe("voices blends", () => {
  it("reads a plain id, a JSON list and the speed, and writes only voices above zero weight", () => {
    expect(parseBlend("af_heart")).toEqual([{ id: "af_heart", weight: 100 }]);
    expect(parseBlend('[{"voiceId":"af_heart","weight":0.6},{"voiceId":"bf_emma","weight":0.4}]')).toEqual([{ id: "af_heart", weight: 60 }, { id: "bf_emma", weight: 40 }]);
    expect(parseBlend("[not json")).toEqual([]);
    expect(serializeBlend([{ id: "af_heart", weight: 60 }, { id: "bf_emma", weight: 0 }, { id: "am_adam", weight: 40 }])).toBe('[{"voiceId":"af_heart","weight":0.6},{"voiceId":"am_adam","weight":0.4}]');
    expect(parseBlend(serializeBlend(parseBlend('[{"voiceId":"af_heart","weight":0.6},{"voiceId":"bf_emma","weight":0.4}]')))).toEqual([{ id: "af_heart", weight: 60 }, { id: "bf_emma", weight: 40 }]);
    expect(serializeSpeed(1.15)).toBe('{"speed":1.15}');
    expect(parseSpeed('{"speed":1.15}')).toBe(1.15);
    expect(parseSpeed(null)).toBe(1);
    expect(parseSpeed("nonsense")).toBe(1);
  });
});

describe("voices settings", () => {
  it("sets up Kokoro on first use without adding a provider by hand", async () => {
    let providers: AudioProviderView[] = [];
    const create = vi.fn((request: { label: string; draft: { configuration: AudioProviderView["configuration"] } }) => {
      const created = provider("kokoro-new", request.label, request.draft.configuration);
      providers = [created];
      return ok(created);
    });
    const { history } = await mount({ calls: { audioProvidersList: () => ok(providers), audioProviderCreate: create, kokoroInventory: () => ok({ ...installedInventory, model: null }) } });
    fireEvent.click(await screen.findByRole("button", { name: /Kokoro studio.*Free voices that run on this device/ }));
    await waitFor(() => expect(create).toHaveBeenCalledTimes(1));
    expect(create.mock.calls[0]![0]).toMatchObject({ label: "Kokoro", draft: { configuration: { type: "kokoro", variant: null }, api_key: null } });
    expect(await screen.findByRole("dialog", { name: /Set up Kokoro/ })).toBeTruthy();
    expect(drawer().getByRole("button", { name: /Download engine/ })).toBeTruthy();
    expect(history.location.search).not.toContain("kokoro=");
  });

  it("points to the audio providers when there are none", async () => {
    const { history } = await mount();
    fireEvent.click(await screen.findByRole("button", { name: /No voice providers yet/ }));
    await waitFor(() => expect(history.location.pathname).toBe("/settings/providers"));
    expect(history.location.search).toContain("providersTab=audio");
    expect(await screen.findByRole("tab", { name: /Audio/ })).toHaveProperty("ariaSelected", "true");
    expect(screen.queryByText("My voices")).toBeNull();
  });

  it("opens a cloud voice in the editor and a Kokoro voice in the blend editor", async () => {
    const blend = voice({ id: "blend1", provider_id: "kokoro", name: "Warm mix", model_id: "fp16", voice_id: '[{"voiceId":"af_heart","weight":0.6},{"voiceId":"bf_emma","weight":0.4}]', prompt: '{"speed":1.15}' });
    const { history } = await mount({ calls: { audioProvidersList: () => ok([openai, kokoro]), userVoicesList: () => ok([voice({}), blend]) } });
    expect(await screen.findByText("My OpenAI · A calm voice")).toBeTruthy();
    expect(screen.getByText("Kokoro · Blend of 2 voices")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Narrator/ }));
    await screen.findByRole("dialog");
    expect(drawer().getByText("Edit voice")).toBeTruthy();
    expect((drawer().getByRole("textbox", { name: "Name" }) as HTMLInputElement).value).toBe("Narrator");
    await click("Close");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(screen.getByRole("button", { name: /Warm mix/ }));
    await waitFor(() => expect(history.location.search).toContain("kokoroBlend=blend1"));
    expect((await screen.findAllByText("Edit blend")).length).toBeGreaterThan(0);
    expect((screen.getByRole("textbox", { name: "Name" }) as HTMLInputElement).value).toBe("Warm mix");
    expect(screen.getByText("60% of the blend")).toBeTruthy();
  });

  it("requires the model and voice id of an OpenAI compatible voice before saving", async () => {
    const create = vi.fn<NonNullable<MockCallHandlers["userVoiceCreate"]>>((request) => ok(voice({ id: "new", name: request.name, model_id: request.model_id, voice_id: request.voice_id })));
    await mount({ path: "/settings/voices?voiceEditor=new", calls: { audioProvidersList: () => ok([openai]), userVoiceCreate: create } });
    await screen.findByRole("dialog");
    await click("Save");
    expect(await drawer().findByText("Give the voice a name.")).toBeTruthy();
    fireEvent.change(drawer().getByRole("textbox", { name: "Name" }), { target: { value: "Narrator" } });
    await click("Save");
    expect(await drawer().findByText("Enter the model ID your endpoint expects.")).toBeTruthy();
    fireEvent.change(drawer().getByRole("textbox", { name: "Model" }), { target: { value: "tts-1" } });
    await click("Save");
    expect(await drawer().findByText("Enter a voice ID your endpoint supports.")).toBeTruthy();
    expect(create).not.toHaveBeenCalled();
    fireEvent.change(drawer().getByRole("textbox", { name: "Voice ID" }), { target: { value: "alloy" } });
    await click("Save");
    await waitFor(() => expect(create).toHaveBeenCalledTimes(1));
    expect(create.mock.calls[0]?.[0]).toMatchObject({ provider_id: "openai", name: "Narrator", model_id: "tts-1", voice_id: "alloy", prompt: null });
  });

  it("makes an ElevenLabs voice only after a design preview, through the create job", async () => {
    const preview = vi.fn<NonNullable<MockCallHandlers["voiceDesignPreview"]>>(() => ok([{ generated_voice_id: "generated1", audio: { asset_id: "audio1", url: "asset://audio1" }, duration_secs: 4 }]));
    const designCreate = vi.fn<NonNullable<MockCallHandlers["voiceDesignCreate"]>>(() => ok({ job_id: "job1" }));
    const create = vi.fn<NonNullable<MockCallHandlers["userVoiceCreate"]>>((request) => ok(voice({ id: "new", provider_id: "eleven", name: request.name, voice_id: request.voice_id })));
    await mount({
      path: "/settings/voices?voiceEditor=new",
      calls: {
        audioProvidersList: () => ok([eleven]),
        ttsVoiceDesignModels: () => ok([{ id: "design1", name: "Design model" }]),
        voiceDesignPreview: preview,
        voiceDesignCreate: designCreate,
        userVoiceCreate: create,
      },
      streams: { jobWatch: () => ok(job({ type: "voice_created", voice_id: "eleven-voice" })) },
    });
    await screen.findByRole("dialog");
    expect(drawer().getByText("Voice description")).toBeTruthy();
    expect(drawer().queryByText("ElevenLabs voice")).toBeNull();
    fireEvent.change(drawer().getByRole("textbox", { name: "Name" }), { target: { value: "Aria" } });
    fireEvent.change(drawer().getByRole("textbox", { name: /Voice description/ }), { target: { value: "A warm narrator" } });
    await click("Save");
    expect(await drawer().findByText("Preview the voice first to create it.")).toBeTruthy();
    expect(designCreate).not.toHaveBeenCalled();
    fireEvent.click(drawer().getByRole("button", { name: "Preview" }));
    await waitFor(() => expect(preview).toHaveBeenCalledTimes(1));
    expect(preview.mock.calls[0]?.[0]).toMatchObject({ provider_id: "eleven", voice_description: "A warm narrator", model_id: "design1", num_previews: 1 });
    await click("Save");
    await waitFor(() => expect(create).toHaveBeenCalledTimes(1));
    expect(designCreate.mock.calls[0]?.[0]).toMatchObject({ provider_id: "eleven", generated_voice_id: "generated1", name: "Aria", description: "A warm narrator" });
    expect(create.mock.calls[0]?.[0]).toMatchObject({ provider_id: "eleven", name: "Aria", voice_id: "eleven-voice", model_id: "design1" });
  });

  it("clears the saved speech and refreshes the count", async () => {
    let count = 3;
    const clear = vi.fn<NonNullable<MockCallHandlers["ttsCacheClear"]>>(() => { count = 0; return ok(null); });
    await mount({ calls: { audioProvidersList: () => ok([openai]), ttsCacheStats: () => ok({ count, size_bytes: count * 1024 }), ttsCacheClear: clear } });
    expect(await screen.findByText("3 files · 3.0 KB")).toBeTruthy();
    await click("Clear saved speech");
    await waitFor(() => expect(clear).toHaveBeenCalledTimes(1));
    expect(await screen.findByText("Saved speech cleared")).toBeTruthy();
    await waitFor(() => expect(screen.getByRole("button", { name: "Clear saved speech" }).hasAttribute("disabled")).toBe(true));
    expect(screen.getByText("0 files · 0 B")).toBeTruthy();
  });
});

describe("Kokoro studio", () => {
  it("sets up the engine in a sheet from the Voices page: pick a quality, then download", async () => {
    let configuration: AudioProviderView["configuration"] = { type: "kokoro", variant: null };
    const update = vi.fn<NonNullable<MockCallHandlers["audioProviderUpdate"]>>((request) => { configuration = request.configuration; return ok(provider("kokoro", "Local Kokoro", configuration)); });
    const install = vi.fn<NonNullable<MockCallHandlers["kokoroInstallModel"]>>(() => ok({ job_id: "job1" }));
    await mount({
      calls: {
        audioProvidersList: () => ok([provider("kokoro", "Local Kokoro", configuration)]),
        audioProviderUpdate: update,
        kokoroInventory: (request) => ok({ ...installedInventory, variant: request.variant, model: null, installed_voices: [] }),
        kokoroInstallModel: install,
      },
    });
    await click(/Kokoro studio.*Not set up yet/);
    expect(drawer().getByRole("button", { name: /Download engine/ }).hasAttribute("disabled")).toBe(true);
    fireEvent.click(drawer().getByRole("button", { name: /Fp16/ }));
    await waitFor(() => expect(update).toHaveBeenCalledTimes(1));
    expect(update.mock.calls[0]?.[0]).toMatchObject({ provider_id: "kokoro", expected_revision: 1, configuration: { type: "kokoro", variant: "fp16" } });
    await waitFor(() => expect(drawer().getByRole("button", { name: /Download engine/ }).hasAttribute("disabled")).toBe(false));
    fireEvent.click(drawer().getByRole("button", { name: /Download engine/ }));
    await waitFor(() => expect(install).toHaveBeenCalledWith({ variant: "fp16" }));
  });

  it("shows the library as chips per language and gender, and downloads one voice or all missing ones", async () => {
    const install = vi.fn<NonNullable<MockCallHandlers["kokoroInstallVoices"]>>(() => ok({ job_id: "job2" }));
    await mount({ path: "/settings/voices?kokoro=kokoro", calls: { audioProvidersList: () => ok([kokoro]), kokoroInstallVoices: install } });
    expect(await screen.findByRole("button", { name: /Kokoro engine/ })).toBeTruthy();
    const american = within(await screen.findByRole("region", { name: "American English" }));
    expect(american.getByText("1 of 2 downloaded")).toBeTruthy();
    expect(american.getByRole("button", { name: "Heart, downloaded" })).toBeTruthy();
    expect(within(screen.getByRole("region", { name: "British English" })).getByRole("button", { name: "Emma, not downloaded" })).toBeTruthy();
    fireEvent.click(american.getByRole("button", { name: "Bella, not downloaded" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Download voice" }));
    await waitFor(() => expect(install).toHaveBeenCalledWith({ voice_ids: ["af_bella"] }));
    fireEvent.change(screen.getByRole("searchbox", { name: "Search voices" }), { target: { value: "british" } });
    await waitFor(() => expect(screen.queryByRole("region", { name: "American English" })).toBeNull());
    await click("Download all (1)");
    await waitFor(() => expect(screen.getAllByRole("button", { name: "Download all (1)" }).length).toBe(2));
    fireEvent.click(screen.getAllByRole("button", { name: "Download all (1)" }).at(-1)!);
    await waitFor(() => expect(install).toHaveBeenLastCalledWith({ voice_ids: ["bf_emma"] }));
  });
});
