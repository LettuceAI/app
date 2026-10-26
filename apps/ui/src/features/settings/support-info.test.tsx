import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { toast } from "@lettuceai/crisp";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { openExternalUrl } from "@/api/external-links";
import type { ModelView } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({
  createWindowController: vi.fn().mockResolvedValue(null),
}));
vi.mock("@/api/external-links", () => ({ openExternalUrl: vi.fn().mockResolvedValue(undefined) }));

beforeEach(() => vi.mocked(openExternalUrl).mockReset().mockResolvedValue(undefined));
afterEach(() => {
  toast.dismiss();
  cleanup();
});

async function mount(
  path: string,
  platform: "other" | "android" = "other",
  language: "en" | "cimode" = "en",
  failMark = false,
  pendingModels?: Promise<{
    status: "ok";
    data: { models: ModelView[]; default_model_id: string | null; revision: number };
  }>,
) {
  let status = {
    ...mockAppStatus,
    version: "3.0.0",
    platform,
    ui_state: {
      onboarding: { completed: true, kept: "saved" },
      legacyMigration: "declined",
      custom: "keep",
    } as Record<string, unknown>,
  };
  const updates: Record<string, unknown>[] = [];
  const update = vi.fn((request: { patch: Record<string, unknown> }) => {
    updates.push(request.patch);
    if (failMark) {
      failMark = false;
      return {
        status: "error" as const,
        error: { code: "unavailable" as const, message: "private write diagnostic", details: null },
      };
    }
    status = { ...status, ui_state: { ...status.ui_state, ...request.patch } };
    return { status: "ok" as const, data: { state: status.ui_state } };
  });
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: () => ({ status: "ok", data: status }),
        appUiStateUpdate: update,
        jobsList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        providerCatalog: () => ({ status: "ok", data: { providers: [] } }),
        ...(pendingModels ? { modelsList: () => pendingModels } : {}),
      },
    }),
    history: createMemoryHistory({ initialEntries: [path] }),
    i18n: await createTestI18n(language),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return { runtime, updates, status: () => status };
}

it.each(["other", "android"] as const)(
  "opens external support entries without changing the current %s page",
  async (platform) => {
    const { runtime } = await mount(
      platform === "android" ? "/settings" : "/settings/about",
      platform,
    );
    const pathname = runtime.router.state.location.pathname;
    for (const [label, url] of [
      [/^Changelog/, "https://www.lettuceai.app/changelog"],
      [/^Documentation/, "https://www.lettuceai.app/docs"],
    ] as const) {
      fireEvent.click(await screen.findByRole("button", { name: label }));
      await waitFor(() => expect(openExternalUrl).toHaveBeenLastCalledWith(url));
      expect(runtime.router.state.location.pathname).toBe(pathname);
    }
  },
);

it("opens Help as an in-app page from the phone settings list", async () => {
  const { runtime } = await mount("/settings", "android");
  fireEvent.click(await screen.findByRole("button", { name: /^Help & FAQ/ }));
  expect(await screen.findByRole("heading", { name: "Help & FAQ" })).toBeTruthy();
  expect(
    screen.getByRole("button", { name: "What is LettuceAI?" }).getAttribute("aria-expanded"),
  ).toBe("true");
  expect(runtime.router.state.location.pathname).toBe("/settings/help");
});

it("opens What's new over the current page and closes its history entry while marking the installed version", async () => {
  const { runtime, status, updates } = await mount("/settings/about");
  fireEvent.click(await screen.findByRole("button", { name: "What's new" }));
  const sheet = await screen.findByRole("dialog", { name: "What's new" });
  expect(within(sheet).getByText("These notes are for 2.2.5. You're running 3.0.0.")).toBeTruthy();
  expect(within(sheet).getByText("Installed version: 3.0.0")).toBeTruthy();
  expect(runtime.router.state.location.pathname).toBe("/settings/about");
  fireEvent.click(within(sheet).getByRole("button", { name: "Full changelog" }));
  await waitFor(() =>
    expect(openExternalUrl).toHaveBeenCalledWith("https://www.lettuceai.app/changelog"),
  );
  fireEvent.click(within(sheet).getByRole("button", { name: "Continue" }));
  await waitFor(() => expect(runtime.router.state.location.search.settingsPanel).toBeUndefined());
  await waitFor(() => expect(status().ui_state.lastSeenAppVersion).toBe("3.0.0"));
  expect(runtime.router.history.canGoBack()).toBe(false);
  expect(updates).toEqual([{ lastSeenAppVersion: "3.0.0" }]);
  expect(status().ui_state.onboarding).toEqual({ completed: true, kept: "saved" });
});

