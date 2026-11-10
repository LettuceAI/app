import { act, cleanup, configure, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CharacterDraft_Serialize, CharacterView_Serialize, ConversationSettingsView, ConversationView, SettingsView, TimelineMessage } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { CHAT_DEFAULTS, type ChatLook } from "@/entities/preferences";
import { createTestI18n } from "@/shared/testing/i18n";
import { WidgetContext, type WidgetActions } from "./widgets/context";
import { widgetLayout } from "./widgets/layout";
import { availableNodes, createActionsPreset, fold, splitFolded, type WidgetNode, type WidgetSlots } from "./widgets/nodes";
import { DiceWidget, StatTrackerWidget } from "./widgets/InputWidgets";
import { effectiveSlots } from "./widgets/use-widget-slots";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

configure({ asyncUtilTimeout: 4000 });

const look = (patch: Partial<ChatLook> = {}): ChatLook => ({ ...CHAT_DEFAULTS, chatWidgetAreaEnabled: true, chatColumnWidth: "normal", chatColumnAlign: "center", chatWidgetCenterMode: "both", ...patch });

describe("widget layout", () => {
  it("shows both panels when each has the 220px minimum", () => {
    expect(widgetLayout(look(), 1240, false)).toEqual({ enabled: true, showLeft: true, showRight: true, columnPx: 800 });
  });
  it("shows nothing when either centred panel would be under 220px", () => {
    expect(widgetLayout(look(), 1239, false).enabled).toBe(false);
    expect(widgetLayout(look(), 1200, false).enabled).toBe(false);
  });
  it("shows one panel for a left or right aligned column when it has room", () => {
    expect(widgetLayout(look({ chatColumnAlign: "left" }), 1020, false)).toMatchObject({ enabled: true, showLeft: false, showRight: true });
    expect(widgetLayout(look({ chatColumnAlign: "right" }), 1020, false)).toMatchObject({ enabled: true, showLeft: true, showRight: false });
    expect(widgetLayout(look({ chatColumnAlign: "left" }), 1019, false).enabled).toBe(false);
  });
  it("shows the chosen side of a centred column", () => {
    expect(widgetLayout(look({ chatWidgetCenterMode: "left" }), 1240, false)).toMatchObject({ showLeft: true, showRight: false });
    expect(widgetLayout(look({ chatWidgetCenterMode: "right" }), 1240, false)).toMatchObject({ showLeft: false, showRight: true });
  });
  it("shows nothing for a full-width column, a phone or the switch off", () => {
    expect(widgetLayout(look({ chatColumnWidth: "full" }), 3000, false)).toMatchObject({ enabled: false, columnPx: null });
    expect(widgetLayout(look(), 3000, true).enabled).toBe(false);
    expect(widgetLayout(look({ chatWidgetAreaEnabled: false }), 3000, false).enabled).toBe(false);
  });
  it("clamps a custom column to 400..2400", () => {
    expect(widgetLayout(look({ chatColumnWidth: "custom", chatColumnWidthPx: 100 }), 2000, false).columnPx).toBe(400);
    expect(widgetLayout(look({ chatColumnWidth: "custom", chatColumnWidthPx: 9000 }), 4000, false).columnPx).toBe(2400);
  });
});

const note = (id: string, content = ""): WidgetNode => ({ type: "scratch_pad", id, design: null, title: "Notes", description: null, content });
const info = (id: string): WidgetNode => ({ type: "character_info", id, design: null, character_id: null });
const dice = (id: string): WidgetNode => ({ type: "dice", id, design: null, title: null, description: null, notation: null });
const authorNote = (id: string): WidgetNode => ({ type: "author_note", id, design: null, title: null, description: null });

