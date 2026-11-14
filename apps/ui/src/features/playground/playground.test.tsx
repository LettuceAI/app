import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { toast } from "@lettuceai/crisp";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  ImageCapabilities,
  ImageGenerateRequest_Deserialize,
  JobView,
  ModelView,
  PlaygroundEntry_Serialize,
  ProviderAccountView,
  SettingsView,
} from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers, type MockStreamHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

beforeEach(() => {
  localStorage.clear();
  vi.spyOn(window, "matchMedia").mockImplementation((query) => ({
    matches: query.includes("min-width: 1024px"),
    media: query,
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  }));
});
afterEach(() => {
  toast.dismiss();
  cleanup();
});

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const accounts: ProviderAccountView[] = ["sdcpp", "openai"].map((kind) => ({
  id: kind, provider_kind: kind, label: kind === "openai" ? "Cloud account" : "Local engine", protocol: "open_ai_compatible", base_url: null,
  enabled: true, streaming_enabled: true, allow_invalid_tls: false, api_key_set: true, config: {}, revision: 1, created_at: 1, updated_at: 1,
}));
const model = (id: string, account: string, output: ModelView["output_scopes"], config: ModelView["config"] = {}): ModelView => ({
  id, provider_account_id: account, external_model_id: `${id}-slug`, display_name: id, kind: "chat", config,
  input_scopes: ["text"], output_scopes: output, revision: 1, created_at: 1, updated_at: 1,
});
const baseLora = { path: "/loras/base.safetensors", multiplier: 0.7, is_high_noise: false, keywords: ["base"] };
const models = [
  model("Local image", "sdcpp", ["image"], { stable_diffusion: { steps: 4, cfg_scale: 1, sampler: "euler", size: "1024x1024", base_loras: [baseLora] } }),
  model("Cloud image", "openai", ["image"]),
  model("Text only", "openai", ["text"]),
];
const capabilities = (local: boolean): ImageCapabilities => local
  ? { provider_kind: "sdcpp", local: true, sizes: ["1024x1024", "832x1216"], default_size: "1024x1024", samplers: ["euler", "dpm++2m"], schedulers: ["karras"], negative_prompt: true, quality: [], styles: [], max_count: 4 }
  : { provider_kind: "openai", local: false, sizes: ["1024x1024", "1792x1024"], default_size: "1024x1024", samplers: [], schedulers: [], negative_prompt: false, quality: ["standard", "hd"], styles: ["vivid"], max_count: 4 };

const asset = (id: string) => ({ asset_id: id, url: `asset://${id}`, preview_url: `asset://${id}-preview` });
const entry = (overrides: Partial<PlaygroundEntry_Serialize>): PlaygroundEntry_Serialize => ({
  id: "entry", origin: "generated", job_id: null, created_at: 1, provider_kind: "sdcpp", model_id: "Local image", model_name: "Local image",
  prompt: "a lighthouse", negative_prompt: null, seed: null, params_json: "{}", status: "complete", failure: null, upscale_of: null,
  images: [{ asset: asset("image-1"), mime_type: "image/png", width: 1024, height: 1024 }], ...overrides,
});
const generatedParams = (extra: Record<string, unknown> = {}) => JSON.stringify({
  format: "generated-v1", settings: { size: "832x1216", steps: 20 }, loras: [], size: "832x1216", quality: null, style: null, count: 1, ...extra,
});
const runningJob = { id: "job-1", kind: "image_generate", subject: { kind: "image_request", id: "job-1" }, subject_detail: null, state: "running", progress: { current: 0, total: null, unit: null, label_code: null, bytes_per_second: null }, created_at: 1, updated_at: 1, failure: null, result: null } as JobView;

interface Options {
  platform?: "other" | "android";
  path?: string;
  history?: PlaygroundEntry_Serialize[];
  calls?: MockCallHandlers;
  streams?: MockStreamHandlers;
}

async function mount(options: Options = {}) {
  const generate = vi.fn<(request: ImageGenerateRequest_Deserialize) => void>();
  const view = { revision: 1, global: { image_generation: { avatar_enabled: true, avatar_model_profile_id: null, scene_enabled: false, scene_automatic_enabled: true, scene_mode: "auto", scene_model_profile_id: null, scene_writer_model_profile_id: null, scene_default_size: "768x768" } } } as unknown as SettingsView;
  const transport = createMockTransport({
    calls: {
      appStatus: () => ok({ ...mockAppStatus, platform: options.platform ?? "other", ui_state: { onboarding: { completed: true } } }),
      settingsGet: () => ok(view),
      promptsList: () => ok({ items: [], next_cursor: null }),
      modelsList: () => ok({ models, default_model_id: null, revision: 1 }),
      providerAccountsList: () => ok(accounts),
      imageCapabilities: (request) => ok(capabilities(request.target.type === "model" && request.target.model_id === "Local image")),
      sdUpscalersList: () => ok({ models: [], hires_upscaler_names: [], recommended_filename: "upscaler.pth", recommended_bytes: 1, recommended_installed: false }),
      lorasList: () => ok({ loras: [] }),
      playgroundHistoryList: () => ok({ entries: options.history ?? [] }),
      imageGenerate: (request) => {
        generate(request);
        return ok({ job_id: "job-1" });
      },
      conversationsList: () => ok({ items: [], next_cursor: null }),
      ...options.calls,
    },
    streams: options.streams ?? {},
  });
  const history = createMemoryHistory({ initialEntries: [options.path ?? "/playground"] });
  const runtime = await createAppRuntime({ transport, history, i18n: await createTestI18n("en") });
  await act(async () => { render(<App runtime={runtime} />); });
  return { runtime, history, generate };
}

