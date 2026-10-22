import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { toast } from "@lettuceai/crisp";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { JobEvent, JobView, ModelView, ProviderAccountView, SdCatalogProfile, SettingsImageGenerationSettings, SettingsUpdateRequest_Deserialize, SettingsView } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers, type MockStreamHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

afterEach(() => { toast.dismiss(); cleanup(); });

const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const initial: SettingsImageGenerationSettings = {
  avatar_enabled: true, avatar_model_profile_id: null,
  scene_enabled: false, scene_automatic_enabled: true, scene_mode: "auto", scene_model_profile_id: null,
  scene_writer_model_profile_id: null, creation_helper_model_profile_id: "keep-helper", scene_default_size: "768x768",
};
const accounts: ProviderAccountView[] = ["openai", "sdcpp"].map((kind) => ({
  id: kind, provider_kind: kind, label: kind === "openai" ? "Cloud account" : "Local engine", protocol: "open_ai_compatible", base_url: null,
  enabled: true, streaming_enabled: true, allow_invalid_tls: false, api_key_set: true, config: {}, revision: 1, created_at: 1, updated_at: 1,
}));
const model = (id: string, account: string, input: ModelView["input_scopes"], output: ModelView["output_scopes"]): ModelView => ({
  id, provider_account_id: account, external_model_id: `${id}-slug`, display_name: id, kind: "chat", config: {},
  input_scopes: input, output_scopes: output, revision: 1, created_at: 1, updated_at: 1,
});
const models = [model("Cloud image", "openai", ["text"], ["image"]), model("Local image", "sdcpp", ["text"], ["image"]), model("Vision writer", "openai", ["text", "image"], ["text"]), model("Text writer", "openai", ["text"], ["text"])];

const runtimeRef = { release: "master-100-abc", asset: "vulkan.zip" };
const runtimeBuild = { ...runtimeRef, backend: "Vulkan", size_bytes: 1000, active: true };
const runtimeAsset = { name: "vulkan.zip", backend: "Vulkan", bytes: 1000, sha256: null, download_url: "https://example.org/engine.zip", dependencies: [] };
const profile: SdCatalogProfile = {
  id: "flux", display_name: "FLUX model", family: "FLUX", description: "Local image model", license: "Apache-2.0", source_url: "https://example.org/flux",
  supports_text_to_image: true, supports_image_edit: false, supports_lora: true, max_reference_images: null, requires_reference_image: false, recommended_for_scenes: true,
  default_width: 1024, default_height: 1024, default_steps: 4, default_cfg: 1, minimum_runtime_build: 90,
  variants: [{ id: "q4", label: "Q4", description: "Quantized", download_bytes: 1024, installed: false, recommended: true, smaller: true }],
};
const testJob = (state: JobView["state"], result: JobView["result"] = null): JobView => ({
  id: "image-job", kind: "artifact_install", subject: { kind: "artifact_install", id: "image-install" }, subject_detail: null, state,
  progress: { current: 1, total: 10, unit: "bytes", label_code: "download", bytes_per_second: 1 }, created_at: 1, updated_at: 1, failure: null, result,
});

async function mount(options: { settings?: Partial<SettingsImageGenerationSettings>; calls?: MockCallHandlers; streams?: MockStreamHandlers; platform?: "other" | "android"; path?: string; language?: "en" | "cimode"; saveGate?: Promise<void>; emitSettingsEvent?: boolean } = {}) {
  let view = { global: { image_generation: { ...initial, ...options.settings } }, revision: 7 } as SettingsView;
  const update = vi.fn(async (request: SettingsUpdateRequest_Deserialize) => {
    if (options.saveGate) await options.saveGate;
    if (request.expected_revision !== view.revision) return { status: "error" as const, error: { code: "conflict" as const, message: "diagnostic", details: null } };
    if (request.patch.section === "image_generation") view = { ...view, global: { ...view.global, image_generation: request.patch.value }, revision: view.revision + 1 };
    if (options.emitSettingsEvent) transport.emitAppEvent({ type: "settings_changed", section: "image_generation" });
    return ok(view);
  });
  const transport = createMockTransport({ calls: {
    appStatus: () => ok({ ...mockAppStatus, platform: options.platform ?? "android", ui_state: { onboarding: { completed: true } } }),
    settingsGet: () => ok(view), settingsUpdate: update,
    modelsList: () => ok({ models, default_model_id: null, revision: 1 }), providerAccountsList: () => ok(accounts),
    sdRuntimeInventory: () => ok({ installed: [runtimeBuild], active: runtimeRef }), sdModelsInstalled: () => ok({ models: [] }),
    sdDiskUsage: () => ok({ components_bytes: 1024, runtimes_bytes: 1024, loras_bytes: 0, total_bytes: 2048, has_engine: true, engine_release: runtimeRef.release, engine_backend: "Vulkan" }),
    sdRuntimeReleases: () => ok({ releases: [{ tag: runtimeRef.release, name: "Build 100", published_at: null, prerelease: false, assets: [runtimeAsset] }] }),
    sdUpscalersList: () => ok({ models: [], hires_upscaler_names: [], recommended_filename: "upscaler.pth", recommended_bytes: 1024, recommended_installed: false }),
    llamaDevices: () => ok({ devices: [] }), jobsList: () => ok({ items: [], next_cursor: null }),
    lorasList: () => ok({ loras: [] }),
    ...options.calls,
  }, streams: options.streams ?? {} });
  const history = createMemoryHistory({ initialEntries: [options.path ?? "/settings/imageGeneration"] });
  const runtime = await createAppRuntime({ transport, history, i18n: await createTestI18n(options.language ?? "en") });
  await act(async () => { render(<App runtime={runtime} />); });
  return { runtime, history, transport, update, current: () => view };
}

