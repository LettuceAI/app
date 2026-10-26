import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiFailure } from "@/api/client";
import { openExternalUrl } from "@/api/external-links";
import { createMockTransport, mockAppStatus } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({
  createWindowController: vi.fn().mockResolvedValue(null),
}));
vi.mock("@/api/external-links", () => ({ openExternalUrl: vi.fn().mockResolvedValue(undefined) }));

beforeEach(() => vi.mocked(openExternalUrl).mockReset().mockResolvedValue(undefined));

async function renderAt(path: string, platform: "other" | "android" = "other") {
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: () => ({
          status: "ok",
          data: {
            ...mockAppStatus,
            version: "9.9.9",
            platform,
            ui_state: { onboarding: { completed: true } },
          },
        }),
        jobsList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
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
  it("opens About community links", async () => {
    await renderAt("/settings/about");
    await screen.findByText("Standard");
    for (const [name, url] of [
      [/^Website/, "https://www.lettuceai.app"],
      [/^Discord/, "https://discord.gg/745bEttw2r"],
      [/^Source code/, "https://github.com/LettuceAI/app"],
    ] as const) {
      const row = screen.getByRole("button", { name });
      await waitFor(() => expect(row.hasAttribute("disabled")).toBe(false));
      fireEvent.click(row);
      await waitFor(() => expect(openExternalUrl).toHaveBeenLastCalledWith(url));
    }
    expect(screen.queryByText("External links are not available in this build.")).toBeNull();
  });

  it("prefills the actual bug-report form with this build's version and platform", async () => {
    await renderAt("/settings/about", "android");
    fireEvent.click(await screen.findByRole("button", { name: /^Report a problem/ }));
    await waitFor(() => expect(openExternalUrl).toHaveBeenCalledOnce());
    const url = new URL(vi.mocked(openExternalUrl).mock.calls[0]![0]);
    expect(url.origin + url.pathname).toBe("https://github.com/LettuceAI/app/issues/new");
    expect(url.searchParams.get("template")).toBe("bug_report.yml");
    expect(url.searchParams.get("version")).toBe("9.9.9 (normal)");
    expect(url.searchParams.get("platform")).toBe("Android");
  });

  it("shows opener failures and retries the same link", async () => {
    vi.mocked(openExternalUrl).mockRejectedValueOnce(
      new ApiFailure("unavailable", "private host diagnostic", null),
    );
    await renderAt("/settings/about");
    fireEvent.click(await screen.findByRole("button", { name: /^Website/ }));
    expect(await screen.findByText("Couldn't open the link")).toBeTruthy();
    expect(screen.queryByText("private host diagnostic")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(openExternalUrl).toHaveBeenCalledTimes(2));
    expect(openExternalUrl).toHaveBeenLastCalledWith("https://www.lettuceai.app");
    await waitFor(() => expect(screen.queryByText("Couldn't open the link")).toBeNull());
  });

  it("lists every group and opens About", async () => {
    const runtime = await renderAt("/settings", "android");
    expect(await screen.findByText("Intelligence")).toBeTruthy();
    expect(screen.getByText("Danger zone")).toBeTruthy();
    expect(screen.getByRole("button", { name: /Providers/ }).hasAttribute("disabled")).toBe(false);
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
    await renderAt("/settings/voices");
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
    expect(screen.getByRole("button", { name: "Providers" }).hasAttribute("disabled")).toBe(false);
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
