import { act, cleanup, configure, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ConversationSettingsView, ConversationSummary, ConversationView, TimelineMessage } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers, type MockStreamHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

function message(id: string, role: TimelineMessage["role"], text: string, patch: Partial<TimelineMessage> = {}): TimelineMessage {
  return {
    id, role, author_participant_id: role === "user" ? "pu" : role === "assistant" ? "pc" : null, parts: [{ type: "text", text }], reasoning: null,
    created_at: 1_700_000_000_000, candidate_index: null, candidate_count: 1, pinned: false, scene_image: null, ...patch,
  };
}

function view(messages: TimelineMessage[], patch: Partial<ConversationView> = {}): ConversationView {
  return {
    id: "c1", origin_conversation_id: null, origin_message_id: null, kind: "direct", title: "Harbour", branch: { branch_id: "b", head_message_id: null },
    participants: [
      { id: "pu", role: "user", name: "Hunter", character_id: null, avatar: null },
      { id: "pc", role: "character", name: "Cassian", character_id: "ch1", avatar: null },
    ],
    messages: { items: messages, next_cursor: null }, pending_turn_id: null, can_send: true, revision: 4, settings_revision: 2, archived: false,
    source: { type: "direct", character_id: "ch1" }, chat_mode: null, missing_models: [], memory_blocked: null, ...patch,
  };
}

const settings: ConversationSettingsView = {
  conversation_id: "c1", kind: "direct", title: "Harbour", revision: 4, settings_revision: 2, author_note: null,
  persona: { id: null, source: "app_default" }, prompt: { id: null, source: "app_default" }, roleplay_prompt: null,
  lorebooks: { ids: [], source: "app_default" }, model: { id: "m1", source: "character" }, background: { asset: null, hidden: false, source: "character" },
  scene: { id: null, source: "app_default" }, memory: { mode: "manual", source: "app_default" }, chat_mode: null,
  disable_character_lorebooks: null, speaker_selection: null, members: null, model_settings: { chat_parameters: {}, llama_cpp: {}, stable_diffusion: {} } as never,
};

interface Setup {
  messages?: () => TimelineMessage[];
  calls?: MockCallHandlers;
  streams?: MockStreamHandlers;
  platform?: "other" | "android";
  path?: string;
}

async function open(setup: Setup = {}) {
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: () => ({ status: "ok", data: { ...mockAppStatus, platform: setup.platform ?? "other", ui_state: { onboarding: { completed: true } } } }),
        conversationOpen: () => ({ status: "ok", data: view((setup.messages ?? (() => [message("a1", "assistant", "Sit down.")]))()) }),
        conversationSettingsGet: () => ({ status: "ok", data: settings }),
        modelsList: () => ({ status: "ok", data: { models: [], default_model_id: null, revision: 1 } }),
        providerAccountsList: () => ({ status: "ok", data: [] }),
        memoryGet: () => ({ status: "ok", data: { items: [], summary: null, revision: 1, status: { run_mode: "auto", interval: 10, messages_since_last_cycle: 0, messages_until_next_cycle: 10, total_conversation_messages: 1, pending_approval_count: null, skipped: false, latest_cycle_status: null, latest_job_id: null, failure: null, paused_reason: null } } }),
        llmMetricsForMessage: () => ({ status: "ok", data: null }),
        charactersList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        conversationsLatestByCharacter: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        conversationsList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        ...setup.calls,
      },
      ...(setup.streams ? { streams: setup.streams } : {}),
    }),
    history: createMemoryHistory({ initialEntries: [setup.path ?? "/chat/c1"] }),
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return runtime;
}



configure({ asyncUtilTimeout: 4000 });

afterEach(() => {
  cleanup();
  window.localStorage.removeItem("chat.authorNote.inlineEditor");
});

const character = {
  id: "ch1", archived: false, assets: [], revision: 1, created_at: 1, updated_at: 1,
  draft: {
    profile: { name: "Cassian" },
    defaults: { interaction_mode: "roleplay", voice: null },
    scenes: [{ id: "s1", ordinal: 0, content: { format_version: 1, parts: [{ kind: "text", text: "A rainy harbour" }] }, direction: null, selected_variant_id: null, assets: [] }],
    starters: [],
  },
};

function summary(id: string, title: string, patch: Partial<ConversationSummary> = {}): ConversationSummary {
  return {
    id, kind: "direct", title, avatars: [], background: null, revision: 3, last_message_preview: `About ${title}`, updated_at: 1_700_000_000_000, archived: false,
    source: { type: "direct", character_id: "ch1" }, message_count: 5, chat_mode: null, missing_models: [], memory_blocked: null, ...patch,
  };
}

