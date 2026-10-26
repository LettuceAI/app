import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createBrowserHistory, createMemoryHistory } from "@tanstack/react-router";
import { toast } from "@lettuceai/crisp";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  ApiErrorCode,
  ModelView,
  PromptSummary,
  ProviderAccountView,
  SettingsHelpMeReplySettings,
  SettingsUpdateRequest_Deserialize,
  SettingsView,
} from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({
  createWindowController: vi.fn().mockResolvedValue(null),
}));
afterEach(() => {
  toast.dismiss();
  cleanup();
  vi.restoreAllMocks();
});
const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const error = (code: ApiErrorCode) => ({
  status: "error" as const,
  error: { code, message: "private diagnostic", details: null },
});
const defaults: SettingsHelpMeReplySettings = {
  enabled: true,
  model_profile_id: null,
  streaming: true,
  max_output_tokens: 150,
  history_count: 10,
  style: "roleplay",
  roleplay_prompt_id: null,
  conversational_prompt_id: null,
};
const account = (id: string, provider_kind = "openai", enabled = true): ProviderAccountView => ({
  id,
  label: id,
  provider_kind,
  enabled,
  protocol: "open_ai_compatible",
  base_url: null,
  streaming_enabled: true,
  allow_invalid_tls: false,
  api_key_set: true,
  config: {},
  revision: 1,
  created_at: 1,
  updated_at: 1,
});
const accounts = [
  account("Cloud"),
  account("Local", "llamacpp"),
  account("Disabled", "openrouter", false),
];
const model = (
  id: string,
  provider = "Cloud",
  output: ModelView["output_scopes"] = ["text"],
): ModelView => ({
  id,
  display_name: id,
  external_model_id: `${id}-slug`,
  provider_account_id: provider,
  kind: "chat",
  input_scopes: ["text"],
  output_scopes: output,
  config: {},
  revision: 1,
  created_at: 1,
  updated_at: 1,
});
const models = [
  model("Default chat"),
  model("Local chat", "Local"),
  model("Disabled chat", "Disabled"),
  model("Image model", "Cloud", ["image"]),
];
const prompt = (id: string, kind: PromptSummary["kind"], builtin = false): PromptSummary => ({
  id,
  name: id,
  kind,
  archived: false,
  app_default: false,
  origin: builtin
    ? { type: "built_in", key: id, protected: true, required: true, edited: false }
    : { type: "user" },
  revision: 1,
  created_at: 1,
  updated_at: 1,
});
const prompts = [
  prompt("Bundled roleplay", "reply_helper_roleplay", true),
  prompt("My roleplay", "reply_helper_roleplay"),
  prompt("Bundled conversation", "reply_helper_conversational", true),
  prompt("My conversation", "reply_helper_conversational"),
];

async function mount(
  options: {
    value?: Partial<SettingsHelpMeReplySettings>;
    calls?: MockCallHandlers;
    gate?: Promise<void>;
    failure?: () => boolean;
    platform?: "other" | "android";
    browser?: boolean;
    path?: string;
    language?: "en" | "cimode";
  } = {},
) {
  let current = {
    global: {
      help_me_reply: { ...defaults, ...options.value },
      creation_helper: { streaming: true, model_profile_id: "keep" },
    },
    default_model_profile_id: "Default chat",
    revision: 7,
  } as SettingsView;
  const requests: SettingsUpdateRequest_Deserialize[] = [];
  const update = vi.fn(async (request: SettingsUpdateRequest_Deserialize) => {
    requests.push(request);
    if (options.gate) await options.gate;
    if (options.failure?.()) return error("unavailable");
    if (request.expected_revision !== current.revision) return error("conflict");
    if (request.patch.section === "help_me_reply")
      current = {
        ...current,
        global: { ...current.global, help_me_reply: request.patch.value },
        revision: current.revision + 1,
      };
    return ok(current);
  });
  const transport = createMockTransport({
    calls: {
      appStatus: () =>
        ok({
          ...mockAppStatus,
          platform: options.platform ?? "android",
          ui_state: { onboarding: { completed: true } },
        }),
      settingsGet: () => ok(current),
      settingsUpdate: update,
      modelsList: () => ok({ models, default_model_id: "Default chat", revision: 1 }),
      providerAccountsList: () => ok(accounts),
      promptsList: ({ kind }) =>
        ok({
          items: prompts.filter((value) => kind === null || value.kind === kind),
          next_cursor: null,
        }),
      jobsList: () => ok({ items: [], next_cursor: null }),
      ...options.calls,
    },
  });
  const path = options.path ?? "/settings/replyHelper";
  if (options.browser) window.history.replaceState(null, "", "/settings");
  const history = options.browser
    ? createBrowserHistory()
    : createMemoryHistory({ initialEntries: ["/settings", path] });
  const runtime = await createAppRuntime({
    transport,
    history,
    i18n: await createTestI18n(options.language ?? "en"),
  });
  if (options.browser)
    await runtime.router.navigate({ to: "/settings/$section", params: { section: "replyHelper" } });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return { runtime, transport, history, update, requests, current: () => current };
}
const sheet = () => within(screen.getAllByRole("dialog").at(-1)!);
const open = async (name: string) => {
  fireEvent.click(
    await screen.findByRole("button", {
      name: new RegExp(`^${name}${name === "Conversational" ? "(?! prompt)" : ""}`),
    }),
  );
  if (name === "Reply model" || name.endsWith("prompt")) await screen.findByRole("dialog");
};