describe("widget nodes", () => {
  it("maps the app-wide camelCase tree onto the per-chat shape when the chat has no override", () => {
    const preferences = {
      chatAppearance: {
        chatWidgetSlots: {
          left: [
            { type: "character_info", id: "a", design: null, characterId: "ch9" },
            { type: "box", id: "b", design: null, variant: null, title: "Box", description: null, children: [{ type: "time", id: "t", design: null, title: null, hourFormat: "12h", showSeconds: true, showDate: false }] },
          ],
          right: [{ type: "image", id: "i", design: null, title: null, description: null, source: { kind: "library", path: "img-1" }, shape: null }],
        },
      },
    };
    const slots = effectiveSlots({ presentation: { format_version: 1 } as never, presentation_overrides: [] }, preferences, false);
    expect(slots.left[0]).toMatchObject({ type: "character_info", character_id: "ch9" });
    expect(slots.left[1]).toMatchObject({ type: "box", children: [{ type: "time", hour_format: "h12", show_seconds: true, show_date: false }] });
    expect(slots.right[0]).toMatchObject({ type: "image", source: { kind: "unresolved_legacy", token: "img-1" } });
  });

  it("prefers the chat's own slots when it overrides them, and reads nothing when the app has none", () => {
    const own: WidgetSlots = { left: [], right: [note("n")] };
    const draft = { presentation: { format_version: 1, chat_widget_slots: own } as never, presentation_overrides: ["chat_widget_slots"] };
    expect(effectiveSlots(draft, { chatAppearance: { chatWidgetSlots: { left: [{ type: "dice", id: "x" }], right: [] } } }, false)).toBe(own);
    expect(effectiveSlots({ presentation: { format_version: 1 } as never, presentation_overrides: [] }, {}, false)).toEqual({ left: [], right: [] });
  });

  it("hides companion state outside a direct companion chat and the voice autoplay button everywhere", () => {
    const nodes: WidgetNode[] = [
      { type: "companion_state", id: "c", design: null, title: null },
      { type: "button", id: "v", design: null, action: "toggle_voice_autoplay", title: null, description: null },
      { type: "box", id: "b", design: null, variant: null, title: null, description: null, children: [{ type: "companion_state", id: "c2", design: null, title: null }, note("n")] },
    ];
    expect(availableNodes(nodes, { companion: false, group: false }).map((node) => node.id)).toEqual(["b"]);
    expect(availableNodes(nodes, { companion: true, group: false }).map((node) => node.id)).toEqual(["c", "b"]);
    expect(availableNodes(nodes, { companion: true, group: true }).map((node) => node.id)).toEqual(["b"]);
  });

  it("builds the Actions preset as a box of regenerate, continue and new chat buttons", () => {
    const preset = createActionsPreset("Actions");
    expect(preset).toMatchObject({ type: "box", title: "Actions" });
    expect(preset.type === "box" && preset.children.map((child) => child.type === "button" && child.action)).toEqual(["regenerate", "continue", "new_session"]);
  });

  it("puts a widget added while one panel shows into its default slot and keeps the rest where they were", () => {
    const current: WidgetSlots = { left: [info("i")], right: [dice("d")] };
    expect(fold(current, { showLeft: true, showRight: false })).toEqual({ left: [info("i"), dice("d")], right: [] });
    expect(fold(current, { showLeft: false, showRight: true })).toEqual({ left: [], right: [dice("d"), info("i")] });
    const next = splitFolded(current, [info("i"), dice("d"), note("n"), authorNote("a")]);
    expect(next.left.map((node) => node.id)).toEqual(["i", "a"]);
    expect(next.right.map((node) => node.id)).toEqual(["d", "n"]);
  });
});

function actions(patch: Partial<WidgetActions> = {}): WidgetActions {
  return {
    api: {} as never, conversationId: "c1", isGroup: false, companion: false, character: null, members: [], persona: null, personaLoading: false,
    settings: undefined, messageCount: 0, sceneName: null, memoryOn: false, memories: [], memoryBusy: false, sending: false, canRegenerate: false,
    canContinue: false, swap: false, assets: new Map(), regenerate: vi.fn(), continueReply: vi.fn(), stop: vi.fn(), toggleSwap: vi.fn(), newChat: vi.fn(),
    openHistory: vi.fn(), openMemories: vi.fn(), openSearch: vi.fn(), openPersona: vi.fn(), openNote: vi.fn(), openModel: vi.fn(),
    saveAuthorNote: vi.fn(async () => true), insertText: vi.fn(), updateNode: vi.fn(async () => true), pickImage: vi.fn(async () => null), setTime: vi.fn(async () => true),
    ...patch,
  };
}

