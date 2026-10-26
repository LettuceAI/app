import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  ModelSaveRequest,
  ModelView,
  ProviderAccountSaveRequest,
  ProviderAccountView,
  ProviderDescriptorContract,
  SettingsView,
} from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { minimalModelConfig } from "@/entities/model";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({
  createWindowController: vi.fn().mockResolvedValue(null),
}));
const account: ProviderAccountView = {
  id: "account",
  provider_kind: "openrouter",
  protocol: "open_ai_compatible",
  label: "My OpenRouter",
  base_url: null,
  enabled: false,
  streaming_enabled: false,
  allow_invalid_tls: true,
  api_key_set: true,
  config: { proxy: "preserve" },
  revision: 7,
  created_at: 0,
  updated_at: 0,
};
const descriptor: ProviderDescriptorContract = {
  kind: "openrouter",
  display_name: "OpenRouter",
  protocol: "open_ai_compatible",
  aliases: [],
  default_endpoint: "https://openrouter.ai/api/v1",
  endpoint_editable: true,
  api_key: "required",
  auth_header: "authorization",
  streaming: true,
  tools: true,
  structured_output: true,
  signed_tool_replay: false,
  reasoning_with_tools: false,
  lists_models: true,
  verifies_key: true,
  reasoning: "none",
  prompt_caching: "none",
  prompt_cache_retentions: [],
  parameters: {
    temperature: true,
    top_p: true,
    max_output_tokens: true,
    context_length: true,
    frequency_penalty: true,
    presence_penalty: true,
    top_k: true,
    reasoning_budget: true,
  },
  extra_body_keys: [],
};
const model: ModelView = {
  id: "model",
  provider_account_id: account.id,
  display_name: "My model",
  external_model_id: "vendor/chat",
  kind: "chat",
  input_scopes: ["text", "image"],
  output_scopes: ["text"],
  config: {
    ...minimalModelConfig,
    chat_parameters: { temperature: 0.8, top_p: 0.93 },
    llama_cpp: { threads: 4 },
    feature_parameters: { preserve: true },
  },
  revision: 9,
  created_at: 0,
  updated_at: 0,
};
async function mount(section: "providers" | "models", calls: MockCallHandlers = {}) {
  const transport = createMockTransport({
    calls: {
      appStatus: () => ({
        status: "ok",
        data: {
          ...mockAppStatus,
          platform: "android",
          ui_state: { onboarding: { completed: true } },
        },
      }),
      providerCatalog: () => ({ status: "ok", data: { providers: [descriptor] } }),
      providerAccountsList: () => ({ status: "ok", data: [account] }),
      modelGet: () => ({ status: "ok", data: model }),
      modelUsage: () => ({ status: "ok", data: { characters: [], groups: [], roles: [] } }),
      audioProvidersList: () => ({ status: "ok", data: [] }),
      modelsList: () => ({
        status: "ok",
        data: { models: [model], default_model_id: null, revision: 21 },
      }),
      ...calls,
    },
  });
  const runtime = await createAppRuntime({
    transport,
    history: createMemoryHistory({ initialEntries: [`/settings/${section}`] }),
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return { runtime, transport };
}
async function confirmDelete() {
  const dialog = (await screen.findAllByRole("dialog")).at(-1) as HTMLElement;
  const button = within(dialog).getByRole("button", { name: "Delete permanently" });
  await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
  fireEvent.click(button);
}
async function click(name: string | RegExp) {
  const button = await screen.findByRole("button", { name });
  await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
  fireEvent.click(button);
}
afterEach(() => {
  cleanup();
});

describe("provider settings", () => {
  it("preserves the saved secret and unedited account fields", async () => {
    let saved: ProviderAccountSaveRequest | undefined;
    await mount("providers", {
      providerAccountSave: (request) => {
        saved = request;
        return { status: "ok", data: account };
      },
    });
    await click(/My OpenRouter/);
    fireEvent.change(screen.getByLabelText("Label"), { target: { value: "Renamed" } });
    await click("Save");
    await waitFor(() => expect(saved).toBeDefined());
    expect(saved).toMatchObject({
      account: {
        id: account.id,
        label: "Renamed",
        config: account.config,
        enabled: false,
        allow_invalid_tls: true,
        streaming_enabled: false,
      },
      api_key: null,
      clear_api_key: false,
      expected_revision: 7,
    });
  });

  it("creates a separate account even when the provider kind already exists", async () => {
    let saved: ProviderAccountSaveRequest | undefined;
    await mount("providers", {
      providerAccountSave: (request) => {
        saved = request;
        return { status: "ok", data: account };
      },
    });
    await click("Add");
    fireEvent.change(screen.getByLabelText("API key"), { target: { value: "secret" } });
    await click("Add provider");
    await waitFor(() => expect(saved?.account.id).toBe(null));
    expect(saved?.expected_revision).toBe(null);
    expect(saved?.api_key).toBe("secret");
  });

  it("keeps an in-use provider visible and never cascades model deletion", async () => {
    const remove = vi.fn<NonNullable<MockCallHandlers["providerAccountDelete"]>>(() => ({
      status: "error",
      error: { code: "in_use", message: "private diagnostics", details: null },
    }));
    await mount("providers", { providerAccountDelete: remove });
    await click(/My OpenRouter/);
    await click("Delete");
    await confirmDelete();
    await waitFor(() => expect(remove).toHaveBeenCalled());
    expect(remove.mock.calls[0]?.[0]).toMatchObject({ delete_models: false, expected_revision: 7 });
    expect(await screen.findByRole("dialog")).toBeTruthy();
    expect(screen.queryByText("private diagnostics")).toBeNull();
  });

  it("creates an audio provider through the audio API", async () => {
    const create = vi.fn<NonNullable<MockCallHandlers["audioProviderCreate"]>>(() => ({
      status: "ok",
      data: {
        id: "voice",
        label: "ElevenLabs",
        configuration: { type: "elevenlabs" },
        has_api_key: true,
        revision: 1,
      },
    }));
    await mount("providers", { audioProviderCreate: create });
    fireEvent.click(await screen.findByRole("tab", { name: /Audio/ }));
    await click("Add");
    fireEvent.change(screen.getByLabelText("API key"), { target: { value: "audio-secret" } });
    await click("Add provider");
    await waitFor(() => expect(create).toHaveBeenCalled());
    expect(create.mock.calls[0]?.[0]).toMatchObject({
      draft: { configuration: { type: "elevenlabs" }, api_key: "audio-secret" },
    });
  });
});

describe("model settings", () => {
  it("automatically loads the selected provider catalog when adding a model", async () => {
    const providerModels = vi.fn<NonNullable<MockCallHandlers["providerModels"]>>(() => ({
      status: "ok",
      data: [],
    }));
    await mount("models", { providerModels });
    expect(providerModels).not.toHaveBeenCalled();
    await click("Add");
    await waitFor(() => expect(providerModels).toHaveBeenCalledWith({ account_id: account.id }));
    expect(screen.queryByRole("button", { name: "Load provider models" })).toBeNull();
  });

  it("does not request a provider catalog without an account", async () => {
    const providerModels = vi.fn<NonNullable<MockCallHandlers["providerModels"]>>(() => ({
      status: "ok",
      data: [],
    }));
    await mount("models", {
      providerAccountsList: () => ({ status: "ok", data: [] }),
      providerModels,
    });
    await click("Add");
    expect(providerModels).not.toHaveBeenCalled();
  });

  it("preserves scopes and unrelated configuration when editing", async () => {
    let saved: ModelSaveRequest | undefined;
    await mount("models", {
      modelSave: (request) => {
        saved = request;
        return { status: "ok", data: model };
      },
    });
    await click(/My model/);
    await screen.findByLabelText("Name");
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Renamed model" } });
    await click("Save");
    await waitFor(() => expect(saved).toBeDefined());
    expect(saved?.model.config).toMatchObject(model.config);
    expect(saved?.model.input_scopes).toEqual(["text", "image"]);
    expect(saved?.model.display_name).toBe("Renamed model");
    expect(saved?.expected_revision).toBe(9);
  });

  it("sets the default using the selection revision and refreshes its badge", async () => {
    let selected: string | null = null;
    const setDefault = vi.fn<NonNullable<MockCallHandlers["modelDefaultSet"]>>((request) => {
      selected = request.model_id;
      return { status: "ok", data: { model_id: selected, revision: 22 } };
    });
    await mount("models", {
      modelsList: () => ({
        status: "ok",
        data: { models: [model], default_model_id: selected, revision: 21 },
      }),
      modelDefaultSet: setDefault,
    });
    fireEvent.contextMenu(await screen.findByRole("button", { name: /My model/ }));
    await click(/Set as default/);
    await waitFor(() => expect(setDefault).toHaveBeenCalled());
    expect(setDefault.mock.calls[0]?.[0]).toMatchObject({
      model_id: model.id,
      expected_revision: 21,
    });
    expect(await screen.findByText("Default")).toBeTruthy();
  });

  it("does not export when the native save picker is cancelled", async () => {
    const exportModel = vi.fn<NonNullable<MockCallHandlers["modelExport"]>>(() => ({
      status: "ok",
      data: { uri: "file", suggested_name: "model.uec", warnings: [] },
    }));
    const pick = vi.fn<NonNullable<MockCallHandlers["filesPickSave"]>>(() => ({
      status: "ok",
      data: { target: null },
    }));
    await mount("models", { filesPickSave: pick, modelExport: exportModel });
    fireEvent.contextMenu(await screen.findByRole("button", { name: /My model/ }));
    await click(/Export/);
    await waitFor(() => expect(pick).toHaveBeenCalled());
    expect(pick.mock.calls[0]?.[0].suggested_name).toBe("My model.uec");
    expect(exportModel).not.toHaveBeenCalled();
  });

  it("searches model names, IDs and providers across views, including collapsed groups", async () => {
    await mount("models", {
      modelsList: () => ({
        status: "ok",
        data: {
          models: [
            model,
            {
              ...model,
              id: "other",
              display_name: "Other model",
              external_model_id: "vendor/other",
            },
          ],
          default_model_id: null,
          revision: 21,
        },
      }),
    });
    const search = await screen.findByRole("searchbox", { name: "Search models or providers" });
    fireEvent.change(search, { target: { value: "  MY MODEL  " } });
    expect(screen.getByRole("button", { name: /My model/ })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Other model/ })).toBeNull();
    fireEvent.change(search, { target: { value: "vendor/other" } });
    expect(screen.getByRole("button", { name: /Other model/ })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /My model/ })).toBeNull();
    fireEvent.change(search, { target: { value: "" } });
    await click("Model list view");
    fireEvent.click(await screen.findByRole("menuitem", { name: "By provider" }));
    await click(account.label);
    fireEvent.change(search, { target: { value: "openrouter" } });
    expect(screen.getByRole("button", { name: /My model/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Other model/ })).toBeTruthy();
    await click("Model list view");
    fireEvent.click(await screen.findByRole("menuitem", { name: "Provider columns" }));
    expect(screen.getByRole("button", { name: /My model/ })).toBeTruthy();
    fireEvent.change(search, { target: { value: "missing" } });
    expect(screen.getByRole("status").textContent).toBe("No matching models found.");
    fireEvent.change(search, { target: { value: "" } });
    expect(screen.getByRole("button", { name: account.label }).getAttribute("aria-expanded")).toBe(
      "false",
    );
  });

  it.each([
    ["By provider", "provider"],
    ["Provider columns", "providerColumns"],
    ["All models", "all"],
  ])(
    "writes the %s view to the device UI state and restores it after remount",
    async (label, value) => {
      let uiState: Record<string, unknown> = {
        onboarding: { completed: true },
        modelsListView: "provider",
      };
      const update = vi.fn<NonNullable<MockCallHandlers["appUiStateUpdate"]>>((request) => {
        uiState = { ...uiState, ...request.patch };
        return { status: "ok", data: { state: uiState } };
      });
      const overrides: MockCallHandlers = {
        appStatus: () => ({
          status: "ok",
          data: { ...mockAppStatus, platform: "android", ui_state: uiState },
        }),
        appUiStateUpdate: update,
      };
      await mount("models", overrides);
      await click("Model list view");
      fireEvent.click(await screen.findByRole("menuitem", { name: label }));
      await waitFor(() =>
        expect(update).toHaveBeenCalledWith({ patch: { modelsListView: value } }),
      );
      expect(window.localStorage.getItem("models-list-view")).toBeNull();
      cleanup();
      await mount("models", overrides);
      expect(
        (await screen.findByRole("button", { name: "Model list view" })).getAttribute("title"),
      ).toBe(label);
    },
  );

  it("falls back to All models for an invalid saved view", async () => {
    await mount("models", {
      appStatus: () => ({
        status: "ok",
        data: {
          ...mockAppStatus,
          platform: "android",
          ui_state: { onboarding: { completed: true }, modelsListView: "invalid" },
        },
      }),
    });
    expect(
      (await screen.findByRole("button", { name: "Model list view" })).getAttribute("title"),
    ).toBe("All models");
  });

  it("deletes a model only after the confirmation is accepted", async () => {
    const remove = vi.fn<NonNullable<MockCallHandlers["modelDelete"]>>(() => ({
      status: "ok",
      data: { revision: 22 } as never,
    }));
    await mount("models", { modelDelete: remove });
    fireEvent.contextMenu(await screen.findByRole("button", { name: /My model/ }));
    await click(/Characters using it fall back/);
    expect(remove).not.toHaveBeenCalled();
    await click("Cancel");
    await waitFor(() =>
      expect(
        screen.queryByText("Delete this model permanently? References to it will be cleared."),
      ).toBeNull(),
    );
    expect(remove).not.toHaveBeenCalled();
    fireEvent.contextMenu(await screen.findByRole("button", { name: /My model/ }));
    await click(/Characters using it fall back/);
    await click("Delete permanently");
    await waitFor(() => expect(remove).toHaveBeenCalled());
    expect(remove.mock.calls[0]?.[0]).toMatchObject({ model_id: model.id, expected_revision: 9 });
  });

  it("collapses providers independently and keeps their state across grouped views", async () => {
    const second = { ...account, id: "second", label: "Second provider" };
    await mount("models", {
      providerAccountsList: () => ({ status: "ok", data: [account, second] }),
      modelsList: () => ({
        status: "ok",
        data: {
          models: [
            model,
            {
              ...model,
              id: "second-model",
              provider_account_id: second.id,
              display_name: "Second model",
            },
          ],
          default_model_id: null,
          revision: 21,
        },
      }),
    });
    await click("Model list view");
    fireEvent.click(await screen.findByRole("menuitem", { name: "By provider" }));
    await click(account.label);
    expect(screen.getByRole("button", { name: account.label }).getAttribute("aria-expanded")).toBe(
      "false",
    );
    expect(screen.queryByRole("button", { name: /My model/ })).toBeNull();
    expect(screen.getByRole("button", { name: /Second model/ })).toBeTruthy();
    await click("Model list view");
    fireEvent.click(await screen.findByRole("menuitem", { name: "Provider columns" }));
    expect(screen.queryByRole("button", { name: /My model/ })).toBeNull();
    await click(account.label);
    expect(screen.getByRole("button", { name: account.label }).getAttribute("aria-expanded")).toBe(
      "true",
    );
    expect(screen.getByRole("button", { name: /My model/ })).toBeTruthy();
    await click(second.label);
    expect(screen.queryByRole("button", { name: /Second model/ })).toBeNull();
    await click("Model list view");
    fireEvent.click(await screen.findByRole("menuitem", { name: "All models" }));
    expect(screen.getByRole("button", { name: /Second model/ })).toBeTruthy();
  });

  it("groups by provider and reacts to backend model changes", async () => {
    let models = [model];
    const { transport } = await mount("models", {
      modelsList: () => ({ status: "ok", data: { models, default_model_id: null, revision: 21 } }),
    });
    await click("Model list view");
    fireEvent.click(await screen.findByRole("menuitem", { name: "By provider" }));
    expect(await screen.findByRole("heading", { name: account.label })).toBeTruthy();
    await click("Model list view");
    fireEvent.click(await screen.findByRole("menuitem", { name: "Provider columns" }));
    expect(await screen.findByRole("heading", { name: account.label })).toBeTruthy();
    expect(screen.getByRole("button", { name: /My model/ })).toBeTruthy();
    models = [];
    await act(async () => transport.emitAppEvent({ type: "models_changed" }));
    expect(await screen.findByText("No models yet. Add one to get started.")).toBeTruthy();
  });
});