const generateButton = () => screen.findByRole("button", { name: /^Generat(e|ing)/ }) as Promise<HTMLButtonElement>;

async function writePrompt(text: string) {
  fireEvent.change(await screen.findByPlaceholderText("Describe the image you want to generate"), { target: { value: text } });
  await waitFor(async () => expect((await generateButton()).disabled).toBe(false));
}

describe("playground generate", () => {
  it("sends a local run with its own LoRAs and the model's settings", async () => {
    const { generate } = await mount();
    await writePrompt("  a lighthouse  ");
    fireEvent.change(screen.getByPlaceholderText("What to avoid"), { target: { value: " blurry " } });
    fireEvent.click(await generateButton());
    await waitFor(() => expect(generate).toHaveBeenCalledTimes(1));
    const request = generate.mock.calls[0]![0];
    expect(request).toMatchObject({
      request_id: expect.any(String),
      model_id: "Local image",
      prompt: "a lighthouse",
      input_images: [],
      loras: [{ path: "/loras/base.safetensors", multiplier: 0.7 }],
      size: "1024x1024",
      source: "playground",
      output: { kind: "retained" },
      settings: { steps: 4, cfg_scale: 1, sampler: "euler", size: "1024x1024", hires_enabled: false, negative_prompt: "blurry" },
    });
    expect(request.settings).not.toHaveProperty("base_loras");
    expect(request.settings).not.toHaveProperty("seed");
    expect(request).not.toHaveProperty("quality");
  });

  it("sends a remote run without the local fields", async () => {
    localStorage.setItem("playground:lastModel", "Cloud image");
    const { generate } = await mount();
    await writePrompt("a lighthouse");
    fireEvent.click(await screen.findByRole("combobox", { name: "Quality" }));
    fireEvent.click(await screen.findByRole("option", { name: "hd" }));
    fireEvent.click(await generateButton());
    await waitFor(() => expect(generate).toHaveBeenCalledTimes(1));
    const request = generate.mock.calls[0]![0];
    expect(request).toMatchObject({ model_id: "Cloud image", loras: [], size: "1024x1024", quality: "hd", source: "playground", settings: { size: "1024x1024" } });
    for (const key of ["steps", "cfg_scale", "sampler", "scheduler", "seed", "hires_enabled", "negative_prompt", "base_loras"]) expect(request.settings).not.toHaveProperty(key);
    expect(request).not.toHaveProperty("style");
  });
});

