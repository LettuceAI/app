import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, expect, it, vi } from "vitest";
import { createMockTransport, mockAppStatus } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

afterEach(() => cleanup());

it("explains that Discover is coming and offers to create a character meanwhile", async () => {
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: () => ({ status: "ok", data: { ...mockAppStatus, ui_state: { onboarding: { completed: true } } } }),
        conversationsList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
      },
    }),
    history: createMemoryHistory({ initialEntries: ["/discover"] }),
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  expect(await screen.findByText("Discover is on its way")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Follow along on Discord" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Create a character" }));
  await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/characters/new"));
});