describe("local model tools", () => {
  it("shows only the Hugging Face browser on mobile, and only with an Ollama provider", async () => {
    await mount("models");
    await screen.findByText("Your models");
    expect(screen.queryByRole("button", { name: /Runtime defaults/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /LoRA library/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /Get models/ })).toBeNull();
    cleanup();
    await mount("models", {
      providerAccountsList: () => ({
        status: "ok",
        data: [account, { ...account, id: "ollama", label: "Ollama", provider_kind: "ollama" }],
      }),
    });
    expect(await screen.findByRole("button", { name: /Get models/ })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Runtime defaults/ })).toBeNull();
  });

  it("saves runtime defaults with the settings revision", async () => {
    const settings = {
      global: { llama_default_context_length: 4096, llama_default_kv_cache_type: "q8_0" },
      revision: 17,
    } as unknown as SettingsView;
    const save = vi.fn<NonNullable<MockCallHandlers["settingsUpdate"]>>(() => ({
      status: "ok",
      data: settings,
    }));
    await mount("models", {
      appStatus: () => ({
        status: "ok",
        data: { ...mockAppStatus, ui_state: { onboarding: { completed: true } } },
      }),
      settingsGet: () => ({ status: "ok", data: settings }),
      settingsUpdate: save,
    });
    await click("Runtime");
    fireEvent.change(await screen.findByLabelText("Context length"), { target: { value: "8192" } });
    await click("Save");
    await waitFor(() => expect(save).toHaveBeenCalled());
    expect(save.mock.calls[0]?.[0]).toEqual({
      expected_revision: 17,
      patch: { section: "local_runtime", context_length: 8192, kv_cache_type: "q8_0" },
    });
  });

  it("imports a LoRA from a native file selection and refreshes the library", async () => {
    let installed = false;
    const lora = {
      path: "/models/style.safetensors",
      filename: "style.safetensors",
      bytes_on_disk: 1024,
      keywords: [],
      keyword_source: "none" as const,
      architecture: null,
      architecture_source: "none" as const,
      compatibility: "unknown" as const,
    };
    const add = vi.fn<NonNullable<MockCallHandlers["lorasImport"]>>(() => {
      installed = true;
      return { status: "ok", data: lora };
    });
    await mount("models", {
      appStatus: () => ({
        status: "ok",
        data: { ...mockAppStatus, ui_state: { onboarding: { completed: true } } },
      }),
      lorasList: () => ({ status: "ok", data: { loras: installed ? [lora] : [] } }),
      filesPickOpen: () => ({
        status: "ok",
        data: { sources: [{ uri: "content://chosen-adapter" }] },
      }),
      lorasImport: add,
    });
    await click("LoRAs");
    await click("Import");
    expect(await screen.findByText("style.safetensors")).toBeTruthy();
    expect(add.mock.calls[0]?.[0]).toEqual({ source: { uri: "content://chosen-adapter" } });
  });

  it("downloads the selected GGUF at its listed revision and requests model registration", async () => {
    const download = vi.fn<NonNullable<MockCallHandlers["hfDownload"]>>(() => ({
      status: "ok",
      data: { job_id: "download" },
    }));
    await mount("models", {
      appStatus: () => ({
        status: "ok",
        data: { ...mockAppStatus, ui_state: { onboarding: { completed: true } } },
      }),
      jobsList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
      hfSearch: () => ({
        status: "ok",
        data: {
          models: [
            {
              model_id: "author/model",
              author: "author",
              likes: 0,
              downloads: 10,
              tags: [],
              pipeline_tag: null,
              last_modified: null,
              trending_score: null,
            },
          ],
        },
      }),
      hfModelFiles: () => ({
        status: "ok",
        data: {
          model_id: "author/model",
          revision: "commit123",
          author: "author",
          likes: 0,
          downloads: 10,
          tags: [],
          architecture: null,
          context_length: null,
          parameter_count: null,
          last_modified: null,
          files: [
            {
              filename: "model.gguf",
              display_name: null,
              parts: [],
              size: 1024,
              quantization: "Q4_K_M",
              is_mmproj: false,
              is_mtp: false,
              imatrix: false,
            },
          ],
        },
      }),
      hfBrowserLinks: () => ({
        status: "ok",
        data: { repository_base: "https://huggingface.co", ollama_prefix: "hf.co/" },
      }),
      hfAuthStatus: () => ({ status: "ok", data: { type: "missing" } }),
      hfReadme: () => ({ status: "ok", data: { markdown: "Model information" } }),
      hfAvatars: () => ({ status: "ok", data: { avatars: [] } }),
      localModelsList: () => ({ status: "ok", data: { files: [] } }),
      hfRecommendation: () => ({
        status: "ok",
        data: {
          preview: null,
          hardware_available: false,
          metadata_available: false,
          available_ram: 0,
          available_vram: 0,
          supports_gpu_offload: false,
          unified_memory: false,
          total_available: 0,
          kv_base_per_token: null,
          kv_context_cap: null,
          model_max_context: 0,
          arch: null,
          files: [],
          best: null,
          gpu_layer_count: null,
          kv_types: [],
          plan: null,
        },
      }),
      hfDownload: download,
    });
    await click(/Get models/);
    await click(/author\/model/);
    await click("Choose a file");
    await click(/Q4_K_M/);
    await click("Download Q4_K_M");
    await waitFor(() => expect(download).toHaveBeenCalled());
    expect(download.mock.calls[0]?.[0]).toMatchObject({
      repo: "author/model",
      revision: "commit123",
      file: "model.gguf",
      setup: { create_model: true },
    });
  });
});