describe("playground feed", () => {
  it("shows a running generation as the live card and cancels it", async () => {
    const cancel = vi.fn<NonNullable<MockCallHandlers["jobCancel"]>>(() => ok(null));
    await mount({
      history: [entry({ id: "job-1", job_id: "job-1", status: "pending", images: [], params_json: generatedParams({ count: 2 }) })],
      calls: { jobCancel: cancel },
      streams: {
        jobWatch: (_request, emit) => {
          emit({ type: "image_progress", progress: { phase: "sampling", step: 2, total_steps: 4, queue_position: null, preview_asset: null } });
          return ok(runningJob);
        },
      },
    });
    await screen.findByText("Sampling 2/4");
    expect(((await generateButton()).textContent)).toBe("Generating...");
    expect((await generateButton()).disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Cancel generation" }));
    await waitFor(() => expect(cancel).toHaveBeenCalledWith({ job_id: "job-1" }));
  });

  it("regenerates from a generated entry without its seed and from an imported legacy entry", async () => {
    const imported = entry({
      id: "legacy", origin: "imported", created_at: 2, negative_prompt: "ugly", images: [{ asset: null, mime_type: null, width: null, height: null }],
      params_json: JSON.stringify({
        advancedModelSettings: { sdSize: "768x768", sdSteps: 12, sdCfgScale: 4, sdSampler: "dpm++2m", sdScheduler: "karras", sdSeed: 5, sdHiresEnabled: true, sdHiresUpscaler: "4x", sdHiresScale: 2, sdHiresSteps: 8, sdHiresDenoisingStrength: 0.4 },
        loras: [{ path: "/loras/b.safetensors", multiplier: 0.9, isHighNoise: true, keywords: ["k"] }],
        size: "768x768", n: 3, quality: null, style: null,
      }),
    });
    const generated = entry({
      id: "generated", created_at: 1, seed: 99, negative_prompt: "blurry",
      params_json: generatedParams({
        count: 2,
        settings: { size: "832x1216", steps: 20, cfg_scale: 5, sampler: "euler", seed: 99, negative_prompt: "blurry", base_loras: [baseLora], not_a_setting: true },
        loras: [{ path: "/loras/a.safetensors", multiplier: 0.5, is_high_noise: false, keywords: ["x"] }],
      }),
    });
    const { generate } = await mount({ history: [imported, generated] });
    const buttons = await screen.findAllByRole("button", { name: "Regenerate with same settings" });
    expect(buttons).toHaveLength(2);

    fireEvent.click(buttons[0]!);
    await waitFor(() => expect(generate).toHaveBeenCalledTimes(1));
    const fromGenerated = generate.mock.calls[0]![0];
    expect(fromGenerated).toMatchObject({
      model_id: "Local image",
      prompt: "a lighthouse",
      input_images: [],
      count: 2,
      size: "832x1216",
      loras: [{ path: "/loras/a.safetensors", multiplier: 0.5, is_high_noise: false, keywords: ["x"] }],
      settings: { size: "832x1216", steps: 20, cfg_scale: 5, sampler: "euler", negative_prompt: "blurry" },
    });
    for (const key of ["seed", "base_loras", "not_a_setting"]) expect(fromGenerated.settings).not.toHaveProperty(key);

    await waitFor(() => expect((screen.getAllByRole("button", { name: "Regenerate with same settings" })[1] as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getAllByRole("button", { name: "Regenerate with same settings" })[1]!);
    await waitFor(() => expect(generate).toHaveBeenCalledTimes(2));
    const fromImported = generate.mock.calls[1]![0];
    expect(fromImported).toMatchObject({
      model_id: "Local image",
      count: 3,
      size: "768x768",
      loras: [{ path: "/loras/b.safetensors", multiplier: 0.9, is_high_noise: true, keywords: ["k"] }],
      settings: {
        size: "768x768", steps: 12, cfg_scale: 4, sampler: "dpm++2m", scheduler: "karras", negative_prompt: "ugly",
        hires_enabled: true, hires_upscaler: "4x", hires_scale: 2, hires_steps: 8, hires_denoising_strength: 0.4,
      },
    });
    expect(fromImported.settings).not.toHaveProperty("seed");
  });

  it("deletes an entry and keeps its images when asked to", async () => {
    const remove = vi.fn<NonNullable<MockCallHandlers["playgroundHistoryDelete"]>>(() => ok({ deleted_images: 0 }));
    await mount({ history: [entry({ id: "entry-1", params_json: generatedParams() })], calls: { playgroundHistoryDelete: remove } });
    fireEvent.click(await screen.findByRole("button", { name: "Delete generation" }));
    fireEvent.click(await screen.findByRole("button", { name: "Delete entry, keep images" }));
    await waitFor(() => expect(remove).toHaveBeenCalledWith({ id: "entry-1", delete_images: false }));
  });
});

describe("playground layout", () => {
  it("opens the prompt and settings panes in sheets on a narrow window", async () => {
    vi.spyOn(window, "matchMedia").mockImplementation((query) => ({
      matches: false, media: query, onchange: null, addEventListener: () => {}, removeEventListener: () => {}, addListener: () => {}, removeListener: () => {}, dispatchEvent: () => false,
    }));
    await mount();
    expect(screen.queryByPlaceholderText("Describe the image you want to generate")).toBeNull();
    fireEvent.click(await within(await screen.findByRole("banner")).findByRole("button", { name: "Settings" }));
    expect(await screen.findByRole("dialog", { name: "Settings" })).toBeTruthy();
    expect(await screen.findByRole("combobox", { name: "Size" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Settings" })).toBeNull());
    fireEvent.click(within(screen.getByRole("banner")).getByRole("button", { name: "Prompt" }));
    expect(await screen.findByPlaceholderText("Describe the image you want to generate")).toBeTruthy();
  });
});

describe("playground entry on the image settings page", () => {
  it("is absent on android", async () => {
    await mount({ platform: "android", path: "/settings/imageGeneration" });
    await screen.findByText("Avatars");
    expect(screen.queryByRole("button", { name: /^Open Playground/ })).toBeNull();
  });

  it("opens the playground elsewhere", async () => {
    const { history } = await mount({ path: "/settings/imageGeneration" });
    fireEvent.click(await screen.findByRole("button", { name: /^Open Playground/ }));
    await waitFor(() => expect(history.location.pathname).toBe("/playground"));
  });
});
