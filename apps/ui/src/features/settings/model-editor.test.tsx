import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import type { ModelSaveRequest, ModelView, ProviderAccountView, SettingsView } from "@/api/generated/bindings";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";
import { formatSequences } from "@/entities/model/model-editor-sequences";
import full from "./fixtures/model-profile-config.json";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));
const account: ProviderAccountView = { id: "provider", provider_kind: "openrouter", protocol: "open_ai_compatible", label: "My provider", base_url: null, enabled: true, streaming_enabled: true, allow_invalid_tls: false, api_key_set: true, config: {}, revision: 1, created_at: 0, updated_at: 0 };
const gpuDevices = [0, 1].map((index) => ({ index, name: `GPU ${index}`, description: "", backend: "CUDA", device_type: "Gpu", memory_free: 8 * 1024 ** 3, memory_total: 8 * 1024 ** 3 }));
const model: ModelView = { id: "model", provider_account_id: account.id, display_name: "Editor model", external_model_id: "vendor/model", kind: "chat", input_scopes: ["text", "image"], output_scopes: ["text"], config: full, revision: 9, created_at: 0, updated_at: 0 };
const settings = { sampler_defaults: { chat_parameters: { temperature: 0.65 }, llama_cpp: { sampler: {} }, stable_diffusion: { cpp: {} } }, revision: 1 } as unknown as SettingsView;
async function mount(path = "/settings/models/model", calls: MockCallHandlers = {}, phone = false) {
  if (phone) vi.spyOn(window, "matchMedia").mockImplementation((query) => ({ matches: query.includes("max-width"), media: query, onchange: null, addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {}, dispatchEvent: () => false }));
  const transport = createMockTransport({ calls: {
    appStatus: () => ({ status: "ok", data: { ...mockAppStatus, ui_state: { onboarding: { completed: true } } } }),
    providerAccountsList: () => ({ status: "ok", data: [account] }), providerCatalog: () => ({ status: "ok", data: { providers: [] } }),
    providerModels: () => ({ status: "ok", data: [] }), modelGet: () => ({ status: "ok", data: model }),
    llamaDevices: () => ({ status: "ok", data: { devices: gpuDevices } }),
    modelsList: () => ({ status: "ok", data: { models: [model], default_model_id: null, revision: 1 } }), settingsGet: () => ({ status: "ok", data: settings }),
    modelUsage: () => ({ status: "ok", data: { characters: [{ id: "c", name: "Alice" }], groups: [{ id: "g", name: "Story group" }], roles: ["default", "dynamic_memory", "group_speaker"] } }),
    ...calls,
  } });
  const runtime = await createAppRuntime({ transport, history: createMemoryHistory({ initialEntries: [path] }), i18n: await createTestI18n("en") });
  await act(async () => { render(<App runtime={runtime} />); });
  return runtime;
}
async function click(name: string | RegExp) { fireEvent.click(await screen.findByRole("button", { name }, { timeout: 3000 })); }
afterEach(cleanup);

