import { act, cleanup, render, screen } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it } from "vitest";
import { createMockTransport, mockAppStatus, type MockTransportOptions } from "@/api/mock-transport";
import { createTestI18n } from "@/shared/testing/i18n";
import { App } from "./App";
import { createAppRuntime } from "./runtime";

async function renderApp(options: MockTransportOptions = {}, language: "en" | "cimode" = "en") {
  const runtime = await createAppRuntime({
    transport: createMockTransport(options),
    history: createMemoryHistory({ initialEntries: ["/"] }),
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
  it("shows the app name and the backend's version and platform", async () => {
    await renderApp({
      calls: { appStatus: () => ({ status: "ok", data: { ...mockAppStatus, version: "9.9.9", platform: "linux" } }) },
    });
    expect(await screen.findByText("9.9.9")).toBeTruthy();
    expect(screen.getByText("Linux")).toBeTruthy();
    expect(screen.getAllByText("LettuceAI").length).toBeGreaterThan(0);
  });

  it("shows the loading state while app_status is pending", async () => {
    let resolve: (() => void) | undefined;
    await renderApp({
      calls: {
        appStatus: () =>
          new Promise((done) => {
            resolve = () => done({ status: "ok", data: mockAppStatus });
          }),
      },
    });
    expect(await screen.findByText("Connecting to the app…")).toBeTruthy();
    await act(async () => resolve?.());
    expect(await screen.findByText(mockAppStatus.version)).toBeTruthy();
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
    expect(screen.getByText("status.version")).toBeTruthy();
    expect(screen.getByText("status.platform")).toBeTruthy();
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
