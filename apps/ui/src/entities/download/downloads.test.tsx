import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { toast } from "@lettuceai/crisp";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { JobView } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));
vi.mock("@/api/opened-files", () => ({ createOpenedFilesInbox: async () => null }));

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

const running = (patch: Partial<JobView> = {}): JobView => ({
  id: "j1", kind: "artifact_install", subject: { kind: "artifact_install", id: "x" } as unknown as JobView["subject"],
  subject_detail: { type: "sd_lora_install", model_id: 1, version_id: 2, file_id: 3, filename: "film.safetensors" }, state: "running",
  progress: { current: 5, total: 10, unit: "bytes", label_code: "download", bytes_per_second: 1 }, created_at: 1, updated_at: 1, failure: null, result: null, ...patch,
});

async function mount(calls: MockCallHandlers = {}) {
  const transport = createMockTransport({ calls: {
    appStatus: () => ok({ ...mockAppStatus, ui_state: { onboarding: { completed: true } } }),
    charactersList: () => ok({ items: [], next_cursor: null }),
    conversationsList: () => ok({ items: [], next_cursor: null }),
    conversationsLatestByCharacter: () => ok({ items: [], next_cursor: null }),
    jobsList: () => ok({ items: [], next_cursor: null }),
    ...calls,
  } });
  const runtime = await createAppRuntime({ transport, history: createMemoryHistory({ initialEntries: ["/"] }), i18n: await createTestI18n("en") });
  await act(async () => { render(<App runtime={runtime} />); });
  return transport;
}

describe("downloads menu", () => {
  it("opens from the title bar and says nothing is downloading", async () => {
    await mount();
    fireEvent.click(await screen.findByRole("button", { name: "Downloads" }));
    expect(await screen.findByText("Nothing downloading.")).toBeTruthy();
    expect(screen.getByText("Installs and updates show up here.")).toBeTruthy();
  });

  it("lists active downloads by their detail name with progress and counts them on the button", async () => {
    await mount({ jobsList: () => ok({ items: [running()], next_cursor: null }) });
    fireEvent.click(await screen.findByRole("button", { name: "Downloads, one active" }));
    const panel = within(await screen.findByRole("dialog"));
    expect(panel.getByText("film.safetensors")).toBeTruthy();
    expect(panel.getByText("50%")).toBeTruthy();
    expect(panel.queryByText("Nothing downloading.")).toBeNull();
  });

  it("follows job events: new downloads appear, finished ones leave with a toast", async () => {
    const transport = await mount();
    fireEvent.click(await screen.findByRole("button", { name: "Downloads" }));
    await screen.findByText("Nothing downloading.");
    await act(async () => { transport.emitAppEvent({ type: "job_updated", job: running() }); });
    expect(await screen.findByText("film.safetensors")).toBeTruthy();
    await act(async () => { transport.emitAppEvent({ type: "job_updated", job: running({ state: "succeeded", updated_at: 2, progress: { current: 10, total: 10, unit: "bytes", label_code: "download", bytes_per_second: null } }) }); });
    await waitFor(() => expect(screen.queryByText("film.safetensors")).toBeNull());
    expect((await screen.findAllByText("film.safetensors is ready")).length).toBeGreaterThan(0);
    expect(screen.getByText("Nothing downloading.")).toBeTruthy();
  });

  it("tells a failed download apart and asks the backend to cancel a running one", async () => {
    const cancel = vi.fn(() => ok(null));
    const transport = await mount({ jobsList: () => ok({ items: [running()], next_cursor: null }), jobCancel: cancel });
    fireEvent.click(await screen.findByRole("button", { name: "Downloads, one active" }));
    fireEvent.click(await within(await screen.findByRole("dialog")).findByRole("button", { name: /Cancel/ }));
    await waitFor(() => expect(cancel).toHaveBeenCalledWith({ job_id: "j1" }));
    await act(async () => { transport.emitAppEvent({ type: "job_updated", job: running({ state: "failed", updated_at: 2 }) }); });
    expect((await screen.findAllByText("film.safetensors could not be downloaded")).length).toBeGreaterThan(0);
  });
});