describe("custom provider editor", () => {
  const customDescriptor = {
    ...descriptor,
    kind: "custom",
    display_name: "Custom OpenAI",
    api_key: "optional" as const,
    default_endpoint: null,
  };
  it("saves custom endpoints, authentication, model mapping, roles and TLS through the account API", async () => {
    let saved: ProviderAccountSaveRequest | undefined;
    await mount("providers", {
      providerCatalog: () => ({ status: "ok", data: { providers: [customDescriptor] } }),
      providerAccountSave: (request) => {
        saved = request;
        return { status: "ok", data: account };
      },
    });
    await click("Add");
    fireEvent.change(screen.getByLabelText("Base URL"), {
      target: { value: "https://custom.test" },
    });
    fireEvent.change(screen.getByLabelText("Authentication"), { target: { value: "query" } });
    fireEvent.change(screen.getByLabelText("Authentication query parameter"), {
      target: { value: "token" },
    });
    fireEvent.change(screen.getByLabelText("API key"), { target: { value: "secret" } });
    fireEvent.change(screen.getByLabelText("Chat endpoint"), { target: { value: "/chat" } });
    fireEvent.click(screen.getByRole("switch", { name: "Fetch models" }));
    fireEvent.change(screen.getByLabelText("Models endpoint"), { target: { value: "/catalog" } });
    fireEvent.change(screen.getByLabelText("Model list JSON path"), {
      target: { value: "result.items" },
    });
    fireEvent.change(screen.getByLabelText("System role"), { target: { value: "developer" } });
    fireEvent.click(screen.getByRole("switch", { name: "Allow invalid TLS certificates" }));
    fireEvent.click(screen.getByRole("switch", { name: "Sync reasoning state" }));
    await click("Add provider");
    await waitFor(() => expect(saved).toBeDefined());
    expect(saved).toMatchObject({
      api_key: "secret",
      account: {
        base_url: "https://custom.test",
        allow_invalid_tls: true,
        config: {
          kind: "custom",
          chat_path: "/chat",
          models_path: "/catalog",
          auth: { query: { name: "token" } },
          model_list: { list_path: "result.items" },
          roles: { system: "developer" },
          send_chat_template_kwargs: true,
        },
      },
    });
  });

  it("shows one streaming switch for a custom provider and writes it to both stream settings", async () => {
    let saved: ProviderAccountSaveRequest | undefined;
    await mount("providers", {
      providerCatalog: () => ({ status: "ok", data: { providers: [customDescriptor] } }),
      providerAccountSave: (request) => {
        saved = request;
        return { status: "ok", data: account };
      },
    });
    await click("Add");
    expect(screen.queryByRole("switch", { name: "Stream responses" })).toBeNull();
    fireEvent.change(screen.getByLabelText("Base URL"), {
      target: { value: "https://custom.test" },
    });
    fireEvent.change(screen.getByLabelText("API key"), { target: { value: "secret" } });
    fireEvent.click(screen.getByRole("switch", { name: "Supports streaming" }));
    await click("Add provider");
    await waitFor(() => expect(saved).toBeDefined());
    expect(saved?.account.streaming_enabled).toBe(false);
    expect(saved?.account.config).toMatchObject({ streaming: false });
  });

  it("supports custom Anthropic without authentication and requires a models endpoint when discovery is enabled", async () => {
    let saved: ProviderAccountSaveRequest | undefined;
    await mount("providers", {
      providerCatalog: () => ({
        status: "ok",
        data: {
          providers: [
            { ...customDescriptor, kind: "custom-anthropic", display_name: "Custom Anthropic" },
          ],
        },
      }),
      providerAccountSave: (request) => {
        saved = request;
        return { status: "ok", data: account };
      },
    });
    await click("Add");
    expect((screen.getByLabelText("Chat endpoint") as HTMLInputElement).value).toBe("/v1/messages");
    expect(screen.queryByLabelText("Tool choice mode")).toBeNull();
    fireEvent.change(screen.getByLabelText("Authentication"), { target: { value: "none" } });
    expect(screen.queryByLabelText("API key")).toBeNull();
    fireEvent.change(screen.getByLabelText("Base URL"), {
      target: { value: "http://localhost:8080" },
    });
    fireEvent.click(screen.getByRole("switch", { name: "Fetch models" }));
    fireEvent.change(screen.getByLabelText("Models endpoint"), { target: { value: "" } });
    expect(screen.getByRole("button", { name: "Add provider" }).hasAttribute("disabled")).toBe(
      true,
    );
    fireEvent.click(screen.getByRole("switch", { name: "Fetch models" }));
    await click("Add provider");
    await waitFor(() => expect(saved).toBeDefined());
    expect(saved).toMatchObject({
      api_key: null,
      account: { config: { auth: "none", models_path: null, chat_path: "/v1/messages" } },
    });
  });
});
