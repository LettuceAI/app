import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createWindowController } from "@/api/window-controls";
import { createMockTransport, mockAppStatus, type MockTransportOptions } from "@/api/mock-transport";
import { createTestI18n } from "@/shared/testing/i18n";
import { App } from "./App";
import { createAppRuntime } from "./runtime";

const completedStatus = { ...mockAppStatus, ui_state: { onboarding: { completed: true } } };

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

async function renderApp(options: MockTransportOptions = {}, language: "en" | "cimode" = "en", path = "/settings/about") {
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      ...options,
      calls: { appStatus: () => ({ status: "ok", data: completedStatus }), ...options.calls },
    }),
    history: createMemoryHistory({ initialEntries: [path] }),
    i18n: await createTestI18n(language),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return runtime;
}

afterEach(() => {
  cleanup();
});

describe("status placeholder", () => {
  it("connects window buttons and removes rounding while maximized", async () => {
    let expanded!: (value: boolean) => void;
    const minimize = vi.fn().mockResolvedValue(undefined);
    const toggleMaximize = vi.fn().mockResolvedValue(undefined);
    const close = vi.fn().mockResolvedValue(undefined);
    const stop = vi.fn();
    vi.mocked(createWindowController).mockResolvedValueOnce({
      minimize, toggleMaximize, close, resize: vi.fn().mockResolvedValue(undefined),
      observeExpanded: async (changed) => { expanded = changed; changed(false); return stop; },
    });
    await renderApp({ calls: { appStatus: () => ({ status: "ok", data: { ...completedStatus, platform: "linux" } }) } });
    fireEvent.click(await screen.findByRole("button", { name: "Minimize" }));
    fireEvent.click(screen.getByRole("button", { name: "Maximize" }));
    fireEvent.click(screen.getByRole("button", { name: "Close window" }));
    expect(minimize).toHaveBeenCalledOnce();
    expect(toggleMaximize).toHaveBeenCalledOnce();
    expect(close).toHaveBeenCalledOnce();
    const frame = document.querySelector(".rounded-2xl");
    expect(frame).not.toBeNull();
    act(() => expanded(true));
    expect(frame!.classList.contains("rounded-2xl")).toBe(false);
    act(() => expanded(false));
    expect(frame!.classList.contains("rounded-2xl")).toBe(true);
    cleanup();
    expect(stop).toHaveBeenCalledOnce();
  });

  it("navigates through the shell destinations and keeps browser history", async () => {
    const runtime = await renderApp({}, "en", "/");
    fireEvent.click(await screen.findByRole("button", { name: "Groups" }));
    expect(await screen.findByRole("heading", { name: "Groups" })).toBeTruthy();
    expect(runtime.router.state.location.pathname).toBe("/groups");
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/settings/about"));
    await act(async () => { runtime.router.history.back(); });
    expect(await screen.findByRole("heading", { name: "Groups" })).toBeTruthy();
  });

  it("keeps every destination reachable on mobile without desktop window controls", async () => {
    await renderApp({ calls: { appStatus: () => ({ status: "ok", data: { ...completedStatus, platform: "android" } }) } });
    expect(await screen.findByRole("button", { name: "Settings" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Chats" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Close window" })).toBeNull();
  });

  it("shows the app name and the backend's version and platform", async () => {
    await renderApp({
      calls: { appStatus: () => ({ status: "ok", data: { ...completedStatus, version: "9.9.9", platform: "linux" } }) },
    });
    expect((await screen.findAllByText("9.9.9")).length).toBeGreaterThan(0);
    expect(screen.getByText("Linux")).toBeTruthy();
    expect(screen.getAllByText("LettuceAI").length).toBeGreaterThan(0);
  });

  it("shows the loading state while app_status is pending", async () => {
    let resolve: (() => void) | undefined;
    await renderApp({
      calls: {
        appStatus: () =>
          new Promise((done) => {
            resolve = () => done({ status: "ok", data: completedStatus });
          }),
      },
    });
    expect(await screen.findByText("Connecting to the app…")).toBeTruthy();
    await act(async () => resolve?.());
    expect((await screen.findAllByText(completedStatus.version)).length).toBeGreaterThan(0);
  });

  it("shows the typed error copy when app_status fails", async () => {
    await renderApp({
      calls: {
        appStatus: () => ({ status: "error", error: { code: "busy", message: "db locked", details: null } }),
      },
    });
    expect(await screen.findByText("The app is busy. Try again in a moment.")).toBeTruthy();
    expect(screen.queryByText("db locked")).toBeNull();
    expect(screen.getByText("Try again")).toBeTruthy();
  });

  it("renders every string through t()", async () => {
    await renderApp({}, "cimode");
    expect(await screen.findByText("platform.other")).toBeTruthy();
    expect(screen.getAllByText("app.name").length).toBeGreaterThan(0);
    expect(screen.getByText("about.version")).toBeTruthy();
    expect(screen.getByText("about.platform")).toBeTruthy();
    expect(screen.queryByText("LettuceAI")).toBeNull();
  });
});

it("boots when persisting the detected device locale fails", async () => {
  const runtime = await createAppRuntime({
    transport: createMockTransport({ calls: {
      appStatus: () => ({ status: "ok", data: { ...mockAppStatus, ui_state: {} } }),
      appUiStateUpdate: () => ({ status: "error", error: { code: "unavailable", message: "storage unavailable", details: null } }),
    } }),
    history: createMemoryHistory({ initialEntries: ["/"] }),
  });
  expect(runtime.i18n).toBeTruthy();
});


it("does not show the startup connection screen when navigating with cached app status", async () => {
  const appStatus = vi.fn(() => ({ status: "ok" as const, data: completedStatus }));
  const runtime = await renderApp({ calls: { appStatus } });
  await screen.findAllByText(completedStatus.version);
  const pendingScreens: string[] = [];
  const observer = new MutationObserver((records) => {
    for (const record of records) {
      for (const node of record.addedNodes) {
        if (node.textContent?.includes("Connecting to the app…")) pendingScreens.push(node.textContent);
      }
    }
  });
  observer.observe(document.body, { childList: true, subtree: true });
  try {
    await act(async () => { await runtime.router.navigate({ to: "/groups" }); });
    await act(async () => { await runtime.router.navigate({ to: "/settings/$section", params: { section: "providers" } }); });
    await act(async () => { await runtime.router.navigate({ to: "/settings/$section", params: { section: "models" } }); });
    await act(async () => { await runtime.router.navigate({ to: "/settings/$section", params: { section: "about" } }); });
    expect(pendingScreens).toEqual([]);
    expect(appStatus).toHaveBeenCalledTimes(1);
  } finally {
    observer.disconnect();
  }
});
