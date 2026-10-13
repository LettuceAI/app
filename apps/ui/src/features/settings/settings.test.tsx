import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createMockTransport, mockAppStatus } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

async function renderAt(path: string, platform: "other" | "android" = "other") {
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: () => ({ status: "ok", data: { ...mockAppStatus, version: "9.9.9", platform, ui_state: { onboarding: { completed: true } } } }),
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
  cleanup();
});

describe("settings routes", () => {
  it("lists every group and opens About, the only section built so far", async () => {
    const runtime = await renderAt("/settings", "android");
    expect(await screen.findByText("Intelligence")).toBeTruthy();
    expect(screen.getByText("Danger zone")).toBeTruthy();
    expect(screen.getByRole("button", { name: /Providers/ }).hasAttribute("disabled")).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: /About/ }));
    expect(await screen.findByText("Standard")).toBeTruthy();
    expect(runtime.router.state.location.pathname).toBe("/settings/about");
  });

  it("shows the version, build and platform on About and goes back to the list", async () => {
    const runtime = await renderAt("/settings/about", "android");
    expect((await screen.findAllByText("9.9.9")).length).toBeGreaterThan(0);
    expect(screen.getByText("Standard")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(await screen.findByText("Intelligence")).toBeTruthy();
    expect(runtime.router.state.location.pathname).toBe("/settings");
  });

  it("does not find a section that has no page, or one that does not exist", async () => {
    await renderAt("/settings/providers");
    expect(await screen.findByText("Nothing here")).toBeTruthy();
    cleanup();
    await renderAt("/settings/nope");
    expect(await screen.findByText("Nothing here")).toBeTruthy();
  });

  it("keeps Settings selected in the navigation on a section page", async () => {
    await renderAt("/settings/about");
    const settings = await screen.findByRole("button", { name: "Settings" });
    expect(settings.getAttribute("aria-current")).toBe("page");
  });

  it("opens About straight away on desktop and has no Back button there", async () => {
    const runtime = await renderAt("/settings");
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/settings/about"));
    expect((await screen.findAllByText("9.9.9")).length).toBeGreaterThan(0);
    expect(screen.queryByRole("button", { name: "Back" })).toBeNull();
  });

  it("swaps the desktop sidebar for the settings groups", async () => {
    const runtime = await renderAt("/settings/about");
    expect(await screen.findByRole("region", { name: "Intelligence" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Danger zone" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Home" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Groups" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Create" })).toBeNull();
    expect(screen.queryByText("Chats you start show up here.")).toBeNull();
    expect(screen.getByRole("button", { name: "About" }).getAttribute("aria-current")).toBe("page");
    expect(screen.getByRole("button", { name: "Providers" }).hasAttribute("disabled")).toBe(true);
    expect(runtime.router.state.location.pathname).toBe("/settings/about");
  });

  it("leaves settings for Home from the gear", async () => {
    const runtime = await renderAt("/settings/about");
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/"));
    expect(await screen.findByRole("button", { name: "Home" })).toBeTruthy();
  });

  it("leaves settings for Home from the Back to app item", async () => {
    const runtime = await renderAt("/settings/about");
    fireEvent.click(await screen.findByRole("button", { name: "Back to app" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/"));
    expect(screen.queryByRole("button", { name: "Back to app" })).toBeNull();
  });

  it("opens settings from the gear outside settings", async () => {
    const runtime = await renderAt("/groups");
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/settings/about"));
  });
});
