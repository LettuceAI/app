import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ModelSaveRequest, ProviderAccountSaveRequest, SettingsView } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus } from "@/api/mock-transport";
import {
  accountView,
  failure,
  modelView,
  remoteModel,
  renderWith,
  type HarnessOptions,
} from "./test-support";

const completedStatus = { ...mockAppStatus, ui_state: { onboarding: { completed: true } } };

const renderApp = (options: HarnessOptions = {}) => renderWith(createMockTransport, options);

afterEach(() => {
  cleanup();
});

const accountId = "00000000-0000-0000-0000-0000000000a1";

function type(label: string | RegExp, value: string) {
  fireEvent.change(screen.getByPlaceholderText(label), { target: { value } });
}

async function click(name: string | RegExp) {
  fireEvent.click(await screen.findByRole("button", { name }));
}

async function chooseProvider(name: RegExp) {
  await click("Get started");
  await click(name);
}

function settingsWith(enabled: boolean, modelId: string | null = null): SettingsView {
  return {
    global: { dynamic_memory: { enabled, min_similarity_basis_points: null } },
    dynamic_memory_model_profile_id: modelId,
    revision: 7,
  } as unknown as SettingsView;
}

describe("gate", () => {
  it("sends a new user to the welcome screen", async () => {
    const { history } = await renderApp({ path: "/" });
    await screen.findByRole("button", { name: "Get started" });
    expect(history.location.pathname).toBe("/welcome");
  });

  it("sends a completed user away from the welcome screen", async () => {
    const { history } = await renderApp({
      path: "/welcome",
      calls: { appStatus: () => ({ status: "ok", data: completedStatus }) },
    });
    await screen.findByRole("heading", { name: "Home" });
    expect(history.location.pathname).toBe("/");
  });

  it("shows the failure state, never onboarding, when app status fails", async () => {
    await renderApp({ path: "/", calls: { appStatus: () => failure("busy") } });
    expect(await screen.findByText("The app is busy. Try again in a moment.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Get started" })).toBeNull();
  });
});

