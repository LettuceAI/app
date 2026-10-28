import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { toast } from "@lettuceai/crisp";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  CertificatesView,
  SettingsPureMode,
  SettingsUpdateRequest_Deserialize,
  SettingsView,
  TrustedCertificateView,
} from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

afterEach(() => {
  toast.dismiss();
  cleanup();
});

const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const failed = (code: "internal" | "conflict" | "invalid_input", details: unknown = null) => ({
  status: "error" as const,
  error: { code, message: "diagnostic", details } as never,
});

function viewOf(
  pure: SettingsPureMode = "off",
  avatars = true,
  certificates: TrustedCertificateView[] = [],
  deviceRevision = 6,
): SettingsView {
  return {
    device: { revision: deviceRevision, trusted_certificates: certificates } as SettingsView["device"],
    global: { pure_mode: pure, auto_download_character_card_avatars: avatars } as SettingsView["global"],
    sampler_defaults: {} as SettingsView["sampler_defaults"],
    default_model_profile_id: null,
    default_prompt_document_id: null,
    dynamic_memory_model_profile_id: null,
    group_speaker_model_profile_id: null,
    revision: 4,
  };
}

const certificate = (patch: Partial<TrustedCertificateView> = {}): TrustedCertificateView => ({
  id: "c1",
  name: "root.pem",
  valid: true,
  reason: null,
  imported_at: Date.now() - 3 * 3_600_000,
  ...patch,
});
const certificatesOf = (certificates: TrustedCertificateView[], revision = 7): CertificatesView => ({
  certificates,
  revision,
});

function backend(initial: SettingsView, fail: () => ReturnType<typeof failed> | null = () => null) {
  let view = initial;
  const requests: SettingsUpdateRequest_Deserialize[] = [];
  const update = vi.fn((request: SettingsUpdateRequest_Deserialize) => {
    requests.push(request);
    const error = fail();
    if (error) return error;
    const patch = request.patch;
    if (patch.section === "general") {
      const global = { ...view.global };
      if (patch.pure_mode !== null) global.pure_mode = patch.pure_mode;
      if (patch.auto_download_character_card_avatars !== null)
        global.auto_download_character_card_avatars = patch.auto_download_character_card_avatars;
      view = { ...view, global, revision: view.revision + 1 };
    }
    return ok(view);
  });
  const get = vi.fn(() => ok(view));
  return {
    update,
    requests,
    get,
    replace: (next: SettingsView) => {
      view = next;
    },
  };
}

async function mount(calls: MockCallHandlers = {}) {
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: () => ok({ ...mockAppStatus, platform: "android", ui_state: { onboarding: { completed: true } } }),
        settingsGet: () => ok(viewOf()),
        ...calls,
      },
    }),
    history: createMemoryHistory({ initialEntries: ["/settings/security"] }),
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return runtime;
}

const level = (name: string) => screen.findByRole("radio", { name });

