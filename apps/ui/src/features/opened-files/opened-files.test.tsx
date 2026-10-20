import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { toast } from "@lettuceai/crisp";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ApiError, JobView, OpenedItem } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers, type MockStreamHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

const inbox = vi.hoisted(() => ({
  items: [] as OpenedItem[],
  listeners: new Set<() => void>(),
  takes: 0,
}));
vi.mock("@/api/opened-files", () => ({
  createOpenedFilesInbox: async () => ({
    take: async () => {
      inbox.takes += 1;
      return inbox.items.splice(0);
    },
    listen: async (onOpened: () => void) => {
      inbox.listeners.add(onOpened);
      return () => { inbox.listeners.delete(onOpened); };
    },
  }),
}));

beforeEach(() => {
  inbox.items = [];
  inbox.listeners.clear();
  inbox.takes = 0;
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(600);
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(600);
});
afterEach(() => {
  toast.dismiss();
  cleanup();
  vi.restoreAllMocks();
});

const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const failure = (code: ApiError["code"]) => ({ status: "error" as const, error: { code, message: "diagnostic", details: null } });
const file = (name: string): OpenedItem => ({ kind: "file", source: { uri: `/files/${name}` } });

async function open(item: OpenedItem[], calls: MockCallHandlers = {}, streams: MockStreamHandlers = {}, completed = true) {
  inbox.items = [...item];
  const status = vi.fn(() => ok({ ...mockAppStatus, ui_state: completed ? { onboarding: { completed: true } } : {} }));
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: status,
        charactersList: () => ok({ items: [], next_cursor: null }),
        conversationsList: () => ok({ items: [], next_cursor: null }),
        conversationsLatestByCharacter: () => ok({ items: [], next_cursor: null }),
        ...calls,
      },
      streams,
    }),
    history: createMemoryHistory({ initialEntries: ["/"] }),
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return { runtime, status };
}

function preview(name: string, kind: "character" | "persona" | "lorebook" = "character") {
  const content = kind === "character" ? { kind, content: { name, description: "A sea captain" } } : kind === "persona" ? { kind, content: { title: name, description: "" } } : { kind, name, entries: [{}, {}] };
  return { kind: "character_card", name, digest: `digest-${name}`, content, uec3: null, images: [], warnings: [] } as never;
}

function job(patch: Partial<JobView>): JobView {
  return {
    id: "j1", kind: "backup_restore", subject: { kind: "backup", id: "b1" } as unknown as JobView["subject"], subject_detail: null, state: "succeeded",
    progress: { current: 0, total: null, unit: null, label_code: null, bytes_per_second: null }, created_at: 1, updated_at: 1, failure: null, result: null, ...patch,
  };
}

const sheet = () => within(screen.getAllByRole("dialog")[0]!);

