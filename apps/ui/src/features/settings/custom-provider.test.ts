import { describe, expect, it } from "vitest";
import { customProviderConfig, customProviderDraft } from "./custom-provider";

describe("custom provider config mapping", () => {
  it("keeps custom Anthropic defaults separate from the OpenAI format", () => {
    expect(customProviderConfig(customProviderDraft("custom"))).toMatchObject({ kind: "custom", chat_path: "/v1/chat/completions", auth: "bearer" });
    expect(customProviderConfig(customProviderDraft("custom-anthropic"))).toMatchObject({ kind: "custom", chat_path: "/v1/messages", auth: { header: { name: "x-api-key" } } });
  });

  it("round trips every stored field including null optional paths and roles", () => {
    const stored = {
      kind: "custom", chat_path: "/chat", models_path: "/models",
      model_list: { list_path: "response.models", id_path: "model.id", display_name_path: null, description_path: "summary", context_length_path: "limits.context" },
      auth: { query: { name: "token" } }, roles: { system: "developer", user: null, assistant: "model" },
      streaming: false, merge_same_role_messages: false, send_chat_template_kwargs: true, tool_choice_mode: "passthrough",
    };
    expect(customProviderConfig(customProviderDraft("custom", stored))).toEqual(stored);
  });

  it("disables model discovery without discarding its response mapping", () => {
    const draft = customProviderDraft("custom", { models_path: "/models", model_list: { list_path: "result.items", id_path: "slug" } });
    expect(customProviderConfig({ ...draft, fetchModels: false, auth: "none" })).toMatchObject({ models_path: null, model_list: { list_path: "result.items", id_path: "slug" }, auth: "none" });
    expect(customProviderConfig({ ...draft, auth: "header", headerName: " X-Token " })).toMatchObject({ auth: { header: { name: "X-Token" } } });
  });
});