describe("model editor page", () => {
  it.each([false, true])("shows the save bar only for changes and hides it after reverting, discarding or saving (phone: %s)", async (phone) => {
    const save = vi.fn<NonNullable<MockCallHandlers["modelSave"]>>((request) => ({ status: "ok", data: { ...model, display_name: request.model.display_name, config: request.model.config } }));
    await mount(undefined, { modelSave: save }, phone);
    const name = await screen.findByRole("textbox", { name: "Name" });
    expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
    fireEvent.change(name, { target: { value: "Changed name" } });
    expect(await screen.findByRole("button", { name: "Save" })).toBeTruthy();
    fireEvent.change(name, { target: { value: model.display_name } });
    await waitFor(() => expect(screen.queryByRole("button", { name: "Save" })).toBeNull());
    fireEvent.change(name, { target: { value: "Changed name" } });
    await click("Discard");
    await waitFor(() => expect(screen.queryByRole("button", { name: "Save" })).toBeNull());
    expect((name as HTMLInputElement).value).toBe(model.display_name);
    fireEvent.change(name, { target: { value: "Changed name" } });
    await click("Save");
    await waitFor(() => expect(save).toHaveBeenCalledOnce());
    await waitFor(() => expect(screen.queryByRole("button", { name: "Save" })).toBeNull());
    expect(screen.queryByRole("button", { name: "Discard" })).toBeNull();
    expect((name as HTMLInputElement).value).toBe("Changed name");
  });
  it("saves a feature override without changing chats or other sections", async () => {
    let saved: ModelSaveRequest | undefined;
    await mount(undefined, { modelSave: (request) => { saved = request; return { status: "ok", data: { ...model, config: request.model.config } }; } });
    await click(/^Generation/);
    fireEvent.click(await screen.findByRole("button", { name: "Settings for" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: /^Creation helper/ }));
    fireEvent.change(await screen.findByRole("slider", { name: "Temperature" }), { target: { value: "0.35" } });
    await click("Save");
    await waitFor(() => expect(saved).toBeDefined());
    expect(saved?.model.config).toMatchObject({ chat_parameters: full.chat_parameters, llama_cpp: full.llama_cpp, stable_diffusion: full.stable_diffusion, feature_parameters: { creation_helper: { parameters: { temperature: { kind: "set", value: 0.35 } } }, dynamic_memory: full.feature_parameters.dynamic_memory } });
    expect(saved?.expected_revision).toBe(9);
  });
  it("resets a feature, shows the inherited chat value and discards edits", async () => {
    await mount();
    await click(/^Generation/);
    fireEvent.click(await screen.findByRole("button", { name: "Settings for" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: /^Dynamic memory/ }));
    fireEvent.click(screen.getAllByRole("button", { name: "Reset" }).find((button) => !button.hasAttribute("disabled"))!);
    expect((await screen.findByRole("slider", { name: "Temperature" }) as HTMLInputElement).value).toBe("0.7");
    expect(screen.queryByRole("button", { name: "Reset Temperature" })).toBeNull();
    await click("Discard");
    expect(await screen.findByRole("button", { name: "Reset Temperature" })).toBeTruthy();
    await waitFor(() => expect(screen.queryByRole("button", { name: "Save" })).toBeNull());
  });
  it("clamps typed slider values on Enter and cancels them on Escape", async () => {
    await mount(); await click(/^Generation/);
    const value = await screen.findByRole("textbox", { name: "Temperature value" });
    act(() => value.focus()); fireEvent.change(value, { target: { value: "99" } }); fireEvent.keyDown(value, { key: "Enter" });
    expect((await screen.findByRole("slider", { name: "Temperature" }) as HTMLInputElement).value).toBe("2");
    act(() => value.focus()); fireEvent.change(value, { target: { value: "0.2" } }); fireEvent.keyDown(value, { key: "Escape" });
    expect((await screen.findByRole("slider", { name: "Temperature" }) as HTMLInputElement).value).toBe("2");
  });
  it.each(["page", "list"])("shows model usage before deleting from the %s", async (place) => {
    const remove = vi.fn<NonNullable<MockCallHandlers["modelDelete"]>>(() => ({ status: "ok", data: null }));
    const runtime = await mount(place === "page" ? undefined : "/settings/models", { modelDelete: remove });
    if (place === "page") { await click("More"); fireEvent.click(await screen.findByRole("menuitem", { name: "Delete" })); }
    else { fireEvent.contextMenu(await screen.findByRole("button", { name: /Editor model/ })); await click(/Characters using it fall back/); }
    const dialog = await screen.findByRole("dialog", { name: "Delete Editor model?" });
    for (const text of ["Alice", "Story group", "Default model", "Dynamic memory", "Group speaker"]) expect(within(dialog).getByText(text)).toBeTruthy();
    expect(remove).not.toHaveBeenCalled();
    await waitFor(() => expect((within(dialog).getByRole("button", { name: "Delete permanently" }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(within(dialog).getByRole("button", { name: "Delete permanently" }));
    await waitFor(() => expect(remove).toHaveBeenCalledWith(expect.objectContaining({ model_id: model.id, expected_revision: 9 })));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/settings/models"));
  });
  it("does not offer deletion if fetching usage fails", async () => {
    const remove = vi.fn();
    await mount(undefined, { modelUsage: () => ({ status: "error", error: { code: "internal", message: "unavailable", details: null } }), modelDelete: remove });
    await click("More"); fireEvent.click(await screen.findByRole("menuitem", { name: "Delete" }));
    await screen.findByText("Couldn't load this");
    expect(screen.queryByRole("dialog", { name: "Delete Editor model?" })).toBeNull();
    expect(remove).not.toHaveBeenCalled();
  });
  it("adds a manually entered model with default naming", async () => {
    let saved: ModelSaveRequest | undefined;
    const runtime = await mount("/settings/models/new", { modelSave: (request) => { saved = request; return { status: "ok", data: { ...model, ...request.model, id: "created", revision: 1 } }; } });
    await click("Choose a model");
    fireEvent.change(screen.getByPlaceholderText(/Search .* models/), { target: { value: "vendor/new-model" } });
    await click(/Use vendor\/new-model/); await click("Add model");
    await waitFor(() => expect(saved?.model.display_name).toBe("new-model"));
    expect(saved?.model.id).toBeNull(); expect(saved?.expected_revision).toBeNull();
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/settings/models/created"));
  });
  it("uses phone section sub-views and returns to Overview without losing edits", async () => {
    await mount(undefined, {}, true);
    expect(screen.queryByRole("navigation", { name: "Model settings" })).toBeNull();
    await click(/^Generation/);
    fireEvent.change(await screen.findByRole("slider", { name: "Temperature" }), { target: { value: "0.25" } });
    await click("Editor model");
    expect(await screen.findByLabelText("Name")).toBeTruthy();
    await click(/^Generation/);
    expect((await screen.findByRole("slider", { name: "Temperature" }) as HTMLInputElement).value).toBe("0.25");
  });
  it("shows Settings sampler defaults without persisting them as overrides", async () => {
    const config = structuredClone(full);
    config.chat_parameters = {} as typeof config.chat_parameters;
    config.feature_parameters = {} as typeof config.feature_parameters;
    let saved: ModelSaveRequest | undefined;
    await mount(undefined, {
      modelGet: () => ({ status: "ok", data: { ...model, config } }),
      modelSave: (request) => { saved = request; return { status: "ok", data: { ...model, config: request.model.config } }; },
    });
    await click(/^Generation/);
    expect((await screen.findByRole("slider", { name: "Temperature" }) as HTMLInputElement).value).toBe("0.65");
    expect((screen.getByLabelText("Top K") as HTMLInputElement).placeholder).toBe("Off");
    for (const name of ["Frequency penalty", "Presence penalty"]) {
      const value = screen.getByRole("textbox", { name: `${name} value` }) as HTMLInputElement;
      expect(value.value).toBe("0.00");
      expect((screen.getByRole("slider", { name }) as HTMLInputElement).value).toBe("0");
      act(() => value.focus()); act(() => value.blur());
      expect(screen.queryByRole("button", { name: `Reset ${name}` })).toBeNull();
    }
    expect(screen.queryByRole("button", { name: "Reset Temperature" })).toBeNull();
    expect(screen.queryByLabelText("Context length")).toBeNull();
    fireEvent.click(await screen.findByRole("button", { name: "Settings for" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: /^Help me reply/ }));
    fireEvent.change(await screen.findByRole("slider", { name: "Top P" }), { target: { value: "0.8" } });
    await click("Save");
    await waitFor(() => expect(saved).toBeDefined());
    expect(saved?.model.config.chat_parameters).toEqual({});
    expect(saved?.model.config.feature_parameters).toEqual({ help_me_reply: { parameters: { top_p: { kind: "set", value: 0.8 } } } });
  });
  it("uses runtime context recommendations and saves local speculative settings", async () => {
    let saved: ModelSaveRequest | undefined;
    const local = { ...model, external_model_id: "/models/example.gguf" };
    await mount(undefined, {
      providerAccountsList: () => ({ status: "ok", data: [{ ...account, provider_kind: "llamacpp" }] }),
      modelGet: () => ({ status: "ok", data: local }),
      llamaContextInfo: () => ({ status: "ok", data: { max_context_length: 32768, recommended_context_length: 6144, available_memory_bytes: null, available_vram_bytes: null, model_size_bytes: null, layer_count: 32, max_gpu_layers: 33, supports_gpu_offload: true, selected_gpu_device_ids: [], per_device_vram: null, estimated_placement: null, memory_fit: [] } }),
      modelSave: (request) => { saved = request; return { status: "ok", data: { ...local, config: request.model.config } }; },
    });
    expect(await screen.findByRole("button", { name: "example.gguf" })).toBeTruthy();
    expect(screen.queryByText("/models/example.gguf")).toBeNull();
    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), { target: { value: "My custom model" } });
    expect(screen.getByRole("button", { name: "example.gguf" })).toBeTruthy();
    await click("Use recommended");
    await click(/^Context & memory/);
    const contextSlider = await screen.findByRole("slider", { name: "Context length" });
    expect((contextSlider as HTMLInputElement).value).toBe("6144");
    expect(contextSlider.getAttribute("max")).toBe("32768");
    expect(contextSlider.getAttribute("min")).toBe("2048");
    expect(contextSlider.getAttribute("step")).toBe("2048");
    expect((await screen.findByRole("textbox", { name: "Context length value" }) as HTMLInputElement).value).toBe("6K");
    expect(await screen.findByText("The model supports up to 32K")).toBeTruthy();
    expect(await screen.findByRole("combobox", { name: "Key cache type" })).toBeTruthy();
    expect(await screen.findByRole("combobox", { name: "Value cache type" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Use 6K" })).toBeNull();
    await click(/^Speculative decoding/);
    fireEvent.change(await screen.findByLabelText("DFlash draft tokens"), { target: { value: "12" } });
    await click("Save");
    await waitFor(() => expect(saved).toBeDefined());
    expect(saved?.model.config).toMatchObject({ chat_parameters: { context_length: 6144 }, llama_cpp: { dflash_draft_tokens: 12 } });
    expect(saved?.model.external_model_id).toBe("/models/example.gguf");
    expect(saved?.model.display_name).toBe("My custom model");
  });
  it.each([false, true])("shows a context skeleton until memory estimates arrive (phone: %s)", async (phone) => {
    let release: (() => void) | undefined;
    const ready = new Promise<void>((resolve) => { release = resolve; });
    await mount(undefined, {
      providerAccountsList: () => ({ status: "ok", data: [{ ...account, provider_kind: "llamacpp" }] }),
      modelGet: () => ({ status: "ok", data: { ...model, external_model_id: "/models/example.gguf", config: { ...full, chat_parameters: {}, llama_cpp: {} } } }),
      llamaContextInfo: async () => {
        await ready;
        return { status: "ok", data: { max_context_length: 32768, recommended_context_length: 6144, available_memory_bytes: null, available_vram_bytes: 12 * 1024 ** 3, model_size_bytes: 5 * 1024 ** 3, layer_count: 32, max_gpu_layers: 33, supports_gpu_offload: true, selected_gpu_device_ids: [], per_device_vram: null, estimated_placement: null, memory_fit: [{ gpu: true, devices: ["Actual GPU"], capacity_bytes: 12 * 1024 ** 3, available_bytes: 12 * 1024 ** 3, model_bytes: 5 * 1024 ** 3, context_bytes: 1024 ** 3, working_bytes: 512 * 1024 ** 2 }] } };
      },
    }, phone);
    await click(/^Context & memory/);
    const loading = await screen.findByRole("status", { name: "Loading context and memory estimates" });
    expect(loading.closest("[aria-busy=true]")).toBeTruthy();
    expect(screen.queryByRole("slider", { name: "Context length" })).toBeNull();
    expect(screen.queryByRole("spinbutton", { name: "Context length" })).toBeNull();
    expect(screen.queryByText("Loading...")).toBeNull();
    act(() => release?.());
    expect(await screen.findByRole("slider", { name: "Context length" })).toHaveProperty("value", "6144");
    expect(await screen.findByText(/Actual GPU.*total/)).toBeTruthy();
    expect(screen.queryByRole("status", { name: "Loading context and memory estimates" })).toBeNull();
    expect(loading.isConnected).toBe(false);
    expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
  });

  it("keeps an exact context recommendation and clamps typed values to the model limit", async () => {
    let saved: ModelSaveRequest | undefined;
    await mount(undefined, {
      providerAccountsList: () => ({ status: "ok", data: [{ ...account, provider_kind: "llamacpp" }] }),
      modelGet: () => ({ status: "ok", data: { ...model, config: { ...full, chat_parameters: { context_length: 16384 } } } }),
      llamaContextInfo: () => ({ status: "ok", data: { max_context_length: 32768, recommended_context_length: 19541, available_memory_bytes: null, available_vram_bytes: null, model_size_bytes: null, layer_count: 32, max_gpu_layers: 33, supports_gpu_offload: true, selected_gpu_device_ids: [], per_device_vram: null, estimated_placement: null, memory_fit: [] } }),
      modelSave: (request) => { saved = request; return { status: "ok", data: { ...model, config: request.model.config } }; },
    });
    await click(/^Context & memory/);
    await click("Use 19K");
    expect((await screen.findByRole("textbox", { name: "Context length value" }) as HTMLInputElement).value).toBe("19K");
    await click("Save");
    await waitFor(() => expect(saved?.model.config).toMatchObject({ chat_parameters: { context_length: 19541 } }));
    const input = screen.getByRole("textbox", { name: "Context length value" });
    act(() => input.focus());
    expect((input as HTMLInputElement).value).toBe("19541");
    fireEvent.change(input, { target: { value: "40000" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect((screen.getByRole("textbox", { name: "Context length value" }) as HTMLInputElement).value).toBe("32K");
    await click("Save");
    await waitFor(() => expect(saved?.model.config).toMatchObject({ chat_parameters: { context_length: 32768 } }));
  });
  it("updates the memory breakdown for the selected context without saving an automatic override", async () => {
    const requests: number[] = [];
    let release: (() => void) | undefined;
    await mount(undefined, {
      providerAccountsList: () => ({ status: "ok", data: [{ ...account, provider_kind: "llamacpp" }] }),
      modelGet: () => ({ status: "ok", data: { ...model, config: { ...full, chat_parameters: { context_length: 8192 }, llama_cpp: {} } } }),
      llamaContextInfo: async (request) => {
        const context = request.context_length ?? 8192;
        requests.push(context);
        if (context === 16384) await new Promise<void>((resolve) => { release = resolve; });
        return { status: "ok", data: { max_context_length: 131072, recommended_context_length: 43008, available_memory_bytes: null, available_vram_bytes: 12 * 1024 ** 3, model_size_bytes: 5 * 1024 ** 3, layer_count: 32, max_gpu_layers: 33, supports_gpu_offload: true, selected_gpu_device_ids: [], per_device_vram: null, estimated_placement: null, memory_fit: [{ gpu: true, devices: ["Actual GPU"], capacity_bytes: 12 * 1024 ** 3, available_bytes: 12 * 1024 ** 3, model_bytes: 5 * 1024 ** 3, context_bytes: context / 8192 * 1024 ** 3, working_bytes: 512 * 1024 ** 2 }] } };
      },
    });
    await click(/^Context & memory/);
    expect(await screen.findByText(/Actual GPU.*total/)).toBeTruthy();
    expect(await screen.findByText("Cache type: f16")).toBeTruthy();
    expect(within(screen.getByRole("group", { name: "Estimated memory for this model" })).getByText("7.0 GB")).toBeTruthy();
    expect(await screen.findByRole("img", { name: /context 1.*GB/i })).toBeTruthy();
    await waitFor(() => expect(screen.queryByRole("button", { name: "Save" })).toBeNull());
    const slider = await screen.findByRole("slider", { name: "Context length" });
    fireEvent.change(slider, { target: { value: "16384" } });
    fireEvent.change(slider, { target: { value: "24576" } });
    fireEvent.change(slider, { target: { value: "16384" } });
    await waitFor(() => expect(release).toBeDefined());
    const card = slider.closest(".overflow-hidden.rounded-xl") as HTMLElement;
    expect(within(card).queryByRole("status")).toBeNull();
    expect(screen.getByRole("img", { name: /context 1.*GB/i })).toBeTruthy();
    expect(requests.filter((value) => value === 16384)).toHaveLength(1);
    expect(requests).not.toContain(24576);
    act(() => release?.());
    expect(await screen.findByRole("img", { name: /context 2.*GB/i })).toBeTruthy();
    expect(requests).toContain(16384);
    expect(screen.queryByText(/^Available GPU memory:/)).toBeNull();
  });
  it("only exposes K and V cache types in Custom mode and saves a valid pair", async () => {
    let saved: ModelSaveRequest | undefined;
    await mount(undefined, {
      providerAccountsList: () => ({ status: "ok", data: [{ ...account, provider_kind: "llamacpp" }] }),
      modelGet: () => ({ status: "ok", data: { ...model, config: { ...full, llama_cpp: { kv_type: "q8_0" } } } }),
      llamaContextInfo: () => ({ status: "error", error: { code: "internal", message: "offline", details: null } }),
      modelSave: (request) => { saved = request; return { status: "ok", data: { ...model, config: request.model.config } }; },
    });
    await click(/^Context & memory/);
    const select = await screen.findByRole("combobox", { name: "Cache type" });
    expect(screen.queryByRole("combobox", { name: "Key cache type" })).toBeNull();
    expect(screen.queryByRole("combobox", { name: "Value cache type" })).toBeNull();
    fireEvent.click(select);
    fireEvent.click(await screen.findByRole("option", { name: "Custom" }));
    expect(await screen.findByRole("combobox", { name: "Value cache type" })).toBeTruthy();
    fireEvent.click(await screen.findByRole("combobox", { name: "Key cache type" }));
    fireEvent.click(await screen.findByRole("option", { name: "q4_0" }));
    await click("Save");
    await waitFor(() => expect(saved?.model.config).toMatchObject({ llama_cpp: { kv_type_k: "q4_0", kv_type_v: "q8_0" } }));
    fireEvent.click(screen.getByRole("combobox", { name: "Cache type" }));
    fireEvent.click(await screen.findByRole("option", { name: "f16" }));
    expect(screen.queryByRole("combobox", { name: "Key cache type" })).toBeNull();
    await click("Save");
    await waitFor(() => expect(saved?.model.config).toMatchObject({ llama_cpp: { kv_type: "f16" } }));
    expect((saved?.model.config.llama_cpp as Record<string, unknown> | undefined)?.kv_type_k).toBeUndefined();
    expect((saved?.model.config.llama_cpp as Record<string, unknown> | undefined)?.kv_type_v).toBeUndefined();
  });
  it.each([false, true])("updates the single GPU and CPU layer distribution with either slider (phone: %s)", async (phone) => {
    let saved: ModelSaveRequest | undefined;
    await mount(undefined, {
      providerAccountsList: () => ({ status: "ok", data: [{ ...account, provider_kind: "llamacpp" }] }),
      modelGet: () => ({ status: "ok", data: { ...model, config: { ...full, llama_cpp: { gpu_layers: 24, multi_gpu_enabled: false } } } }),
      modelSave: (request) => { saved = request; return { status: "ok", data: { ...model, config: request.model.config } }; },
      llamaDevices: () => ({ status: "ok", data: { devices: [{ index: 3, name: "CUDA device 3", description: "", backend: "CUDA", device_type: "Gpu", memory_free: 8 * 1024 ** 3, memory_total: 8 * 1024 ** 3 }] } }),
      llamaContextInfo: (request) => {
        const layers = request.target.type === "draft" ? request.target.settings.gpu_layers ?? 24 : 24;
        return { status: "ok", data: { max_context_length: 32768, recommended_context_length: 6144, available_memory_bytes: null, available_vram_bytes: null, model_size_bytes: null, layer_count: 32, max_gpu_layers: 33, supports_gpu_offload: true, selected_gpu_device_ids: null, per_device_vram: null, estimated_placement: { total_gpu_layers: layers, per_device_layers: [layers] }, memory_fit: [] } };
      },
    }, phone);
    await click(/^Context & memory/);
    const bar = await screen.findByRole("img", { name: "Estimated GPU layer distribution" });
    expect(bar.textContent).toContain("CUDA device 3");
    expect(bar.textContent).toContain("24 layers");
    expect(bar.textContent).toContain("CPU · 9 layers");
    expect(screen.queryByRole("button", { name: /^GPU/ })).toBeNull();
    expect(screen.getByRole("region", { name: "GPU" }).contains(bar)).toBe(true);
    expect(screen.queryByRole("switch", { name: "Use more than one GPU" })).toBeNull();
    const device = await screen.findByRole("combobox", { name: "Run on" });
    expect(device.textContent).toContain("CUDA device 3");
    expect(device.matches(":disabled")).toBe(true);
    expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
    const slider = await screen.findByRole("slider", { name: "GPU layers" });
    const cpu = screen.getByRole("slider", { name: "Keep on the CPU" });
    expect(cpu).toHaveProperty("value", "9");
    fireEvent.change(slider, { target: { value: "0" } });
    await waitFor(() => expect(bar.textContent).toBe("CPU · 33 layers"));
    expect(cpu).toHaveProperty("value", "33");
    fireEvent.change(slider, { target: { value: "33" } });
    await waitFor(() => expect(bar.textContent).toBe("CUDA device 3 · 33 layers"));
    expect(bar.textContent).not.toContain("CPU");
    expect(cpu).toHaveProperty("value", "0");
    fireEvent.change(cpu, { target: { value: "5" } });
    expect(slider).toHaveProperty("value", "28");
    await waitFor(() => expect(bar.textContent).toContain("CPU · 5 layers"));
    await click("Save");
    await waitFor(() => expect(saved?.model.config).toMatchObject({ llama_cpp: { gpu_layers: 28 } }));
    expect(saved?.model.config.llama_cpp).not.toHaveProperty("cpu_layers");
  });
  it("groups GPU controls, converts GiB to bytes and preserves the other settings", async () => {
    let saved: ModelSaveRequest | undefined;
    await mount(undefined, {
      providerAccountsList: () => ({ status: "ok", data: [{ ...account, provider_kind: "llamacpp" }] }),
      modelGet: () => ({ status: "ok", data: { ...model, config: { ...full, llama_cpp: { ...full.llama_cpp, multi_gpu_enabled: false } } } }),
      llamaContextInfo: () => ({ status: "ok", data: { max_context_length: 32768, recommended_context_length: 6144, available_memory_bytes: null, available_vram_bytes: null, model_size_bytes: null, layer_count: 32, max_gpu_layers: 33, supports_gpu_offload: true, selected_gpu_device_ids: [], per_device_vram: null, estimated_placement: { total_gpu_layers: 24, per_device_layers: [24] }, memory_fit: [] } }),
      llamaDevices: () => ({ status: "ok", data: { devices: gpuDevices } }),
      modelSave: (request) => { saved = request; return { status: "ok", data: { ...model, config: request.model.config } }; },
    });
    await click(/^Context & memory/);
    expect((await screen.findByRole("slider", { name: "GPU layers" })).getAttribute("max")).toBe("33");
    expect(screen.getByRole("slider", { name: "GPU layers" }).matches(":disabled")).toBe(false);
    expect(screen.queryByLabelText("Distribution")).toBeNull();
    fireEvent.click(screen.getByRole("switch", { name: "Use more than one GPU" }));
    const limit = await screen.findByRole("spinbutton", { name: "VRAM limit per GPU" });
    fireEvent.change(limit, { target: { value: "12.5" } });
    await click("Save");
    await waitFor(() => expect(saved?.model.config).toMatchObject({ llama_cpp: { multi_gpu_enabled: true, priority_vram_limit_bytes: 13421772800 } }));
  });
  it.each([false, true])("uses one KV placement control on a single GPU (phone: %s)", async (phone) => {
    let saved: ModelSaveRequest | undefined;
    await mount(undefined, {
      providerAccountsList: () => ({ status: "ok", data: [{ ...account, provider_kind: "llamacpp" }] }),
      modelGet: () => ({ status: "ok", data: { ...model, config: { ...full, llama_cpp: { kv_placement: "split", offload_kqv: false, multi_gpu_enabled: false } } } }),
      llamaContextInfo: () => ({ status: "ok", data: { max_context_length: 32768, recommended_context_length: 6144, available_memory_bytes: null, available_vram_bytes: null, model_size_bytes: null, layer_count: 32, max_gpu_layers: 33, supports_gpu_offload: true, selected_gpu_device_ids: [], per_device_vram: null, estimated_placement: null, memory_fit: [] } }),
      modelSave: (request) => { saved = request; return { status: "ok", data: { ...model, config: request.model.config } }; },
    }, phone);
    await click(/^Context & memory/);
    const placement = await screen.findByRole("combobox", { name: "KV cache placement" });
    expect(placement.textContent).toContain("GPU");
    expect(screen.queryByRole("switch", { name: "Offload attention to the GPU" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
    fireEvent.click(placement);
    expect(await screen.findByRole("option", { name: "Automatic" })).toBeTruthy();
    expect(screen.getByRole("option", { name: "System RAM" })).toBeTruthy();
    expect(screen.getByRole("option", { name: "GPU" })).toBeTruthy();
    expect(screen.queryByRole("option", { name: "Split with layers" })).toBeNull();
    expect(screen.queryByRole("option", { name: "Pin to one GPU" })).toBeNull();
    fireEvent.click(screen.getByRole("option", { name: "System RAM" }));
    await click("Save");
    await waitFor(() => expect(saved?.model.config).toMatchObject({ llama_cpp: { kv_placement: "system_ram", offload_kqv: false, multi_gpu_enabled: false } }));
    fireEvent.click(placement);
    fireEvent.click(await screen.findByRole("option", { name: "Automatic" }));
    expect(await screen.findByText("Uses system RAM")).toBeTruthy();
    await click("Discard");
    expect((await screen.findByRole("combobox", { name: "KV cache placement" })).textContent).toContain("System RAM");
  });
  it("shows detailed KV placement only with multiple GPUs and restores a saved split choice", async () => {
    await mount(undefined, {
      providerAccountsList: () => ({ status: "ok", data: [{ ...account, provider_kind: "llamacpp" }] }),
      modelGet: () => ({ status: "ok", data: { ...model, config: { ...full, llama_cpp: { kv_placement: "split", multi_gpu_enabled: false } } } }),
      llamaContextInfo: () => ({ status: "ok", data: { max_context_length: 32768, recommended_context_length: 6144, available_memory_bytes: null, available_vram_bytes: null, model_size_bytes: null, layer_count: 32, max_gpu_layers: 33, supports_gpu_offload: true, selected_gpu_device_ids: [], per_device_vram: null, estimated_placement: null, memory_fit: [] } }),
      llamaDevices: () => ({ status: "ok", data: { devices: gpuDevices } }),
    });
    await click(/^Context & memory/);
    await waitFor(() => expect(screen.queryByRole("status", { name: "Loading context and memory estimates" })).toBeNull());
    expect(screen.queryByRole("img", { name: "Estimated GPU layer distribution" })).toBeNull();
    fireEvent.click(await screen.findByRole("switch", { name: "Use more than one GPU" }));
    const placement = await screen.findByRole("combobox", { name: "KV cache placement" });
    expect(placement.textContent).toContain("Split with layers");
    fireEvent.click(placement);
    expect(await screen.findByRole("option", { name: "Pin to one GPU" })).toBeTruthy();
    expect(screen.getByRole("option", { name: "Split with layers" })).toBeTruthy();
    expect(screen.queryByRole("option", { name: "GPU" })).toBeNull();
    fireEvent.keyDown(placement, { key: "Escape" });
    await click("Discard");
    expect((await screen.findByRole("combobox", { name: "KV cache placement" })).textContent).toContain("GPU");
    await waitFor(() => expect(screen.queryByRole("button", { name: "Save" })).toBeNull());
  });
  it("hides the GPU section and disables GPU placement on a CPU backend without changing saved settings", async () => {
    await mount(undefined, {
      providerAccountsList: () => ({ status: "ok", data: [{ ...account, provider_kind: "llamacpp" }] }),
      llamaDevices: () => ({ status: "ok", data: { devices: gpuDevices } }),
      llamaContextInfo: () => ({ status: "ok", data: { max_context_length: 131072, recommended_context_length: 16384, available_memory_bytes: null, available_vram_bytes: null, model_size_bytes: null, layer_count: 32, max_gpu_layers: 33, supports_gpu_offload: false, selected_gpu_device_ids: null, per_device_vram: null, estimated_placement: null, memory_fit: [] } }),
    });
    await click(/^Context & memory/);
    expect(screen.queryByRole("switch", { name: "Offload attention to the GPU" })).toBeNull();
    fireEvent.click(await screen.findByRole("combobox", { name: "KV cache placement" }));
    expect(await screen.findByRole("option", { name: "Automatic" })).toBeTruthy();
    expect(screen.queryByRole("option", { name: "Auto" })).toBeNull();
    expect((await screen.findByRole("option", { name: "Split with layers" })).getAttribute("aria-disabled")).toBe("true");
    expect(screen.getByRole("option", { name: "Pin to one GPU" }).getAttribute("aria-disabled")).toBe("true");
    expect(screen.getByRole("option", { name: "System RAM" }).getAttribute("aria-disabled")).not.toBe("true");
    fireEvent.keyDown(screen.getByRole("combobox", { name: "KV cache placement" }), { key: "Escape" });
    fireEvent.click(screen.getByRole("combobox", { name: "Flash attention" }));
    expect(await screen.findByRole("option", { name: "Automatic" })).toBeTruthy();
    expect(screen.queryByRole("option", { name: "Auto" })).toBeNull();
    fireEvent.keyDown(screen.getByRole("combobox", { name: "Flash attention" }), { key: "Escape" });
    expect(screen.queryByRole("button", { name: /^GPU/ })).toBeNull();
    expect(screen.queryByRole("region", { name: "GPU" })).toBeNull();
    await waitFor(() => expect(screen.queryByRole("button", { name: "Save" })).toBeNull());
    await click(/^Speculative decoding/);
    await screen.findByRole("switch", { name: "Use MTP" });
    fireEvent.click(await screen.findByRole("combobox", { name: "Run on" }));
    expect((await screen.findByRole("option", { name: "GPU" })).getAttribute("aria-disabled")).toBe("true");
    expect(screen.getByRole("option", { name: "CPU" }).getAttribute("aria-disabled")).not.toBe("true");
    expect(screen.queryByRole("option", { name: "Auto" })).toBeNull();
    expect(screen.getByRole("option", { name: "Automatic" })).toBeTruthy();
  });
  it("hides the GPU section when no GPU devices are available", async () => {
    await mount(undefined, {
      providerAccountsList: () => ({ status: "ok", data: [{ ...account, provider_kind: "llamacpp" }] }),
      llamaDevices: () => ({ status: "ok", data: { devices: [] } }),
    });
    await screen.findByRole("button", { name: /^Context & memory/ });
    expect(screen.queryByRole("button", { name: /^GPU/ })).toBeNull();
    await click(/^Context & memory/);
    expect(screen.queryByRole("region", { name: "GPU" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
  });
  it("keeps native file picking busy inside its field and clears it on cancel or selection", async () => {
    let release: (() => void) | undefined;
    let picks = 0;
    let saved: ModelSaveRequest | undefined;
    await mount(undefined, {
      providerAccountsList: () => ({ status: "ok", data: [{ ...account, provider_kind: "llamacpp" }] }),
      modelGet: () => ({ status: "ok", data: { ...model, config: { ...full, llama_cpp: {} } } }),
      filesPickOpen: async () => {
        picks += 1;
        await new Promise<void>((resolve) => { release = resolve; });
        return { status: "ok", data: { sources: picks === 1 ? [] : [{ uri: "/models/mmproj.gguf" }] } };
      },
      modelSave: (request) => { saved = request; return { status: "ok", data: { ...model, config: request.model.config } }; },
    });
    await click(/^Template & output/);
    const file = await screen.findByRole("button", { name: "None" });
    fireEvent.click(file);
    await waitFor(() => expect(release).toBeDefined());
    expect(file.getAttribute("aria-busy")).toBe("true");
    expect((file as HTMLButtonElement).disabled).toBe(true);
    expect(screen.queryByText("Loading...")).toBeNull();
    await waitFor(() => expect(screen.queryByRole("button", { name: "Save" })).toBeNull());
    await act(async () => { release?.(); });
    await waitFor(() => expect(file.hasAttribute("aria-busy")).toBe(false));
    expect(file.textContent).toBe("None");
    expect(screen.queryByRole("button", { name: "Reset Vision projector" })).toBeNull();
    release = undefined;
    fireEvent.click(file);
    await waitFor(() => expect(release).toBeDefined());
    await act(async () => { release?.(); });
    expect(await screen.findByRole("button", { name: "mmproj.gguf" })).toBeTruthy();
    expect(screen.queryByText("Loading...")).toBeNull();
    await click("Save");
    await waitFor(() => expect(saved?.model.config).toMatchObject({ llama_cpp: { mmproj_path: "/models/mmproj.gguf" } }));
  });
  it("views a highlighted template in a drawer and saves edits only after Apply", async () => {
    const source = "{% for message in messages %}\n{{ message['content'] }}\n{% endfor %}";
    const changed = "{% if messages %}\n{{ messages[0]['content'] }}\n{% endif %}";
    let saved: ModelSaveRequest | undefined;
    await mount(undefined, {
      providerAccountsList: () => ({ status: "ok", data: [{ ...account, provider_kind: "llamacpp" }] }),
      modelGet: () => ({ status: "ok", data: { ...model, config: { ...full, llama_cpp: {} } } }),
      llamaChatTemplate: () => ({ status: "ok", data: { template: source } }),
      modelSave: (request) => { saved = request; return { status: "ok", data: { ...model, config: request.model.config } }; },
    });
    await click(/^Template & output/);
    expect(screen.queryByRole("textbox", { name: "Custom template" })).toBeNull();
    await click("View the model template");
    const drawer = await screen.findByRole("dialog", { name: "Model template" });
    await waitFor(() => expect(drawer.querySelector("code")?.textContent).toBe(source));
    expect(drawer.querySelector(".text-secondary")?.textContent).toBe("for");
    expect(within(drawer).queryByRole("textbox")).toBeNull();
    fireEvent.click(within(drawer).getByRole("button", { name: "Use as custom template" }));
    const editor = await screen.findByRole("textbox", { name: "Custom template" });
    fireEvent.change(editor, { target: { value: changed } });
    await click("Cancel");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(screen.queryByRole("button", { name: "Save" })).toBeNull());
    await click("Edit custom template");
    fireEvent.change(await screen.findByRole("textbox", { name: "Custom template" }), { target: { value: changed } });
    await click("Apply");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(saved).toBeUndefined();
    await click("Save");
    await waitFor(() => expect(saved?.model.config).toMatchObject({ llama_cpp: { chat_template_override: changed, chat_template_preset: "custom" } }));
  });
  it("shows endpoints only in pinned routing mode and saves the actual endpoint id", async () => {
    let saved: ModelSaveRequest | undefined;
    await mount(undefined, {
      modelGet: () => ({ status: "ok", data: { ...model, config: { ...full, chat_parameters: { ...full.chat_parameters, openrouter: null } } } }),
      providerOpenrouterEndpoints: () => ({ status: "ok", data: [{ id: "endpoint-one", name: "Endpoint one", prompt_price: "0.000001", completion_price: "0.000002", context_length: 32000, uptime_last_30m: 99.9, supports_prompt_caching: false, cache_read_price: null, cache_write_price: null }] }),
      modelSave: (request) => { saved = request; return { status: "ok", data: { ...model, config: request.model.config } }; },
    });
    await click(/^Routing/);
    expect(screen.queryByText("Endpoint one")).toBeNull();
    await click(/^Always use one provider/);
    await click(/^Endpoint one/);
    await click("Save");
    await waitFor(() => expect(saved?.model.config).toMatchObject({ chat_parameters: { openrouter: { pinned_provider: "endpoint-one" } } }));
  });
  it("keeps inherited OpenRouter pins visible and cannot silently clear a global pin", async () => {
    await mount(undefined, {
      modelGet: () => ({ status: "ok", data: { ...model, config: { ...full, chat_parameters: { ...full.chat_parameters, openrouter: null } } } }),
      settingsGet: () => ({ status: "ok", data: { ...settings, sampler_defaults: { ...settings.sampler_defaults, chat_parameters: { ...settings.sampler_defaults.chat_parameters, openrouter: { pinned_provider: "global-endpoint" } } } } }),
      providerOpenrouterEndpoints: () => ({ status: "ok", data: [] }),
    });
    await click(/^Routing/);
    expect((await screen.findByRole("button", { name: /^Let OpenRouter choose/ }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByRole("button", { name: /^Always use one provider/ }).getAttribute("aria-pressed")).toBe("true");
  });
  it("saves LoRA strength without losing its keywords or noise flag", async () => {
    let saved: ModelSaveRequest | undefined;
    await mount(undefined, {
      providerAccountsList: () => ({ status: "ok", data: [{ ...account, provider_kind: "sdcpp" }] }),
      modelGet: () => ({ status: "ok", data: { ...model, kind: "image", config: { ...full, stable_diffusion: { base_loras: [{ path: "/loras/style.safetensors", multiplier: 0.6, keywords: ["film grain"], is_high_noise: true }] } } } }),
      lorasList: () => ({ status: "ok", data: { loras: [] } }),
      modelSave: (request) => { saved = request; return { status: "ok", data: { ...model, kind: "image", config: request.model.config } }; },
    });
    await click(/^LoRAs/);
    fireEvent.change(await screen.findByRole("slider", { name: "style.safetensors strength" }), { target: { value: "0.85" } });
    await click("Save");
    await waitFor(() => expect(saved?.model.config).toMatchObject({ stable_diffusion: { base_loras: [{ path: "/loras/style.safetensors", multiplier: 0.85, keywords: ["film grain"], is_high_noise: true }] } }));
  });
  it.each([false, true])("reorders, removes and adds sampler stages and saves an explicit empty chain (phone: %s)", async (phone) => {
    let saved: ModelSaveRequest | undefined;
    await mount(undefined, {
      providerAccountsList: () => ({ status: "ok", data: [{ ...account, provider_kind: "llamacpp" }] }),
      modelGet: () => ({ status: "ok", data: { ...model, config: { ...full, llama_cpp: { sampler: { order: ["top_k", "top_p", "temp"] } } } } }),
      modelSave: (request) => { saved = request; return { status: "ok", data: { ...model, config: request.model.config } }; },
    }, phone);
    await click(/^Generation/);
    const order = await screen.findByRole("list", { name: "Order" });
    const labels = () => within(order).getAllByRole("listitem").map((item) => item.querySelector("span.min-w-0")?.textContent);
    expect(labels()).toEqual(["Top K", "Top P", "Temperature"]);
    expect((within(order).getByRole("button", { name: "Move Top K up" }) as HTMLButtonElement).disabled).toBe(true);
    await click("Move Top P up");
    expect(labels()).toEqual(["Top P", "Top K", "Temperature"]);
    fireEvent.keyDown(within(order).getByRole("button", { name: "Drag Temperature to reorder" }), { key: "ArrowUp" });
    expect(labels()).toEqual(["Top P", "Temperature", "Top K"]);
    await click("Remove Top K");
    expect(labels()).toEqual(["Top P", "Temperature"]);
    await click("Top K");
    expect(labels()).toEqual(["Top P", "Temperature", "Top K"]);
    await click("Save");
    await waitFor(() => expect(saved?.model.config).toMatchObject({ llama_cpp: { sampler: { order: ["top_p", "temp", "top_k"] } } }));
    for (const name of ["Top P", "Temperature", "Top K"]) await click(`Remove ${name}`);
    expect(await screen.findByText("No sampler stages. Add a stage below.")).toBeTruthy();
    await click("Save");
    await waitFor(() => expect(saved?.model.config).toMatchObject({ llama_cpp: { sampler: { order: [] } } }));
    expect(screen.queryByRole("button", { name: "Remove Top K" })).toBeNull();
    expect(screen.getByRole("button", { name: "Top K" })).toBeTruthy();
  });
  it("clears an incomplete sequence escape when discarding local sampler edits", async () => {
    await mount(undefined, {
      providerAccountsList: () => ({ status: "ok", data: [{ ...account, provider_kind: "llamacpp" }] }),
      llamaContextInfo: () => ({ status: "error", error: { code: "internal", message: "offline", details: null } }),
    });
    await click(/^Generation/);
    const input = await screen.findByLabelText("Sequence breakers");
    fireEvent.change(input, { target: { value: "unfinished\\" } });
    expect((await screen.findByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
    await click("Discard");
    expect((await screen.findByLabelText("Sequence breakers") as HTMLInputElement).value).toBe(formatSequences(full.llama_cpp.sampler.dry_sequence_breakers));
  });

  it("detects an image file and saves component bindings with image scopes", async () => {
    let saved: ModelSaveRequest | undefined;
    const detect = vi.fn<NonNullable<MockCallHandlers["sdDetectModelFile"]>>(() => ({ status: "ok", data: { exists: true, profile: { id: "detected-profile", display_name: "Detected image model", family: "test", description: "", minimum_runtime_build: null, required_roles: ["diffusion_model"], diffusion_markers: [], encoder_markers: [], encoder_parameter_billions: null, recommended_repositories: [], supports_text_to_image: true, supports_image_edit: true, max_reference_images: 1, requires_reference_image: false, recommended_for_scenes: true, default_width: 1024, default_height: 1024, default_steps: 20, default_cfg: 3 } } }));
    await mount("/settings/models/new", {
      providerAccountsList: () => ({ status: "ok", data: [{ ...account, provider_kind: "sdcpp" }] }),
      imageCapabilities: () => ({ status: "ok", data: { provider_kind: "sdcpp", local: true, sizes: ["1024x1024"], default_size: "1024x1024", samplers: ["euler"], schedulers: ["discrete"], negative_prompt: true, quality: [], styles: [], max_count: 1 } }),
      sdModelsInstalled: () => ({ status: "ok", data: { models: [] } }),
      filesPickOpen: () => ({ status: "ok", data: { sources: [{ uri: "/models/image.gguf" }] } }),
      sdDetectModelFile: detect,
      modelSave: (request) => { saved = request; return { status: "ok", data: { ...model, ...request.model, id: "created", revision: 1 } }; },
    });
    expect(screen.queryByRole("switch", { name: "Inputs Text" })).toBeNull();
    await click("Choose a model"); await click("Choose a file on this computer");
    await waitFor(() => expect(detect).toHaveBeenCalledWith({ source: { uri: "/models/image.gguf" } }));
    await screen.findByText(/detected-profile/);
    await click("Add model");
    await waitFor(() => expect(saved).toBeDefined());
    expect(saved?.model).toMatchObject({ kind: "image", external_model_id: "/models/image.gguf", input_scopes: ["text", "image"], output_scopes: ["image"], config: { stable_diffusion: { cpp: { profile_id: "detected-profile", supports_image_edit: true } } } });
  });

  it("matches the feature menu and reasoning groups without native selects or duplicate default hints", async () => {
    await mount();
    await click(/^Generation/);
    const trigger = await screen.findByRole("button", { name: "Settings for" });
    fireEvent.click(trigger);
    expect(await screen.findByText("The main settings")).toBeTruthy();
    fireEvent.click(await screen.findByRole("menuitem", { name: /^Dynamic memory/ }));
    await waitFor(() => expect(trigger.textContent).toMatch(/Dynamic memory.*changed/));
    expect(document.querySelector("select")).toBeNull();
    expect(screen.queryByText("Runtime default")).toBeNull();
    await click(/^Reasoning/);
    expect(await screen.findByText("Compatibility")).toBeTruthy();
    expect(await screen.findByRole("combobox", { name: "Effort" })).toBeTruthy();
    fireEvent.click(await screen.findByRole("switch", { name: "Let the model think" }));
    await waitFor(() => expect(screen.queryByRole("combobox", { name: "Effort" })).toBeNull());
    expect(screen.getByRole("switch", { name: "Always send the thinking setting" })).toBeTruthy();
  });

  it("uses a caching switch and a separate retention menu with the existing config encoding", async () => {
    const config = structuredClone(full);
    config.chat_parameters.prompt_caching = "disabled" as unknown as typeof config.chat_parameters.prompt_caching;
    let saved: ModelSaveRequest | undefined;
    await mount(undefined, {
      modelGet: () => ({ status: "ok", data: { ...model, config } }),
      modelSave: (request) => { saved = request; return { status: "ok", data: { ...model, config: request.model.config } }; },
    });
    await click(/^Prompt caching/);
    const toggle = await screen.findByRole("switch", { name: "Cache the prompt" });
    expect(screen.queryByRole("combobox", { name: "Keep for" })).toBeNull();
    fireEvent.click(toggle);
    const retention = await screen.findByRole("combobox", { name: "Keep for" });
    expect(retention.textContent).toContain("5 minutes");
    fireEvent.click(retention);
    fireEvent.click(await screen.findByRole("option", { name: "1 hour" }));
    await click("Save");
    await waitFor(() => expect(saved?.model.config).toMatchObject({ chat_parameters: { prompt_caching: { enabled: { retention: "one_hour" } } } }));
    fireEvent.click(await screen.findByRole("switch", { name: "Cache the prompt" }));
    await waitFor(() => expect(screen.queryByRole("combobox", { name: "Keep for" })).toBeNull());
  });

});
