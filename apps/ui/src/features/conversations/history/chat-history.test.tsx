import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CharacterView, ConversationSummary } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

const character: CharacterView = {
  id: "ch1", archived: false, revision: 1, created_at: 1, updated_at: 1, assets: [],
  draft: { profile: { name: "Cassian" } } as CharacterView["draft"],
};

function chat(id: string, patch: Partial<ConversationSummary> = {}): ConversationSummary {
  return {
    id, title: `Story ${id}`, kind: "direct", revision: 1, updated_at: 1_700_000_000_000,
    archived: false, avatars: [], background: null, last_message_preview: "At the harbour", message_count: 5,
    source: { type: "direct", character_id: "ch1" }, chat_mode: null, missing_models: [], memory_blocked: null, ...patch,
  };
}

async function open(calls: MockCallHandlers = {}, initialEntries = ["/characters/ch1/history"]) {
  const runtime = await createAppRuntime({
    transport: createMockTransport({ calls: {
      appStatus: () => ({ status: "ok", data: { ...mockAppStatus, ui_state: { onboarding: { completed: true } } } }),
      characterGet: () => ({ status: "ok", data: character }),
      charactersList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
      conversationsLatestByCharacter: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
      ...calls,
      conversationsList: (request) => request.character_id === null
        ? { status: "ok", data: { items: [], next_cursor: null } }
        : calls.conversationsList?.(request) ?? { status: "ok", data: { items: [chat("c1")], next_cursor: null } },
    } }),
    history: createMemoryHistory({ initialEntries }),
    i18n: await createTestI18n("en"),
  });
  await act(async () => { render(<App runtime={runtime} />); });
  return runtime;
}

afterEach(cleanup);

describe("character chat history page", () => {
  it("loads every page for the routed character without a current chat and opens a search result", async () => {
    const list = vi.fn<NonNullable<MockCallHandlers["conversationsList"]>>((request) => ({ status: "ok", data: request.cursor === null
      ? { items: Array.from({ length: 10 }, (_, index) => chat(`c${index}`, { updated_at: 100 - index })), next_cursor: "older" }
      : { items: [chat("c11", { updated_at: 1, last_message_preview: "The lighthouse keeper", archived: true })], next_cursor: null },
    }));
    const runtime = await open({ conversationsList: list });
    expect(await screen.findByText("Cassian")).toBeTruthy();
    expect(await screen.findByText("11 chats")).toBeTruthy();
    expect(list).toHaveBeenCalledWith({ kind: "direct", character_id: "ch1", source_group_id: null, lifecycle: "all", cursor: "older", limit: null });
    expect(screen.queryByRole("button", { name: /^Story c11/ })).toBeNull();
    fireEvent.change(screen.getByRole("searchbox", { name: "Search chats" }), { target: { value: "lighthouse" } });
    fireEvent.click(await screen.findByRole("button", { name: /^Story c11/ }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/chat/c11"));
  });

  it("offers a new chat in empty history and guards a pending launch", async () => {
    let release!: () => void;
    const pending = new Promise<void>((resolve) => { release = resolve; });
    const launch = vi.fn(async () => {
      await pending;
      return { status: "ok" as const, data: { conversation_id: "new-chat" } };
    });
    const runtime = await open({
      conversationsList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
      conversationLaunchDirect: launch,
    });
    expect(await screen.findByText("No chats yet")).toBeTruthy();
    const buttons = screen.getAllByRole("button", { name: "New chat" });
    fireEvent.click(buttons[1]!);
    fireEvent.click(buttons[0]!);
    expect(launch).toHaveBeenCalledTimes(1);
    expect(launch).toHaveBeenCalledWith({ character_id: "ch1", title: null, scene_id: null, starter_id: null, client_operation_id: expect.any(String) });
    await act(async () => { release(); });
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/chat/new-chat"));
  });

  it("retries a failed list and keeps Back usable on a deep link", async () => {
    let fail = true;
    const runtime = await open({ conversationsList: () => fail
      ? { status: "error", error: { code: "unavailable", message: "Unavailable", details: null } }
      : { status: "ok", data: { items: [chat("c1")], next_cursor: null } },
    });
    expect(await screen.findByText("Couldn't load this")).toBeTruthy();
    fail = false;
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("button", { name: /^Story c1/ })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/"));
  });

  it("keeps archived character history readable and disables starting a chat", async () => {
    await open({ characterGet: () => ({ status: "ok", data: { ...character, archived: true } }) });
    expect(await screen.findByRole("button", { name: /^Story c1/ })).toBeTruthy();
    expect((screen.getByRole("button", { name: "New chat" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText("Restore this character in your library to start a new chat.")).toBeTruthy();
  });

  it("closes only the compact actions sheet on Escape and restores an archived chat", async () => {
    const original = window.matchMedia;
    window.matchMedia = (query) => ({ ...original(query), matches: query.includes("max-width") });
    try {
      let archived = true;
      const restore = vi.fn(() => { archived = false; return { status: "ok" as const, data: { revision: 2, settings_revision: 1 } }; });
      const runtime = await open({
        conversationsList: () => ({ status: "ok", data: { items: [chat("c1", { archived })], next_cursor: null } }),
        conversationRestore: restore,
      });
      fireEvent.click(await screen.findByRole("button", { name: "Actions for Story c1" }));
      fireEvent.keyDown(await screen.findByRole("dialog", { name: "Actions for Story c1" }), { key: "Escape" });
      await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
      expect(runtime.router.state.location.pathname).toBe("/characters/ch1/history");
      fireEvent.click(screen.getByRole("button", { name: "Actions for Story c1" }));
      fireEvent.click(within(await screen.findByRole("dialog", { name: "Actions for Story c1" })).getByRole("button", { name: "Restore" }));
      await waitFor(() => expect(restore).toHaveBeenCalledWith({ conversation_id: "c1" }));
      await waitFor(() => expect(screen.queryByText("Archived", { selector: "span" })).toBeNull());
    } finally {
      window.matchMedia = original;
    }
  });
});
