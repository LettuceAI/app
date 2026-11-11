import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SyncConflictView, SyncCounts, SyncStatus } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(600);
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(600);
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const sharing: SyncStatus = { type: "sharing", addresses: ["192.168.1.20", "10.0.0.5"], port: 40111, pin: "482913" };
const counts: SyncCounts = { sent_changes: 4, received_changes: 12, received_assets: 0, received_blobs: 0, received_bytes: 0, unavailable_assets: 0, received_secrets: 0, skipped_secrets: 0 };
const noConflicts = { conflicts: [], legacy_conflicts: [], forks: [] };

const conflict: SyncConflictView = {
  conflict_id: "k1",
  entity_kind: "persona_default",
  entity_id: "default",
  detected_at: 1_000,
  current: { change_id: "change-current", device_id: "d1", timestamp: 2_000, value: { type: "persona_default", value: { persona_id: null, revision: 1, created_at: 1, updated_at: 1 } } },
  other: { change_id: "change-other", device_id: "d2", timestamp: 3_000, value: { type: "deleted" } },
};

async function openPage(path: string, calls: MockCallHandlers = {}) {
  const transport = createMockTransport({
    calls: {
      appStatus: () => ok({ ...mockAppStatus, ui_state: { onboarding: { completed: true } } }),
      syncStatus: () => ok<SyncStatus>({ type: "idle" }),
      syncConflictsList: () => ok(noConflicts),
      ...calls,
    },
  });
  const runtime = await createAppRuntime({ transport, history: createMemoryHistory({ initialEntries: [path] }), i18n: await createTestI18n("en") });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return transport;
}

const sheet = () => within(screen.getAllByRole("dialog").at(-1)!);

describe("sync page", () => {
  it("starts hosting and shows the PIN, the address and the QR code", async () => {
    let status: SyncStatus = { type: "idle" };
    const start = vi.fn(() => {
      status = sharing;
      return ok({ addresses: ["192.168.1.20", "10.0.0.5"], port: 40111, pin: "482913" });
    });
    await openPage("/settings/sync", { syncStatus: () => ok(status), syncShareStart: start });
    fireEvent.click(await screen.findByRole("button", { name: /Host on this device/ }));
    fireEvent.click(screen.getByRole("button", { name: "Start hosting" }));
    expect(await screen.findByText("482913")).toBeTruthy();
    expect(start).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("img", { name: /QR code/ })).toBeTruthy();
    expect(screen.getAllByText("192.168.1.20").length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Stop hosting" })).toBeTruthy();
  });

  it("asks before accepting a connection and approves it", async () => {
    const approve = vi.fn(() => ok<SyncStatus>({ type: "pending_start", device_name: "Pixel" }));
    let status: SyncStatus = sharing;
    const transport = await openPage("/settings/sync", { syncStatus: () => ok(status), syncShareApprove: approve });
    await screen.findByText("482913");
    status = { type: "pending_approval", device_name: "Pixel" };
    await act(async () => transport.emitAppEvent({ type: "sync_status_changed", status }));
    await screen.findByRole("dialog");
    expect(sheet().getByText("Pixel wants to sync with this device.")).toBeTruthy();
    fireEvent.click(sheet().getByRole("button", { name: "Accept connection" }));
    await waitFor(() => expect(approve).toHaveBeenCalledWith({ approve: true }));
  });

  it("connects only with a valid address and PIN", async () => {
    const connect = vi.fn(() => ok<SyncStatus>({ type: "connecting" }));
    await openPage("/settings/sync", { syncConnect: connect });
    const submit = await screen.findByRole("button", { name: "Connect" }) as HTMLButtonElement;
    fireEvent.change(screen.getByLabelText(/Host address/), { target: { value: "not an address" } });
    fireEvent.blur(screen.getByLabelText(/Host address/));
    expect(screen.getByText(/Enter an IPv4 address and port/)).toBeTruthy();
    fireEvent.paste(screen.getByLabelText(/1 of 6/), { clipboardData: { getData: () => "123456" } });
    expect(submit.disabled).toBe(true);
    fireEvent.change(screen.getByLabelText(/Host address/), { target: { value: "192.168.1.20:40111" } });
    expect(submit.disabled).toBe(false);
    fireEvent.click(submit);
    await waitFor(() => expect(connect).toHaveBeenCalledWith({ address: "192.168.1.20", port: 40111, pin: "123456" }));
  });

  it("says why a wrong PIN failed", async () => {
    await openPage("/settings/sync", {
      syncStatus: () => ok<SyncStatus>({ type: "failed", reason: { code: "unavailable", message: "diagnostic", details: { type: "sync", reason: { type: "authentication_failed" } } } }),
    });
    expect(await screen.findByText("The PIN is wrong. Check the PIN on the host device and try again.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  });

  it("summarises a finished sync with only the counts that are not zero and links to the conflicts", async () => {
    const list = { ...noConflicts, conflicts: [conflict] };
    let status: SyncStatus = { type: "idle" };
    const transport = await openPage("/settings/sync", { syncStatus: () => ok(status), syncConflictsList: () => ok(list) });
    await screen.findByRole("button", { name: /Join a device/ });
    status = { type: "completed", counts, conflicts: 1, forks: 1 };
    await act(async () => transport.emitAppEvent({ type: "sync_status_changed", status }));
    expect(await screen.findByText("Sync complete")).toBeTruthy();
    expect(screen.getByText("Changes sent")).toBeTruthy();
    expect(screen.getByText("Changes received")).toBeTruthy();
    expect(screen.queryByText("Secrets skipped")).toBeNull();
    expect(screen.getAllByRole("button", { name: /2 items need your choice/ }).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: /Conflicts/ })).toBeTruthy();
  });
});

describe("sync conflicts page", () => {
  it("resolves a conflict against the change it was shown with", async () => {
    const resolve = vi.fn(() => ok({ change_id: "change-new" }));
    await openPage("/settings/sync/conflicts", { syncConflictsList: () => ok({ ...noConflicts, conflicts: [conflict] }), syncConflictResolve: resolve });
    fireEvent.click(await screen.findByRole("button", { name: /Default persona/ }));
    expect(await sheet().findByText("This device")).toBeTruthy();
    expect(sheet().getByText("Other device")).toBeTruthy();
    fireEvent.click(sheet().getByRole("button", { name: "Use the other device's version" }));
    await waitFor(() =>
      expect(resolve).toHaveBeenCalledWith({ conflict_id: "k1", expected_current_change: "change-current", choice: "other", client_operation_id: expect.any(String) }),
    );
  });

  it("shows an empty state when nothing is open", async () => {
    await openPage("/settings/sync/conflicts");
    expect(await screen.findByText("Nothing to resolve")).toBeTruthy();
  });
});
