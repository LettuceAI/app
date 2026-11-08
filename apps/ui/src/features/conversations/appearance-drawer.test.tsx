import { act, cleanup, configure, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createBrowserHistory, createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CharacterDraft_Serialize, CharacterView_Serialize, ConversationSettingsView, ConversationView, GroupView_Serialize, SettingsView, TimelineMessage } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

configure({ asyncUtilTimeout: 4000 });

const characterDraft = {
  profile: { name: "Cassian" },
  presentation: { format_version: 1, card_style: "circle", chat_appearance: { format_version: 1 }, chat_appearance_overrides: [] },
  scenes: [], variants: [], starters: [], media: [],
} as unknown as CharacterDraft_Serialize;
const reply: TimelineMessage = {
  id: "a1", role: "assistant", author_participant_id: "pc", parts: [{ type: "text", text: "Sit down." }], reasoning: null,
  created_at: 1_700_000_000_000, candidate_index: null, candidate_count: 1, pinned: false, scene_image: null,
};

function chatView(group: boolean): ConversationView {
  return {
    id: "c1", origin_conversation_id: null, origin_message_id: null, kind: group ? "group" : "direct", title: "Harbour", branch: { branch_id: "b", head_message_id: null },
    participants: [
      { id: "pu", role: "user", name: "Hunter", character_id: null, avatar: null },
      { id: "pc", role: "character", name: "Cassian", character_id: "ch1", avatar: null },
    ],
    messages: { items: [reply], next_cursor: null }, pending_turn_id: null, can_send: true, revision: 4, settings_revision: 2, archived: false,
    source: group ? { type: "group", group_id: "g1" } : { type: "direct", character_id: "ch1" }, chat_mode: null, missing_models: [], memory_blocked: null,
  };
}

const settings = (group: boolean): ConversationSettingsView => ({
  conversation_id: "c1", kind: group ? "group" : "direct", title: "Harbour", revision: 4, settings_revision: 2, author_note: null,
  persona: { id: null, source: "app_default" }, prompt: { id: null, source: "app_default" },
  lorebooks: { ids: [], source: "app_default" }, model: { id: null, source: "character" }, background: { asset: null, hidden: false, source: "character" },
  scene: { id: null, source: "app_default" }, memory: { mode: "manual", source: "app_default" }, chat_mode: null,
  disable_character_lorebooks: null, disable_character_notes: null, speaker_selection: null, members: null,
  model_settings: { chat_parameters: {}, llama_cpp: {}, stable_diffusion: {} } as never,
  time: { enabled: false, mode: "live", anchor_at: null, set_at: null, effective_at: 1_700_000_000_000 }, companion: false,
});

const characterView = (patch: Partial<CharacterView_Serialize> = {}): CharacterView_Serialize => ({
  id: "ch1", archived: false, draft: characterDraft, assets: [], revision: 1, created_at: 1, updated_at: 1, ...patch,
});
const groupView = (): GroupView_Serialize => ({
  id: "g1", archived: false, assets: [], revision: 1, created_at: 1, updated_at: 1,
  draft: { name: "Harbor crew", presentation: { format_version: 1 }, presentation_overrides: [], members: [] } as unknown as GroupView_Serialize["draft"],
});

interface Setup {
  group?: boolean;
  calls?: MockCallHandlers;
  platform?: "other" | "android";
  browser?: boolean;
}

async function browserHistory() {
  window.history.replaceState(null, "", "/chat/c1");
  const history = createBrowserHistory();
  history.push("/chat/c1?panel=settings");
  await act(async () => { await Promise.resolve(); });
  return history;
}