describe("reply helper settings", () => {
  it.each(["android", "other"] as const)(
    "lists the helper sections directly instead of Advanced on %s",
    async (platform) => {
      await mount({ platform, path: platform === "android" ? "/settings" : "/settings/about" });
      const helper = await screen.findByRole("button", { name: /^Reply helper/ });
      expect(helper.hasAttribute("disabled")).toBe(false);
      expect(screen.queryByRole("button", { name: /^Advanced/ })).toBeNull();
      expect(screen.getByRole("button", { name: /^Smart creator/ })).toBeTruthy();
      fireEvent.click(helper);
      expect(await screen.findByRole("switch", { name: "Reply helper" })).toBeTruthy();
    },
  );

  it("preserves all settings when disabling and enabling the feature", async () => {
    const value = {
      model_profile_id: "Local chat",
      style: "conversational" as const,
      history_count: 32,
      max_output_tokens: 300,
      roleplay_prompt_id: "My roleplay",
      conversational_prompt_id: "My conversation",
    };
    const store = await mount({ value });
    fireEvent.click(await screen.findByRole("switch", { name: "Reply helper" }));
    await waitFor(() => expect(screen.queryByText("Length and context")).toBeNull());
    fireEvent.click(screen.getByRole("switch", { name: "Reply helper" }));
    expect(await screen.findByText("Length and context")).toBeTruthy();
    expect(store.current().global.help_me_reply).toEqual({ ...defaults, ...value });
    expect(store.current().global.creation_helper.model_profile_id).toBe("keep");
    expect(store.requests.map((request) => request.expected_revision)).toEqual([7, 8]);
  });

  it("serializes rapid style and streaming writes against the latest revision", async () => {
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const store = await mount({
      gate,
      value: { model_profile_id: "Local chat", roleplay_prompt_id: "My roleplay" },
    });
    await open("Conversational");
    fireEvent.click(screen.getByRole("switch", { name: "Stream suggestions" }));
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(1));
    await act(async () => {
      release();
    });
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(2));
    expect(store.requests.map((request) => request.expected_revision)).toEqual([7, 8]);
    expect(store.current().global.help_me_reply).toMatchObject({
      style: "conversational",
      streaming: false,
      model_profile_id: "Local chat",
      roleplay_prompt_id: "My roleplay",
    });
    expect(screen.getByText(/Example: Wait, did you hear/)).toBeTruthy();
  });

  it("keeps numeric drafts on failure and retries without losing other changes", async () => {
    let fail = true;
    const store = await mount({ failure: () => fail });
    const number = (await screen.findByLabelText("Max length")) as HTMLInputElement;
    fireEvent.change(number, { target: { value: "8200" } });
    fireEvent.blur(number);
    expect(await screen.findByText("Not saved")).toBeTruthy();
    expect(number.value).toBe("8200");
    expect(screen.queryByText("private diagnostic")).toBeNull();
    fail = false;
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(store.current().global.help_me_reply.max_output_tokens).toBe(8200));
    expect(screen.queryByText("Not saved")).toBeNull();
    fireEvent.change(screen.getByLabelText("Messages sent"), { target: { value: "500" } });
    fireEvent.keyDown(screen.getByLabelText("Messages sent"), { key: "Enter" });
    await waitFor(() => expect(store.current().global.help_me_reply.history_count).toBe(500));
    expect(store.current().global.help_me_reply).toMatchObject({
      max_output_tokens: 8200,
      history_count: 500,
      roleplay_prompt_id: null,
    });
  });

  it("keeps a failed numeric draft when a conflict refresh also fails", async () => {
    let reads = 0;
    const loaded = {
      global: { help_me_reply: defaults },
      default_model_profile_id: null,
      revision: 7,
    } as SettingsView;
    await mount({
      calls: {
        settingsGet: () => (reads++ === 0 ? ok(loaded) : error("unavailable")),
        settingsUpdate: () => error("conflict"),
      },
    });
    const number = (await screen.findByLabelText("Max length")) as HTMLInputElement;
    fireEvent.change(number, { target: { value: "8200" } });
    fireEvent.blur(number);
    expect(await screen.findByText("It isn't available right now.")).toBeTruthy();
    expect(await screen.findByText("Not saved")).toBeTruthy();
    expect((screen.getByLabelText("Max length") as HTMLInputElement).value).toBe("8200");
  });

  it("retains a newer numeric edit while an older value is still saving", async () => {
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const store = await mount({ gate });
    const number = await screen.findByLabelText("Max length");
    fireEvent.change(number, { target: { value: "200" } });
    fireEvent.blur(number);
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(1));
    fireEvent.change(number, { target: { value: "150" } });
    fireEvent.blur(number);
    await act(async () => {
      release();
    });
    await waitFor(() => expect(store.update).toHaveBeenCalledTimes(2));
    expect(store.requests.map((request) => request.expected_revision)).toEqual([7, 8]);
    expect(store.current().global.help_me_reply.max_output_tokens).toBe(150);
  });

  it("filters model choices, identifies the default and clears an unavailable selection", async () => {
    const store = await mount({ value: { model_profile_id: "gone" } });
    expect(await screen.findByText("Selected model is unavailable")).toBeTruthy();
    await open("Reply model");
    expect(sheet().queryByRole("button", { name: /Image model/ })).toBeNull();
    expect(
      sheet()
        .getByRole("button", { name: /Disabled chat/ })
        .hasAttribute("disabled"),
    ).toBe(true);
    fireEvent.click(sheet().getByRole("button", { name: /Local chat/ }));
    await waitFor(() =>
      expect(store.current().global.help_me_reply.model_profile_id).toBe("Local chat"),
    );
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await open("Reply model");
    expect(
      sheet()
        .getByRole("button", { name: /Local chat/ })
        .getAttribute("aria-pressed"),
    ).toBe("true");
    fireEvent.click(sheet().getByRole("button", { name: /App default/ }));
    await waitFor(() => expect(store.current().global.help_me_reply.model_profile_id).toBeNull());
    expect(await screen.findByText("App default · Default chat")).toBeTruthy();
  });

  it("pages each prompt kind independently and saves separate style selections", async () => {
    const calls = vi.fn(({ kind, cursor }) =>
      ok({
        items: !cursor ? [prompt(`Bundled ${kind}`, kind, true)] : [prompt(`Custom ${kind}`, kind)],
        next_cursor: cursor ? null : "more",
      }),
    );
    const store = await mount({ calls: { promptsList: calls } });
    await open("Roleplay prompt");
    expect(
      await sheet().findByRole("button", { name: "Custom reply_helper_roleplay" }),
    ).toBeTruthy();
    expect(sheet().queryByText("Custom reply_helper_conversational")).toBeNull();
    fireEvent.click(sheet().getByRole("button", { name: "Custom reply_helper_roleplay" }));
    await waitFor(() =>
      expect(store.current().global.help_me_reply.roleplay_prompt_id).toBe(
        "Custom reply_helper_roleplay",
      ),
    );
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await open("Conversational prompt");
    fireEvent.click(
      await sheet().findByRole("button", { name: "Custom reply_helper_conversational" }),
    );
    await waitFor(() =>
      expect(store.current().global.help_me_reply).toMatchObject({
        roleplay_prompt_id: "Custom reply_helper_roleplay",
        conversational_prompt_id: "Custom reply_helper_conversational",
      }),
    );
    expect(calls).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "reply_helper_roleplay", cursor: "more" }),
    );
    expect(calls).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "reply_helper_conversational", cursor: "more" }),
    );
  });

  it("closes the picker on browser Back and returns from System prompts without reopening it", async () => {
    const { history, runtime } = await mount({ browser: true });
    await open("Roleplay prompt");
    await screen.findByRole("dialog");
    await act(async () => {
      history.back();
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(runtime.router.state.location.pathname).toBe("/settings/replyHelper");
    await open("Conversational prompt");
    fireEvent.click(await sheet().findByRole("button", { name: /^System prompts/ }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/settings/prompts"));
    await act(async () => {
      history.back();
    });
    expect(await screen.findByRole("switch", { name: "Reply helper" })).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
    history.destroy();
  });

  it("keeps the zero-history sentinel and shows typed read failures", async () => {
    const store = await mount({ value: { history_count: 0 } });
    expect(await screen.findByText("Uses the default history length")).toBeTruthy();
    await open("Conversational");
    await waitFor(() => expect(store.current().global.help_me_reply.style).toBe("conversational"));
    expect(store.current().global.help_me_reply.history_count).toBe(0);
    cleanup();
    await mount({ calls: { settingsGet: () => error("unavailable") } });
    expect(await screen.findByText("It isn't available right now.")).toBeTruthy();
    expect(screen.queryByText("private diagnostic")).toBeNull();
  });
});
