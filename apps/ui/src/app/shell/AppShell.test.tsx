import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ConversationSummary } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockTransportOptions } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

const ui_state = { onboarding: { completed: true } };

function chat(id: string, title: string, updatedAt: number): ConversationSummary {
  return {
    id, kind: "direct", title, avatars: [], background: null, revision: 1, last_message_preview: null, updated_at: updatedAt,
    archived: false, source: { type: "direct", character_id: "c1" }, message_count: 1, chat_mode: null, missing_models: [],
  } as unknown as ConversationSummary;
}

async function renderShell(options: MockTransportOptions = {}, uiState: Record<string, unknown> = ui_state) {
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      ...options,
      calls: {
        appStatus: () => ({ status: "ok", data: { ...mockAppStatus, ui_state: uiState } }),
        charactersList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        conversationsLatestByCharacter: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        conversationsList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        ...options.calls,
      },
    }),
    history: createMemoryHistory({ initialEntries: ["/"] }),
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return runtime;
}

afterEach(() => {
  cleanup();
});

describe("desktop sidebar", () => {
  it("names the first destination Home and starts expanded", async () => {
    await renderShell();
    expect(await screen.findByRole("button", { name: "Home" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Chats" })).toBeNull();
    expect(await screen.findByRole("button", { name: "Collapse sidebar" })).toBeTruthy();
  });

  it("saves the folded state in the device UI state", async () => {
    const update = vi.fn((request: { patch: Record<string, unknown> }) => ({ status: "ok" as const, data: { state: request.patch } }));
    await renderShell({ calls: { appUiStateUpdate: update } });
    fireEvent.click(await screen.findByRole("button", { name: "Collapse sidebar" }));
    expect(await screen.findByRole("button", { name: "Expand sidebar" })).toBeTruthy();
    await waitFor(() => expect(update).toHaveBeenCalledWith({ patch: { sidebarExpanded: false } }));
  });

  it("starts folded when the stored choice says so", async () => {
    await renderShell({}, { ...ui_state, sidebarExpanded: false });
    expect(await screen.findByRole("button", { name: "Expand sidebar" })).toBeTruthy();
  });

  it("lists active chats grouped by when they last moved", async () => {
    const now = Date.now();
    const day = 24 * 60 * 60 * 1000;
    const items = [chat("a", "Harbour night", now - 1000), chat("b", "Old road", now - 3 * day), chat("c", "Ancient tale", now - 30 * day)];
    await renderShell({ calls: { conversationsList: () => ({ status: "ok", data: { items, next_cursor: null } }) } });
    expect(await screen.findByText("Harbour night")).toBeTruthy();
    expect(screen.getByText("Old road")).toBeTruthy();
    expect(screen.getByText("Ancient tale")).toBeTruthy();
    expect(screen.getByRole("region", { name: "Today" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "This week" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Earlier" })).toBeTruthy();
  });

  it("says where chats will show up when there are none", async () => {
    await renderShell();
    expect(await screen.findByText("Chats you start show up here.")).toBeTruthy();
  });

  it("opens the create sheet and goes to Discover from it", async () => {
    const runtime = await renderShell();
    fireEvent.click(await screen.findByRole("button", { name: "Create" }));
    expect(await screen.findByText("Import a card")).toBeTruthy();
    fireEvent.click(screen.getByText("Browse Discover"));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/discover"));
  });
});
