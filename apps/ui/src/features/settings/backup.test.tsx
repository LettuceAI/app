import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { toast } from "@lettuceai/crisp";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ApiError, BackupIndexView, JobView } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers, type MockStreamHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

beforeEach(() => {
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

const entry = (patch: Partial<BackupIndexView> = {}): BackupIndexView => ({
  id: "b1", uri: "/backups/one.lettuce", header: { version: 2, created_at: new Date(2026, 9, 1, 12, 0).getTime(), app_version: "2.3.0" }, size: 2048, availability: "available", ...patch,
});

function job(patch: Partial<JobView>): JobView {
  return {
    id: "j1", kind: "backup_export", subject: { kind: "backup", id: "database-export" } as unknown as JobView["subject"], subject_detail: null, state: "succeeded",
    progress: { current: 0, total: null, unit: null, label_code: null, bytes_per_second: null }, created_at: 1, updated_at: 1, failure: null, result: null, ...patch,
  };
}

async function openPage(calls: MockCallHandlers = {}, streams: MockStreamHandlers = {}, platform: "other" | "android" = "other") {
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: () => ok({ ...mockAppStatus, platform, ui_state: { onboarding: { completed: true } } }),
        backupsList: () => ok([]),
        ...calls,
      },
      streams,
    }),
    history: createMemoryHistory({ initialEntries: ["/settings/backup"] }),
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return runtime;
}

const sheet = () => within(screen.getAllByRole("dialog").at(-1)!);