async function mountWidget(node: React.ReactNode, patch: Partial<WidgetActions> = {}) {
  const i18n = await createTestI18n("en");
  return render(
    <I18nextProvider i18n={i18n}>
      <WidgetContext.Provider value={actions(patch)}>{node}</WidgetContext.Provider>
    </I18nextProvider>,
  );
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("widget bodies", () => {
  it("draws a stat's bar at its place between min and max and shows value/max", async () => {
    await mountWidget(<StatTrackerWidget node={{ type: "stat_tracker", id: "s", design: null, title: "Stats", description: null, stats: [{ id: "a", label: "HP", value: 25, min: 0, max: 100 }, { id: "b", label: "Gold", value: 7, min: null, max: null }] }} />);
    const bars = screen.getAllByTestId("stat-bar");
    expect(bars).toHaveLength(1);
    await waitFor(() => expect(bars[0]!.style.width).toBe("25%"));
    expect(screen.getByText("/100")).toBeTruthy();
  });

  it("labels a natural maximum and a natural 1 of a single die, and no other roll", async () => {
    const random = vi.spyOn(Math, "random");
    const node = (notation: string) => ({ type: "dice" as const, id: "d", design: null, title: null, description: null, notation });
    await mountWidget(<DiceWidget node={node("1d20")} />);
    random.mockReturnValue(0.9999);
    fireEvent.click(screen.getByRole("button", { name: "Roll" }));
    expect(await screen.findByText("Natural 20")).toBeTruthy();
    random.mockReturnValue(0);
    fireEvent.click(screen.getByRole("button", { name: "Roll" }));
    expect(await screen.findByText("Natural 1")).toBeTruthy();
    cleanup();
    await mountWidget(<DiceWidget node={node("2d6")} />);
    random.mockReturnValue(0.9999);
    fireEvent.click(screen.getByRole("button", { name: "Roll" }));
    expect(await screen.findByText("6 + 6")).toBeTruthy();
    expect(screen.queryByText(/^Natural/)).toBeNull();
  });
});

const characterDraft = (slots: WidgetSlots, overrides: string[]) =>
  ({
    profile: { name: "Cassian" },
    presentation: { format_version: 1, card_style: "circle", chat_appearance: { format_version: 1, chat_widget_slots: slots }, chat_appearance_overrides: overrides },
    scenes: [], variants: [], starters: [], media: [],
  }) as unknown as CharacterDraft_Serialize;
const reply: TimelineMessage = {
  id: "a1", role: "assistant", author_participant_id: "pc", parts: [{ type: "text", text: "Sit down." }], reasoning: null,
  created_at: 1_700_000_000_000, candidate_index: null, candidate_count: 1, pinned: false, scene_image: null,
};
const chatView: ConversationView = {
  id: "c1", origin_conversation_id: null, origin_message_id: null, kind: "direct", title: "Harbour", branch: { branch_id: "b", head_message_id: null },
  participants: [
    { id: "pu", role: "user", name: "Hunter", character_id: null, avatar: null },
    { id: "pc", role: "character", name: "Cassian", character_id: "ch1", avatar: null },
  ],
  messages: { items: [reply], next_cursor: null }, pending_turn_id: null, can_send: true, revision: 4, settings_revision: 2, archived: false,
  source: { type: "direct", character_id: "ch1" }, missing_models: [], memory_blocked: null,
};
const chatSettings: ConversationSettingsView = {
  conversation_id: "c1", kind: "direct", title: "Harbour", revision: 4, settings_revision: 2, author_note: null,
  persona: { id: null, source: "app_default" }, prompt: { id: null, source: "app_default" },
  lorebooks: { ids: [], source: "app_default" }, model: { id: null, source: "character" }, background: { asset: null, hidden: false, source: "character" },
  scene: { id: null, source: "app_default" }, memory: { mode: "manual", source: "app_default" },
  disable_character_lorebooks: null, disable_character_notes: null, companion_mode: null, companion_personal_memories: null, companion_memory_write_back: null, speaker_selection: null, members: null,
  model_settings: { chat_parameters: {}, llama_cpp: {}, stable_diffusion: {} } as never,
  time: { enabled: false, mode: "live", anchor_at: null, set_at: null, effective_at: 1_700_000_000_000 }, companion: false,
};

async function mountChat(character: () => CharacterView_Serialize, calls: MockCallHandlers = {}) {
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: () => ({ status: "ok", data: { ...mockAppStatus, ui_state: { onboarding: { completed: true } } } }),
        settingsGet: () => ({ status: "ok", data: { global: { ui_preferences: { chatAppearance: { chatWidgetAreaEnabled: true, chatColumnWidth: "normal", chatColumnAlign: "center", chatWidgetCenterMode: "both" } } }, revision: 1 } as unknown as SettingsView }),
        conversationOpen: () => ({ status: "ok", data: chatView }),
        conversationSettingsGet: () => ({ status: "ok", data: chatSettings }),
        characterGet: () => ({ status: "ok", data: character() }),
        modelsList: () => ({ status: "ok", data: { models: [], default_model_id: null, revision: 1 } }),
        providerAccountsList: () => ({ status: "ok", data: [] }),
        memoryGet: () => ({ status: "ok", data: { items: [], allowed_categories: [], summary: null, revision: 1, status: {} } as never }),
        llmMetricsForMessage: () => ({ status: "ok", data: null }),
        conversationMessageCount: () => ({ status: "ok", data: { count: 1 } }),
        personasList: () => ({ status: "ok", data: { items: [], next_cursor: null, default_persona: { persona_id: null, revision: 1, created_at: 1, updated_at: 1 } } }),
        ...calls,
      },
    }),
    history: createMemoryHistory({ initialEntries: ["/chat/c1"] }),
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
}