async function openSettings(setup: Setup = {}) {
  const group = setup.group ?? false;
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: () => ({ status: "ok", data: { ...mockAppStatus, platform: setup.platform ?? "other", ui_state: { onboarding: { completed: true } } } }),
        settingsGet: () => ({ status: "ok", data: { global: { ui_preferences: {} }, revision: 1 } as unknown as SettingsView }),
        conversationOpen: () => ({ status: "ok", data: chatView(group) }),
        conversationSettingsGet: () => ({ status: "ok", data: settings(group) }),
        characterGet: () => ({ status: "ok", data: characterView() }),
        groupGet: () => ({ status: "ok", data: groupView() }),
        modelsList: () => ({ status: "ok", data: { models: [], default_model_id: null, revision: 1 } }),
        providerAccountsList: () => ({ status: "ok", data: [] }),
        memoryGet: () => ({ status: "ok", data: { items: [], allowed_categories: [], summary: null, revision: 1, status: {} } as never }),
        llmMetricsForMessage: () => ({ status: "ok", data: null }),
        charactersList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        conversationMessageCount: () => ({ status: "ok", data: { count: 1 } }),
        conversationsList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        conversationsLatestByCharacter: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        conversationParticipationStats: () => ({ status: "ok", data: { total_messages: 0, items: [] } }),
        personasList: () => ({ status: "ok", data: { items: [], next_cursor: null, default_persona: { persona_id: null, revision: 1, created_at: 1, updated_at: 1 } } }),
        lorebooksList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        promptsList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        ...setup.calls,
      },
    }),
    history: setup.browser ? await browserHistory() : createMemoryHistory({ initialEntries: ["/chat/c1?panel=settings"] }),
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return runtime;
}

async function openDrawer(setup: Setup = {}) {
  await openSettings(setup);
  fireEvent.click(await screen.findByRole("button", { name: /^Appearance/ }));
  const drawer = await screen.findByRole("dialog", { name: "Appearance" });
  await within(drawer).findByRole("switch", { name: "Show names" });
  return drawer;
}

const authorNames = () => screen.queryAllByText("Cassian").length;

afterEach(() => {
  cleanup();
  window.localStorage.removeItem("chatAppearanceDrawer.side");
});