describe("backup settings", () => {
  it("lists saved backups newest first and shows the empty state when there are none", async () => {
    await openPage({ backupsList: () => ok([entry({ id: "old", header: { version: 2, created_at: 1000, app_version: "2.0.0" } }), entry()]) });
    const rows = await screen.findAllByRole("button", { name: /v2\./ });
    expect(rows[0]!.textContent).toContain("Oct 1, 2026");
    cleanup();
    await openPage();
    expect(await screen.findByText("No backups yet")).toBeTruthy();
  });

  it("shows a typed error with a retry when the list cannot load", async () => {
    await openPage({ backupsList: () => failure("unavailable") });
    expect(await screen.findByText("It isn't available right now.")).toBeTruthy();
    expect(screen.queryByText("diagnostic")).toBeNull();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  });

  it("asks for a matching password, then a destination, and exports to the chosen file", async () => {
    const save = vi.fn(() => ok({ target: { uri: "/backups/new.lettuce" } }));
    const exportBackup = vi.fn(() => ok({ job_id: "j1" }));
    await openPage({ filesPickSave: save, backupExport: exportBackup }, { jobWatch: () => ok(job({ result: { type: "backup_exported", backup: entry(), warnings: [] } })) });
    fireEvent.click(await screen.findByRole("button", { name: /New backup/ }));
    const create = await screen.findByRole("button", { name: "Create" }) as HTMLButtonElement;
    expect(create.disabled).toBe(true);
    fireEvent.change(sheet().getByLabelText(/^Password/), { target: { value: "secret1" } });
    fireEvent.change(sheet().getByLabelText(/^Confirm password/), { target: { value: "secret2" } });
    expect(sheet().getByText("The passwords don't match")).toBeTruthy();
    expect((sheet().getByRole("button", { name: "Create" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(sheet().getByLabelText(/^Confirm password/), { target: { value: "secret1" } });
    fireEvent.click(sheet().getByRole("button", { name: "Create" }));
    await waitFor(() => expect(exportBackup).toHaveBeenCalledTimes(1));
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ kind: "backup", suggested_name: expect.stringMatching(/^lettuce_backup_\d{8}_\d{6}\.lettuce$/) }));
    expect((exportBackup.mock.calls[0] as unknown as [object])[0]).toMatchObject({ password: "secret1", target: { uri: "/backups/new.lettuce" } });
    expect(await screen.findAllByText("Backup created")).not.toHaveLength(0);
  });

  it("exports nothing when the save dialog is cancelled", async () => {
    const exportBackup = vi.fn();
    await openPage({ filesPickSave: () => ok({ target: null }), backupExport: exportBackup });
    fireEvent.click(await screen.findByRole("button", { name: /New backup/ }));
    await screen.findByRole("button", { name: "Create" });
    fireEvent.change(sheet().getByLabelText(/^Password/), { target: { value: "secret1" } });
    fireEvent.change(sheet().getByLabelText(/^Confirm password/), { target: { value: "secret1" } });
    fireEvent.click(sheet().getByRole("button", { name: "Create" }));
    await act(async () => { await new Promise((resolve) => setTimeout(resolve, 50)); });
    expect(exportBackup).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog")).toBeTruthy();
  });

  it("lets Android write to Downloads without a save dialog", async () => {
    const save = vi.fn();
    const exportBackup = vi.fn(() => ok({ job_id: "j1" }));
    await openPage({ filesPickSave: save, backupExport: exportBackup }, { jobWatch: () => ok(job({ result: { type: "backup_exported", backup: entry(), warnings: [] } })) }, "android");
    fireEvent.click(await screen.findByRole("button", { name: /New backup/ }));
    await screen.findByRole("button", { name: "Create" });
    expect(sheet().getByText("Saved to Downloads/LettuceAI.")).toBeTruthy();
    fireEvent.change(sheet().getByLabelText(/^Password/), { target: { value: "secret1" } });
    fireEvent.change(sheet().getByLabelText(/^Confirm password/), { target: { value: "secret1" } });
    fireEvent.click(sheet().getByRole("button", { name: "Create" }));
    await waitFor(() => expect(exportBackup).toHaveBeenCalledTimes(1));
    expect(save).not.toHaveBeenCalled();
    expect((exportBackup.mock.calls[0] as unknown as [object])[0]).toMatchObject({ target: null });
  });

  it("keeps the sheet and says why when the export job fails", async () => {
    await openPage({ filesPickSave: () => ok({ target: { uri: "/backups/new.lettuce" } }), backupExport: () => ok({ job_id: "j1" }) },
      { jobWatch: () => ok(job({ state: "failed", failure: { code: "unavailable", reason: "backup_file_busy", retryable: true } as never })) });
    fireEvent.click(await screen.findByRole("button", { name: /New backup/ }));
    await screen.findByRole("button", { name: "Create" });
    fireEvent.change(sheet().getByLabelText(/^Password/), { target: { value: "secret1" } });
    fireEvent.change(sheet().getByLabelText(/^Confirm password/), { target: { value: "secret1" } });
    fireEvent.click(sheet().getByRole("button", { name: "Create" }));
    expect(await sheet().findByText("The file is in use by another app.")).toBeTruthy();
    expect((sheet().getByLabelText(/^Password/) as HTMLInputElement).value).toBe("secret1");
  });

  it("removes a backup from the list without touching the file, and deletes the file after confirming", async () => {
    const forget = vi.fn(() => ok(null));
    const erase = vi.fn(() => ok(null));
    await openPage({ backupsList: () => ok([entry()]), backupForget: forget, backupDelete: erase });
    fireEvent.click(await screen.findByRole("button", { name: /v2\.3\.0/ }));
    fireEvent.click(await sheet().findByRole("button", { name: /Remove from list/ }));
    await waitFor(() => expect(forget).toHaveBeenCalledWith(expect.objectContaining({ id: "b1" })));
    expect(erase).not.toHaveBeenCalled();
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(await screen.findByRole("button", { name: /v2\.3\.0/ }));
    fireEvent.click(await sheet().findByRole("button", { name: /Delete file/ }));
    expect(erase).not.toHaveBeenCalled();
    fireEvent.click(within((await screen.findAllByRole("dialog")).at(-1)!).getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(erase).toHaveBeenCalledWith(expect.objectContaining({ id: "b1" })));
  });

  it("offers only removal for a backup whose file is gone", async () => {
    await openPage({ backupsList: () => ok([entry({ availability: "missing" })]) });
    fireEvent.click(await screen.findByRole("button", { name: /The file is missing/ }));
    expect((await sheet().findAllByText("The file is missing")).length).toBeGreaterThan(0);
    expect(sheet().queryByRole("button", { name: /^Restore/ })).toBeNull();
    expect(sheet().queryByRole("button", { name: /Delete file/ })).toBeNull();
    expect(sheet().getByRole("button", { name: /Remove from list/ })).toBeTruthy();
  });

  it("restores a saved backup from its sheet with the password", async () => {
    const restore = vi.fn(() => ok({ job_id: "j1" }));
    await openPage({ backupsList: () => ok([entry()]), backupRestore: restore },
      { jobWatch: () => ok(job({ kind: "backup_restore", result: { type: "backup_restored", kept_file: "old.db", warnings: [] } })) });
    fireEvent.click(await screen.findByRole("button", { name: /v2\.3\.0/ }));
    fireEvent.click(await sheet().findByRole("button", { name: /^Restore/ }));
    expect(await screen.findByText("Restore backup")).toBeTruthy();
    fireEvent.change(sheet().getByLabelText(/Password/), { target: { value: "secret1" } });
    fireEvent.click(sheet().getByRole("button", { name: "Restore" }));
    fireEvent.click(within((await screen.findAllByRole("dialog")).at(-1)!).getByRole("button", { name: "Restore" }));
    await waitFor(() => expect(restore).toHaveBeenCalledTimes(1));
    expect((restore.mock.calls[0] as unknown as [object])[0]).toMatchObject({ source: { uri: "/backups/one.lettuce" }, password: "secret1" });
  });

  it("inspects a picked file before offering to restore it and rejects other files", async () => {
    const pick = vi.fn(() => ok({ sources: [{ uri: "/downloads/x.lettuce" }] }));
    let valid = true;
    await openPage({ filesPickOpen: pick, backupInspect: () => (valid ? ok({ version: 2, created_at: 1, app_version: "2.3.0" }) : failure("malformed")) });
    fireEvent.click(await screen.findByRole("button", { name: /Restore from a file/ }));
    expect(await screen.findByText("Restore backup")).toBeTruthy();
    expect(pick).toHaveBeenCalledWith({ kinds: ["backup"], multiple: false });
    fireEvent.click(sheet().getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByText("Restore backup")).toBeNull());
    valid = false;
    fireEvent.click(screen.getByRole("button", { name: /Restore from a file/ }));
    expect((await screen.findAllByText("That file is not a LettuceAI backup.")).length).toBeGreaterThan(0);
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