function wideRow() {
  const original = Element.prototype.getBoundingClientRect;
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    return String(this.className).includes("h-full min-h-0 flex-col") ? ({ width: 1600, height: 800, top: 0, left: 0, right: 1600, bottom: 800, x: 0, y: 0, toJSON: () => ({}) } as DOMRect) : original.call(this);
  });
}

describe("widgets in the chat", () => {
  const slots: WidgetSlots = { left: [], right: [note("n1", "hello")] };
  const view = (revision = 1, draft = characterDraft(slots, ["chat_widget_slots"])): CharacterView_Serialize => ({ id: "ch1", archived: false, draft, assets: [], revision, created_at: 1, updated_at: 1 });

  it("saves a scratch pad on blur with the slots added to the overrides, and not when nothing changed", async () => {
    wideRow();
    let current = view();
    const save = vi.fn<NonNullable<MockCallHandlers["characterSave"]>>((request) => {
      current = { ...current, draft: request.draft as CharacterDraft_Serialize, revision: current.revision + 1 };
      return { status: "ok", data: current };
    });
    await mountChat(() => current, { characterSave: save });
    const field = await screen.findByRole("textbox", { name: "Notes" });
    fireEvent.blur(field);
    expect(save).not.toHaveBeenCalled();
    fireEvent.change(await screen.findByRole("textbox", { name: "Notes" }), { target: { value: "hello world" } });
    fireEvent.blur(screen.getByRole("textbox", { name: "Notes" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    const draft = save.mock.calls[0]![0].draft as CharacterDraft_Serialize;
    expect(draft.presentation.chat_appearance_overrides).toEqual(["chat_widget_slots"]);
    const saved = draft.presentation.chat_appearance.chat_widget_slots.right[0];
    expect(saved).toMatchObject({ type: "scratch_pad", id: "n1", content: "hello world" });
    expect(save.mock.calls[0]![0]).toMatchObject({ character_id: "ch1", expected_revision: 1 });
    await waitFor(() => expect(within(screen.getByRole("complementary", { name: "right widget area" })).getByRole<HTMLTextAreaElement>("textbox", { name: "Notes" }).value).toBe("hello world"));
  });

  it("reloads the character and applies the change once more after a revision conflict", async () => {
    wideRow();
    let current = view();
    const save = vi.fn<NonNullable<MockCallHandlers["characterSave"]>>((request) => {
      if (save.mock.calls.length === 1) {
        current = view(2, characterDraft({ left: [], right: [note("n1", "hello"), dice("d1")] }, ["chat_widget_slots"]));
        return { status: "error", error: { code: "conflict", message: "stale", details: null } };
      }
      current = { ...current, draft: request.draft as CharacterDraft_Serialize, revision: current.revision + 1 };
      return { status: "ok", data: current };
    });
    await mountChat(() => current, { characterSave: save });
    fireEvent.change(await screen.findByRole("textbox", { name: "Notes" }), { target: { value: "again" } });
    fireEvent.blur(screen.getByRole("textbox", { name: "Notes" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(2));
    expect(save.mock.calls[1]![0]).toMatchObject({ expected_revision: 2 });
    const right = (save.mock.calls[1]![0].draft as CharacterDraft_Serialize).presentation.chat_appearance.chat_widget_slots.right;
    expect(right.map((node) => node.id)).toEqual(["n1", "d1"]);
    expect(right[0]).toMatchObject({ content: "again" });
  });
});