const panelCalls: MockCallHandlers = {
  characterGet: () => ({ status: "ok", data: character as never }),
  conversationMessageCount: () => ({ status: "ok", data: { count: 12 } }),
  conversationsList: () => ({ status: "ok", data: { items: [summary("c1", "Harbour"), summary("c2", "Lighthouse")], next_cursor: null } }),
  personasList: () => ({ status: "ok", data: { items: [], next_cursor: null, default_persona: { persona_id: null, revision: 1, created_at: 1, updated_at: 1 } } }),
  lorebooksList: () => ({ status: "ok", data: { items: [{ id: "lb1", name: "Harbour lore", status: "ready", detection: "keyword", icon: null, revision: 1, created_at: 1, updated_at: 1 } as never], next_cursor: null } }),
  promptsList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
};

describe("chat settings panel", () => {
  it("opens the shared note bottom menu and synchronizes its inline toggle", async () => {
    const update = vi.fn((_args: unknown) => ({ status: "ok" as const, data: { ...settings, author_note: "Be kind" } }));
    await open({ path: "/chat/c1?panel=settings", calls: { ...panelCalls, conversationSettingsUpdate: update } });
    fireEvent.click(await screen.findByRole("button", { name: /Author's note/ }));
    const dialogs = await screen.findAllByRole("dialog");
    expect(dialogs).toHaveLength(2);
    expect(dialogs[1]?.className).not.toContain("right-0");
    fireEvent.click(await screen.findByRole("switch", { name: "Show inline author note editor" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Author's note" }), { target: { value: "Be kind" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(update).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.getAllByRole("dialog")).toHaveLength(1));
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(await screen.findByRole("button", { name: "Edit inline author's note" })).toBeTruthy();
  });

  it("navigates branches to its own placeholder page and omits scene and prompt controls", async () => {
    const runtime = await open({ path: "/chat/c1?panel=settings", calls: panelCalls });
    expect(screen.queryByRole("button", { name: /^Scene/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /^Prompt/ })).toBeNull();
    fireEvent.click(await screen.findByRole("button", { name: /^Branches/ }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/chat/c1/branches"));
    expect(await screen.findByText("Branches")).toBeTruthy();
  });

  it("opens from the search parameter, closes back to the chat and redirects the old route", async () => {
    const runtime = await open({ path: "/chat/c1?panel=settings", calls: panelCalls });
    expect((await screen.findAllByText("Harbour")).length).toBeGreaterThan(0);
    expect(await screen.findByRole("dialog")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() => expect(runtime.router.state.location.search).toEqual({}));
    expect(runtime.router.state.location.pathname).toBe("/chat/c1");
    cleanup();
    const redirected = await open({ path: "/chat/c1/settings", calls: panelCalls });
    await waitFor(() => expect(redirected.router.state.location.pathname).toBe("/chat/c1"));
    expect(redirected.router.state.location.search).toEqual({ panel: "settings" });
  });

  it("opens from the header options button", async () => {
    const runtime = await open({ calls: panelCalls });
    fireEvent.click(await screen.findByRole("button", { name: "Chat options" }));
    await waitFor(() => expect(runtime.router.state.location.search).toEqual({ panel: "settings" }));
    expect(await screen.findByRole("dialog")).toBeTruthy();
  });

  it("is a side panel on desktop and a full-height sheet on a phone", async () => {
    await open({ path: "/chat/c1?panel=settings", calls: panelCalls });
    const side = await screen.findByRole("dialog");
    expect(side.className).toContain("right-0");
    cleanup();
    const original = window.matchMedia;
    window.matchMedia = ((query: string) => ({
      matches: query.includes("max-width"), media: query, onchange: null, addEventListener: () => undefined, removeEventListener: () => undefined,
      addListener: () => undefined, removeListener: () => undefined, dispatchEvent: () => false,
    })) as typeof window.matchMedia;
    try {
      await open({ path: "/chat/c1?panel=settings", calls: panelCalls, platform: "android" });
      const sheet = await screen.findByRole("dialog");
      expect(sheet.className).not.toContain("right-0");
      expect(sheet.getAttribute("style") ?? "").toContain("100");
    } finally {
      window.matchMedia = original;
    }
  });

  it("shows where each value comes from", async () => {
    await open({ path: "/chat/c1?panel=settings", calls: panelCalls });
    const dialog = await screen.findByRole("dialog");
    expect((await within(dialog).findAllByText("From the character")).length).toBeGreaterThan(0);
    expect(within(dialog).getAllByText("App default").length).toBeGreaterThan(0);
  });

  it("resets a chat-only model with a reset change", async () => {
    const update = vi.fn((_args: unknown) => ({ status: "ok" as const, data: settings }));
    await open({
      path: "/chat/c1?panel=settings",
      calls: { ...panelCalls, conversationSettingsGet: () => ({ status: "ok", data: { ...settings, model: { id: "m1", source: "conversation" } } }), conversationSettingsUpdate: update },
    });
    fireEvent.click(await screen.findByRole("button", { name: "Reset" }));
    await waitFor(() => expect(update).toHaveBeenCalled());
    expect(update.mock.calls[0]?.[0]).toMatchObject({ expected_settings_revision: 2, patch: { model: { type: "reset" } } });
  });

  it("toggles a lorebook for this chat", async () => {
    const update = vi.fn((_args: unknown) => ({ status: "ok" as const, data: settings }));
    await open({ path: "/chat/c1?panel=settings", calls: { ...panelCalls, conversationSettingsUpdate: update } });
    fireEvent.click(await screen.findByRole("button", { name: /^Lorebooks/ }));
    fireEvent.click(await screen.findByRole("switch", { name: "Harbour lore" }));
    await waitFor(() => expect(update).toHaveBeenCalled());
    expect(update.mock.calls[0]?.[0]).toMatchObject({ patch: { lorebooks: { type: "set", ids: ["lb1"] } } });
  });

  it("changes the memory mode", async () => {
    const update = vi.fn((_args: unknown) => ({ status: "ok" as const, data: settings }));
    await open({ path: "/chat/c1?panel=settings", calls: { ...panelCalls, conversationSettingsUpdate: update } });
    fireEvent.click(await screen.findByRole("radio", { name: "Dynamic" }));
    await waitFor(() => expect(update).toHaveBeenCalled());
    expect(update.mock.calls[0]?.[0]).toMatchObject({ patch: { memory: { type: "set", mode: "dynamic" } } });
  });

  it("renames the chat against its revision", async () => {
    const rename = vi.fn((_args: unknown) => ({ status: "ok" as const, data: { revision: 5, settings_revision: 2 } }));
    await open({ path: "/chat/c1?panel=settings", calls: { ...panelCalls, conversationRename: rename } });
    fireEvent.click(await screen.findByRole("button", { name: "Rename chat" }));
    const field = await screen.findByRole("textbox", { name: "Chat title" });
    fireEvent.change(field, { target: { value: "Harbour nights" } });
    fireEvent.keyDown(field, { key: "Enter" });
    await waitFor(() => expect(rename).toHaveBeenCalled());
    expect(rename.mock.calls[0]?.[0]).toMatchObject({ conversation_id: "c1", expected_revision: 4, title: "Harbour nights" });
  });

  it("opens another chat from the history", async () => {
    const runtime = await open({ path: "/chat/c1?panel=settings", calls: panelCalls });
    fireEvent.click(await screen.findByRole("button", { name: /^Chat history/ }));
    fireEvent.click(await screen.findByRole("button", { name: /Lighthouse/ }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/chat/c2"));
  });

  it("asks before deleting the chat and then goes home", async () => {
    const remove = vi.fn((_args: unknown) => ({ status: "ok" as const, data: null }));
    const runtime = await open({ path: "/chat/c1?panel=settings", calls: { ...panelCalls, conversationDelete: remove } });
    fireEvent.click(await screen.findByRole("button", { name: /Delete chat/ }));
    expect(remove).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole("button", { name: "Delete" }));
    await waitFor(() => expect(remove).toHaveBeenCalledWith({ conversation_id: "c1" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/"));
  });

  it("counts the chat's own sampler values and clears them with reset all", async () => {
    const update = vi.fn((_args: unknown) => ({ status: "ok" as const, data: settings }));
    const own = { chat_parameters: { temperature: 0.5 }, llama_cpp: {}, stable_diffusion: {} } as never;
    await open({
      path: "/chat/c1?panel=settings",
      calls: {
        ...panelCalls,
        conversationSettingsGet: () => ({ status: "ok", data: { ...settings, model_settings: own } }),
        conversationSettingsUpdate: update,
        settingsGet: () => ({ status: "ok", data: { sampler_defaults: { chat_parameters: {}, llama_cpp: {}, stable_diffusion: {} }, global: { help_me_reply: { enabled: true } } } as never }),
        modelsList: () => ({ status: "ok", data: { models: [{ id: "m1", provider_account_id: "a", external_model_id: "x", display_name: "Vision", kind: "chat", config: { capabilities: { parameter_support: {} } }, input_scopes: ["text"], output_scopes: ["text"], revision: 1, created_at: 1, updated_at: 1 }], default_model_id: "m1", revision: 1 } }),
      },
    });
    expect(await screen.findByText("1 value changed for this chat", undefined, { timeout: 4000 })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /^Model settings/ }));
    fireEvent.click(await within(screen.getByRole("dialog")).findByRole("button", { name: "Reset all" }));
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(update).toHaveBeenCalled());
    expect(update.mock.calls[0]?.[0]).toMatchObject({ patch: { model_settings: { chat_parameters: {} } } });
  });

  it("closes a pushed panel by going back, leaving no extra history entry", async () => {
    const runtime = await open({ calls: panelCalls });
    fireEvent.click(await screen.findByRole("button", { name: "Chat options" }));
    await screen.findByRole("dialog");
    expect(runtime.router.history.location.state.panelPushed).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() => expect(runtime.router.state.location.search).toEqual({}));
    expect(runtime.router.history.canGoBack()).toBe(false);
  });

  it("lands on the root again after going back with a nested sheet open", async () => {
    const runtime = await open({ calls: { ...panelCalls, personasList: () => ({ status: "ok", data: { items: [{ id: "p1", title: "Captain", description: "", nickname: null, avatar: null }], next_cursor: null, default_persona: { persona_id: null, revision: 1, created_at: 1, updated_at: 1 } } }) } });
    fireEvent.click(await screen.findByRole("button", { name: "Chat options" }));
    fireEvent.click(await screen.findByRole("button", { name: /^Persona/ }));
    await screen.findByRole("button", { name: /Captain/ });
    await act(async () => {
      runtime.router.history.back();
    });
    await waitFor(() => expect(screen.queryByRole("button", { name: /Captain/ })).toBeNull());
    fireEvent.click(await screen.findByRole("button", { name: "Chat options" }));
    expect(await screen.findByRole("button", { name: /^Persona/ })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Captain/ })).toBeNull();
  });

  it("deletes the current chat from the history and leaves no way back to it", async () => {
    const remove = vi.fn((_args: unknown) => ({ status: "ok" as const, data: null }));
    const runtime = await open({ calls: { ...panelCalls, conversationDelete: remove } });
    fireEvent.click(await screen.findByRole("button", { name: "Chat options" }));
    fireEvent.click(await screen.findByRole("button", { name: /^Chat history/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Actions for Harbour" }, { timeout: 4000 }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Delete" }));
    fireEvent.click(await screen.findByRole("button", { name: "Delete" }));
    await waitFor(() => expect(remove).toHaveBeenCalledWith({ conversation_id: "c1" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/"));
    expect(runtime.router.history.canGoBack()).toBe(false);
  });

  it("resets a chat-only persona", async () => {
    const update = vi.fn((_args: unknown) => ({ status: "ok" as const, data: settings }));
    await open({
      path: "/chat/c1?panel=settings",
      calls: { ...panelCalls, conversationSettingsGet: () => ({ status: "ok", data: { ...settings, persona: { id: "p1", source: "conversation" } } }), conversationSettingsUpdate: update },
    });
    fireEvent.click(await screen.findByRole("button", { name: "Reset" }));
    await waitFor(() => expect(update).toHaveBeenCalled());
    expect(update.mock.calls[0]?.[0]).toMatchObject({ patch: { persona: { type: "reset" } } });
  });

  it("keeps the rename field open when renaming a history chat fails", async () => {
    await open({
      path: "/chat/c1?panel=settings",
      calls: { ...panelCalls, conversationRename: () => ({ status: "error", error: { code: "conflict", message: "x", details: null } }) },
    });
    fireEvent.click(await screen.findByRole("button", { name: /^Chat history/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Actions for Lighthouse" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Rename" }));
    const field = await screen.findByRole("textbox", { name: "Chat title" });
    fireEvent.change(field, { target: { value: "Lantern" } });
    fireEvent.keyDown(field, { key: "Enter" });
    expect(await screen.findByText("It changed somewhere else. Reload and try again.")).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "Chat title" })).toBeTruthy();
  });
});