const clickRow = async (name: string) => fireEvent.click(await screen.findByRole("button", { name: new RegExp(`^${name}`) }));
const openEngineInstall = async () => { await clickRow("Install a build"); return screen.findByRole("dialog", { name: "Install a build" }); };

describe("image settings", () => {
  it("opens the page and preserves selections and hidden fields when toggling and changing mode", async () => {
    const { update, current } = await mount({ settings: { scene_enabled: true, scene_model_profile_id: "Cloud image" } });
    await clickRow("Ask first");
    await waitFor(() => expect(current().global.image_generation.scene_mode).toBe("ask_first"));
    fireEvent.click(screen.getByRole("switch", { name: "Scene pictures" }));
    await waitFor(() => expect(current().global.image_generation.scene_enabled).toBe(false));
    expect(screen.queryByText("Ask first")).toBeNull();
    expect(screen.getByRole("button", { name: /^Scene model/ }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("button", { name: /^Prompt writer/ }).hasAttribute("disabled")).toBe(false);
    fireEvent.click(screen.getByRole("switch", { name: "Scene pictures" }));
    await waitFor(() => expect(update).toHaveBeenCalledTimes(3));
    expect(update.mock.calls.map(([request]) => request.expected_revision)).toEqual([7, 8, 9]);
    expect(current().global.image_generation).toMatchObject({ scene_model_profile_id: "Cloud image", scene_mode: "ask_first", creation_helper_model_profile_id: "keep-helper", scene_default_size: "768x768" });
  });

  it("filters image models on mobile and keeps the vision writer separate", async () => {
    const { current } = await mount();
    await clickRow("Avatar model");
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByRole("button", { name: /Cloud image/ })).toBeTruthy();
    expect(within(dialog).queryByRole("button", { name: /Local image|Vision writer|Text writer/ })).toBeNull();
    fireEvent.click(within(dialog).getByRole("button", { name: /Cloud image/ }));
    await waitFor(() => expect(current().global.image_generation.avatar_model_profile_id).toBe("Cloud image"));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await clickRow("Prompt writer");
    const writer = await screen.findByRole("dialog");
    expect(within(writer).getByRole("button", { name: /Vision writer/ })).toBeTruthy();
    expect(within(writer).queryByRole("button", { name: /Text writer|Cloud image/ })).toBeNull();
  });

  it("keeps scene pictures available without automatic prompts and retains the mode and choice", async () => {
    const { current, update } = await mount({ settings: { scene_enabled: true, scene_mode: "ask_first", scene_model_profile_id: "Cloud image" } });
    const automatic = await screen.findByRole("switch", { name: "Automatic scene images" });
    expect(automatic.getAttribute("aria-checked")).toBe("true");
    fireEvent.click(automatic);
    await waitFor(() => expect(current().global.image_generation.scene_automatic_enabled).toBe(false));
    expect(screen.queryByText("Ask first")).toBeNull();
    expect(screen.getByRole("switch", { name: "Scene pictures" }).getAttribute("aria-checked")).toBe("true");
    expect(screen.getByRole("button", { name: /^Scene model/ }).hasAttribute("disabled")).toBe(false);
    expect(screen.getByRole("button", { name: /^Prompt writer/ }).hasAttribute("disabled")).toBe(false);
    fireEvent.click(screen.getByRole("switch", { name: "Scene pictures" }));
    await waitFor(() => expect(automatic.hasAttribute("disabled")).toBe(true));
    fireEvent.click(screen.getByRole("switch", { name: "Scene pictures" }));
    await waitFor(() => expect(automatic.hasAttribute("disabled")).toBe(false));
    expect(automatic.getAttribute("aria-checked")).toBe("false");
    expect(screen.queryByText("Ask first")).toBeNull();
    fireEvent.click(automatic);
    await waitFor(() => expect(screen.getByRole("button", { name: /^Ask first/ }).getAttribute("aria-pressed")).toBe("true"));
    expect(current().global.image_generation).toMatchObject({ scene_enabled: true, scene_automatic_enabled: true, scene_mode: "ask_first", scene_model_profile_id: "Cloud image", creation_helper_model_profile_id: "keep-helper", scene_default_size: "768x768" });
    expect(update.mock.calls.map(([request]) => request.expected_revision)).toEqual([7, 8, 9, 10]);
  });

  it("keeps a missing selection explicit, supports automatic selection and closes on browser Back", async () => {
    const { history, current } = await mount({ settings: { avatar_model_profile_id: "deleted" } });
    expect(await screen.findByText("Selected model is unavailable")).toBeTruthy();
    await clickRow("Avatar model");
    const picker = await screen.findByRole("dialog");
    expect(within(picker).getByRole("button", { name: /Use the first available/ }).getAttribute("aria-pressed")).toBe("false");
    fireEvent.change(within(picker).getByRole("searchbox"), { target: { value: "nothing" } });
    await act(async () => { history.back(); });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(history.location.pathname).toBe("/settings/imageGeneration");
    await clickRow("Avatar model");
    const reopened = await screen.findByRole("dialog");
    expect((within(reopened).getByRole("searchbox") as HTMLInputElement).value).toBe("");
    fireEvent.click(within(reopened).getByRole("button", { name: /Use the first available/ }));
    await waitFor(() => expect(current().global.image_generation.avatar_model_profile_id).toBeNull());
  });

  it("keeps descriptions and other controls unchanged while a save fails", async () => {
    let fail!: () => void;
    const gate = new Promise<void>((resolve) => { fail = resolve; });
    const update = vi.fn(async () => { await gate; return { status: "error" as const, error: { code: "conflict" as const, message: "private diagnostic", details: null } }; });
    await mount({ calls: { settingsUpdate: update } });
    const avatar = await screen.findByRole("switch", { name: "Avatar generation" });
    fireEvent.click(avatar);
    await waitFor(() => expect(update).toHaveBeenCalledTimes(1));
    expect(avatar.getAttribute("aria-checked")).toBe("true");
    expect(screen.getByText("Draw avatars from a description in the avatar picker")).toBeTruthy();
    expect(screen.getByText("Pictures of the moment, drawn from the chat")).toBeTruthy();
    expect(screen.getByRole("switch", { name: "Scene pictures" }).hasAttribute("disabled")).toBe(false);
    await act(async () => { fail(); });
    await waitFor(() => expect(avatar.getAttribute("aria-checked")).toBe("true"));
    expect(screen.queryByText("private diagnostic")).toBeNull();
    await waitFor(() => expect(avatar.hasAttribute("disabled")).toBe(false));
  });

  it("offers retry instead of treating a failed model list as empty", async () => {
    let failed = true;
    const read = vi.fn(() => failed ? { status: "error" as const, error: { code: "unavailable" as const, message: "diagnostic", details: null } } : ok({ models, default_model_id: null, revision: 1 }));
    await mount({ calls: { modelsList: read } });
    expect(await screen.findByRole("button", { name: "Try again" })).toBeTruthy();
    expect(screen.queryByText("No image models yet")).toBeNull();
    expect(screen.queryByRole("switch")).toBeNull();
    failed = false;
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("switch", { name: "Avatar generation" })).toBeTruthy();
  });

  it("serializes edits without replacing the page or its controls on settings events", async () => {
    let finish!: () => void;
    const gate = new Promise<void>((resolve) => { finish = resolve; });
    const { update, current, history } = await mount({ saveGate: gate, emitSettingsEvent: true });
    const avatar = await screen.findByRole("switch", { name: "Avatar generation" });
    const main = avatar.closest("main");
    if (main) main.scrollTop = 120;
    fireEvent.click(avatar);
    fireEvent.click(screen.getByRole("switch", { name: "Scene pictures" }));
    await waitFor(() => expect(update).toHaveBeenCalledTimes(1));
    await act(async () => { finish(); });
    await waitFor(() => expect(current().global.image_generation).toMatchObject({ avatar_enabled: false, scene_enabled: true }));
    expect(update.mock.calls.map(([request]) => request.expected_revision)).toEqual([7, 8]);
    expect(screen.getByRole("switch", { name: "Avatar generation" })).toBe(avatar);
    expect(avatar.closest("main")).toBe(main);
    expect(main?.scrollTop).toBe(120);
    expect(screen.queryByLabelText("Loading image settings")).toBeNull();
    expect(history.location.pathname).toBe("/settings/imageGeneration");
  });

  it("offers local management and local models on a compact desktop, and hides management on Android", async () => {
    await mount({ platform: "other" });
    expect(await screen.findByRole("button", { name: /^Engine/ })).toBeTruthy();
    await clickRow("Avatar model");
    expect(await within(await screen.findByRole("dialog")).findByRole("button", { name: /Local image/ })).toBeTruthy();
    cleanup();
    await mount();
    await screen.findByRole("switch", { name: "Avatar generation" });
    expect(screen.queryByRole("button", { name: /^Engine/ })).toBeNull();
  });

  it("installs the selected engine and upscaler through jobs that stay alive when the menu closes", async () => {
    const install = vi.fn(() => ok({ job_id: "image-job" }));
    const upscale = vi.fn(() => ok({ job_id: "upscale-job" }));
    const cancel = vi.fn(() => ok(null));
    const watch = vi.fn(() => ok(testJob("running")));
    await mount({ platform: "other", calls: { sdRuntimeInventory: () => ok({ installed: [], active: null }), sdRuntimeInstall: install, sdUpscalersInstall: upscale, jobCancel: cancel }, streams: { jobWatch: watch } });
    await clickRow("Engine");
    const page = await screen.findByRole("heading", { name: "Engine" });
    const drawer = await openEngineInstall();
    fireEvent.click(await within(drawer).findByRole("button", { name: "Install" }));
    await waitFor(() => expect(install).toHaveBeenCalledWith({ ...runtimeRef, then_register: null }));
    await waitFor(() => expect(watch).toHaveBeenCalled());
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click((await screen.findAllByRole("button", { name: "Install" })).at(-1)!);
    await waitFor(() => expect(upscale).toHaveBeenCalledTimes(1));
    fireEvent.click(within(page.closest("main")!).getByRole("button", { name: "Image generation" }));
    await waitFor(() => expect(page.isConnected).toBe(false));
    expect(cancel).not.toHaveBeenCalled();
    expect(screen.getByRole("switch", { name: "Avatar generation" })).toBeTruthy();
  });

  it("shows a shaped engine build skeleton until releases arrive", async () => {
    let finish!: () => void;
    const gate = new Promise<void>((resolve) => { finish = resolve; });
    await mount({ platform: "other", calls: { sdRuntimeReleases: async () => { await gate; return ok({ releases: [] }); } } });
    await clickRow("Engine");
    await openEngineInstall();
    expect(await screen.findByRole("status", { name: "Loading engine builds" })).toBeTruthy();
    expect(screen.queryByText("Loading...")).toBeNull();
    await act(async () => { finish(); });
    expect(await screen.findByText("No builds available for this computer")).toBeTruthy();
    expect(screen.queryByRole("status", { name: "Loading engine builds" })).toBeNull();
  });

  it("keeps Install locked for the running engine and shows Eidos download details until it settles", async () => {
    const install = vi.fn(() => ok({ job_id: "image-job" }));
    let emit!: (event: JobEvent) => void;
    const running = testJob("running");
    await mount({ platform: "other", calls: { sdRuntimeInventory: () => ok({ installed: [], active: null }), sdRuntimeInstall: install }, streams: { jobWatch: (_request, next) => { emit = next; return ok(running); } } });
    await clickRow("Engine");
    const drawer = await openEngineInstall();
    const button = await within(drawer).findByRole("button", { name: "Install" });
    fireEvent.click(button);
    expect(await screen.findByText(/9s left/)).toBeTruthy();
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await openEngineInstall();
    const installing = await screen.findByRole("button", { name: "Installing" });
    expect(installing.hasAttribute("disabled")).toBe(true);
    fireEvent.click(installing);
    expect(install).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("Downloading...")).toBeNull();
    const card = screen.getByText(`${runtimeRef.release} · Vulkan`).closest(".rounded-xl");
    expect(card?.parentElement?.className).not.toContain("rounded");
    await act(async () => { emit({ type: "failed", job: { ...running, state: "failed" } }); });
    await waitFor(() => expect(within(screen.getByRole("dialog")).getByRole("button", { name: "Install" }).hasAttribute("disabled")).toBe(false));
  });

  it("restores exact-build install locks from active jobs when reopening Engine without inventing an ETA", async () => {
    const running = { ...testJob("running"), subject_detail: { type: "sd_runtime_install" as const, ...runtimeRef, backend: "Vulkan" }, progress: { ...testJob("running").progress, bytes_per_second: null } };
    const { history } = await mount({ platform: "other", calls: {
      sdRuntimeInventory: () => ok({ installed: [], active: null }),
      sdRuntimeReleases: () => ok({ releases: [{ tag: runtimeRef.release, name: "Build 100", published_at: null, prerelease: false, assets: [runtimeAsset, { ...runtimeAsset, name: "cuda.zip", backend: "CUDA" }] }] }),
      jobsList: () => ok({ items: [running], next_cursor: null }),
    }, streams: { jobWatch: () => ok(running) } });
    await clickRow("Engine");
    const page = await screen.findByRole("heading", { name: "Engine" });
    await openEngineInstall();
    const installing = await screen.findByRole("button", { name: "Installing" });
    expect(installing.hasAttribute("disabled")).toBe(true);
    expect(screen.queryByText(/left$/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /^CUDA/ }));
    await waitFor(() => expect(screen.queryByRole("button", { name: "Installing" })).toBeNull());
    expect(screen.getAllByRole("button", { name: "Install" }).every((button) => !button.hasAttribute("disabled"))).toBe(true);
    await act(async () => { history.back(); });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(screen.getByRole("heading", { name: "Engine" })).toBe(page);
    fireEvent.click(within(page.closest("main")!).getByRole("button", { name: "Image generation" }));
    await clickRow("Engine");
    await openEngineInstall();
    expect((await screen.findByRole("button", { name: "Installing" })).hasAttribute("disabled")).toBe(true);
  });

  it("validates GPU budgets and saves the policy against the exact engine build", async () => {
    const save = vi.fn<NonNullable<MockCallHandlers["sdComputePolicySave"]>>((request) => ok({ runtime_release: request.release, runtime_asset: request.asset, backend: "Vulkan", supports_row_split: false, policy: request.policy, devices: [] }));
    await mount({ platform: "other", calls: {
      sdComputePolicyGet: () => ok({ runtime_release: runtimeRef.release, runtime_asset: runtimeRef.asset, backend: "Vulkan", supports_row_split: false, policy: { multi_gpu_enabled: false, single_gpu_device_id: null, gpu_device_ids: [], device_budgets_gib: [], split_mode: "layer" }, devices: [{ id: 0, name: "Test GPU", description: "GPU", total_bytes: 8 * 1024 ** 3, free_bytes: 6 * 1024 ** 3, budget_bytes: 0 }] }), sdComputePolicySave: save,
    } });
    await clickRow("Engine");
    await clickRow("GPUs");
    const input = await screen.findByRole("textbox", { name: "Memory budget for Test GPU" });
    fireEvent.change(input, { target: { value: "9" } });
    expect(screen.getByRole("button", { name: "Save" }).hasAttribute("disabled")).toBe(true);
    fireEvent.change(input, { target: { value: "2.5" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledWith({ ...runtimeRef, policy: { multi_gpu_enabled: false, single_gpu_device_id: null, gpu_device_ids: [], device_budgets_gib: [{ device_id: 0, gib: 2.5 }], split_mode: "layer" } }));
  });

  it("uses the catalog variant and engine pair, including the immediate-registration install result", async () => {
    const install = vi.fn(() => ok({ job_id: null, model_id: "registered" }));
    const probe = vi.fn(() => ok({ job_id: "image-job" }));
    const verdict = { status: "estimated_runnable" as const, method: "estimate", exact: false, scope: "model", placement_policy: "auto", elapsed_ms: null, reason: "diagnostic", estimate: null };
    await mount({ platform: "other", calls: { sdCatalog: () => ok({ runtime_supported: true, unsupported_reason: null, runtime_releases: [], profiles: [profile] }), sdModelInstall: install, sdRunnability: probe }, streams: { jobWatch: () => ok(testJob("succeeded", { type: "runnability", verdict })) } });
    await clickRow("Image models");
    await clickRow("FLUX model");
    expect((await screen.findAllByText("Estimated to fit")).length).toBeGreaterThan(0);
    expect(probe).toHaveBeenCalledWith(expect.objectContaining({ profile_id: "flux", variant_id: "q4", runtime_release: runtimeRef.release, runtime_asset: runtimeRef.asset, full_execution: false }));
    fireEvent.click(screen.getByRole("button", { name: /^Download / }));
    await waitFor(() => expect(install).toHaveBeenCalledWith({ profile_id: "flux", variant_id: "q4", ...runtimeRef }));
  });

  it("checks every variant on open, recommends the one that fits the GPU and asks before a CPU offload download", async () => {
    const estimateOf = (cpu: boolean) => ({ model_bytes: 1024, available_ram_bytes: 8192, plan_mode: "concurrent" as const, devices: [{ id: 0, name: "gpu", description: "Test GPU", total_bytes: 8192, free_bytes: 6144, budget_bytes: 6144 }], placements: [{ component: "dit" as const, params_bytes: 1024, compute_reserve_bytes: 128, targets: [cpu ? "CPU" : "Test GPU"], cpu, split: false }] });
    const verdictOf = (cpu: boolean) => ({ status: "estimated_runnable" as const, method: "estimate", exact: false, scope: "model", placement_policy: "auto", elapsed_ms: null, reason: "diagnostic", estimate: estimateOf(cpu) });
    const catalog = { ...profile, variants: [{ ...profile.variants[0]!, id: "q3", label: "Q3 (smaller)", download_bytes: 512, recommended: false }, { ...profile.variants[0]!, id: "q4", label: "Q4 (recommended)", download_bytes: 1024, recommended: true }] };
    const install = vi.fn(() => ok({ job_id: null, model_id: "registered" }));
    const probe = vi.fn((request: { variant_id: string }) => ok({ job_id: `fit-${request.variant_id}` }));
    await mount({ platform: "other", calls: { sdCatalog: () => ok({ runtime_supported: true, unsupported_reason: null, runtime_releases: [], profiles: [catalog] }), sdModelInstall: install, sdRunnability: probe as never }, streams: { jobWatch: ([request]) => ok(testJob("succeeded", { type: "runnability", verdict: verdictOf(request.job_id === "fit-q4") })) } });
    await clickRow("Image models");
    await clickRow("FLUX model");
    await waitFor(() => expect(probe).toHaveBeenCalledTimes(2));
    expect(await screen.findByText("Best fit")).toBeTruthy();
    expect(screen.getByRole("button", { name: /^Q3.*Fits your GPU/ }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByRole("button", { name: /^Q4.*Uses CPU offload/ }).getAttribute("aria-pressed")).toBe("false");
    expect(screen.queryByText(/\(recommended\)|\(smaller\)/)).toBeNull();
    expect(screen.getByText("Runs on your GPU")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /^Q4/ }));
    expect(await screen.findByText("Partly runs on the CPU")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /^Download / }));
    const dialogs = await screen.findAllByRole("dialog");
    expect(install).not.toHaveBeenCalled();
    fireEvent.click(within(dialogs.at(-1)!).getByRole("button", { name: "Download" }));
    await waitFor(() => expect(install).toHaveBeenCalledWith({ profile_id: "flux", variant_id: "q4", ...runtimeRef }));
  });

  it("uses a catalog skeleton while image models load", async () => {
    let finish!: () => void;
    const gate = new Promise<void>((resolve) => { finish = resolve; });
    await mount({ platform: "other", calls: { sdCatalog: async () => { await gate; return ok({ runtime_supported: true, unsupported_reason: null, runtime_releases: [], profiles: [profile] }); } } });
    await clickRow("Image models");
    expect(await screen.findByRole("status", { name: "Loading image models" })).toBeTruthy();
    expect(screen.queryByText("Loading...")).toBeNull();
    await act(async () => { finish(); });
    expect(await screen.findByRole("button", { name: /^FLUX model/ })).toBeTruthy();
    expect(screen.queryByRole("status", { name: "Loading image models" })).toBeNull();
  });

  it("keeps a catalog download locked after its drawer closes and releases it on cancellation", async () => {
    const install = vi.fn(() => ok({ job_id: "image-job", model_id: null }));
    let emit!: (event: JobEvent) => void;
    const running = testJob("running");
    await mount({ platform: "other", calls: { sdCatalog: () => ok({ runtime_supported: true, unsupported_reason: null, runtime_releases: [], profiles: [profile] }), sdModelInstall: install }, streams: { jobWatch: (_request, next) => { emit = next; return ok(running); } } });
    await clickRow("Image models");
    await clickRow("FLUX model");
    fireEvent.click(await screen.findByRole("button", { name: /^Download / }));
    await waitFor(() => expect(install).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(await screen.findByText(/9s left/)).toBeTruthy();
    await clickRow("FLUX model");
    const button = await screen.findByRole("button", { name: "Installing" });
    expect(button.hasAttribute("disabled")).toBe(true);
    fireEvent.click(button);
    expect(install).toHaveBeenCalledTimes(1);
    await act(async () => { emit({ type: "cancelled", job: { ...running, state: "cancelled" } }); });
    await waitFor(() => expect(screen.getByRole("button", { name: /^Download / }).hasAttribute("disabled")).toBe(false));
  });

  it("restores catalog download locks by model and variant without blocking another variant", async () => {
    const running = { ...testJob("running"), subject_detail: { type: "sd_model_install" as const, profile_id: profile.id, variant_id: "q4", runtime_release: runtimeRef.release, runtime_asset: runtimeRef.asset, display_name: "FLUX model · Q4" } };
    const catalog = { ...profile, variants: [...profile.variants, { ...profile.variants[0]!, id: "q8", label: "Q8", recommended: false }] };
    await mount({ platform: "other", calls: { sdCatalog: () => ok({ runtime_supported: true, unsupported_reason: null, runtime_releases: [], profiles: [catalog] }), jobsList: () => ok({ items: [running], next_cursor: null }) }, streams: { jobWatch: () => ok(running) } });
    await clickRow("Image models");
    expect(await screen.findByText("FLUX model · Q4")).toBeTruthy();
    await clickRow("FLUX model");
    expect((await screen.findByRole("button", { name: "Installing" })).hasAttribute("disabled")).toBe(true);
    await clickRow("Q8");
    expect((await screen.findByRole("button", { name: /^Download / })).hasAttribute("disabled")).toBe(false);
    await clickRow("Q4");
    expect(screen.getByRole("button", { name: "Installing" }).hasAttribute("disabled")).toBe(true);
  });

  it("assembles pinned HF components, estimates the bundle and submits the exact chosen files", async () => {
    const asset = { selection_id: "selection", profile_id: "flux", role: "diffusion_model" as const, model_id: "org/model", revision: "revision", relative_path: "model.gguf", format: "gguf", quantization: "Q4", size: 1024, sha256: "hash", architecture: "flux", gated: false };
    const install = vi.fn(() => ok({ job_id: "bundle-job", bundle_id: "bundle" }));
    const estimate = vi.fn(() => ok({ job_id: "image-job" }));
    await mount({ platform: "other", calls: {
      sdCatalog: () => ok({ runtime_supported: true, unsupported_reason: null, runtime_releases: [], profiles: [] }),
      hfImageBundleProfiles: () => ok({ profiles: [{ id: "flux", display_name: "FLUX architecture", family: "FLUX", description: "Architecture", minimum_runtime_build: null, required_roles: ["diffusion_model"], diffusion_markers: [], encoder_markers: [], encoder_parameter_billions: null, recommended_repositories: [], supports_text_to_image: true, supports_image_edit: false, max_reference_images: null, requires_reference_image: false, recommended_for_scenes: true, default_width: 1024, default_height: 1024, default_steps: 4, default_cfg: 1 }] }),
      hfImageBundleSearch: () => ok({ models: [{ model_id: "org/model", author: "org", downloads: 1, likes: 0, trending_score: 0, tags: [], pipeline_tag: null, last_modified: null }] }),
      hfImageBundleFiles: () => ok({ assets: [asset] }), hfImageBundleInstall: install, sdBundleRunnability: estimate,
    }, streams: { jobWatch: () => ok(testJob("succeeded", { type: "runnability", verdict: { status: "estimated_runnable", method: "estimate", exact: false, scope: "bundle", placement_policy: "auto", elapsed_ms: null, reason: "diagnostic", estimate: null } })) } });
    await clickRow("Image models");
    await clickRow("Build from Hugging Face files");
    await clickRow("FLUX architecture");
    await clickRow("Diffusion model");
    await clickRow("org/model");
    await clickRow("model.gguf");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(await screen.findByRole("button", { name: "Check fit" }));
    expect(await screen.findByText("Estimated to fit")).toBeTruthy();
    expect(estimate).toHaveBeenCalledWith(expect.objectContaining({ profile_id: "flux", diffusion_bytes: 1024, text_encoder_bytes: 0, runtime_release: runtimeRef.release, runtime_asset: runtimeRef.asset }));
    fireEvent.click(screen.getByRole("button", { name: "Download and create model" }));
    await waitFor(() => expect(install).toHaveBeenCalledWith({ profile_id: "flux", display_name: "FLUX architecture", runtime_release: runtimeRef.release, runtime_asset: runtimeRef.asset, assets: [asset] }));
  });

  it("edits shared LoRA keywords without changing an empty manual choice into undiscovered keywords", async () => {
    const lora = { filename: "style.safetensors", path: "style.safetensors", bytes_on_disk: 1000, keywords: ["style"], keyword_source: "manual" as const, architecture: "flux", architecture_source: "metadata" as const, compatibility: "unknown" as const };
    const save = vi.fn<NonNullable<MockCallHandlers["lorasUpdateKeywords"]>>((request) => ok({ keywords: request.keywords, source: "manual", sha256: null, architecture: "flux", architecture_source: "metadata", compatibility: "unknown" }));
    await mount({ platform: "other", calls: { lorasList: () => ok({ loras: [lora] }), lorasUpdateKeywords: save } });
    await clickRow("LoRA library");
    await clickRow("style.safetensors");
        fireEvent.change(await screen.findByRole("textbox", { name: "Edit keywords" }), { target: { value: "" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledWith({ path: "style.safetensors", profile_id: null, keywords: [] }));
  });

  it("counts the library, previews at most four trigger words and imports a file with a confirmation", async () => {
    const base = { path: "a.safetensors", filename: "a.safetensors", bytes_on_disk: 1000, keyword_source: "manual" as const, architecture: "flux", architecture_source: "metadata" as const, compatibility: "unknown" as const };
    const many = { ...base, keywords: ["one", "two", "three", "four", "five", "six"] };
    const fewer = { ...base, path: "b.safetensors", filename: "b.safetensors", keywords: ["solo"] };
    const pick = vi.fn(() => ok({ sources: [{ uri: "/loras/c.safetensors" }] }));
    const importLora = vi.fn(() => ok({ ...base, filename: "c.safetensors", path: "c.safetensors", keywords: [] }));
    await mount({ platform: "other", calls: { lorasList: () => ok({ loras: [many, fewer] }), filesPickOpen: pick, lorasImport: importLora } });
    await clickRow("LoRA library");
    expect(await screen.findByRole("tab", { name: /Library.*2/ })).toBeTruthy();
    expect(screen.getByText(/Adds: one, two, three, four and 2 more/)).toBeTruthy();
    expect(screen.getByText(/Adds: solo/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Import file" }));
    await waitFor(() => expect(importLora).toHaveBeenCalledWith({ source: { uri: "/loras/c.safetensors" } }));
    expect(await screen.findByText("c.safetensors is in your library")).toBeTruthy();
  });

  it("says why a LoRA that a model uses cannot be deleted and why a same-named import is refused", async () => {
    const lora = { filename: "style.safetensors", path: "style.safetensors", bytes_on_disk: 1000, keywords: [], keyword_source: "none" as const, architecture: null, architecture_source: "unknown" as never, compatibility: "unknown" as const };
    const conflict = { status: "error" as const, error: { code: "conflict" as const, message: "diagnostic", details: null } };
    await mount({ platform: "other", calls: { lorasList: () => ok({ loras: [lora] }), lorasDelete: () => conflict, filesPickOpen: () => ok({ sources: [{ uri: "/loras/style.safetensors" }] }), lorasImport: () => conflict } });
    await clickRow("LoRA library");
    fireEvent.click(await screen.findByRole("button", { name: "Import file" }));
    expect(await screen.findByText("A different LoRA with this name is already in your library.")).toBeTruthy();
    await clickRow("style.safetensors");
    fireEvent.click(await screen.findByRole("button", { name: /^Delete/ }));
    fireEvent.click(within((await screen.findAllByRole("dialog")).at(-1)!).getByRole("button", { name: "Delete" }));
    expect(await screen.findByText("Can't delete it yet")).toBeTruthy();
    expect(screen.getByText("An image model uses this LoRA. Remove it from that model first.")).toBeTruthy();
    expect(screen.queryByText("It changed somewhere else. Reload and try again.")).toBeNull();
  });

  it("downloads a CivitAI version by its IDs and keeps search filters in the request", async () => {
    const download = vi.fn(() => ok({ job_id: "image-job" }));
    const search = vi.fn<NonNullable<MockCallHandlers["civitaiSearch"]>>(() => ok({ items: [{ id: 10, name: "Film style", nsfw: false, nsfw_level: 1, creator_username: "Artist", download_count: 2, thumbs_up_count: 1, preview_image: null, base_models: ["FLUX"], latest_version_id: 20 }], next_cursor: null }));
    await mount({ platform: "other", calls: {
      lorasList: () => ok({ loras: [] }), civitaiAuthStatus: () => ok({ saved: true, valid: true, error_kind: null }), civitaiSearch: search,
      civitaiModel: () => ok({ id: 10, name: "Film style", description: null, nsfw: false, nsfw_level: 1, creator_username: "Artist", download_count: 2, thumbs_up_count: 1, tags: [], versions: [{ id: 20, name: "v1", base_model: "FLUX", architecture: "flux", published_at: null, trained_words: ["film"], images: [], files: [{ id: 30, name: "film.safetensors", size_kb: 1, primary: true, format: "safetensors", fp: null, sha256: null }] }] }), civitaiLoraDownload: download,
    }, streams: { jobWatch: () => ok(testJob("running")) } });
    await clickRow("LoRA library");
    fireEvent.click(await screen.findByRole("tab", { name: "Browse CivitAI" }));
    await clickRow("Film style");
    fireEvent.click(await screen.findByRole("button", { name: "Download" }));
    await waitFor(() => expect(download).toHaveBeenCalledWith({ model_id: 10, version_id: 20, file_id: 30 }));
    expect(search).toHaveBeenCalledWith({ query: null, sort: "Highest Rated", period: "AllTime", base_models: [], cursor: null, limit: null });
  });
});
