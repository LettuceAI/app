import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  ConversationSummary_Serialize,
  LorebookEntryView,
  LorebookTriggerPreview,
  LorebookTriggerPreviewRequest,
  LorebookView_Serialize,
  TimelineMessage_Serialize,
} from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";
import { literalSpans } from "./highlights";

vi.mock("@/api/window-controls", () => ({
  createWindowController: vi.fn().mockResolvedValue(null),
}));
beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(600);
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(1000);
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});
const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const entry = (id: string, patch: Partial<LorebookEntryView> = {}): LorebookEntryView => ({
  id,
  title: id,
  content: `${id} content`,
  keywords: ["dragon"],
  enabled: true,
  always_active: false,
  case_sensitive: false,
  keyword_mode: "literal",
  priority: 7,
  ordinal: 0,
  revision: 1,
  created_at: 1,
  updated_at: 1,
  ...patch,
});
const book: LorebookView_Serialize = {
  lorebook: {
    id: "book",
    name: "World of Welan",
    status: "active",
    detection: "recent_messages",
    icon: null,
    revision: 4,
    created_at: 1,
    updated_at: 1,
  },
  entries: [
    entry("Dragon"),
    entry("Always", { always_active: true }),
    entry("Disabled", { enabled: false }),
    entry("No keys", { keywords: [] }),
    entry("Waiting", { keywords: ["gate"] }),
  ],
};
const preview: LorebookTriggerPreview = {
  scan_depth: 2,
  scanned_message_ids: ["recent", "last"],
  active_lorebook_ids: ["book"],
  skipped: [],
  entries: [
    {
      position: 0,
      source: { type: "conversation" },
      lorebook_id: "book",
      lorebook_name: "World of Welan",
      entry_id: "Dragon",
      title: "Dragon",
      matched_keywords: ["dragon"],
      matched_message_ids: ["recent"],
      matched_in_composer: true,
      always_active: false,
      token_count: 9,
    },
    {
      position: 1,
      source: { type: "conversation" },
      lorebook_id: "book",
      lorebook_name: "World of Welan",
      entry_id: "Always",
      title: "Always",
      matched_keywords: [],
      matched_message_ids: [],
      matched_in_composer: false,
      always_active: true,
      token_count: 4,
    },
  ],
};
const chat: ConversationSummary_Serialize = {
  id: "chat",
  kind: "direct",
  title: "At the gate",
  avatars: [],
  background: null,
  revision: 1,
  last_message_preview: "Another dragon",
  updated_at: 1,
  archived: false,
  source: { type: "direct", character_id: "character" },
  message_count: 3,
  chat_mode: null,
  missing_models: [],
  memory_blocked: null,
  origin_conversation_id: null,
  origin_message_id: null,
};
const message = (id: string, text: string): TimelineMessage_Serialize => ({
  id,
  role: "user",
  author_participant_id: null,
  parts: [{ type: "text", text }],
  reasoning: null,
  created_at: 1,
  candidate_index: null,
  candidate_count: 0,
  pinned: false,
  scene_image: null,
});
async function setup(override: MockCallHandlers = {}, query = "") {
  const requests: LorebookTriggerPreviewRequest[] = [];
  const transport = createMockTransport({
    calls: {
      appStatus: () => ok({ ...mockAppStatus, ui_state: { onboarding: { completed: true } } }),
      settingsGet: () => ok({ global: { dynamic_memory: { enabled: false } } } as never),
      charactersList: () => ok({ items: [], next_cursor: null }),
      conversationsLatestByCharacter: () => ok({ items: [], next_cursor: null }),
      conversationsList: () => ok({ items: [chat], next_cursor: null }),
      conversationMessages: () =>
        ok({
          items: [
            message("old", "Old dragon"),
            message("recent", "A dragon arrives"),
            message("last", "Another dragon"),
          ],
          next_cursor: null,
        }),
      jobsList: () => ok({ items: [], next_cursor: null }),
      lorebookGet: () => ok(book),
      lorebookTriggerPreview: (request) => {
        requests.push(request);
        return ok(preview);
      },
      ...override,
    },
  });
  const runtime = await createAppRuntime({
    transport,
    history: createMemoryHistory({ initialEntries: [`/library/lorebooks/book/preview${query}`] }),
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return { runtime, requests, transport };
}

describe("lorebook trigger preview", () => {
  it("shows native activation, reasons, token counts and exact scan boundaries", async () => {
    await setup();
    expect(await screen.findByText("Comes in · 2")).toBeTruthy();
    expect(screen.getByText("Stays out · 3")).toBeTruthy();
    expect(screen.getByText("Turned off, never comes in")).toBeTruthy();
    expect(screen.getByText("No keywords, so it never comes in")).toBeTruthy();
    expect(screen.getByText("Waits for gate")).toBeTruthy();
    expect(screen.getByText("2 of 5 entries come in · 13 tokens")).toBeTruthy();
    const old = screen.getByText("Old dragon").closest("article")!;
    const recent = document.querySelector<HTMLElement>('[data-message-id="recent"]')!;
    expect(old.getAttribute("data-scanned")).toBe("false");
    expect(recent.getAttribute("data-scanned")).toBe("true");
    expect(screen.getByText("Scanned from here")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Dragon.*Matched/ }));
    expect(within(recent).getByRole("button", { name: "dragon" })).toBeTruthy();
    expect(within(old).queryByRole("button", { name: "dragon" })).toBeNull();
    const last = document.querySelector('[data-message-id="last"]')!;
    expect(within(last as HTMLElement).queryByRole("button", { name: "dragon" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /What gets added/ }));
    expect(document.querySelector("pre")?.textContent).toBe("Dragon content\n\nAlways content");
  });
  it("shows when the current book is not attached to a chat", async () => {
    await setup({
      lorebookTriggerPreview: () => ok({ ...preview, active_lorebook_ids: [], entries: [] }),
    });
    expect(await screen.findByText("This lorebook is not used in this chat")).toBeTruthy();
    expect(screen.queryByText("Stays out · 5")).toBeNull();
  });
  it("keeps an explicitly selected chat outside the loaded recent page", async () => {
    const messageRequests: string[] = [];
    const { requests } = await setup(
      {
        conversationMessages: (request) => {
          messageRequests.push(request.conversation_id);
          return ok({ items: [message("recent", "A dragon arrives")], next_cursor: null });
        },
      },
      "?chat=older-chat",
    );
    await screen.findByText("Comes in · 2");
    expect(messageRequests).toEqual(["older-chat"]);
    expect(requests).toContainEqual({
      mode: "conversation",
      conversation_id: "older-chat",
      composer_text: null,
      speaker_character_id: null,
    });
  });
  it("keeps the visible messages in place when loading older history", async () => {
    await setup({
      conversationMessages: (request) =>
        ok({
          items: request.before_cursor
            ? [message("older", "Earlier history")]
            : [message("recent", "A dragon arrives")],
          next_cursor: request.before_cursor ? null : "older-page",
        }),
    });
    await screen.findByText("Comes in · 2");
    const scroller = document.querySelector('[data-message-id="recent"]')!.parentElement!;
    Object.defineProperty(scroller, "scrollHeight", {
      configurable: true,
      get: () => (scroller.querySelector('[data-message-id="older"]') ? 1400 : 1000),
    });
    scroller.scrollTop = 120;
    fireEvent.scroll(scroller);
    fireEvent.click(screen.getByRole("button", { name: "Load older messages" }));
    await screen.findByText("Earlier history");
    expect(scroller.scrollTop).toBe(520);
  });
  it("opens the owning entry from a highlighted word and refreshes after a chat turn", async () => {
    const { transport, requests } = await setup();
    await screen.findByText("Comes in · 2");
    const message = document.querySelector<HTMLElement>('[data-message-id="recent"]')!;
    fireEvent.click(within(message).getByRole("button", { name: "dragon" }));
    expect(await screen.findByText("1 entry uses this word")).toBeTruthy();
    fireEvent.click(screen.getByRole("menuitem", { name: "Dragon" }));
    expect(await screen.findByRole("button", { name: "Edit" })).toBeTruthy();
    const before = requests.length;
    await act(async () =>
      transport.emitAppEvent({
        type: "generation_settled",
        conversation_id: "chat",
        turn_id: "turn",
      }),
    );
    await waitFor(() => expect(requests.length).toBeGreaterThan(before));
    const afterTurn = requests.length;
    await act(async () => transport.emitAppEvent({ type: "group_changed", group_id: "group" }));
    await waitFor(() => expect(requests.length).toBeGreaterThan(afterTurn));
  });
  it("uses compact Chat, Compose and Entries tabs on mobile", async () => {
    await setup({
      appStatus: () =>
        ok({
          ...mockAppStatus,
          platform: "android",
          ui_state: { onboarding: { completed: true } },
        }),
    });
    await screen.findByRole("tab", { name: "Entries 2/5" });
    fireEvent.click(screen.getByRole("tab", { name: "Entries 2/5" }));
    expect(await screen.findByText("Comes in · 2")).toBeTruthy();
    expect(screen.queryByText("Old dragon")).toBeNull();
    fireEvent.click(screen.getByRole("tab", { name: "Compose" }));
    expect(await screen.findByRole("textbox", { name: "Text to test" })).toBeTruthy();
  });
  it("confirms removal and sends only the chosen entry mutation", async () => {
    const writes: unknown[] = [];
    await setup({
      lorebookEntriesMutate: (request) => {
        writes.push(request);
        return ok(book);
      },
    });
    fireEvent.click(await screen.findByRole("button", { name: /Dragon.*Matched/ }));
    fireEvent.click(screen.getByRole("button", { name: "Delete entry" }));
    expect(writes).toEqual([]);
    fireEvent.click(await screen.findByRole("button", { name: "Delete" }));
    await waitFor(() => expect(writes).toHaveLength(1));
    expect(writes[0]).toMatchObject({
      expected_revision: 4,
      lorebook_id: "book",
      mutations: [{ type: "remove", entry_id: "Dragon" }],
    });
  });
  it("debounces compose text into the editor-mode preview", async () => {
    const { requests } = await setup({}, "?mode=editor");
    const input = await screen.findByRole("textbox", { name: "Text to test" });
    fireEvent.change(input, { target: { value: "dragon in the gate" } });
    await waitFor(() =>
      expect(requests).toContainEqual({
        mode: "editor",
        lorebook_id: "book",
        text: "dragon in the gate",
      }),
    );
    expect(requests.every((request) => request.mode === "editor")).toBe(true);
  });
  it("retains the entry draft and exact operation payload after a failed save", async () => {
    const writes: unknown[] = [];
    await setup({
      lorebookEntriesMutate: (request) => {
        writes.push(structuredClone(request));
        return writes.length === 1
          ? { status: "error", error: { code: "unavailable", message: "temporary", details: null } }
          : ok({ ...book, lorebook: { ...book.lorebook, revision: 5 } });
      },
    });
    fireEvent.click(await screen.findByRole("button", { name: /Dragon.*Matched/ }));
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    const content = await screen.findByRole("textbox", { name: "Content" });
    fireEvent.change(content, { target: { value: "Changed content" } });
    fireEvent.click(screen.getByRole("button", { name: "Save entry" }));
    const retry = await screen.findByRole("button", { name: "Try again" });
    expect((content as HTMLTextAreaElement).value).toBe("Changed content");
    fireEvent.click(retry);
    await waitFor(() => expect(writes).toHaveLength(2));
    expect(writes[0]).toEqual(writes[1]);
    expect(writes[0]).toMatchObject({
      expected_revision: 4,
      mutations: [
        {
          type: "update",
          entry_id: "Dragon",
          entry: {
            content: "Changed content",
            priority: 7,
            enabled: true,
            keyword_mode: "literal",
          },
        },
      ],
    });
  });
});

describe("literal highlight decoration", () => {
  it("uses backend provenance and suppresses regex and wildcard marks", () => {
    const item = { entry: entry("Dragon"), match: preview.entries[0]! };
    expect(literalSpans("DRAGON dragon", [item], "recent")).toHaveLength(2);
    expect(literalSpans("dragon", [item], "last")).toEqual([]);
    expect(
      literalSpans(
        "dragon",
        [{ ...item, entry: { ...item.entry, case_sensitive: true } }],
        "recent",
      ),
    ).toHaveLength(1);
    expect(
      literalSpans(
        "DRAGON",
        [{ ...item, entry: { ...item.entry, case_sensitive: true } }],
        "recent",
      ),
    ).toEqual([]);
    expect(
      literalSpans(
        "dragon",
        [{ ...item, entry: { ...item.entry, keyword_mode: "regex" } }],
        "recent",
      ),
    ).toEqual([]);
    expect(
      literalSpans(
        "dragon",
        [{ ...item, match: { ...item.match, matched_keywords: ["drag*"] } }],
        "recent",
      ),
    ).toEqual([]);
  });
});