it("uses the same sheet for About's release-notes row and supports browser Back", async () => {
  const { runtime, updates } = await mount("/settings/about");
  fireEvent.click(await screen.findByRole("button", { name: /Release notes for 3\.0\.0/ }));
  await screen.findByRole("dialog", { name: "What's new" });
  await act(async () => runtime.router.history.back());
  await waitFor(() => expect(runtime.router.state.location.search.settingsPanel).toBeUndefined());
  expect(runtime.router.state.location.pathname).toBe("/settings/about");
  await waitFor(() => expect(updates).toEqual([{ lastSeenAppVersion: "3.0.0" }]));
  await act(async () => runtime.router.history.forward());
  const reopened = await screen.findByRole("dialog", { name: "What's new" });
  fireEvent.click(within(reopened).getByRole("button", { name: "Continue" }));
  await waitFor(() => expect(runtime.router.state.location.search.settingsPanel).toBeUndefined());
});

it("closes a directly linked sheet without leaving its underlying page", async () => {
  const { runtime } = await mount("/settings/help?settingsPanel=whatsNew");
  const sheet = await screen.findByRole("dialog", { name: "What's new" });
  fireEvent.click(within(sheet).getByRole("button", { name: "Continue" }));
  await waitFor(() => expect(runtime.router.state.location.search.settingsPanel).toBeUndefined());
  expect(runtime.router.state.location.pathname).toBe("/settings/help");
  expect(runtime.router.history.canGoBack()).toBe(false);
});

it("dismisses notes even if the viewed-version write fails, and retries just that write", async () => {
  const { runtime, updates } = await mount(
    "/settings/about?settingsPanel=whatsNew",
    "other",
    "en",
    true,
  );
  const sheet = await screen.findByRole("dialog", { name: "What's new" });
  fireEvent.click(within(sheet).getByRole("button", { name: "Continue" }));
  expect(await screen.findByText("Couldn't save the viewed version")).toBeTruthy();
  expect(runtime.router.state.location.search.settingsPanel).toBeUndefined();
  expect(screen.queryByText("private write diagnostic")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Try again" }));
  await waitFor(() =>
    expect(updates).toEqual([{ lastSeenAppVersion: "3.0.0" }, { lastSeenAppVersion: "3.0.0" }]),
  );
});

it("reopens setup and returns without changing the saved completion or migration state", async () => {
  const { runtime, updates, status } = await mount("/settings", "android");
  fireEvent.click(await screen.findByRole("button", { name: /^Setup guide/ }));
  expect(await screen.findByRole("button", { name: "Get started" })).toBeTruthy();
  expect(runtime.router.state.location.pathname).toBe("/welcome");
  expect(updates).toEqual([]);
  fireEvent.click(screen.getByRole("button", { name: "Back to settings" }));
  await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/settings"));
  expect(updates).toEqual([]);
  expect(status().ui_state).toEqual({
    onboarding: { completed: true, kept: "saved" },
    legacyMigration: "declined",
    custom: "keep",
  });
});

it("offers an exit for a directly linked setup replay", async () => {
  const { runtime, updates } = await mount("/welcome?setup=true", "android");
  fireEvent.click(await screen.findByRole("button", { name: "Back to settings" }));
  await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/settings"));
  expect(updates).toEqual([]);
});

it("does not navigate after a pending setup operation finishes after browser Back", async () => {
  let finish!: (value: {
    status: "ok";
    data: { models: ModelView[]; default_model_id: string | null; revision: number };
  }) => void;
  const pending = new Promise<Parameters<typeof finish>[0]>((resolve) => {
    finish = resolve;
  });
  const { runtime, updates } = await mount("/settings", "android", "en", false, pending);
  fireEvent.click(await screen.findByRole("button", { name: /^Setup guide/ }));
  fireEvent.click(await screen.findByRole("button", { name: "Other options" }));
  fireEvent.click(await screen.findByRole("button", { name: /^Skip setup/ }));
  await act(async () => runtime.router.history.back());
  await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/settings"));
  const model: ModelView = {
    id: "existing",
    provider_account_id: "provider",
    external_model_id: "model",
    display_name: "Existing model",
    kind: "chat",
    config: {},
    input_scopes: ["text"],
    output_scopes: ["text"],
    revision: 1,
    created_at: 0,
    updated_at: 0,
  };
  await act(async () =>
    finish({ status: "ok", data: { models: [model], default_model_id: model.id, revision: 1 } }),
  );
  expect(runtime.router.state.location.pathname).toBe("/settings");
  expect(updates).toEqual([]);
});

it("localizes notes without treating translated metadata keys as release data", async () => {
  await mount("/settings/about?settingsPanel=whatsNew", "other", "cimode");
  const sheet = await screen.findByRole("dialog", { name: "title" });
  expect(within(sheet).getByText("release.title")).toBeTruthy();
  expect(within(sheet).getByText("release.changes.contextUsage")).toBeTruthy();
  expect(within(sheet).getByRole("button", { name: "fullChangelog" })).toBeTruthy();
});
