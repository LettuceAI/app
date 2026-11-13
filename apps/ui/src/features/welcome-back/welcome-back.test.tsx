import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AppUiStateUpdateRequest, JobView_Serialize } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({
  createWindowController: vi.fn().mockResolvedValue(null),
}));

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const importJob: JobView_Serialize = {
  id: "import",
  kind: "backup_restore",
  subject: { type: "none" } as unknown as JobView_Serialize["subject"],
  subject_detail: null,
  state: "succeeded",
  progress: { current: 1, total: 1, unit: null, label_code: "switch", bytes_per_second: null },
  created_at: 0,
  updated_at: 0,
  failure: null,
  result: null,
};

async function reachDone() {
  const updates: AppUiStateUpdateRequest[] = [];
  const transport = createMockTransport({
    calls: {
      appStatus: () => ok({ ...mockAppStatus, legacy_state: { type: "ready" } as never }),
      appUiStateUpdate: (request) => {
        updates.push(request);
        return ok({ state: { ...mockAppStatus.ui_state, ...request.patch } } as never);
      },
      legacyDatabaseImport: () => ok({ job_id: importJob.id }),
      settingsGet: () => ok({ global: { dynamic_memory: { enabled: false } } } as never),
      embeddingStatus: () => ok([]),
      conversationsList: () => ok({ items: [], next_cursor: null }),
    },
    streams: { jobWatch: () => ok(importJob) },
  });
  const runtime = await createAppRuntime({
    transport,
    history: createMemoryHistory({ initialEntries: ["/welcome-back"] }),
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  fireEvent.click(await screen.findByRole("button", { name: "Migrate data" }, { timeout: 8_000 }));
  await screen.findByRole("heading", { name: "You're all set." }, { timeout: 8_000 });
  return { updates };
}

const heading = (name: string) => screen.findByRole("heading", { name }, { timeout: 8_000 });
const press = (name: string) => fireEvent.click(screen.getByRole("button", { name }));

describe("welcome after Welcome back", () => {
  it("walks the 3.0 pages with the app's own screens, opens every change and finishes like the done step", async () => {
    const { updates } = await reachDone();

    press("See what's new");
    await heading("We redesigned everything.");
    expect(screen.queryByRole("button", { name: "Open LettuceAI" })).toBeNull();

    press("See what's new");
    await heading("Groups can do it all now.");
    expect(await screen.findByText("The northern gate")).toBeTruthy();
    expect(screen.getAllByText("DeepSeek V3").length).toBeGreaterThan(0);

    press("Back");
    await heading("We redesigned everything.");
    press("See what's new");
    await heading("Groups can do it all now.");

    press("Next");
    await heading("Companion group chats.");
    press("Next");
    await heading("Make a character by talking.");
    expect(await screen.findByText("Review changes")).toBeTruthy();
    press("Next");
    await heading("Our own models.");
    expect(await screen.findByText("Thymos")).toBeTruthy();
    expect(screen.getByText("Eidos")).toBeTruthy();
    press("Next");
    await heading("Lorebooks that understand sentences.");
    expect(await screen.findByText("House Welan")).toBeTruthy();

    press("Every change");
    await heading("Everything in 3.0");
    fireEvent.click(screen.getByRole("button", { name: "What's new" }));
    await waitFor(() => expect(screen.queryByRole("heading", { name: "Everything in 3.0" })).toBeNull());

    expect(updates).toHaveLength(1);
    press("Open LettuceAI");
    await waitFor(() => expect(updates).toHaveLength(2));
    expect(updates[1]!.patch).toMatchObject({ onboarding: { completed: true } });
  }, 40_000);

  it("still opens the app straight from the done step", async () => {
    const { updates } = await reachDone();
    press("Open LettuceAI");
    await waitFor(() => expect(updates).toHaveLength(2));
    expect(updates[1]!.patch).toMatchObject({ onboarding: { completed: true } });
  }, 20_000);
});