describe("opened files", () => {
  it("takes the inbox on mount and on every event, and navigates without a sheet", async () => {
    const { runtime } = await open([{ kind: "navigate", path: "/library" }]);
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/library"));
    expect(screen.queryByRole("dialog")).toBeNull();
    inbox.items.push({ kind: "navigate", path: "/characters/new" });
    await act(async () => { for (const listener of inbox.listeners) listener(); });
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/characters/new"));
    expect(inbox.takes).toBeGreaterThanOrEqual(2);
  });

  it("keeps items queued while onboarding is on screen", async () => {
    const preview = vi.fn();
    const { runtime } = await open([{ kind: "navigate", path: "/library" }, file("a.uec")], { importPreview: preview }, {}, false);
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/welcome"));
    await act(async () => { await new Promise((resolve) => setTimeout(resolve, 100)); });
    expect(runtime.router.state.location.pathname).toBe("/welcome");
    expect(preview).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("previews a card, imports it with the preview digest and offers to open it", async () => {
    const commit = vi.fn(() => ok({ kind: "imported" as const, file_kind: "character_card" as never, id: "c1", warnings: [] }));
    const { runtime } = await open([file("aria.uec")], { importPreview: () => ok(preview("Aria")), importCommit: commit });
    expect(await screen.findByText("Import character")).toBeTruthy();
    expect(sheet().getByText("Aria")).toBeTruthy();
    expect(sheet().getByText("A sea captain")).toBeTruthy();
    fireEvent.click(sheet().getByRole("button", { name: "Import" }));
    await waitFor(() => expect(commit).toHaveBeenCalledTimes(1));
    expect((commit.mock.calls[0] as unknown as [object])[0]).toMatchObject({ source: { uri: "/files/aria.uec" }, digest: "digest-Aria", options: {} });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(await screen.findByRole("button", { name: "Open" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/characters/c1/edit"));
  });

  it("commits nothing on cancel", async () => {
    const commit = vi.fn();
    await open([file("aria.uec")], { importPreview: () => ok(preview("Aria", "lorebook")), importCommit: commit });
    expect(await screen.findByText("Import lorebook")).toBeTruthy();
    expect(sheet().getByText("2 entries")).toBeTruthy();
    fireEvent.click(sheet().getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(commit).not.toHaveBeenCalled();
  });

  it("keeps the sheet with a typed error and retries the same commit", async () => {
    let fail = true;
    const commit = vi.fn(() => (fail ? failure("unavailable") : ok({ kind: "imported" as const, file_kind: "persona_file" as never, id: "p1", warnings: [] })));
    await open([file("hunter.uec")], { importPreview: () => ok(preview("Hunter", "persona")), importCommit: commit });
    await screen.findByText("Import persona");
    fireEvent.click(sheet().getByRole("button", { name: "Import" }));
    expect(await sheet().findByText("It isn't available right now.")).toBeTruthy();
    expect(screen.queryByText("diagnostic")).toBeNull();
    fail = false;
    fireEvent.click(sheet().getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(commit).toHaveBeenCalledTimes(2));
    const ids = (commit.mock.calls as unknown as Array<[{ client_operation_id: string }]>).map(([request]) => request.client_operation_id);
    expect(ids[1]).toBe(ids[0]);
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("opens the restore sheet for a backup, asks for a password and confirms before restoring", async () => {
    const restore = vi.fn(() => ok({ job_id: "j1" }));
    const { status } = await open([file("all.lettuce")], {
      importPreview: () => failure("unsupported"),
      backupInspect: () => ok({ version: 1, created_at: new Date(2026, 9, 1, 12, 0).getTime(), app_version: "2.3.0" }),
      backupRestore: restore,
    }, { jobWatch: () => ok(job({ result: { type: "backup_restored", kept_file: "old.db", warnings: [] } })) });
    expect(await screen.findByText("Restore backup")).toBeTruthy();
    expect(sheet().getByText("2.3.0")).toBeTruthy();
    expect(sheet().getByText(/Oct 1, 2026/)).toBeTruthy();
    expect(sheet().getByText(/kept as a separate file/)).toBeTruthy();
    const action = sheet().getByRole("button", { name: "Restore" }) as HTMLButtonElement;
    expect(action.disabled).toBe(true);
    fireEvent.change(sheet().getByLabelText(/Password/), { target: { value: "secret" } });
    const before = status.mock.calls.length;
    fireEvent.click(sheet().getByRole("button", { name: "Restore" }));
    const dialogs = await screen.findAllByRole("dialog");
    expect(restore).not.toHaveBeenCalled();
    fireEvent.click(within(dialogs.at(-1)!).getByRole("button", { name: "Restore" }));
    await waitFor(() => expect(restore).toHaveBeenCalledTimes(1));
    expect((restore.mock.calls[0] as unknown as [object])[0]).toMatchObject({ source: { uri: "/files/all.lettuce" }, password: "secret" });
    await waitFor(() => expect(screen.queryByText("Restore backup")).toBeNull());
    await waitFor(() => expect(status.mock.calls.length).toBeGreaterThan(before));
    expect(await screen.findAllByText("Backup restored")).not.toHaveLength(0);
  });

  it("reads the job once when the restore watch ends without its last event", async () => {
    const get = vi.fn(() => ok(job({ result: { type: "backup_restored", kept_file: "old.db", warnings: [] } })));
    await open([file("all.lettuce")], {
      importPreview: () => failure("unsupported"),
      backupInspect: () => ok({ version: 1, created_at: 1, app_version: "2.3.0" }),
      backupRestore: () => ok({ job_id: "j1" }),
      jobGet: get,
    }, { jobWatch: () => failure("unavailable") });
    await screen.findByText("Restore backup");
    fireEvent.change(sheet().getByLabelText(/Password/), { target: { value: "secret" } });
    fireEvent.click(sheet().getByRole("button", { name: "Restore" }));
    fireEvent.click(within((await screen.findAllByRole("dialog")).at(-1)!).getByRole("button", { name: "Restore" }));
    expect(await screen.findAllByText("Backup restored")).not.toHaveLength(0);
    expect(get).toHaveBeenCalledWith({ job_id: "j1" });
    await waitFor(() => expect(screen.queryByText("Restore backup")).toBeNull());
  });

  it("keeps the sheet and the password when the password is wrong", async () => {
    await open([file("all.lettuce")], {
      importPreview: () => failure("malformed"),
      backupInspect: () => ok({ version: 1, created_at: 1, app_version: "2.3.0" }),
      backupRestore: () => ok({ job_id: "j1" }),
    }, { jobWatch: () => ok(job({ state: "failed", failure: { code: "authentication", reason: "backup_authentication", retryable: false } as never })) });
    await screen.findByText("Restore backup");
    fireEvent.change(sheet().getByLabelText(/Password/), { target: { value: "wrong" } });
    fireEvent.click(sheet().getByRole("button", { name: "Restore" }));
    fireEvent.click(within((await screen.findAllByRole("dialog")).at(-1)!).getByRole("button", { name: "Restore" }));
    expect(await sheet().findByText("The password is wrong or the backup is damaged.")).toBeTruthy();
    expect((sheet().getByLabelText(/Password/) as HTMLInputElement).value).toBe("wrong");
  });

  it("toasts an unrecognized file and opens the next queued item", async () => {
    const previewCall = vi.fn((request: { source: { uri: string } }) => (request.source.uri.endsWith("junk.txt") ? failure("unsupported") : ok(preview("Aria"))));
    await open([file("junk.txt"), file("aria.uec")], {
      importPreview: previewCall,
      backupInspect: () => failure("malformed"),
    });
    expect((await screen.findAllByText("This file is not a LettuceAI card or backup.")).length).toBeGreaterThan(0);
    expect(await screen.findByText("Import character")).toBeTruthy();
    expect(previewCall).toHaveBeenCalledTimes(2);
  });

  it("shows other typed errors through the error mapping", async () => {
    await open([file("gone.uec")], { importPreview: () => failure("not_found") });
    expect((await screen.findAllByText("It no longer exists.")).length).toBeGreaterThan(0);
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
