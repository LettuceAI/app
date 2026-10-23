import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { toast } from "@lettuceai/crisp";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { JobView, SettingsDynamicMemorySettings, SettingsPatch_Deserialize, SettingsUpdateRequest_Deserialize, SettingsView } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers, type MockStreamHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";
import { presets } from "./memory/presets";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

afterEach(() => {
  toast.dismiss();
  cleanup();
});

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const memory: SettingsDynamicMemorySettings = {
  enabled: true,
  ...presets.balanced,
  min_similarity_basis_points: null,
  run_mode: "auto",
  context_enrichment_enabled: true,
  recursive_memory_loops: false,
  recursive_memory_loop_hard_cap: 20,
  delete_confidence_basis_points: 5000,
  max_hard_delete_ratio_basis_points: 5000,
  duplicate_threshold_basis_points: 7800,
  structured_fallback_format: "xml",
};

function viewOf(patch: Partial<SettingsView["global"]> = {}, revision = 7): SettingsView {
  return {
    device: { revision: 1, embedding_model_version: null, embedding_max_tokens: null, embedding_keep_model_loaded: false, llm_models_dir: null, dictation_model_id: null, trusted_certificates: [] },
    global: {
      dynamic_memory: memory,
      group_dynamic_memory: null,
      dynamic_memory_prompts: { summarizer_prompt_id: null, manager_prompt_id: null },
      dynamic_memory_llama_sampler_overwrite_enabled: true,
      manual_mode_context_window: 50,
      ...patch,
    } as SettingsView["global"],
    sampler_defaults: {} as SettingsView["sampler_defaults"],
    default_model_profile_id: null,
    default_prompt_document_id: null,
    dynamic_memory_model_profile_id: null,
    group_speaker_model_profile_id: null,
    revision,
  };
}

function backend(initial: SettingsView) {
  let view = initial;
  const requests: SettingsUpdateRequest_Deserialize[] = [];
  const get = vi.fn(() => ok(view));
  const update = vi.fn((request: SettingsUpdateRequest_Deserialize) => {
    requests.push(request);
    if (request.expected_revision !== view.revision) return { status: "error" as const, error: { code: "conflict" as const, message: "stale", details: null } };
    const patch: SettingsPatch_Deserialize = request.patch;
    const global = { ...view.global };
    if (patch.section === "dynamic_memory") global.dynamic_memory = patch.value;
    else if (patch.section === "group_dynamic_memory") global.group_dynamic_memory = patch.value;
    else if (patch.section === "dynamic_memory_sampler") global.dynamic_memory_llama_sampler_overwrite_enabled = patch.enabled;
    else if (patch.section === "dynamic_memory_prompts") global.dynamic_memory_prompts = patch.value;
    else if (patch.section === "general" && patch.manual_mode_context_window !== null) global.manual_mode_context_window = patch.manual_mode_context_window;
    view = { ...view, global, revision: view.revision + 1 };
    return ok(view);
  });
  return { get, update, requests, current: () => view };
}

async function mount(initial: SettingsView, calls: MockCallHandlers = {}, streams: MockStreamHandlers = {}, store = backend(initial)) {
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: () => ok({ ...mockAppStatus, platform: "android", ui_state: { onboarding: { completed: true } } }),
        settingsGet: store.get,
        settingsUpdate: store.update,
        embeddingStatus: () => ok([{ family: "lettuce_eidos_v5", revision: "r", native_dimensions: 768, active: true }]),
        modelsList: () => ok({ models: [], default_model_id: null, revision: 1 }),
        providerAccountsList: () => ok([]),
        promptsList: () => ok({ items: [], next_cursor: null }),
        ...calls,
      },
      streams,
    }),
    history: createMemoryHistory({ initialEntries: ["/settings/memory"] }),
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  await screen.findByText("Manual memory");
  return { runtime, store };
}

function numberFor(title: string): HTMLInputElement {
  let node: HTMLElement | null = screen.getByText(title);
  while (node && !node.querySelector("input[inputmode='decimal']")) node = node.parentElement;
  return node!.querySelector("input[inputmode='decimal']") as HTMLInputElement;
}