describe("connect", () => {
  it("keeps the typed key and says the key was not accepted", async () => {
    await renderApp({
      calls: {
        providerVerify: () =>
          failure("invalid_input", { type: "provider_verification", status: 401, provider_message: "bad key", reason: null }),
      },
    });
    await chooseProvider(/OpenRouter/);
    type("Paste your key", "sk-wrong");
    await click("Connect");
    expect(await screen.findByText("This key was not accepted")).toBeTruthy();
    expect(screen.getByText("bad key")).toBeTruthy();
    expect((screen.getByPlaceholderText("Paste your key") as HTMLInputElement).value).toBe("sk-wrong");
  });

  it("says the provider could not be reached when it is unavailable", async () => {
    await renderApp({ calls: { providerVerify: () => failure("unavailable") } });
    await chooseProvider(/OpenRouter/);
    type("Paste your key", "sk-1");
    await click("Connect");
    expect(await screen.findByText("Could not reach OpenRouter. Check your connection.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  });

  it("reuses the saved account when saving again after a failure", async () => {
    const saves: ProviderAccountSaveRequest[] = [];
    let attempt = 0;
    await renderApp({
      calls: {
        providerVerify: () => ({ status: "ok", data: { valid: true, status: 200 } }),
        providerAccountSave: (request) => {
          saves.push(request);
          attempt += 1;
          return { status: "ok", data: accountView({ id: accountId, revision: attempt }) };
        },
        providerAccountsList: () => ({ status: "ok", data: saves.length ? [accountView({ id: accountId, revision: attempt })] : [] }),
        providerModels: () => failure("unavailable"),
      },
    });
    await chooseProvider(/OpenRouter/);
    type("Paste your key", "sk-1");
    await click("Connect");
    await screen.findByText("Choose a model");
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    await click("Connect");
    await waitFor(() => expect(saves.length).toBe(2));
    expect(saves[0]?.account.id).toBeNull();
    expect(saves[1]?.account.id).toBe(accountId);
    expect(saves[1]?.expected_revision).toBe(1);
    expect(saves[0]?.account).toMatchObject({ label: "My OpenRouter", enabled: true, streaming_enabled: true, allow_invalid_tls: false, config: {} });
  });

  it("reuses an existing account of the same kind", async () => {
    const saves: ProviderAccountSaveRequest[] = [];
    await renderApp({
      calls: {
        providerVerify: () => ({ status: "ok", data: { valid: true, status: 200 } }),
        providerAccountsList: () => ({ status: "ok", data: [accountView({ id: accountId, revision: 4 })] }),
        providerAccountSave: (request) => {
          saves.push(request);
          return { status: "ok", data: accountView({ id: accountId, revision: 5 }) };
        },
        providerModels: () => ({ status: "ok", data: [] }),
      },
    });
    await chooseProvider(/OpenRouter/);
    type("Paste your key", "sk-1");
    await click("Connect");
    await screen.findByText("Choose a model");
    expect(saves[0]?.account.id).toBe(accountId);
    expect(saves[0]?.expected_revision).toBe(4);
  });
});

async function reachModel(options: HarnessOptions & { provider?: RegExp; kind?: string } = {}) {
  const saves: ModelSaveRequest[] = [];
  const kind = options.kind ?? "openrouter";
  await renderApp({
    ...options,
    calls: {
      providerVerify: () => ({ status: "ok", data: { valid: true, status: 200 } }),
      providerAccountSave: () => ({ status: "ok", data: accountView({ id: accountId, provider_kind: kind }) }),
      modelSave: (request) => {
        saves.push(request);
        return { status: "ok", data: modelView({ external_model_id: request.model.external_model_id }) };
      },
      ...options.calls,
    },
  });
  await chooseProvider(options.provider ?? /OpenRouter/);
  const address = screen.queryByPlaceholderText("https://api.example.com/v1");
  if (address) type("https://api.example.com/v1", "https://llm.example.test/v1");
  type("Paste your key", "sk-1");
  await click("Connect");
  await screen.findByText("Choose a model");
  return saves;
}

const pricedModels = [
  remoteModel({ id: "anthropic/claude-sonnet-5", display_name: "Anthropic: Claude Sonnet 5", input_modalities: ["text", "image"], input_price: 0.000003, output_price: 0.000015 }),
  remoteModel({ id: "deepseek/deepseek-chat:free", display_name: "DeepSeek: Chat (free)", input_price: 0, output_price: 0 }),
  remoteModel({ id: "qwen/qwen3-max", display_name: "Qwen: Max", input_price: 0.0000012, output_price: 0.000006 }),
];

describe("model", () => {
  it("saves a picked model with scopes, metadata and a default display name", async () => {
    const saves = await reachModel({ calls: { providerModels: () => ({ status: "ok", data: pricedModels }) } });
    await click(/Choose from 3 models/);
    fireEvent.click(await screen.findByText("Anthropic: Claude Sonnet 5"));
    await waitFor(() => expect((screen.getByPlaceholderText("Anthropic: Claude Sonnet 5") as HTMLInputElement).value).toBe("Anthropic: Claude Sonnet 5"));
    await click("Use this model");
    await waitFor(() => expect(saves.length).toBe(1));
    expect(saves[0]?.model).toMatchObject({
      provider_account_id: accountId,
      external_model_id: "anthropic/claude-sonnet-5",
      display_name: "Anthropic: Claude Sonnet 5",
      kind: "chat",
      input_scopes: ["text", "image"],
      output_scopes: ["text"],
      remote_metadata: pricedModels[0],
    });
    expect(saves[0]?.model.config).toHaveProperty("capabilities");
  });

  it("lets a provider that cannot list models take a typed id", async () => {
    const saves = await reachModel({ provider: /Custom/, kind: "custom", calls: {} });
    expect(screen.queryByText(/Choose from/)).toBeNull();
    type("model-name", "my-model");
    type("My model", "Mine");
    await click("Use this model");
    await waitFor(() => expect(saves.length).toBe(1));
    expect(saves[0]?.model).toMatchObject({ external_model_id: "my-model", display_name: "Mine", remote_metadata: null, input_scopes: ["text"] });
  });

  it("applies the filter draft only when Show is pressed", async () => {
    await reachModel({ calls: { providerModels: () => ({ status: "ok", data: pricedModels }) } });
    await click(/Choose from 3 models/);
    await screen.findByText("Qwen: Max");
    await click("Filter models");
    const sheet = await screen.findByText("Capabilities");
    fireEvent.click(within(sheet.closest("section") as HTMLElement).getByRole("switch"));
    expect(screen.getByText("Qwen: Max")).toBeTruthy();
    await click("Show 1 model");
    await waitFor(() => expect(screen.queryByText("Qwen: Max")).toBeNull());
    expect(screen.getByText("Anthropic: Claude Sonnet 5")).toBeTruthy();
  });

  it("offers Only free models for OpenRouter only", async () => {
    await reachModel({ calls: { providerModels: () => ({ status: "ok", data: pricedModels }) } });
    await click(/Choose from 3 models/);
    expect(await screen.findByText("Only free models")).toBeTruthy();
    cleanup();
    await reachModel({ provider: /Google/, kind: "gemini", calls: { providerModels: () => ({ status: "ok", data: pricedModels }) } });
    await click(/Choose from 3 models/);
    await screen.findByText("Qwen: Max");
    expect(screen.queryByText("Only free models")).toBeNull();
  });
});

async function reachMemory(calls: HarnessOptions["calls"], streams?: HarnessOptions["streams"]) {
  const saves = await reachModel({
    calls: { providerModels: () => ({ status: "ok", data: pricedModels }), ...calls },
    ...(streams ? { streams } : {}),
  });
  await click(/Choose from 3 models/);
  fireEvent.click(await screen.findByText("Qwen: Max"));
  await click("Use this model");
  await screen.findByText("How should characters remember?");
  return saves;
}

describe("memory", () => {
  it("manual writes the disabled setting and completes", async () => {
    const updates: unknown[] = [];
    const ui = vi.fn((request: { patch: Record<string, unknown> }) => ({ status: "ok" as const, data: { state: request.patch } }));
    await reachMemory({
      settingsGet: () => ({ status: "ok", data: settingsWith(true) }),
      settingsUpdate: (request) => {
        updates.push(request);
        return { status: "ok", data: settingsWith(false) };
      },
      appUiStateUpdate: ui,
    });
    fireEvent.click(screen.getByRole("radio", { name: /Manual/ }));
    await click("Finish setup");
    await waitFor(() => expect(ui).toHaveBeenCalled());
    expect(updates).toMatchObject([{ patch: { section: "dynamic_memory", value: { enabled: false } }, expected_revision: 7 }]);
  });

  it("manual does not write when memory is already disabled", async () => {
    const update = vi.fn(() => ({ status: "ok" as const, data: settingsWith(false) }));
    const ui = vi.fn((request: { patch: Record<string, unknown> }) => ({ status: "ok" as const, data: { state: request.patch } }));
    await reachMemory({ settingsGet: () => ({ status: "ok", data: settingsWith(false) }), settingsUpdate: update, appUiStateUpdate: ui });
    fireEvent.click(screen.getByRole("radio", { name: /Manual/ }));
    await click("Finish setup");
    await waitFor(() => expect(ui).toHaveBeenCalled());
    expect(update).not.toHaveBeenCalled();
  });

  it("automatic with Eidos installed skips the sheet", async () => {
    const ui = vi.fn((request: { patch: Record<string, unknown> }) => ({ status: "ok" as const, data: { state: request.patch } }));
    const updates: Array<{ patch: { section: string } }> = [];
    await reachMemory({
      embeddingStatus: () => ({ status: "ok", data: [{ family: "lettuce_eidos_v5", revision: "r", native_dimensions: 768, active: true }] }),
      settingsGet: () => ({ status: "ok", data: settingsWith(false) }),
      settingsUpdate: (request) => {
        updates.push(request as { patch: { section: string } });
        return { status: "ok", data: settingsWith(true, "m") };
      },
      appUiStateUpdate: ui,
    });
    await click("Finish setup");
    await waitFor(() => expect(ui).toHaveBeenCalled());
    expect(screen.queryByText("Download the memory model")).toBeNull();
    expect(updates.map((update) => update.patch.section)).toContain("dynamic_memory");
  });

  it("cannot be dismissed while the download runs, and a failure offers retry and manual", async () => {
    let emitJob: ((state: "running" | "failed") => void) | undefined;
    const job = (state: "running" | "failed") => ({
      id: "j1",
      kind: "artifact_install",
      subject: { kind: "artifact_install", id: "x" },
      subject_detail: null,
      state,
      progress: { current: 10, total: 100, unit: "bytes", label_code: "download", bytes_per_second: 5 },
      created_at: 0,
      updated_at: 0,
      failure: null,
      result: null,
    });
    await reachMemory(
      { embeddingInstall: () => ({ status: "ok", data: { job_id: "j1" } }) },
      {
        jobWatch: (_args, emit) => {
          emitJob = (state) => emit({ type: state === "failed" ? "failed" : "progress", job: job(state) } as never);
          return new Promise(() => {});
        },
      },
    );
    await click("Finish setup");
    await click("Download and finish");
    await waitFor(() => expect(emitJob).toBeDefined());
    emitJob?.("running");
    await screen.findByText("Keep the app open until it finishes.");
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.getByText("Keep the app open until it finishes.")).toBeTruthy();
    emitJob?.("failed");
    expect(await screen.findByRole("button", { name: "Use manual memory instead" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  });
});

describe("completion and skip", () => {
  it("writes the completion state once and refreshes app status", async () => {
    const ui = vi.fn((request: { patch: Record<string, unknown> }) => ({ status: "ok" as const, data: { state: request.patch } }));
    const status = vi.fn(() => ({ status: "ok" as const, data: { ...mockAppStatus, version: "3.1.4" } }));
    await reachMemory({ settingsGet: () => ({ status: "ok", data: settingsWith(false) }), appStatus: status, appUiStateUpdate: ui });
    fireEvent.click(screen.getByRole("radio", { name: /Manual/ }));
    const before = status.mock.calls.length;
    await click("Finish setup");
    await waitFor(() => expect(ui).toHaveBeenCalledTimes(1));
    expect(ui.mock.calls[0]?.[0]).toEqual({ patch: { onboarding: { completed: true }, lastSeenAppVersion: "3.1.4" } });
    await waitFor(() => expect(status.mock.calls.length).toBeGreaterThan(before));
  });

  it("shows the typed error and does not navigate when the write fails", async () => {
    const { history } = await renderApp({
      calls: {
        modelsList: () => ({ status: "ok", data: { models: [modelView()], default_model_id: null, revision: 1 } }),
        appUiStateUpdate: () => failure("busy"),
      },
    });
    await click("Other options");
    await click(/Skip setup/);
    expect(await screen.findByText("The app is busy. Try again in a moment.")).toBeTruthy();
    expect(history.location.pathname).toBe("/welcome");
  });

  it("skips directly when a model exists", async () => {
    const ui = vi.fn((request: { patch: Record<string, unknown> }) => ({ status: "ok" as const, data: { state: request.patch } }));
    await renderApp({
      calls: {
        modelsList: () => ({ status: "ok", data: { models: [modelView()], default_model_id: null, revision: 1 } }),
        appUiStateUpdate: ui,
      },
    });
    await click("Other options");
    await click(/Skip setup/);
    await waitFor(() => expect(ui).toHaveBeenCalledTimes(1));
    expect(screen.queryByText("Chats need an AI before they can reply")).toBeNull();
  });

  it("warns first when no model exists", async () => {
    const ui = vi.fn((request: { patch: Record<string, unknown> }) => ({ status: "ok" as const, data: { state: request.patch } }));
    await renderApp({ calls: { appUiStateUpdate: ui } });
    await click("Other options");
    await click(/Skip setup/);
    expect(await screen.findByText("Chats need an AI before they can reply")).toBeTruthy();
    expect(ui).not.toHaveBeenCalled();
    await click("Skip anyway");
    await waitFor(() => expect(ui).toHaveBeenCalledTimes(1));
  });
});

describe("localization", () => {
  it("renders the welcome and connect screens through t() only", async () => {
    await renderApp({ language: "cimode" });
    expect(await screen.findByText("welcome.pitch")).toBeTruthy();
    expect(screen.queryByText(/Chats and characters/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "welcome.start" }));
    expect(await screen.findByText("connect.title")).toBeTruthy();
    expect(document.body.textContent).not.toMatch(/Connect an AI|Cloud services/);
  });
});
