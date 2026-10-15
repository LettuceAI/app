import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { toast } from "@lettuceai/crisp";
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

async function renderShell(options: MockTransportOptions = {}, uiState: Record<string, unknown> = ui_state, path = "/") {
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
    history: createMemoryHistory({ initialEntries: [path] }),
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return runtime;
}

afterEach(() => {
  toast.dismiss();
  cleanup();
});

describe("primary navigation", () => {
  it.each(["/", "/groups", "/discover", "/library"])("puts mobile settings last in the title row on %s", async (path) => {
    const runtime = await renderShell({ calls: {
      appStatus: () => ({ status: "ok", data: { ...mockAppStatus, platform: "android", ui_state } }),
    } }, ui_state, path);
    const navigation = await screen.findByRole("navigation", { name: "Main navigation" });
    expect(within(navigation).getAllByRole("button")).toHaveLength(4);
    expect(within(navigation).queryByRole("button", { name: "Settings" })).toBeNull();
    const settings = screen.getByRole("button", { name: "Settings" });
    const header = settings.closest("header");
    expect(header).not.toBeNull();
    expect(within(header!).getByRole("heading", { level: 1 })).toBeTruthy();
    expect(within(header!).getAllByRole("button").at(-1)).toBe(settings);
    fireEvent.click(settings);
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/settings"));
    expect(screen.queryByRole("navigation", { name: "Main navigation" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Back to app" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/"));
    expect(await screen.findByRole("navigation", { name: "Main navigation" })).toBeTruthy();
  });

  it.each(["/settings", "/settings/about", "/settings/models/new", "/unknown"])("hides mobile navigation on %s even when desktop detail navigation is enabled", async (path) => {
    await renderShell({ calls: {
      appStatus: () => ({ status: "ok", data: { ...mockAppStatus, platform: "ios", ui_state: { ...ui_state, sidebarOnDetailPages: true } } }),
    } }, ui_state, path);
    expect(screen.queryByRole("navigation", { name: "Main navigation" })).toBeNull();
  });

  it("keeps desktop settings reachable after opting out of the detail sidebar", async () => {
    const runtime = await renderShell({}, { ...ui_state, sidebarOnDetailPages: false }, "/settings/about");
    expect(screen.queryByRole("navigation", { name: "Main navigation" })).toBeNull();
    fireEvent.click(await screen.findByRole("button", { name: "Back" }));
    expect(await screen.findByRole("heading", { name: "Settings" })).toBeTruthy();
    expect(runtime.router.state.location.pathname).toBe("/settings");
    fireEvent.click(screen.getByRole("button", { name: "Back to app" }));
    expect(await screen.findByRole("button", { name: "Home" })).toBeTruthy();
  });

  it("saves the optional desktop detail sidebar choice from the view menu", async () => {
    let state: Record<string, unknown> = { ...ui_state };
    const update = vi.fn((request: { patch: Record<string, unknown> }) => {
      state = { ...state, ...request.patch };
      return { status: "ok" as const, data: { state } };
    });
    const runtime = await renderShell({ calls: {
      appStatus: () => ({ status: "ok", data: { ...mockAppStatus, ui_state: state } }),
      appUiStateUpdate: update,
    } });
    fireEvent.click(await screen.findByRole("button", { name: "View: Home" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Show sidebar on other pages" }));
    await waitFor(() => expect(update).toHaveBeenCalledWith({ patch: { sidebarOnDetailPages: false } }));
    await waitFor(() => expect(runtime.queryClient.getQueryData<{ ui_state: Record<string, unknown> }>(["app", "status"])?.ui_state.sidebarOnDetailPages).toBe(false));
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(await screen.findByRole("heading", { name: "Settings" })).toBeTruthy();
    expect(screen.queryByRole("navigation", { name: "Main navigation" })).toBeNull();
  });

  it("keeps the desktop sidebar choice when saving fails", async () => {
    const runtime = await renderShell({ calls: {
      appUiStateUpdate: () => ({ status: "error", error: { code: "unavailable", message: "storage unavailable", details: null } }),
    } });
    fireEvent.click(await screen.findByRole("button", { name: "View: Home" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Show sidebar on other pages" }));
    expect(await screen.findByText("It isn't available right now.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/settings/about"));
    expect(screen.getByRole("navigation", { name: "Main navigation" })).toBeTruthy();
  });

  it("also hides detail navigation in a narrow desktop window", async () => {
    vi.spyOn(window, "matchMedia").mockImplementation((query) => ({
      matches: query.includes("max-width"), media: query, onchange: null,
      addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {}, dispatchEvent: () => false,
    }));
    await renderShell({}, { ...ui_state, sidebarOnDetailPages: true }, "/settings/about");
    expect(screen.queryByRole("navigation", { name: "Main navigation" })).toBeNull();
  });
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
