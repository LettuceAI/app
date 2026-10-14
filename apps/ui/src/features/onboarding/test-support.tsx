import { act, render } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import type { CallCommandName, CommandArgs, CommandData, StreamArgs, StreamCommandName, StreamEvent, Transport } from "@/api/transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import type {
  ApiError,
  ModelView,
  ProviderAccountView,
  ProviderDescriptorContract,
  RemoteModelContract_Serialize,
} from "@/api/generated/bindings";
import { createTestI18n } from "@/shared/testing/i18n";

export function descriptor(overrides: Partial<ProviderDescriptorContract> & { kind: string }): ProviderDescriptorContract {
  return {
    display_name: overrides.kind,
    protocol: "open_ai_compatible",
    aliases: [],
    default_endpoint: `https://api.${overrides.kind}.test/v1`,
    endpoint_editable: false,
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
    ...overrides,
  };
}

export const catalog: ProviderDescriptorContract[] = [
  descriptor({ kind: "openrouter", display_name: "OpenRouter" }),
  descriptor({ kind: "gemini", display_name: "Google (Gemini)" }),
  descriptor({ kind: "ollama", display_name: "Ollama (Local)", default_endpoint: "http://127.0.0.1:11434", endpoint_editable: true, api_key: "optional", verifies_key: false }),
  descriptor({ kind: "custom", display_name: "Custom (OpenAI-format)", default_endpoint: null, endpoint_editable: true, api_key: "optional", verifies_key: false, lists_models: false }),
];

export function remoteModel(overrides: Partial<RemoteModelContract_Serialize> & { id: string }): RemoteModelContract_Serialize {
  return {
    cache_read_price: null,
    display_name: null,
    description: null,
    context_length: 128000,
    input_modalities: ["text"],
    output_modalities: ["text"],
    supported_endpoints: null,
    input_price: null,
    output_price: null,
    ...overrides,
  };
}

export function accountView(overrides: Partial<ProviderAccountView> = {}): ProviderAccountView {
  return {
    id: "00000000-0000-0000-0000-0000000000a1",
    provider_kind: "openrouter",
    protocol: "open_ai_compatible",
    label: "My OpenRouter",
    base_url: null,
    enabled: true,
    streaming_enabled: true,
    allow_invalid_tls: false,
    api_key_set: true,
    config: {},
    revision: 1,
    created_at: 0,
    updated_at: 0,
    ...overrides,
  };
}

export function modelView(overrides: Partial<ModelView> = {}): ModelView {
  return {
    id: "00000000-0000-0000-0000-0000000000b1",
    provider_account_id: "00000000-0000-0000-0000-0000000000a1",
    external_model_id: "x",
    display_name: "x",
    kind: "chat",
    config: {},
    input_scopes: ["text"],
    output_scopes: ["text"],
    revision: 1,
    created_at: 0,
    updated_at: 0,
    ...overrides,
  };
}

export function failure(code: ApiError["code"], details: ApiError["details"] = null) {
  return { status: "error" as const, error: { code, message: "diagnostic", details } };
}

type Awaitable<T> = T | Promise<T>;
type Outcome<K extends CallCommandName> = { status: "ok"; data: CommandData<K> } | { status: "error"; error: ApiError };
type MockCallHandlers = { [K in CallCommandName]?: (...args: CommandArgs<K>) => Awaitable<Outcome<K>> };
type MockCalls = MockCallHandlers;
type MockStreamHandlers = {
  [K in StreamCommandName]?: (args: StreamArgs<K>, emit: (event: StreamEvent<K>) => void) => Awaitable<{ status: "ok"; data: CommandData<K> } | { status: "error"; error: ApiError }>;
};
type MockStreams = MockStreamHandlers;

export interface HarnessOptions {
  path?: string;
  calls?: MockCallHandlers;
  streams?: MockStreamHandlers;
  language?: "en" | "cimode";
}

export async function renderWith(createMockTransport: (options: { calls: MockCalls; streams?: MockStreams }) => Transport, options: HarnessOptions = {}) {
  const transport = createMockTransport({
    calls: {
      providerCatalog: () => ({ status: "ok", data: { providers: catalog } }),
      providerAccountsList: () => ({ status: "ok", data: [] }),
      modelsList: () => ({ status: "ok", data: { models: [], default_model_id: null, revision: 0 } }),
      embeddingStatus: () => ({ status: "ok", data: [] }),
      ...options.calls,
    },
    ...(options.streams ? { streams: options.streams } : {}),
  });
  const history = createMemoryHistory({ initialEntries: [options.path ?? "/welcome"] });
  const runtime = await createAppRuntime({ transport, history, i18n: await createTestI18n(options.language ?? "en") });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return { runtime, history };
}
