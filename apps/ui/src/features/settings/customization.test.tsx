import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SettingsUpdateRequest_Deserialize, SettingsView } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({
  createWindowController: vi.fn().mockResolvedValue(null),
}));
vi.mock("@/api/haptics", () => ({ typingHaptic: vi.fn().mockResolvedValue(undefined) }));
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});
const ok = <T,>(data: T) => ({ status: "ok" as const, data });

async function mount(
  options: {
    tab?: string;
    mobile?: boolean;
    value?: Record<string, unknown>;
    calls?: MockCallHandlers;
    fail?: () => boolean;
    gate?: Promise<void>;
  } = {},
) {
  let current = { global: { ui_preferences: { ...options.value } }, revision: 7 } as SettingsView;
  const requests: SettingsUpdateRequest_Deserialize[] = [];
  const update = vi.fn(async (request: SettingsUpdateRequest_Deserialize) => {
    requests.push(request);
    if (options.gate) await options.gate;
    if (options.fail?.())
      return {
        status: "error" as const,
        error: { code: "unavailable" as const, message: "private diagnostic", details: null },
      };
    if (request.expected_revision !== current.revision)
      return {
        status: "error" as const,
        error: { code: "conflict" as const, message: "stale", details: null },
      };
    if (request.patch.section === "ui_preferences") {
      const value = { ...current.global.ui_preferences };
      for (const change of request.patch.changes) {
        if (change.value === null) delete value[change.key];
        else value[change.key] = change.value;
      }
      current = {
        ...current,
        global: { ...current.global, ui_preferences: value },
        revision: current.revision + 1,
      };
    }
    return ok(current);
  });
  const transport = createMockTransport({
    calls: {
      appStatus: () =>
        ok({
          ...mockAppStatus,
          platform: options.mobile === false ? "other" : "android",
          ui_state: { onboarding: { completed: true }, locale: "en" },
        }),
      settingsGet: () => ok(current),
      settingsUpdate: update,
      jobsList: () => ok({ items: [], next_cursor: null }),
      ...options.calls,
    },
  });
  const history = createMemoryHistory({
    initialEntries: ["/settings", `/settings/customization?custTab=${options.tab ?? "general"}`],
  });
  const runtime = await createAppRuntime({ transport, history, i18n: await createTestI18n() });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return { runtime, transport, requests, update, current: () => current.global.ui_preferences };
}