async function commit(title: string, value: string) {
  const input = numberFor(title);
  fireEvent.change(input, { target: { value } });
  fireEvent.blur(input);
}

const lastPatch = (store: ReturnType<typeof backend>) => store.requests.at(-1)!.patch;

describe("memory settings page", () => {
  it("is listed with its subtitle and the direct helper sections", async () => {
    const i18n = await createTestI18n("en");
    const runtime = await createAppRuntime({
      transport: createMockTransport({ calls: { appStatus: () => ok({ ...mockAppStatus, platform: "android", ui_state: { onboarding: { completed: true } } }) } }),
      history: createMemoryHistory({ initialEntries: ["/settings"] }),
      i18n,
    });
    await act(async () => { render(<App runtime={runtime} />); });
    const memoryRow = await screen.findByRole("button", { name: /^Memory/ });
    expect(within(memoryRow).getByText("How characters remember")).toBeTruthy();
    expect(memoryRow.hasAttribute("disabled")).toBe(false);
    expect(screen.queryByRole("button", { name: /^Advanced/ })).toBeNull();
    expect(screen.getByRole("button", { name: /^Reply helper/ }).hasAttribute("disabled")).toBe(false);
    expect(screen.getByRole("button", { name: /^Smart creator/ })).toBeTruthy();
  });

  it("saves the context window when the field is left", async () => {
    const { store } = await mount(viewOf());
    await commit("Context window", "80");
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(1));
    expect(store.requests[0]).toMatchObject({ expected_revision: 7, patch: { section: "general", manual_mode_context_window: 80, pure_mode: null } });
  });

  it("turns dynamic memory on when the model is installed and off again", async () => {
    const { store } = await mount(viewOf({ dynamic_memory: { ...memory, enabled: false } }));
    fireEvent.click(screen.getByRole("switch", { name: "Dynamic memory" }));
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(1));
    expect(lastPatch(store)).toMatchObject({ section: "dynamic_memory", value: { enabled: true, run_mode: "auto", max_entries: 200 } });
    expect(await screen.findByText("Memory updates")).toBeTruthy();
    fireEvent.click(screen.getByRole("switch", { name: "Dynamic memory" }));
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(2));
    expect(lastPatch(store)).toMatchObject({ section: "dynamic_memory", value: { enabled: false } });
  });

  it("keeps dynamic memory off when the download sheet is declined", async () => {
    const { store } = await mount(viewOf({ dynamic_memory: { ...memory, enabled: false } }), { embeddingStatus: () => ok([]) });
    fireEvent.click(screen.getByRole("switch", { name: "Dynamic memory" }));
    expect(await screen.findByText("Download the memory model")).toBeTruthy();
    expect(store.update).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Use manual memory instead" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(store.update).not.toHaveBeenCalled();
    expect(screen.getByRole("switch", { name: "Dynamic memory" }).getAttribute("aria-checked")).toBe("false");
  });

  it("downloads the model without closing, then turns dynamic memory on", async () => {
    let installed = false;
    const install = vi.fn(() => ok({ job_id: "j1" }));
    let finish!: () => void;
    const settled = new Promise<void>((resolve) => { finish = resolve; });
    const job = (state: JobView["state"]): JobView => ({
      id: "j1", kind: "artifact_install", subject: { kind: "artifact", id: "a" } as unknown as JobView["subject"], subject_detail: null, state,
      progress: { current: 5, total: 10, unit: "bytes", label_code: "download", bytes_per_second: 1 }, created_at: 1, updated_at: 1, failure: null, result: null,
    });
    const { store } = await mount(viewOf({ dynamic_memory: { ...memory, enabled: false } }), {
      embeddingStatus: () => ok(installed ? [{ family: "lettuce_eidos_v5", revision: "r", native_dimensions: 768, active: true }] : []),
      embeddingInstall: install,
    }, {
      jobWatch: async () => {
        await settled;
        installed = true;
        return ok(job("succeeded"));
      },
    });
    fireEvent.click(screen.getByRole("switch", { name: "Dynamic memory" }));
    fireEvent.click(await screen.findByRole("button", { name: /Download and finish/ }));
    await waitFor(() => expect(install).toHaveBeenCalledWith({ family: "lettuce_eidos_v5", enable_dynamic_memory: false }));
    await screen.findByText("Keep the app open until it finishes.");
    fireEvent.keyDown(document.body, { key: "Escape" });
    expect(screen.getByText("Download the memory model")).toBeTruthy();
    await act(async () => finish());
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(1));
    expect(lastPatch(store)).toMatchObject({ section: "dynamic_memory", value: { enabled: true } });
    await waitFor(() => expect(screen.queryByText("Download the memory model")).toBeNull());
  });

  it("offers a retry and manual memory when the download fails", async () => {
    const install = vi.fn(() => ok({ job_id: "j1" }));
    const { store } = await mount(viewOf({ dynamic_memory: { ...memory, enabled: false } }), { embeddingStatus: () => ok([]), embeddingInstall: install }, {
      jobWatch: () => ok({
        id: "j1", kind: "artifact_install", subject: { kind: "artifact", id: "a" } as unknown as JobView["subject"], subject_detail: null, state: "failed",
        progress: { current: 0, total: null, unit: null, label_code: null, bytes_per_second: null }, created_at: 1, updated_at: 1, failure: { code: "worker_failed" } as never, result: null,
      } satisfies JobView),
    });
    fireEvent.click(screen.getByRole("switch", { name: "Dynamic memory" }));
    fireEvent.click(await screen.findByRole("button", { name: /Download and finish/ }));
    const retry = await screen.findByRole("button", { name: "Try again" });
    expect(screen.getByRole("button", { name: "Use manual memory instead" })).toBeTruthy();
    fireEvent.click(retry);
    await waitFor(() => expect(install).toHaveBeenCalledTimes(2));
    fireEvent.click(await screen.findByRole("button", { name: "Use manual memory instead" }));
    expect(store.update).not.toHaveBeenCalled();
  });

  it("selects a preset in basis points and shows Custom when a value differs", async () => {
    const { store } = await mount(viewOf());
    expect(screen.getByText("A good mix of recall and speed for most chats.")).toBeTruthy();
    fireEvent.click(screen.getByRole("radio", { name: "Comprehensive" }));
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(1));
    expect(lastPatch(store)).toMatchObject({ section: "dynamic_memory", value: { summary_message_interval: 15, max_entries: 400, retrieval_limit: 8, hot_memory_token_budget: 10240, decay_rate_basis_points: 500, cold_threshold_basis_points: 2000, retrieval_strategy: "smart" } });
    fireEvent.click(screen.getByRole("button", { name: /Advanced options/ }));
    await commit("Max memory entries", "250");
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(2));
    expect(await screen.findByText(/^Custom: your own values/)).toBeTruthy();
    expect(within(screen.getByRole("radiogroup", { name: "Memory profile" })).getAllByRole("radio").every((radio) => radio.getAttribute("aria-checked") === "false")).toBe(true);
  });

  it("shows the backend default memory as Balanced and a single changed value as Custom", async () => {
    const defaults = { ...memory, max_entries: 200, hot_memory_token_budget: 6144, summary_message_interval: 20, retrieval_limit: 5, retrieval_strategy: "smart" as const, decay_rate_basis_points: 800, cold_threshold_basis_points: 3000, min_similarity_basis_points: null };
    await mount(viewOf({ dynamic_memory: defaults }));
    expect(screen.getByRole("radio", { name: "Balanced" }).getAttribute("aria-checked")).toBe("true");
    cleanup();
    await mount(viewOf({ dynamic_memory: { ...defaults, retrieval_limit: 6 } }));
    expect(screen.getByRole("radio", { name: "Balanced" }).getAttribute("aria-checked")).toBe("false");
    expect(screen.getByText(/^Custom: your own values/)).toBeTruthy();
  });

  it("leaves the relevance threshold alone when a profile is picked", async () => {
    const { store } = await mount(viewOf({ dynamic_memory: { ...memory, min_similarity_basis_points: 4200 } }));
    fireEvent.click(screen.getByRole("radio", { name: "Minimal" }));
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(1));
    expect(lastPatch(store)).toMatchObject({ section: "dynamic_memory", value: { max_entries: 100, min_similarity_basis_points: 4200 } });
  });

  it("uses the model's calibrated threshold until Custom is chosen", async () => {
    const { store } = await mount(viewOf());
    fireEvent.click(screen.getByRole("button", { name: /Advanced options/ }));
    expect(screen.getByText("Uses Lettuce Eidos's calibrated threshold.")).toBeTruthy();
    expect(screen.queryByText("Threshold")).toBeNull();
    fireEvent.click(screen.getByRole("radio", { name: "Custom" }));
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(1));
    expect(lastPatch(store)).toMatchObject({ section: "dynamic_memory", value: { min_similarity_basis_points: 3500 } });
    await waitFor(() => expect(screen.getByRole("radio", { name: "Custom" }).getAttribute("aria-checked")).toBe("true"));
    await commit("Threshold", "0");
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(2));
    expect(lastPatch(store)).toMatchObject({ value: { min_similarity_basis_points: 0 } });
    await waitFor(() => expect(store.current().global.dynamic_memory.min_similarity_basis_points).toBe(0));
    await waitFor(() => expect(numberFor("Threshold").value).toBe("0.00"));
    fireEvent.click(screen.getByRole("radio", { name: "Auto" }));
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(3));
    expect(lastPatch(store)).toMatchObject({ value: { min_similarity_basis_points: null } });
  });

  it("changes the token capacity and shows it as 4096 when unset", async () => {
    const calls: Array<Record<string, unknown>> = [];
    const store = backend(viewOf());
    store.update.mockImplementationOnce((request) => {
      calls.push(request.patch as unknown as Record<string, unknown>);
      return ok(store.current());
    });
    await mount(viewOf(), {}, {}, store);
    expect(screen.getByText(/4,096 token capacity/)).toBeTruthy();
    expect(screen.getByRole("radio", { name: "4096" }).getAttribute("aria-checked")).toBe("true");
    fireEvent.click(screen.getByRole("radio", { name: "2048" }));
    await waitFor(() => expect(calls).toHaveLength(1));
    expect(calls[0]).toEqual({ section: "device_embedding", model_version: null, max_tokens: 2048, keep_model_loaded: false });
  });

  it("edits the scope being shown: one-on-one, then a group copy that can go back to the same settings", async () => {
    const { store } = await mount(viewOf());
    fireEvent.click(screen.getByRole("radio", { name: "Ask first" }));
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(1));
    expect(lastPatch(store)).toMatchObject({ section: "dynamic_memory", value: { run_mode: "ask_first" } });
    await waitFor(() => expect(screen.getByRole("radio", { name: "Ask first" }).getAttribute("aria-checked")).toBe("true"));

    fireEvent.click(screen.getByRole("radio", { name: "Group chats" }));
    expect(screen.getByText("Group chats remember the same way as one-on-one chats")).toBeTruthy();
    expect(screen.queryByText("Memory updates")).toBeNull();
    fireEvent.click(screen.getByRole("switch", { name: "Use one-on-one settings" }));
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(2));
    expect(lastPatch(store)).toMatchObject({ section: "group_dynamic_memory", value: { run_mode: "ask_first", enabled: true, max_entries: 200 } });
    expect(await screen.findByText("Memory updates")).toBeTruthy();
    fireEvent.click(screen.getByRole("radio", { name: "Manual" }));
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(3));
    expect(lastPatch(store)).toMatchObject({ section: "group_dynamic_memory", value: { run_mode: "manual", enabled: true } });
    expect(store.current().global.dynamic_memory.run_mode).toBe("ask_first");
    fireEvent.click(screen.getByRole("switch", { name: "Use one-on-one settings" }));
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(4));
    expect(lastPatch(store)).toMatchObject({ section: "group_dynamic_memory", value: null });
    await waitFor(() => expect(screen.queryByText("Memory updates")).toBeNull());
  });

  it("writes the sampler switch and an advanced field", async () => {
    const { store } = await mount(viewOf());
    fireEvent.click(screen.getByRole("switch", { name: "Overwrite sampler configuration" }));
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(1));
    expect(lastPatch(store)).toEqual({ section: "dynamic_memory_sampler", enabled: false });
    fireEvent.click(screen.getByRole("button", { name: /Context enrichment and memory manager/ }));
    await commit("Delete confidence default", "0.7");
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(2));
    expect(lastPatch(store)).toMatchObject({ section: "dynamic_memory", value: { delete_confidence_basis_points: 7000 } });
    fireEvent.click(screen.getByRole("switch", { name: "Recursive memory loops" }));
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(3));
    expect(await screen.findByText("Recursive loop hard cap")).toBeTruthy();
  });

  it("keeps the page and shows the typed error when the revision is stale", async () => {
    const store = backend(viewOf());
    await mount(viewOf(), {}, {}, store);
    const loads = store.get.mock.calls.length;
    store.update.mockImplementationOnce(() => ({ status: "error" as const, error: { code: "conflict" as const, message: "stale", details: null } }));
    fireEvent.click(screen.getByRole("switch", { name: "Overwrite sampler configuration" }));
    expect((await screen.findAllByText("It changed somewhere else. Reload and try again.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(store.get.mock.calls.length).toBeGreaterThan(loads));
    expect(screen.getByRole("switch", { name: "Overwrite sampler configuration" })).toBeTruthy();
  });

  it("asks before removing the embedding model", async () => {
    const remove = vi.fn(() => ok(true));
    await mount(viewOf(), { embeddingRemove: remove });
    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    const dialog = (await screen.findAllByRole("dialog")).at(-1)!;
    expect(within(dialog).getByText(/Dynamic memory needs this model/)).toBeTruthy();
    fireEvent.click(within(dialog).getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(remove).toHaveBeenCalledWith({ family: "lettuce_eidos_v5" }));
  });

  it("applies two quick edits in a row without losing either", async () => {
    const { store } = await mount(viewOf());
    fireEvent.click(screen.getByRole("radio", { name: "Ask first" }));
    fireEvent.click(screen.getByRole("switch", { name: "Overwrite sampler configuration" }));
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(2));
    expect(store.requests.map((request) => request.expected_revision)).toEqual([7, 8]);
    expect(store.current().global.dynamic_memory.run_mode).toBe("ask_first");
    expect(store.current().global.dynamic_memory_llama_sampler_overwrite_enabled).toBe(false);
  });

  it("commits a typed number and a following switch tap together", async () => {
    const { store } = await mount(viewOf());
    const input = numberFor("Context window");
    fireEvent.change(input, { target: { value: "90" } });
    fireEvent.blur(input);
    fireEvent.click(screen.getByRole("switch", { name: "Overwrite sampler configuration" }));
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(2));
    expect(store.current().global.manual_mode_context_window).toBe(90);
    expect(store.current().global.dynamic_memory_llama_sampler_overwrite_enabled).toBe(false);
  });

  it("sends one write when the dynamic memory switch is tapped twice", async () => {
    const { store } = await mount(viewOf());
    const toggle = screen.getByRole("switch", { name: "Dynamic memory" });
    fireEvent.click(toggle);
    fireEvent.click(toggle);
    await waitFor(() => expect(store.update.mock.calls.length).toBeGreaterThan(0));
    await act(async () => { await new Promise((resolve) => setTimeout(resolve, 50)); });
    expect(store.current().global.dynamic_memory.enabled).toBe(false);
    expect(store.requests.every((request) => request.patch.section === "dynamic_memory" && request.patch.value.enabled === false)).toBe(true);
  });

  it("turns the group copy off with dynamic memory", async () => {
    const { store } = await mount(viewOf({ group_dynamic_memory: { ...memory, run_mode: "manual" } }));
    fireEvent.click(screen.getByRole("switch", { name: "Dynamic memory" }));
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(2));
    expect(store.requests[0]!.patch).toMatchObject({ section: "dynamic_memory", value: { enabled: false } });
    expect(store.requests[1]!.patch).toMatchObject({ section: "group_dynamic_memory", value: { enabled: false, run_mode: "manual" } });
  });

  it("keeps the group copy's enabled equal to dynamic memory when editing it", async () => {
    const { store } = await mount(viewOf({ group_dynamic_memory: { ...memory, enabled: false } }));
    fireEvent.click(screen.getByRole("radio", { name: "Group chats" }));
    fireEvent.click(await screen.findByRole("radio", { name: "Manual" }));
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(1));
    expect(lastPatch(store)).toMatchObject({ section: "group_dynamic_memory", value: { run_mode: "manual", enabled: true } });
  });

  it("commits a pending number before the scope switch", async () => {
    const { store } = await mount(viewOf());
    fireEvent.click(screen.getByRole("button", { name: /Advanced options/ }));
    fireEvent.change(numberFor("Max memory entries"), { target: { value: "250" } });
    fireEvent.click(screen.getByRole("radio", { name: "Group chats" }));
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(1));
    expect(lastPatch(store)).toMatchObject({ section: "dynamic_memory", value: { max_entries: 250 } });
  });

  it("takes a typed value into a field whose minimum is above one digit", async () => {
    const { store } = await mount(viewOf());
    fireEvent.click(screen.getByRole("button", { name: /Advanced options/ }));
    await commit("Summary interval", "30");
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(1));
    expect(lastPatch(store)).toMatchObject({ section: "dynamic_memory", value: { summary_message_interval: 30 } });
  });

  it("turns dynamic memory off after the model is removed", async () => {
    const remove = vi.fn(() => ok(true));
    const { store } = await mount(viewOf(), { embeddingRemove: remove });
    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    fireEvent.click(within((await screen.findAllByRole("dialog")).at(-1)!).getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(remove).toHaveBeenCalled());
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(1));
    expect(lastPatch(store)).toMatchObject({ section: "dynamic_memory", value: { enabled: false } });
  });

  it("downloads only when asked from the embedding row", async () => {
    const { store } = await mount(viewOf(), { embeddingStatus: () => ok([]) });
    fireEvent.click(screen.getByRole("button", { name: "Download" }));
    const dialog = within(await screen.findByRole("dialog"));
    expect(dialog.getByRole("button", { name: "Download" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Use manual memory instead" })).toBeNull();
    expect(dialog.getByRole("button", { name: "Cancel" })).toBeTruthy();
    expect(store.update).not.toHaveBeenCalled();
  });

  it("shows the typed error when the download cannot start", async () => {
    await mount(viewOf({ dynamic_memory: { ...memory, enabled: false } }), {
      embeddingStatus: () => ok([]),
      embeddingInstall: () => ({ status: "error" as const, error: { code: "unavailable" as const, message: "diagnostic", details: null } }),
    });
    fireEvent.click(screen.getByRole("switch", { name: "Dynamic memory" }));
    fireEvent.click(await screen.findByRole("button", { name: /Download and finish/ }));
    expect(await screen.findByText("It isn't available right now.")).toBeTruthy();
    expect(screen.queryByText("diagnostic")).toBeNull();
  });

  it("names a missing summarization model and the selected prompt", async () => {
    await mount(
      { ...viewOf({ dynamic_memory_prompts: { summarizer_prompt_id: "p1", manager_prompt_id: null } }), dynamic_memory_model_profile_id: "gone" },
      {
        modelsList: () => ok({ models: [{ id: "m1", kind: "chat", display_name: "Other", provider_account_id: "a", external_model_id: "e" }], default_model_id: null, revision: 1 } as never),
        promptsList: () => ok({ items: [{ id: "p1", name: "My summary prompt", origin: { type: "custom" }, app_default: false }], next_cursor: null } as never),
      },
    );
    expect(await screen.findByText("Missing model")).toBeTruthy();
    expect(await screen.findByText("My summary prompt")).toBeTruthy();
  });

});
