import { act, cleanup, configure, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ConversationBranchList, ConversationSummary } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

configure({ asyncUtilTimeout: 4000 });

afterEach(() => cleanup());

function chat(id: string, title: string, origin: string | null, updatedAt: number): ConversationSummary {
  return {
    id, title, kind: "direct", revision: 1, updated_at: updatedAt, archived: false, avatars: [], background: null, last_message_preview: null,
    source: { type: "direct", character_id: "ch1" }, message_count: 5, chat_mode: null, missing_models: [], memory_blocked: null,
    origin_conversation_id: origin, origin_message_id: origin ? "m1" : null,
  };
}

const chats = [
  chat("c0", "Harbour", null, 1_700_000_000_000),
  chat("c1", "Branch to Cassian", "c0", 1_700_000_300_000),
  chat("c2", "Lighthouse", "c0", 1_700_000_200_000),
  chat("c3", "Storm night", "c1", 1_700_000_400_000),
];

const list: ConversationBranchList = {
  revision: 7,
  branches: [
    { id: "root", label: "Branch to Cassian", parent_branch_id: null, fork_message_id: null, message_count: 16, updated_at: 1_700_000_000_000, active: false },
    { id: "side", label: "Dock fight", parent_branch_id: "root", fork_message_id: "m4", message_count: 3, updated_at: 1_700_000_100_000, active: true },
    { id: "other", label: "Storm", parent_branch_id: "root", fork_message_id: "m9", message_count: 2, updated_at: 1_700_000_050_000, active: false },
  ],
};

async function open(calls: MockCallHandlers = {}) {
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: () => ({ status: "ok", data: { ...mockAppStatus, platform: "other", ui_state: { onboarding: { completed: true } } } }),
        conversationBranches: () => ({ status: "ok", data: list }),
        conversationsList: () => ({ status: "ok", data: { items: chats, next_cursor: null } }),
        ...calls,
      },
    }),
    history: createMemoryHistory({ initialEntries: ["/chat/c1/branches"] }),
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return runtime;
}

describe("branches page", () => {
  it("shows the origin chat above this chat, with its branches and the chats branched from it nested below", async () => {
    await open();
    const tree = await screen.findByRole("list", { name: "Branches" });
    const origin = within(tree).getByRole("button", { name: /^Harbour/ }).closest("li");
    expect(origin).toBeTruthy();
    const here = within(origin as HTMLElement).getByRole("button", { name: /^Branch to Cassian/ }).closest("li") as HTMLElement;
    expect(within(here).getByText("This chat")).toBeTruthy();
    expect(within(here).getByRole("button", { name: /^Original/ })).toBeTruthy();
    expect(within(within(here).getByRole("button", { name: /^Dock fight/ })).getByText("Current")).toBeTruthy();
    expect(within(here).getByRole("button", { name: /^Storm night/ })).toBeTruthy();
    expect(within(origin as HTMLElement).getByRole("button", { name: /^Lighthouse/ })).toBeTruthy();
  });

  it("opens another chat of the tree", async () => {
    const runtime = await open();
    const tree = await screen.findByRole("list", { name: "Branches" });
    fireEvent.click(within(tree).getByRole("button", { name: /^Harbour/ }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/chat/c0"));
  });

  it("switches to another branch with the list revision", async () => {
    const select = vi.fn((_args: unknown) => ({ status: "ok" as const, data: { branch_id: "other", revision: 8 } }));
    await open({ conversationBranchSelect: select });
    fireEvent.click(await screen.findByRole("button", { name: /^Storm(?! night)/ }));
    await waitFor(() => expect(select).toHaveBeenCalledTimes(1));
    expect(select.mock.calls[0]?.[0]).toMatchObject({ branch_id: "other", expected_revision: 7 });
  });

  it("renames a branch inline", async () => {
    const rename = vi.fn((_args: unknown) => ({ status: "ok" as const, data: { branch_id: "other", revision: 8 } }));
    await open({ conversationBranchRename: rename });
    fireEvent.click(await screen.findByRole("button", { name: "Actions for Storm" }));
    fireEvent.click(await screen.findByRole("button", { name: /^Rename/ }));
    fireEvent.change(await screen.findByRole("textbox", { name: "Branch name" }), { target: { value: "Calm" } });
    fireEvent.click(screen.getByRole("button", { name: "Save name" }));
    await waitFor(() => expect(rename).toHaveBeenCalledTimes(1));
    expect(rename.mock.calls[0]?.[0]).toMatchObject({ branch_id: "other", label: "Calm", expected_revision: 7 });
  });

  it("keeps the original branch from being deleted", async () => {
    await open();
    fireEvent.click(await screen.findByRole("button", { name: "Actions for Original" }));
    expect((await screen.findByRole("button", { name: /^Delete/ })).hasAttribute("disabled")).toBe(true);
    expect(screen.getByText("The original branch can't be deleted")).toBeTruthy();
  });

  it("deletes another branch after confirming", async () => {
    const remove = vi.fn((_args: unknown) => ({ status: "ok" as const, data: { branch_id: "other", revision: 8 } }));
    await open({ conversationBranchDelete: remove });
    fireEvent.click(await screen.findByRole("button", { name: "Actions for Storm" }));
    fireEvent.click(await screen.findByRole("button", { name: /^Delete/ }));
    const dialog = (await screen.findAllByRole("dialog")).find((candidate) => candidate.textContent?.includes("Delete this branch?"));
    expect(dialog).toBeTruthy();
    fireEvent.click(within(dialog as HTMLElement).getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(remove).toHaveBeenCalledTimes(1));
    expect(remove.mock.calls[0]?.[0]).toMatchObject({ branch_id: "other", expected_revision: 7 });
  });
});