describe("chat appearance drawer", () => {
  it("opens from the Appearance row of a direct chat and of a group chat", async () => {
    const drawer = await openDrawer();
    expect(drawer.className).toContain("right-0");
    expect(within(drawer).getByText("For Cassian")).toBeTruthy();
    expect(within(drawer).queryByText("Composer text size")).toBeNull();
    cleanup();
    const groupDrawer = await openDrawer({ group: true });
    expect(within(groupDrawer).getByText("For every chat in this group")).toBeTruthy();
  });

  it("restyles the chat with the draft without saving, and Discard reverts it", async () => {
    const save = vi.fn<NonNullable<MockCallHandlers["characterSave"]>>(() => ({ status: "ok", data: characterView() }));
    const drawer = await openDrawer({ calls: { characterSave: save } });
    const shown = authorNames();
    expect(shown).toBeGreaterThan(0);
    fireEvent.click(within(drawer).getByRole("switch", { name: "Show names" }));
    await waitFor(() => expect(authorNames()).toBeLessThan(shown));
    expect(save).not.toHaveBeenCalled();
    expect(within(drawer).getAllByRole("img", { name: "Set for this character" })).toHaveLength(1);
    fireEvent.click(within(drawer).getByRole("button", { name: "Discard" }));
    await waitFor(() => expect(authorNames()).toBe(shown));
    expect(within(drawer).getByRole("switch", { name: "Show names" }).getAttribute("aria-checked")).toBe("true");
    expect(within(drawer).queryByRole("button", { name: "Save" })).toBeNull();
    expect(save).not.toHaveBeenCalled();
  });

  it("saves the character's per-field overrides through the character save", async () => {
    let current = characterView();
    const save = vi.fn<NonNullable<MockCallHandlers["characterSave"]>>((request) => {
      current = { ...current, draft: request.draft as CharacterDraft_Serialize, revision: current.revision + 1 };
      return { status: "ok", data: current };
    });
    const hidden = () => (current.draft.presentation.chat_appearance_overrides ?? []).includes("show_message_author");
    const drawer = await openDrawer({ calls: {
      characterSave: save,
      characterGet: () => ({ status: "ok", data: current }),
      conversationSettingsGet: () => ({ status: "ok", data: { ...settings(false), appearance: hidden() ? { showMessageAuthor: false } : {} } as ConversationSettingsView }),
    } });
    const shown = authorNames();
    fireEvent.click(within(drawer).getByRole("switch", { name: "Show names" }));
    fireEvent.click(within(drawer).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0]![0]).toMatchObject({
      character_id: "ch1",
      expected_revision: 1,
      draft: { presentation: { chat_appearance: { show_message_author: false }, chat_appearance_overrides: ["show_message_author"] } },
    });
    expect(save.mock.calls[0]![0].draft.profile).toEqual(characterDraft.profile);
    await waitFor(() => expect(within(drawer).queryByRole("button", { name: "Save" })).toBeNull());
    await waitFor(() => expect(authorNames()).toBeLessThan(shown));
  });

  it("saves a group's presentation through the group save", async () => {
    const save = vi.fn<NonNullable<MockCallHandlers["groupSave"]>>((request) => ({ status: "ok", data: { ...groupView(), draft: request.draft as GroupView_Serialize["draft"], revision: 2 } }));
    const drawer = await openDrawer({ group: true, calls: { groupSave: save } });
    fireEvent.click(within(drawer).getByRole("switch", { name: "Show names" }));
    fireEvent.click(within(drawer).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0]![0]).toMatchObject({
      group_id: "g1",
      expected_revision: 1,
      draft: { name: "Harbor crew", presentation: { show_message_author: false }, presentation_overrides: ["show_message_author"] },
    });
  });

  it.each([false, true])("renders AM/PM timestamps and saves the presentation for group=%s", async (group) => {
    const characterSave = vi.fn<NonNullable<MockCallHandlers["characterSave"]>>((request) => ({ status: "ok", data: { ...characterView(), draft: request.draft as CharacterDraft_Serialize, revision: 2 } }));
    const groupSave = vi.fn<NonNullable<MockCallHandlers["groupSave"]>>((request) => ({ status: "ok", data: { ...groupView(), draft: request.draft as GroupView_Serialize["draft"], revision: 2 } }));
    const drawer = await openDrawer({ group, calls: { characterSave, groupSave } });
    fireEvent.click(within(drawer).getByRole("combobox", { name: "Time format" }));
    fireEvent.click(await screen.findByRole("option", { name: "AM/PM" }));
    expect(await screen.findByText(/^\d{1,2}:\d{2}\s[AP]M$/)).toBeTruthy();
    fireEvent.click(within(drawer).getByRole("button", { name: "Save" }));
    if (group) {
      await waitFor(() => expect(groupSave).toHaveBeenCalledTimes(1));
      expect(groupSave.mock.calls[0]![0].draft).toMatchObject({ presentation: { timestamp_format: "time12" }, presentation_overrides: ["timestamp_format"] });
    } else {
      await waitFor(() => expect(characterSave).toHaveBeenCalledTimes(1));
      expect(characterSave.mock.calls[0]![0].draft).toMatchObject({ presentation: { chat_appearance: { timestamp_format: "time12" }, chat_appearance_overrides: ["timestamp_format"] } });
    }
  });

  it("asks before closing with unsaved changes and closes straight away when clean", async () => {
    const drawer = await openDrawer();
    fireEvent.click(within(drawer).getByRole("button", { name: "Close appearance" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Appearance" })).toBeNull());
    const baseline = authorNames();
    fireEvent.click(await screen.findByRole("button", { name: "Chat options" }));
    fireEvent.click(await screen.findByRole("button", { name: /^Appearance/ }));
    const again = await screen.findByRole("dialog", { name: "Appearance" });
    await within(again).findByRole("switch", { name: "Show names" });
    fireEvent.click(within(again).getByRole("switch", { name: "Show names" }));
    fireEvent.click(within(again).getByRole("button", { name: "Close appearance" }));
    const confirmation = await screen.findByRole("dialog", { name: "Discard your changes?" });
    fireEvent.click(within(confirmation).getByRole("button", { name: "Keep editing" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Discard your changes?" })).toBeNull());
    expect(screen.getByRole("dialog", { name: "Appearance" })).toBeTruthy();
    fireEvent.click(within(again).getByRole("button", { name: "Close appearance" }));
    fireEvent.click(within(await screen.findByRole("dialog", { name: "Discard your changes?" })).getByRole("button", { name: "Discard changes" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Appearance" })).toBeNull());
    expect(authorNames()).toBe(baseline);
  });

  it("closes on history back when clean and asks first when there are unsaved changes", async () => {
    const runtime = await openSettings({ browser: true });
    fireEvent.click(await screen.findByRole("button", { name: /^Appearance/ }));
    const drawer = await screen.findByRole("dialog", { name: "Appearance" });
    await within(drawer).findByRole("switch", { name: "Show names" });
    const path = runtime.router.state.location.pathname;
    await act(async () => runtime.router.history.back());
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Appearance" })).toBeNull());
    expect(runtime.router.state.location.pathname).toBe(path);

    fireEvent.click(await screen.findByRole("button", { name: "Chat options" }));
    fireEvent.click(await screen.findByRole("button", { name: /^Appearance/ }));
    const again = await screen.findByRole("dialog", { name: "Appearance" });
    await within(again).findByRole("switch", { name: "Show names" });
    fireEvent.click(within(again).getByRole("switch", { name: "Show names" }));
    await act(async () => runtime.router.history.back());
    const keep = await screen.findByRole("dialog", { name: "Discard your changes?" });
    fireEvent.click(within(keep).getByRole("button", { name: "Keep editing" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Discard your changes?" })).toBeNull());
    expect(screen.getByRole("dialog", { name: "Appearance" })).toBeTruthy();
    expect(within(again).getByRole("switch", { name: "Show names" }).getAttribute("aria-checked")).toBe("false");
    expect(runtime.router.state.location.pathname).toBe(path);

    await act(async () => runtime.router.history.back());
    fireEvent.click(within(await screen.findByRole("dialog", { name: "Discard your changes?" })).getByRole("button", { name: "Discard changes" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Appearance" })).toBeNull());
    expect(runtime.router.state.location.pathname).toBe(path);
  });

  it("has a fixed width and remembers the side", async () => {
    const drawer = await openDrawer();
    expect(drawer.className).toContain("w-[min(30rem,calc(100%-2.5rem))]");
    expect(within(drawer).queryByRole("separator")).toBeNull();
    fireEvent.click(within(drawer).getByRole("button", { name: "Move to the left" }));
    expect(window.localStorage.getItem("chatAppearanceDrawer.side")).toBe("left");
    expect(drawer.className).toContain("left-0");
    cleanup();
    const reopened = await openDrawer();
    expect(reopened.className).toContain("left-0");
    fireEvent.click(within(reopened).getByRole("button", { name: "Move to the right" }));
    expect(window.localStorage.getItem("chatAppearanceDrawer.side")).toBe("right");
  });

  it("shows a following row as only its title and control, and an own row with a dot and a reset", async () => {
    const drawer = await openDrawer();
    expect(within(drawer).queryByText("Follows app appearance")).toBeNull();
    expect(within(drawer).queryByText("Set for this character")).toBeNull();
    expect(within(drawer).queryAllByRole("img", { name: "Set for this character" })).toHaveLength(0);
    expect(within(drawer).getByRole("radiogroup", { name: "Line spacing" })).toBeTruthy();
    expect(within(drawer).queryByRole("button", { name: /^Reset .* to the app appearance$/ })).toBeNull();
    fireEvent.click(within(drawer).getByRole("switch", { name: "Show names" }));
    expect(within(drawer).getAllByRole("img", { name: "Set for this character" })).toHaveLength(1);
    fireEvent.click(within(drawer).getByRole("button", { name: "Reset Show names to the app appearance" }));
    expect(within(drawer).queryAllByRole("img", { name: "Set for this character" })).toHaveLength(0);
    expect(within(drawer).getByRole("switch", { name: "Show names" }).getAttribute("aria-checked")).toBe("true");
  });

  it("keeps working when storage throws", async () => {
    const getItem = vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => { throw new Error("blocked"); });
    const setItem = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new Error("blocked"); });
    const drawer = await openDrawer();
    expect(getItem).toHaveBeenCalled();
    fireEvent.click(within(drawer).getByRole("button", { name: "Move to the left" }));
    expect(setItem).toHaveBeenCalled();
    expect(drawer.className).toContain("left-0");
  });

  it("opens from the right at near full width on a phone, without a side toggle", async () => {
    const original = window.matchMedia;
    window.matchMedia = ((query: string) => ({
      matches: query.includes("max-width"), media: query, onchange: null, addEventListener: () => undefined, removeEventListener: () => undefined,
      addListener: () => undefined, removeListener: () => undefined, dispatchEvent: () => false,
    })) as typeof window.matchMedia;
    try {
      window.localStorage.setItem("chatAppearanceDrawer.side", "left");
      const drawer = await openDrawer({ platform: "android" });
      expect(drawer.className).toContain("right-0");
      expect(drawer.className).toContain("w-[min(30rem,calc(100%-2.5rem))]");
      expect(within(drawer).queryByRole("button", { name: "Move to the left" })).toBeNull();
      expect(within(drawer).getByRole("radiogroup", { name: "Line spacing" })).toBeTruthy();
      expect(within(drawer).getByRole("combobox", { name: "Text size" })).toBeTruthy();
    } finally {
      window.matchMedia = original;
    }
  });
});