describe("security settings page", () => {
  it("shows no level while settings load", async () => {
    await mount({ settingsGet: () => new Promise(() => undefined) });
    await screen.findByText("Trusted certificates");
    expect(screen.queryByRole("radio")).toBeNull();
    expect(screen.queryByText("All content allowed")).toBeNull();
  });

  it("offers retry when settings fail to load", async () => {
    let broken = true;
    await mount({ settingsGet: () => (broken ? failed("internal") : ok(viewOf("low", true, [certificate()]))) });
    await screen.findByRole("button", { name: "Try again" });
    broken = false;
    fireEvent.click(await screen.findByRole("button", { name: "Try again" }));
    expect(await screen.findByText("root.pem")).toBeTruthy();
    expect((await level("Low")).getAttribute("aria-checked")).toBe("true");
  });

  it("sends only pure_mode for a level change and shows its description", async () => {
    const store = backend(viewOf("off"));
    await mount({ settingsGet: store.get, settingsUpdate: store.update });
    fireEvent.click(await level("Strict"));
    await waitFor(() => expect(store.requests).toHaveLength(1));
    expect(store.requests[0]).toEqual({
      expected_revision: 4,
      patch: {
        section: "general",
        pure_mode: "strict",
        analytics_enabled: null,
        update_checks_enabled: null,
        developer_mode_enabled: null,
        auto_download_character_card_avatars: null,
        manual_mode_context_window: null,
        lorebook_scan_depth: null,
      },
    });
    expect(await screen.findByText("Maximum filtering and no suggestive tone")).toBeTruthy();
  });

  it("selects a level at once and rolls it back when the save fails", async () => {
    let release: (() => void) | undefined;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const update = vi.fn(async () => {
      await gate;
      return failed("internal");
    });
    await mount({ settingsGet: () => ok(viewOf("off")), settingsUpdate: update });
    expect((await level("Off")).getAttribute("aria-checked")).toBe("true");
    fireEvent.click(await level("Low"));
    expect((await level("Low")).getAttribute("aria-checked")).toBe("true");
    expect((await level("Off")).getAttribute("aria-checked")).toBe("false");
    await act(async () => release?.());
    await waitFor(() => expect(screen.getByRole("radio", { name: "Off" }).getAttribute("aria-checked")).toBe("true"));
    expect(screen.getByRole("radio", { name: "Low" }).getAttribute("aria-checked")).toBe("false");
    expect(await screen.findByText("The app ran into an internal error.")).toBeTruthy();
  });

  it("keeps the last clicked level through rapid changes", async () => {
    const store = backend(viewOf("off"));
    const gates: Array<() => void> = [];
    const update = vi.fn(async (request: SettingsUpdateRequest_Deserialize) => {
      await new Promise<void>((resolve) => gates.push(resolve));
      return store.update(request);
    });
    await mount({ settingsGet: store.get, settingsUpdate: update });
    for (const name of ["Low", "Strict", "Low"]) fireEvent.click(await level(name));
    const checked = (name: string) => screen.getByRole("radio", { name }).getAttribute("aria-checked");
    expect(checked("Low")).toBe("true");
    for (let step = 0; step < 3; step += 1) {
      await waitFor(() => expect(gates.length).toBeGreaterThan(step));
      await act(async () => gates[step]?.());
      expect(checked("Strict")).toBe("false");
      expect(checked("Low")).toBe("true");
    }
    await waitFor(() => expect(store.requests.map((request) => request.patch)).toHaveLength(3));
    expect(checked("Low")).toBe("true");
  });

  it("sends only the avatar field for the switch", async () => {
    const store = backend(viewOf("low", true));
    await mount({ settingsGet: store.get, settingsUpdate: store.update });
    fireEvent.click(await screen.findByRole("switch", { name: "Download avatars from imported cards" }));
    await waitFor(() => expect(store.requests).toHaveLength(1));
    expect(store.requests[0]?.patch).toEqual({
      section: "general",
      pure_mode: null,
      analytics_enabled: null,
      update_checks_enabled: null,
      developer_mode_enabled: null,
      auto_download_character_card_avatars: false,
      manual_mode_context_window: null,
      lorebook_scan_depth: null,
    });
  });

  it("puts the avatar switch back when the save fails", async () => {
    const store = backend(viewOf("low", true), () => failed("internal"));
    await mount({ settingsGet: store.get, settingsUpdate: store.update });
    const toggle = await screen.findByRole("switch", { name: "Download avatars from imported cards" });
    fireEvent.click(toggle);
    expect(await screen.findByText("The app ran into an internal error.")).toBeTruthy();
    await waitFor(() =>
      expect(screen.getByRole("switch", { name: "Download avatars from imported cards" }).getAttribute("aria-checked")).toBe("true"),
    );
  });

  it("imports nothing when the picker is cancelled", async () => {
    const pick = vi.fn(() => ok({ sources: [] }));
    const importer = vi.fn(() => ok(certificatesOf([])));
    await mount({ filesPickOpen: pick, certificatesImport: importer });
    fireEvent.click(await screen.findByRole("button", { name: /Import certificate/ }));
    await waitFor(() => expect(pick).toHaveBeenCalledWith({ kinds: ["certificate"], multiple: false }));
    await waitFor(() => expect(screen.getByRole("button", { name: /Import certificate/ }).getAttribute("aria-busy")).toBe("false"));
    expect(importer).not.toHaveBeenCalled();
  });

  it("adds an imported certificate to the list", async () => {
    let stored: TrustedCertificateView[] = [];
    const importer = vi.fn(() => {
      stored = [certificate({ name: "fresh.pem" })];
      return ok(certificatesOf(stored, 7));
    });
    await mount({
      settingsGet: () => ok(viewOf("off", true, stored)),
      filesPickOpen: () => ok({ sources: [{ uri: "/certs/fresh.pem" }] }),
      certificatesImport: importer,
    });
    fireEvent.click(await screen.findByRole("button", { name: /Import certificate/ }));
    expect(await screen.findByText("fresh.pem")).toBeTruthy();
    expect(importer).toHaveBeenCalledWith({ source: { uri: "/certs/fresh.pem" }, client_operation_id: expect.any(String) });
    expect(screen.getByText("Imported 3 h ago")).toBeTruthy();
  });

  it("says a certificate is already imported", async () => {
    await mount({
      filesPickOpen: () => ok({ sources: [{ uri: "/certs/root.pem" }] }),
      certificatesImport: () => failed("conflict", { type: "certificate_already_imported", certificate_id: "c1" }),
    });
    fireEvent.click(await screen.findByRole("button", { name: /Import certificate/ }));
    expect(await screen.findByText("That certificate is already imported.")).toBeTruthy();
  });

  it("says a file is not a PEM certificate", async () => {
    await mount({
      filesPickOpen: () => ok({ sources: [{ uri: "/certs/notes.txt" }] }),
      certificatesImport: () => failed("invalid_input", { type: "invalid_field", field: "source" }),
    });
    fireEvent.click(await screen.findByRole("button", { name: /Import certificate/ }));
    expect(await screen.findByText("The selected file is not a PEM certificate.")).toBeTruthy();
  });

  it("refetches settings after a failed import", async () => {
    const get = vi.fn(() => ok(viewOf()));
    await mount({
      settingsGet: get,
      filesPickOpen: () => ok({ sources: [{ uri: "/certs/root.pem" }] }),
      certificatesImport: () => failed("internal"),
    });
    await screen.findByRole("button", { name: /Import certificate/ });
    const before = get.mock.calls.length;
    fireEvent.click(screen.getByRole("button", { name: /Import certificate/ }));
    expect(await screen.findByText("The app ran into an internal error.")).toBeTruthy();
    await waitFor(() => expect(get.mock.calls.length).toBeGreaterThan(before));
  });

  async function openRemove() {
    fireEvent.contextMenu(await screen.findByRole("button", { name: /root\.pem/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Remove" }));
    return (await screen.findAllByRole("dialog")).at(-1) as HTMLElement;
  }

  it("asks before removing a certificate and sends the device revision", async () => {
    let stored = [certificate()];
    const remove = vi.fn(() => {
      stored = [];
      return ok(certificatesOf([], 7));
    });
    await mount({
      settingsGet: () => ok(viewOf("off", true, stored, 6)),
      certificatesRemove: remove,
    });
    const dialog = await openRemove();
    expect(within(dialog).getByText("Connections that rely on root.pem stop being trusted.")).toBeTruthy();
    expect(remove).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(remove).toHaveBeenCalledWith({ certificate_id: "c1", expected_revision: 6 }));
    await waitFor(() => expect(screen.queryByText("root.pem")).toBeNull());
  });

  it("sends the current revision when the device revision changed elsewhere", async () => {
    const remove = vi.fn(() => ok(certificatesOf([], 10)));
    const runtime = await mount({
      settingsGet: () => ok(viewOf("off", true, [certificate()], 6)),
      certificatesRemove: remove,
    });
    await screen.findByText("root.pem");
    act(() => {
      runtime.queryClient.setQueryData<SettingsView>(queryKeys.settings.detail(), (current) =>
        current ? { ...current, device: { ...current.device, revision: 9 } } : current,
      );
    });
    const dialog = await openRemove();
    fireEvent.click(within(dialog).getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(remove).toHaveBeenCalledWith({ certificate_id: "c1", expected_revision: 9 }));
  });

  it("keeps the certificate when removal is not confirmed", async () => {
    const remove = vi.fn(() => ok(certificatesOf([], 7)));
    await mount({
      settingsGet: () => ok(viewOf("off", true, [certificate()])),
      certificatesRemove: remove,
    });
    const dialog = await openRemove();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByText("Connections that rely on root.pem stop being trusted.")).toBeNull());
    expect(remove).not.toHaveBeenCalled();
    expect(screen.getByText("root.pem")).toBeTruthy();
  });

  it("refetches and reports a stale revision on removal", async () => {
    const get = vi.fn(() => ok(viewOf("off", true, [certificate()])));
    await mount({ settingsGet: get, certificatesRemove: () => failed("conflict") });
    const dialog = await openRemove();
    const before = get.mock.calls.length;
    fireEvent.click(within(dialog).getByRole("button", { name: "Remove" }));
    expect(await screen.findByText("It changed somewhere else. Reload and try again.")).toBeTruthy();
    await waitFor(() => expect(get.mock.calls.length).toBeGreaterThan(before));
  });

  it("refetches settings after any other removal failure", async () => {
    const get = vi.fn(() => ok(viewOf("off", true, [certificate()])));
    await mount({ settingsGet: get, certificatesRemove: () => failed("internal") });
    const dialog = await openRemove();
    const before = get.mock.calls.length;
    fireEvent.click(within(dialog).getByRole("button", { name: "Remove" }));
    expect(await screen.findByText("The app ran into an internal error.")).toBeTruthy();
    await waitFor(() => expect(get.mock.calls.length).toBeGreaterThan(before));
  });

  it("shows why an invalid certificate cannot be used", async () => {
    await mount({
      settingsGet: () => ok(viewOf("off", true, [certificate({ valid: false, reason: "invalid_pem", name: "broken.pem" })])),
    });
    expect(await screen.findByText("broken.pem")).toBeTruthy();
    expect(screen.getByText("Not a valid certificate")).toBeTruthy();
  });

  it("writes the imported time in lower case after the label", async () => {
    await mount({
      settingsGet: () => ok(viewOf("off", true, [certificate({ imported_at: Date.now() })])),
    });
    expect(await screen.findByText("Imported just now")).toBeTruthy();
  });
});