describe("customization settings", () => {
  it("shows shaped skeletons until settings are loaded", async () => {
    let release!: (value: ReturnType<typeof ok<SettingsView>>) => void;
    await mount({
      calls: {
        settingsGet: () =>
          new Promise((resolve) => {
            release = resolve;
          }),
      },
    });
    expect(screen.getByRole("status", { name: "Loading customization settings" })).toBeTruthy();
    expect(screen.queryByRole("switch", { name: "Message sent" })).toBeNull();
    await act(async () => {
      release(ok({ global: { ui_preferences: {} }, revision: 1 } as SettingsView));
    });
    expect(await screen.findByRole("switch", { name: "Message sent" })).toBeTruthy();
  });

  it("keeps sound defaults off and preserves neighboring sound and haptic settings", async () => {
    const page = await mount({
      value: {
        accessibility: {
          success: { enabled: true, volume: 0.3 },
          haptics: true,
          hapticIntensity: "heavy",
        },
      },
    });
    const send = await screen.findByRole("switch", { name: "Message sent" });
    expect(send.getAttribute("aria-checked")).toBe("false");
    fireEvent.click(send);
    await waitFor(() =>
      expect(page.current().accessibility).toEqual({
        send: { enabled: true },
        success: { enabled: true, volume: 0.3 },
        haptics: true,
        hapticIntensity: "heavy",
      }),
    );
    expect(page.requests[0]?.expected_revision).toBe(7);
  });

  it("rolls back a failed switch and retries the same edit", async () => {
    let fail = true;
    const page = await mount({ fail: () => fail });
    const toggle = await screen.findByRole("switch", { name: "Beetroot rain" });
    fireEvent.click(toggle);
    expect(await screen.findByText(/available right now/)).toBeTruthy();
    expect(toggle.getAttribute("aria-checked")).toBe("true");
    expect(screen.queryByText("private diagnostic")).toBeNull();
    fail = false;
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(page.current().beetrootRain).toBe(false));
  });

  it("serializes rapid edits with fresh revisions and preserves both changes", async () => {
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const page = await mount({ gate });
    fireEvent.click(await screen.findByRole("switch", { name: "Message sent" }));
    fireEvent.click(screen.getByRole("switch", { name: "Reply finished" }));
    await act(async () => release());
    await waitFor(() => expect(page.update).toHaveBeenCalledTimes(2));
    expect(page.requests.map((request) => request.expected_revision)).toEqual([7, 8]);
    expect(page.current().accessibility).toEqual({
      send: { enabled: true },
      success: { enabled: true },
    });
  });

  it("retries the failed edit after a different queued edit succeeds", async () => {
    let attempts = 0;
    const page = await mount({ fail: () => ++attempts === 1 });
    fireEvent.click(await screen.findByRole("switch", { name: "Message sent" }));
    fireEvent.click(screen.getByRole("switch", { name: "Reply finished" }));
    await waitFor(() =>
      expect(page.current().accessibility).toEqual({ success: { enabled: true } }),
    );
    fireEvent.click(await screen.findByRole("button", { name: "Try again" }));
    await waitFor(() =>
      expect(page.current().accessibility).toEqual({
        send: { enabled: true },
        success: { enabled: true },
      }),
    );
    expect(page.requests.map((request) => request.expected_revision)).toEqual([7, 7, 8]);
  });

  it("discards queued edits and ignores an old response after a database replacement", async () => {
    let release!: (view: ReturnType<typeof ok<SettingsView>>) => void;
    const stale = new Promise<ReturnType<typeof ok<SettingsView>>>((resolve) => {
      release = resolve;
    });
    const write = vi.fn(() => stale);
    let loaded = { global: { ui_preferences: {} }, revision: 7 } as SettingsView;
    const page = await mount({ calls: { settingsUpdate: write, settingsGet: () => ok(loaded) } });
    fireEvent.click(await screen.findByRole("switch", { name: "Message sent" }));
    fireEvent.click(screen.getByRole("switch", { name: "Reply finished" }));
    await waitFor(() => expect(write).toHaveBeenCalledOnce());
    loaded = {
      global: { ui_preferences: { beetrootRain: false } },
      revision: 22,
    } as unknown as SettingsView;
    await act(async () => page.transport.emitAppEvent({ type: "database_reloaded" }));
    await waitFor(() =>
      expect(
        screen.getByRole("switch", { name: "Beetroot rain" }).getAttribute("aria-checked"),
      ).toBe("false"),
    );
    await act(async () =>
      release(
        ok({
          global: { ui_preferences: { accessibility: { send: { enabled: true } } } },
          revision: 8,
        } as unknown as SettingsView),
      ),
    );
    expect(screen.getByRole("switch", { name: "Message sent" }).getAttribute("aria-checked")).toBe(
      "false",
    );
    expect(write).toHaveBeenCalledOnce();
  });

  it("only exposes haptics on native mobile platforms", async () => {
    await mount({ mobile: false });
    await screen.findByRole("switch", { name: "Message sent" });
    expect(screen.queryByRole("switch", { name: "Vibrate while replying" })).toBeNull();
  });

  it("keeps the final slider value without sending every queued drag position", async () => {
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const page = await mount({ gate });
    fireEvent.click(await screen.findByRole("switch", { name: "Message sent" }));
    const volume = await screen.findByRole("slider", { name: "Message sent volume" });
    fireEvent.change(volume, { target: { value: "25" } });
    fireEvent.change(volume, { target: { value: "50" } });
    fireEvent.change(volume, { target: { value: "70" } });
    await act(async () => release());
    await waitFor(() =>
      expect(page.current().accessibility).toEqual({ send: { enabled: true, volume: 0.7 } }),
    );
    expect(page.update).toHaveBeenCalledTimes(2);
  });

  it("applies and saves the additional themes to the backend", async () => {
    const page = await mount({ tab: "theme", mobile: false });
    fireEvent.click(await screen.findByRole("button", { name: /Forest/ }));
    await waitFor(() => expect(page.current().themePreset).toBe("forest"));
    expect(document.documentElement.style.getPropertyValue("--color-surface")).toBe("#0a1510");
    fireEvent.click(screen.getByRole("button", { name: "Save current" }));
    await waitFor(() =>
      expect(page.current().customColorPresets).toMatchObject([
        { name: "My theme 1", colors: { surface: "#0a1510" } },
      ]),
    );
  });

  it("persists the new composer text size without dropping hidden appearance fields", async () => {
    const slots = { left: [], right: [] };
    const page = await mount({
      tab: "chat",
      value: { chatAppearance: { chatWidgetSlots: slots } },
    });
    fireEvent.click(await screen.findByRole("combobox", { name: "Composer text size" }));
    fireEvent.click(await screen.findByRole("option", { name: "Extra large" }));
    await waitFor(() =>
      expect(page.current().chatAppearance).toMatchObject({
        composerFontSize: "xlarge",
        chatWidgetSlots: slots,
      }),
    );
  });

  it("persists and resets the composer background without dropping its font size", async () => {
    const page = await mount({
      tab: "chat",
      value: { chatAppearance: { composerFontSize: "large" } },
    });
    const label = (await screen.findAllByText("Composer")).find((element) =>
      element.closest(".row-py"),
    );
    fireEvent.click(within(label!.closest(".row-py") as HTMLElement).getByRole("button"));
    const hex = await screen.findByRole("textbox", { name: "Hex color" });
    fireEvent.change(hex, { target: { value: "#123456" } });
    fireEvent.keyDown(hex, { key: "Enter" });
    await waitFor(() =>
      expect(page.current().chatAppearance).toMatchObject({
        footerInputColorHex: "#123456",
        composerFontSize: "large",
      }),
    );
    fireEvent.keyDown(document, { key: "Escape" });
    fireEvent.click(screen.getByRole("button", { name: "Reset Composer" }));
    await waitFor(() =>
      expect(page.current().chatAppearance).toEqual({ composerFontSize: "large" }),
    );
  });

  it("imports a native-validated theme and its shape in one settings update", async () => {
    const read = vi.fn(() =>
      ok({
        format_version: 1,
        name: "Imported",
        colors: { surface: "#08141e", fg: "#e5edf3", accent: "#72bed5" },
        radius: "round" as const,
        density: "compact" as const,
      }),
    );
    const page = await mount({
      tab: "theme",
      calls: {
        filesPickOpen: () => ok({ sources: [{ uri: "/theme.json" }] }),
        settingsThemeRead: read,
      },
    });
    fireEvent.click(await screen.findByRole("button", { name: "Import" }));
    await waitFor(() =>
      expect(page.current().customColorPresets).toMatchObject([{ name: "Imported" }]),
    );
    expect(read).toHaveBeenCalledWith({ uri: "/theme.json" });
    expect(page.update).toHaveBeenCalledOnce();
    expect(page.current()).toMatchObject({
      themeRadius: "round",
      themeDensity: "compact",
      customColors: { surface: "#08141e" },
    });
  });

  it("keeps settings unchanged when native theme validation rejects an import", async () => {
    const page = await mount({
      tab: "theme",
      calls: {
        filesPickOpen: () => ok({ sources: [{ uri: "/broken.json" }] }),
        settingsThemeRead: () => ({
          status: "error",
          error: { code: "malformed", message: "private parser error", details: null },
        }),
      },
    });
    fireEvent.click(await screen.findByRole("button", { name: "Import" }));
    expect(await screen.findByText("Couldn't read this theme")).toBeTruthy();
    expect(page.update).not.toHaveBeenCalled();
    expect(screen.queryByText("private parser error")).toBeNull();
  });

  it.each([false, true])(
    "exports themes through native publication on Android=%s",
    async (mobile) => {
      const pick = vi.fn(() => ok({ target: { uri: "/saved.json" } }));
      const publish = vi.fn(() =>
        ok({
          uri: mobile ? "content://themes/1" : "/saved.json",
          suggested_name: "theme.json",
          warnings: [],
        }),
      );
      await mount({
        tab: "theme",
        mobile,
        value: {
          customColorPresets: [
            {
              id: "saved",
              name: "Saved",
              colors: { surface: "#08141e", fg: "#e5edf3", accent: "#72bed5" },
              createdAt: 1,
            },
          ],
        },
        calls: { filesPickSave: pick, settingsThemeExport: publish },
      });
      fireEvent.click(await screen.findByRole("button", { name: "Saved actions" }));
      fireEvent.click(await screen.findByRole("menuitem", { name: "Export" }));
      await waitFor(() => expect(publish).toHaveBeenCalledOnce());
      expect(publish).toHaveBeenCalledWith(
        expect.objectContaining({
          document: expect.objectContaining({ name: "Saved", format_version: 1 }),
          target: mobile ? null : { uri: "/saved.json" },
        }),
      );
      expect(pick).toHaveBeenCalledTimes(mobile ? 0 : 1);
    },
  );

  it("closes the mobile preview with Back while retaining the Chat tab", async () => {
    const { runtime } = await mount({ tab: "chat" });
    fireEvent.click(await screen.findByRole("button", { name: "Preview" }));
    expect(await screen.findByRole("dialog")).toBeTruthy();
    await act(async () => runtime.router.history.back());
    await waitFor(() => expect(runtime.router.state.location.search.custPanel).toBeUndefined());
    expect(runtime.router.state.location.search.custTab).toBe("chat");
  });

  it("previews a disabled sound using its stored volume", async () => {
    const play = vi.spyOn(HTMLMediaElement.prototype, "play").mockResolvedValue(undefined);
    await mount({ value: { accessibility: { send: { enabled: false, volume: 0.23 } } } });
    const row = (await screen.findByRole("switch", { name: "Message sent" })).closest(".row-py")!;
    fireEvent.click(within(row as HTMLElement).getByRole("button", { name: "Play" }));
    await waitFor(() => expect(play).toHaveBeenCalledOnce());
    expect((play.mock.instances[0] as HTMLMediaElement | undefined)?.volume).toBe(0.23);
  });
});
