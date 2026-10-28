import { act, cleanup, configure, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ConversationSettingsUpdateRequest_Deserialize, ConversationSettingsView, ConversationSummary, ConversationView, TimelineMessage } from "@/api/generated/bindings";
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
        memoryGet: () => ({ status: "ok", data: { items: [], allowed_categories: ["character_trait", "relationship", "plot_event", "world_detail", "preference", "other"], summary: null, revision: 1, status: { run_mode: "auto", interval: 10, messages_since_last_cycle: 0, messages_until_next_cycle: 10, total_conversation_messages: 1, pending_approval_count: null, skipped: false, latest_cycle_status: null, latest_job_id: null, failure: null, paused_reason: null } } }),
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
    source: { type: "direct", character_id: "ch1" }, message_count: 5, chat_mode: null, missing_models: [], memory_blocked: null, origin_conversation_id: null, origin_message_id: null, ...patch,
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
  it("starts each history visit at the top without resetting scroll while searching", async () => {
    await open({ path: "/chat/c1?panel=settings", calls: panelCalls });
    const settingsDialog = await screen.findByRole("dialog", { name: "Chat settings" });
    const scroller = settingsDialog.querySelector<HTMLElement>(".overflow-y-auto");
    expect(scroller).toBeTruthy();
    if (!scroller) throw new Error("Missing settings scroller");
    scroller.scrollTop = 320;
    fireEvent.click(await within(settingsDialog).findByRole("button", { name: /^Chat history/ }));
    scroller.scrollTop = 140;
    const search = await screen.findByRole("searchbox", { name: "Search chats" });
    expect(scroller.scrollTop).toBe(0);
    scroller.scrollTop = 180;
    fireEvent.change(search, { target: { value: "Lighthouse" } });
    const history = within(screen.getByRole("dialog", { name: "Chat history" }));
    expect(await history.findByRole("button", { name: /^Lighthouse/ })).toBeTruthy();
    expect(scroller.scrollTop).toBe(180);
    fireEvent.click(history.getByRole("button", { name: "Back" }));
    await screen.findByRole("button", { name: /^Chat history/ });
    expect(scroller.scrollTop).toBe(0);
    scroller.scrollTop = 240;
    fireEvent.click(screen.getByRole("button", { name: /^Chat history/ }));
    await screen.findByRole("searchbox", { name: "Search chats" });
    expect(scroller.scrollTop).toBe(0);
  });

  it.each([
    /^Persona/,
    /^Model(?! settings)/,
    /^Author's note/,
    /^Chat background/,
    /^Lorebooks/,
  ])("closes only the %s bottom menu on the first Escape", async (name) => {
    const runtime = await open({ path: "/chat/c1?panel=settings", calls: panelCalls });
    fireEvent.click(await screen.findByRole("button", { name }));
    await waitFor(() => expect(screen.getAllByRole("dialog")).toHaveLength(2));
    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => expect(screen.getAllByRole("dialog")).toHaveLength(1));
    expect(runtime.router.state.location.search).toEqual({ panel: "settings" });
    expect(screen.getByRole("dialog", { name: "Chat settings" })).toBeTruthy();
    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => expect(runtime.router.state.location.search).toEqual({}));
  });

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

  it("navigates branches to its own page and omits scene and prompt controls", async () => {
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

  it("opens from the character name in the header", async () => {
    const runtime = await open({ calls: panelCalls });
    fireEvent.click(await screen.findByRole("button", { name: /Cassian/ }));
    await waitFor(() => expect(runtime.router.state.location.search).toEqual({ panel: "settings" }));
    expect(await screen.findByRole("dialog")).toBeTruthy();
  });

  it("is a side panel on desktop and a full-page drawer on a phone", async () => {
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
      const drawer = await screen.findByRole("dialog");
      expect(drawer.className).toContain("right-0");
      expect(drawer.className).toContain("w-full");
      expect(within(drawer).getByRole("button", { name: "Back" })).toBeTruthy();
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

  it("toggles a lorebook in a bottom menu while keeping chat settings mounted", async () => {
    const update = vi.fn((_args: unknown) => ({ status: "ok" as const, data: settings }));
    await open({ path: "/chat/c1?panel=settings", calls: { ...panelCalls, conversationSettingsUpdate: update } });
    fireEvent.click(await screen.findByRole("button", { name: /^Lorebooks/ }));
    const dialogs = await screen.findAllByRole("dialog");
    expect(dialogs).toHaveLength(2);
    expect(dialogs[1]?.className).not.toContain("right-0");
    expect(within(dialogs[0]!).getByText("Model")).toBeTruthy();
    fireEvent.click(await screen.findByRole("checkbox", { name: "Harbour lore" }));
    await waitFor(() => expect(update).toHaveBeenCalled());
    expect(update.mock.calls[0]?.[0]).toMatchObject({ patch: { lorebooks: { type: "set", ids: ["lb1"] } } });
  });

  it("opens a lorebook in its editor from the picker and comes back to the chat", async () => {
    const runtime = await open({ path: "/chat/c1?panel=settings", calls: panelCalls });
    fireEvent.click(await screen.findByRole("button", { name: /^Lorebooks/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Edit Harbour lore" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/library/lorebooks/lb1"));
    await act(async () => {
      runtime.router.history.back();
    });
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/chat/c1"));
  });

  it("clears the lorebook bottom menu when browser back closes settings", async () => {
    const runtime = await open({ calls: panelCalls });
    fireEvent.click(await screen.findByRole("button", { name: "Chat options" }));
    fireEvent.click(await screen.findByRole("button", { name: /^Lorebooks/ }));
    await screen.findByRole("checkbox", { name: "Harbour lore" });
    await act(async () => {
      runtime.router.history.back();
    });
    await waitFor(() => expect(screen.queryByRole("checkbox", { name: "Harbour lore" })).toBeNull());
    fireEvent.click(await screen.findByRole("button", { name: "Chat options" }));
    expect(await screen.findByRole("button", { name: /^Lorebooks/ })).toBeTruthy();
    expect(screen.queryByRole("checkbox", { name: "Harbour lore" })).toBeNull();
  });

  it("keeps memory management available without a chat memory mode override", async () => {
    const update = vi.fn((_args: unknown) => ({ status: "ok" as const, data: settings }));
    await open({ path: "/chat/c1?panel=settings", calls: { ...panelCalls, conversationSettingsUpdate: update } });
    expect(await screen.findByRole("button", { name: /^Open memories/ })).toBeTruthy();
    expect(screen.queryByRole("radiogroup", { name: "Memory mode" })).toBeNull();
    expect(screen.queryByText("Memory mode")).toBeNull();
    expect(update).not.toHaveBeenCalled();
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

  it("always shows history search and searches message previews", async () => {
    await open({ path: "/chat/c1?panel=settings", calls: panelCalls });
    fireEvent.click(await screen.findByRole("button", { name: /^Chat history/ }));
    const search = await screen.findByRole("searchbox", { name: "Search chats" });
    fireEvent.change(search, { target: { value: "ABOUT LIGHTHOUSE" } });
    const history = within(screen.getByRole("dialog", { name: "Chat history" }));
    expect(await history.findByRole("button", { name: /^Lighthouse/ })).toBeTruthy();
    expect(history.queryByRole("button", { name: /^Harbour/ })).toBeNull();
    expect(history.getByRole("status").textContent).toBe("1 chat");
    fireEvent.keyDown(search, { key: "Escape" });
    expect(search).toHaveProperty("value", "");
    expect(screen.getByRole("dialog", { name: "Chat history" })).toBeTruthy();
  });

  it("paginates all fetched history results and resets search and sort to the first page", async () => {
    const items = Array.from({ length: 21 }, (_, index) => summary(`h${index}`, `Chat ${String(index + 1).padStart(2, "0")}`, { updated_at: 1_700_000_000_000 + index }));
    await open({ path: "/chat/c1?panel=settings", calls: {
      ...panelCalls,
      conversationsList: ({ character_id, cursor }) => ({ status: "ok", data: character_id ? { items: cursor ? items.slice(11) : items.slice(0, 11), next_cursor: cursor ? null : "next" } : { items: [], next_cursor: null } }),
    } });
    fireEvent.click(await screen.findByRole("button", { name: /^Chat history/ }));
    expect(await screen.findByRole("button", { name: /^Chat 21/ })).toBeTruthy();
    expect(screen.getAllByRole("button", { name: /^Chat \d/ })).toHaveLength(10);
    expect(screen.queryByRole("button", { name: /^Chat 01/ })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Page 3" }));
    expect(screen.getAllByRole("button", { name: /^Chat \d/ })).toHaveLength(1);
    expect(screen.getByRole("button", { name: /^Chat 01/ })).toBeTruthy();
    fireEvent.change(screen.getByRole("searchbox", { name: "Search chats" }), { target: { value: "About Chat 21" } });
    expect(screen.getByRole("button", { name: /^Chat 21/ })).toBeTruthy();
    expect(screen.queryByRole("navigation", { name: "Pagination" })).toBeNull();
    fireEvent.change(screen.getByRole("searchbox", { name: "Search chats" }), { target: { value: "" } });
    fireEvent.click(screen.getByRole("button", { name: "Page 3" }));
    fireEvent.click(screen.getByRole("button", { name: "Sort chats" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Oldest first" }));
    expect(screen.getByRole("button", { name: "Page 1" }).getAttribute("aria-current")).toBe("page");
    expect(screen.getAllByRole("button", { name: /^Chat \d/ })[0]?.textContent).toMatch(/^Chat 01/);
    expect(screen.queryByRole("button", { name: /^Chat 21/ })).toBeNull();
  });

  it("filters active and archived history and shows an activity date", async () => {
    await open({ path: "/chat/c1?panel=settings", calls: {
      ...panelCalls,
      conversationsList: ({ character_id }) => ({ status: "ok", data: { items: character_id ? [summary("c1", "Harbour"), summary("c2", "Lighthouse", { archived: true })] : [], next_cursor: null } }),
    } });
    fireEvent.click(await screen.findByRole("button", { name: /^Chat history/ }));
    expect((await screen.findByRole("button", { name: /^Harbour/ })).textContent).toContain("2023");
    fireEvent.click(screen.getByRole("radio", { name: "Archived" }));
    expect(screen.getByRole("button", { name: /^Lighthouse/ })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /^Harbour/ })).toBeNull();
    fireEvent.click(screen.getByRole("radio", { name: "Active" }));
    expect(screen.getByRole("button", { name: /^Harbour/ })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /^Lighthouse/ })).toBeNull();
    fireEvent.click(screen.getByRole("radio", { name: "All" }));
    expect(screen.getByRole("status").textContent).toBe("2 chats");
  });

  it("keeps a valid history page after deleting its last row", async () => {
    let items = Array.from({ length: 21 }, (_, index) => summary(`h${index}`, `Chat ${String(index + 1).padStart(2, "0")}`, { updated_at: 1_700_000_000_000 + index }));
    await open({ path: "/chat/c1?panel=settings", calls: {
      ...panelCalls,
      conversationsList: ({ character_id }) => ({ status: "ok", data: { items: character_id ? items : [], next_cursor: null } }),
      conversationDelete: ({ conversation_id }) => { items = items.filter((chat) => chat.id !== conversation_id); return { status: "ok", data: null }; },
    } });
    fireEvent.click(await screen.findByRole("button", { name: /^Chat history/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Page 3" }));
    fireEvent.click(screen.getByRole("button", { name: "Actions for Chat 01" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Delete" }));
    fireEvent.click(await screen.findByRole("button", { name: "Delete" }));
    await waitFor(() => expect(screen.queryByRole("button", { name: "Page 3" })).toBeNull());
    expect(screen.getByRole("button", { name: "Page 2" }).getAttribute("aria-current")).toBe("page");
    expect(screen.getAllByRole("button", { name: /^Chat \d/ })).toHaveLength(10);
    expect(screen.queryByRole("button", { name: /^Chat 01/ })).toBeNull();
  });

  it("portals the desktop history menu outside the drawer and keeps item clicks working", async () => {
    await open({ path: "/chat/c1?panel=settings", calls: panelCalls });
    fireEvent.click(await screen.findByRole("button", { name: /^Chat history/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Actions for Lighthouse" }));
    const menu = await screen.findByRole("menu");
    expect(screen.getByRole("dialog", { name: "Chat history" }).contains(menu)).toBe(false);
    expect(within(menu).getAllByRole("menuitem")).toHaveLength(4);
    const rename = within(menu).getByRole("menuitem", { name: "Rename" });
    fireEvent.pointerDown(rename);
    fireEvent.click(rename);
    expect(await screen.findByRole("textbox", { name: "Chat title" })).toHaveProperty("value", "Lighthouse");
  });

  it("closes only the desktop history menu on Escape and returns focus to its trigger", async () => {
    const runtime = await open({ path: "/chat/c1?panel=settings", calls: panelCalls });
    fireEvent.click(await screen.findByRole("button", { name: /^Chat history/ }));
    const trigger = await screen.findByRole("button", { name: "Actions for Lighthouse" });
    fireEvent.click(trigger);
    const item = await screen.findByRole("menuitem", { name: "Rename" });
    fireEvent.keyDown(item, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    expect(screen.getByRole("dialog", { name: "Chat history" })).toBeTruthy();
    expect(runtime.router.state.location.search).toEqual({ panel: "settings" });
    expect(document.activeElement).toBe(trigger);
  });

  it("uses a bottom menu for history actions on a phone and preserves the drawer on Escape", async () => {
    const original = window.matchMedia;
    window.matchMedia = ((query: string) => ({
      matches: query.includes("max-width"), media: query, onchange: null, addEventListener: () => undefined, removeEventListener: () => undefined,
      addListener: () => undefined, removeListener: () => undefined, dispatchEvent: () => false,
    })) as typeof window.matchMedia;
    try {
      const runtime = await open({ path: "/chat/c1?panel=settings", calls: panelCalls, platform: "android" });
      fireEvent.click(await screen.findByRole("button", { name: /^Chat history/ }));
      fireEvent.click(await screen.findByRole("button", { name: "Actions for Lighthouse" }));
      const menu = await screen.findByRole("dialog", { name: "Actions for Lighthouse" });
      expect(menu.className).not.toContain("right-0");
      expect(screen.queryByRole("menu")).toBeNull();
      for (const name of ["Rename", "Export", "Archive", "Delete"]) {
        expect(within(menu).getByRole("button", { name })).toBeTruthy();
      }
      fireEvent.keyDown(menu, { key: "Escape" });
      await waitFor(() => expect(screen.queryByRole("dialog", { name: "Actions for Lighthouse" })).toBeNull());
      expect(screen.getByRole("dialog", { name: "Chat history" })).toBeTruthy();
      expect(runtime.router.state.location.search).toEqual({ panel: "settings" });
    } finally {
      window.matchMedia = original;
    }
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

  it("uses the shared model editor rows and keeps draft actions outside the scroller", async () => {
    let stored = { ...settings, model_settings: { chat_parameters: { frequency_penalty: 0.25 }, llama_cpp: {}, stable_diffusion: {} } };
    const update = vi.fn((args: ConversationSettingsUpdateRequest_Deserialize) => {
      stored = { ...stored, model_settings: args.patch.model_settings as typeof stored.model_settings };
      return { status: "ok" as const, data: stored as ConversationSettingsView };
    });
    await open({
      path: "/chat/c1?panel=settings",
      calls: {
        ...panelCalls,
        conversationSettingsGet: () => ({ status: "ok", data: stored as ConversationSettingsView }),
        conversationSettingsUpdate: update,
        settingsGet: () => ({ status: "ok", data: { sampler_defaults: { chat_parameters: { top_p: 0.9 }, llama_cpp: {}, stable_diffusion: {} }, global: { help_me_reply: { enabled: true } } } as never }),
        modelsList: () => ({ status: "ok", data: { models: [{ id: "m1", provider_account_id: "a", external_model_id: "x", display_name: "Vision", kind: "chat", config: { chat_parameters: { temperature: 0.6 }, capabilities: { parameter_support: {} } }, input_scopes: ["text"], output_scopes: ["text"], revision: 1, created_at: 1, updated_at: 1 }], default_model_id: "m1", revision: 1 } }),
      },
    });
    fireEvent.click(await screen.findByRole("button", { name: /^Model settings/ }));
    const dialog = await screen.findByRole("dialog", { name: "Model settings" });
    const panel = within(dialog);
    const temperature = await panel.findByRole("slider", { name: "Temperature" });
    const temperatureRow = temperature.closest("fieldset")!;
    expect((temperature as HTMLInputElement).value).toBe("0.6");
    expect((panel.getByRole("slider", { name: "Top P" }) as HTMLInputElement).value).toBe("0.9");
    expect((panel.getByRole("slider", { name: "Frequency penalty" }) as HTMLInputElement).value).toBe("0.25");
    expect(panel.queryByRole("button", { name: "Save" })).toBeNull();
    fireEvent.change(temperature, { target: { value: "0.8" } });
    expect(within(temperatureRow).getByRole("button", { name: "Reset Temperature" })).toBeTruthy();
    const scroller = dialog.querySelector(".overflow-y-auto")!;
    const save = panel.getByRole("button", { name: "Save" });
    expect(scroller.contains(save)).toBe(false);
    fireEvent.click(panel.getByRole("button", { name: "Reset Temperature" }));
    expect((temperature as HTMLInputElement).value).toBe("0.6");
    expect(panel.queryByRole("button", { name: "Save" })).toBeNull();
    fireEvent.change(temperature, { target: { value: "0.8" } });
    fireEvent.click(panel.getByRole("button", { name: "Discard" }));
    expect((temperature as HTMLInputElement).value).toBe("0.6");
    expect(update).not.toHaveBeenCalled();
    fireEvent.change(temperature, { target: { value: "0.8" } });
    fireEvent.click(panel.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(update).toHaveBeenCalledTimes(1));
    expect(update.mock.calls[0]?.[0]).toMatchObject({ patch: { model_settings: { chat_parameters: { temperature: 0.8, frequency_penalty: 0.25 } } } });
    await waitFor(() => expect(panel.queryByRole("button", { name: "Save" })).toBeNull());
    fireEvent.click(panel.getByRole("button", { name: "Back" }));
    expect(await within(screen.getByRole("dialog")).findByRole("button", { name: /^Model settings/ })).toBeTruthy();
    expect(within(screen.getByRole("dialog")).queryByRole("status")).toBeNull();
  });

  it("keeps the sampler draft after a failed save and cancels numeric edits with Escape", async () => {
    const update = vi.fn()
      .mockReturnValueOnce({ status: "error", error: { code: "internal", message: "diagnostic", details: null } })
      .mockImplementation((args: ConversationSettingsUpdateRequest_Deserialize) => ({ status: "ok", data: { ...settings, model_settings: args.patch.model_settings } }));
    await open({
      path: "/chat/c1?panel=settings",
      calls: {
        ...panelCalls,
        conversationSettingsUpdate: update,
        settingsGet: () => ({ status: "ok", data: { sampler_defaults: { chat_parameters: {}, llama_cpp: {}, stable_diffusion: {} }, global: { help_me_reply: { enabled: true } } } as never }),
      },
    });
    fireEvent.click(await screen.findByRole("button", { name: /^Model settings/ }));
    const dialog = await screen.findByRole("dialog", { name: "Model settings" });
    const panel = within(dialog);
    const value = await panel.findByRole("textbox", { name: "Temperature value" });
    fireEvent.focus(value);
    fireEvent.change(value, { target: { value: "1.2" } });
    fireEvent.keyDown(value, { key: "Escape" });
    expect(screen.getByRole("dialog", { name: "Model settings" })).toBeTruthy();
    expect(panel.queryByRole("button", { name: "Save" })).toBeNull();
    fireEvent.change(panel.getByRole("slider", { name: "Temperature" }), { target: { value: "1.1" } });
    fireEvent.click(panel.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(update).toHaveBeenCalledTimes(1));
    await waitFor(() => expect((panel.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(false));
    expect((value as HTMLInputElement).value).toBe("1.10");
    expect(screen.queryByText("diagnostic")).toBeNull();
    fireEvent.click(panel.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(update).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(panel.queryByRole("button", { name: "Save" })).toBeNull());
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
