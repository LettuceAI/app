import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ModelView, ProviderAccountView, ProviderDescriptorContract } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { minimalModelConfig } from "@/entities/model";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));
const account: ProviderAccountView = { id: "account", provider_kind: "openrouter", protocol: "open_ai_compatible", label: "My OpenRouter", base_url: null, enabled: false, streaming_enabled: false, allow_invalid_tls: true, api_key_set: true, config: { proxy: "preserve" }, revision: 7, created_at: 0, updated_at: 0 };
const descriptor: ProviderDescriptorContract = { kind: "openrouter", display_name: "OpenRouter", protocol: "open_ai_compatible", aliases: [], default_endpoint: "https://openrouter.ai/api/v1", endpoint_editable: true, api_key: "required", auth_header: "authorization", streaming: true, tools: true, structured_output: true, signed_tool_replay: false, reasoning_with_tools: false, lists_models: true, verifies_key: true, reasoning: "none", prompt_caching: "none", prompt_cache_retentions: [], parameters: { temperature: true, top_p: true, max_output_tokens: true, context_length: true, frequency_penalty: true, presence_penalty: true, top_k: true, reasoning_budget: true }, extra_body_keys: [] };
const model: ModelView = { id: "model", provider_account_id: account.id, display_name: "My model", external_model_id: "vendor/chat", kind: "chat", input_scopes: ["text", "image"], output_scopes: ["text"], config: { ...minimalModelConfig, chat_parameters: { temperature: 0.8, top_p: 0.93 }, llama_cpp: { threads: 4 }, feature_parameters: { preserve: true } }, revision: 9, created_at: 0, updated_at: 0 };
async function mount(section: "providers" | "models", calls: MockCallHandlers = {}) {
  const transport = createMockTransport({ calls: {
    appStatus: () => ({ status: "ok", data: { ...mockAppStatus, platform: "android", ui_state: { onboarding: { completed: true } } } }),
    providerCatalog: () => ({ status: "ok", data: { providers: [descriptor] } }),
    providerAccountsList: () => ({ status: "ok", data: [account] }),
    audioProvidersList: () => ({ status: "ok", data: [] }),
    modelsList: () => ({ status: "ok", data: { models: [model], default_model_id: null, revision: 21 } }),
    ...calls,
  } });
  const runtime = await createAppRuntime({ transport, history: createMemoryHistory({ initialEntries: [`/settings/${section}`] }), i18n: await createTestI18n("en") });
  await act(async () => { render(<App runtime={runtime} />); });
  return { runtime, transport };
}
async function click(name: string | RegExp) { const button = await screen.findByRole("button", { name }); await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false)); fireEvent.click(button); }
async function confirmDelete() {
  const dialog = (await screen.findAllByRole("dialog")).at(-1) as HTMLElement;
  const button = within(dialog).getByRole("button", { name: "Delete permanently" });
  await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
  fireEvent.click(button);
}
afterEach(() => {
  cleanup();
});

describe("provider delete", () => {
  it("asks before deleting a provider together with its models, then deletes both", async () => {
    const remove = vi.fn<NonNullable<MockCallHandlers["providerAccountDelete"]>>((request) =>
      request.delete_models
        ? { status: "ok", data: null }
        : { status: "error", error: { code: "in_use", message: "in use", details: { type: "provider_models_in_use", models: [model.id] } } });
    await mount("providers", { providerAccountDelete: remove });
    await click(/My OpenRouter/); await click("Delete"); await confirmDelete();
    expect(await screen.findByText("This provider still has a model: My model. Deleting the provider deletes it too.")).toBeTruthy();
    await click("Delete provider and 1 model");
    await waitFor(() => expect(remove).toHaveBeenCalledTimes(2));
    expect(remove.mock.calls[1]?.[0]).toMatchObject({ account_id: account.id, delete_models: true });
  });

  it("deletes nothing more when the person cancels", async () => {
    const remove = vi.fn<NonNullable<MockCallHandlers["providerAccountDelete"]>>(() =>
      ({ status: "error", error: { code: "in_use", message: "in use", details: { type: "provider_models_in_use", models: [model.id] } } }));
    await mount("providers", { providerAccountDelete: remove });
    await click(/My OpenRouter/); await click("Delete"); await confirmDelete();
    const dialog = (await screen.findByText("Delete provider and 1 model")).closest("[role=dialog], [role=alertdialog]");
    fireEvent.click(within(dialog as HTMLElement).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByText("Delete provider and 1 model")).toBeNull());
    expect(remove).toHaveBeenCalledTimes(1);
  });

  it("does not list the providers the app manages itself", async () => {
    const local = { ...account, id: "local", provider_kind: "llamacpp", label: "llama.cpp" };
    const sd = { ...account, id: "sd", provider_kind: "sdcpp", label: "stable-diffusion.cpp" };
    await mount("providers", { providerAccountsList: () => ({ status: "ok", data: [account, local, sd] }) });
    expect(await screen.findByText("My OpenRouter")).toBeTruthy();
    expect(screen.queryByText("llama.cpp")).toBeNull();
    expect(screen.queryByText("stable-diffusion.cpp")).toBeNull();
  });

  it("groups providers, counts their models and shows an address only when it says something", async () => {
    const lmstudio: ProviderDescriptorContract = { ...descriptor, kind: "lmstudio", display_name: "LM Studio", api_key: "optional", default_endpoint: "http://127.0.0.1:1234" };
    const moved = { ...account, id: "moved", label: "Proxy OpenRouter", base_url: "https://proxy.example/api" };
    const local = { ...account, id: "lm", provider_kind: "lmstudio", label: "LM Studio", api_key_set: false };
    await mount("providers", {
      providerCatalog: () => ({ status: "ok", data: { providers: [descriptor, lmstudio] } }),
      providerAccountsList: () => ({ status: "ok", data: [account, moved, local] }),
      modelsList: () => ({ status: "ok", data: { models: [model, { ...model, id: "second" }], default_model_id: null, revision: 1 } }),
    });
    expect(await screen.findByRole("region", { name: "Cloud" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Local" })).toBeTruthy();
    expect(await screen.findByText("OpenRouter · 2 models")).toBeTruthy();
    expect(screen.getByText("OpenRouter · No models yet · https://proxy.example/api")).toBeTruthy();
    expect(screen.getByText("LM Studio · No models yet · http://127.0.0.1:1234")).toBeTruthy();
  });
});
