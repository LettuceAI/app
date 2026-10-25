import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { toast } from "@lettuceai/crisp";
import type {
  JobView,
  ModelView,
  PromptSummary,
  ProviderAccountView,
  SettingsCompanionSoulWriterSettings,
  SettingsUpdateRequest_Deserialize,
  SettingsView,
  ThymosStatus,
} from "@/api/generated/bindings";
import {
  createMockTransport,
  mockAppStatus,
  type MockCallHandlers,
  type MockStreamHandlers,
} from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({
  createWindowController: vi.fn().mockResolvedValue(null),
}));
afterEach(() => {
  toast.dismiss();
  cleanup();
  vi.restoreAllMocks();
});
const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const error = () => ({
  status: "error" as const,
  error: { code: "unavailable" as const, message: "private diagnostic", details: null },
});
const defaults: SettingsCompanionSoulWriterSettings = {
  model_profile_id: null,
  fallback_model_profile_id: null,
  prompt_id: null,
  structured_fallback_format: "json",
};
const account = (id: string, provider_kind = "openai", enabled = true): ProviderAccountView => ({
  id,
  label: id,
  provider_kind,
  enabled,
  protocol: "open_ai_compatible",
  base_url: null,
  streaming_enabled: true,
  allow_invalid_tls: false,
  api_key_set: true,
  config: {},
  revision: 1,
  created_at: 1,
  updated_at: 1,
});
const accounts = [
  account("Cloud"),
  account("Local", "llamacpp"),
  account("Disabled", "openrouter", false),
];
const model = (
  id: string,
  provider = "Cloud",
  output: ModelView["output_scopes"] = ["text"],
): ModelView => ({
  id,
  display_name: id,
  external_model_id: id,
  provider_account_id: provider,
  kind: "chat",
  input_scopes: ["text"],
  output_scopes: output,
  config: {},
  revision: 1,
  created_at: 1,
  updated_at: 1,
});
const models = [
  model("Default model"),
  model("Local writer", "Local"),
  model("Disabled model", "Disabled"),
  model("Image model", "Cloud", ["image"]),
];
const prompt = (id: string, builtin = false): PromptSummary => ({
  id,
  name: id,
  kind: "companion_soul_writer",
  archived: false,
  app_default: false,
  origin: builtin
    ? { type: "built_in", key: id, protected: true, required: true, edited: false }
    : { type: "user" },
  revision: 1,
  created_at: 1,
  updated_at: 1,
});
const prompts = [prompt("Built-in writer", true), prompt("My writer")];
const running = (patch: Partial<JobView> = {}): JobView => ({
  id: "thymos-job",
  kind: "artifact_install",
  subject: { kind: "artifact_install", id: "thymos:revision", display_name: "Lettuce Thymos" },
  subject_detail: null,
  state: "running",
  progress: {
    current: 2_000_000,
    total: 10_000_000,
    unit: "bytes",
    label_code: "download",
    bytes_per_second: 1_000_000,
  },
  created_at: 1,
  updated_at: 1,
  failure: null,
  result: null,
  ...patch,
});

async function mount(
  options: {
    tab?: string;
    platform?: "android" | "other";
    path?: string;
    value?: Partial<SettingsCompanionSoulWriterSettings>;
    status?: ThymosStatus;
    jobs?: JobView[];
    calls?: MockCallHandlers;
    streams?: MockStreamHandlers;
    gate?: Promise<void>;
    fail?: () => boolean;
  } = {},
) {
  let current = {
    global: {
      ui_preferences: {},
      companion_soul_writer: { ...defaults, ...options.value },
      creation_helper: { model_profile_id: "preserved" },
    },
    device: {
      revision: 12,
      thymos_keep_model_loaded: false,
      embedding_keep_model_loaded: true,
      kokoro_keep_model_loaded: true,
    },
    default_model_profile_id: "Default model",
    revision: 7,
  } as unknown as SettingsView;
  let status = options.status ?? ({ type: "not_installed" } as ThymosStatus);
  let jobs = options.jobs ?? [];
  const requests: SettingsUpdateRequest_Deserialize[] = [];
  const write = vi.fn(async (request: SettingsUpdateRequest_Deserialize) => {
    requests.push(request);
    if (options.gate) await options.gate;
    if (options.fail?.()) return error();
    const device = request.patch.section === "device_onnx";
    expect(request.expected_revision).toBe(device ? current.device.revision : current.revision);
    if (request.patch.section === "companion_soul_writer")
      current = {
        ...current,
        global: { ...current.global, companion_soul_writer: request.patch.value },
        revision: current.revision + 1,
      };
    if (request.patch.section === "device_onnx")
      current = {
        ...current,
        device: {
          ...current.device,
          thymos_keep_model_loaded: request.patch.keep_model_loaded,
          revision: current.device.revision + 1,
        },
      };
    return ok(current);
  });
  const install = vi.fn(() => ok({ job_id: "new-job" }));
  const remove = vi.fn(() => {
    status = { type: "not_installed" };
    return ok(true);
  });
  const cancel = vi.fn(() => ok(null));
  const transport = createMockTransport({
    calls: {
      appStatus: () =>
        ok({
          ...mockAppStatus,
          platform: options.platform ?? "android",
          ui_state: { onboarding: { completed: true } },
        }),
      settingsGet: () => ok(current),
      settingsUpdate: write,
      modelsList: () => ok({ models, default_model_id: "Default model", revision: 1 }),
      providerAccountsList: () => ok(accounts),
      promptsList: ({ kind }) =>
        ok({ items: kind === "companion_soul_writer" ? prompts : [], next_cursor: null }),
      companionEmotionStatus: () => ok(status),
      companionEmotionInstall: install,
      companionEmotionRemove: remove,
      jobsList: () => ok({ items: jobs, next_cursor: null }),
      jobCancel: cancel,
      ...options.calls,
    },
    streams: { jobWatch: ([{ job_id }]) => ok(running({ id: job_id })), ...options.streams },
  });
  const history = createMemoryHistory({
    initialEntries: [
      "/settings",
      options.path ?? `/settings/companions?compTab=${options.tab ?? "writer"}`,
    ],
  });
  const runtime = await createAppRuntime({ transport, history, i18n: await createTestI18n() });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return {
    runtime,
    history,
    transport,
    requests,
    write,
    install,
    remove,
    cancel,
    current: () => current,
    setStatus: (next: ThymosStatus) => {
      status = next;
    },
    setJobs: (next: JobView[]) => {
      jobs = next;
    },
  };
}
const open = async (name: string) => {
  fireEvent.click(await screen.findByRole("button", { name: new RegExp(`^${name}`) }));
  return within(await screen.findByRole("dialog"));
};

describe("Companions settings", () => {
  it.each(["android", "other"] as const)(
    "opens Companions from settings on %s",
    async (platform) => {
      await mount({ platform, path: platform === "android" ? "/settings" : "/settings/about" });
      fireEvent.click(await screen.findByRole("button", { name: /^Companions/ }));
      expect(await screen.findByRole("tab", { name: "Soul writer" })).toBeTruthy();
      expect((await screen.findByRole("button", { name: /^Writer model/ })).textContent).toContain(
        "Default model",
      );
    },
  );

  it("loads settings with row skeletons rather than invented defaults", async () => {
    let finish!: (value: ReturnType<typeof ok<SettingsView>>) => void;
    const view = await mount({
      calls: {
        settingsGet: () =>
          new Promise((resolve) => {
            finish = resolve;
          }),
      },
    });
    expect(screen.getByRole("status", { name: "Loading companion settings" })).toBeTruthy();
    expect(screen.queryByText("Backup model")).toBeNull();
    await act(async () => finish(ok(view.current())));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /^Backup model/ }).textContent).toContain("None"),
    );
  });

  it("serializes optimistic format and model edits without losing neighboring settings", async () => {
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const view = await mount({ gate });
    fireEvent.click(await screen.findByRole("button", { name: /^XML/ }));
    expect(screen.getByRole("button", { name: /^XML/ }).getAttribute("aria-pressed")).toBe("true");
    const picker = await open("Backup model");
    fireEvent.click(picker.getByRole("button", { name: /^Local writer/ }));
    await act(async () => release());
    await waitFor(() =>
      expect(view.current().global.companion_soul_writer).toEqual({
        ...defaults,
        structured_fallback_format: "xml",
        fallback_model_profile_id: "Local writer",
      }),
    );
    expect(view.requests.map((request) => request.expected_revision)).toEqual([7, 8]);
    expect(view.current().global.creation_helper.model_profile_id).toBe("preserved");
  });

  it("rolls back a failed choice and retries only that edit", async () => {
    let failed = true;
    const view = await mount({ fail: () => failed });
    fireEvent.click(await screen.findByRole("button", { name: /^XML/ }));
    expect(await screen.findByText("It isn't available right now.")).toBeTruthy();
    expect(screen.getByRole("button", { name: /^JSON/ }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.queryByText("private diagnostic")).toBeNull();
    failed = false;
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() =>
      expect(view.current().global.companion_soul_writer.structured_fallback_format).toBe("xml"),
    );
  });

  it("filters non-text models, disables unavailable providers and keeps picker Back on its tab", async () => {
    const view = await mount();
    const picker = await open("Writer model");
    expect(picker.queryByRole("button", { name: /^Image model/ })).toBeNull();
    expect(
      (picker.getByRole("button", { name: /^Disabled model/ }) as HTMLButtonElement).disabled,
    ).toBe(true);
    await act(async () => view.history.back());
    await waitFor(() =>
      expect(view.runtime.router.state.location.search.compPicker).toBeUndefined(),
    );
    expect(view.runtime.router.state.location.search.compTab).toBe("writer");
    expect(view.requests).toHaveLength(0);
  });

  it("discards queued edits and late settings replies after a database replacement", async () => {
    let source = {
      global: { ui_preferences: {}, companion_soul_writer: defaults },
      device: { revision: 12 },
      default_model_profile_id: "Default model",
      revision: 7,
    } as unknown as SettingsView;
    let finish!: (value: ReturnType<typeof ok<SettingsView>>) => void;
    const old = new Promise<ReturnType<typeof ok<SettingsView>>>((resolve) => {
      finish = resolve;
    });
    const write = vi.fn(() => old);
    const view = await mount({ calls: { settingsGet: () => ok(source), settingsUpdate: write } });
    fireEvent.click(await screen.findByRole("button", { name: /^XML/ }));
    const picker = await open("Backup model");
    fireEvent.click(picker.getByRole("button", { name: /^Local writer/ }));
    await waitFor(() => expect(write).toHaveBeenCalledOnce());
    const stale = {
      ...source,
      revision: 8,
      global: {
        ...source.global,
        companion_soul_writer: { ...defaults, structured_fallback_format: "xml" as const },
      },
    };
    source = { ...source, revision: 22 };
    await act(async () => view.transport.emitAppEvent({ type: "database_reloaded" }));
    await waitFor(() =>
      expect(
        view.runtime.queryClient.getQueryData<SettingsView>(["settings", "detail"])?.revision,
      ).toBe(22),
    );
    await act(async () => finish(ok(stale)));
    expect(write).toHaveBeenCalledOnce();
    expect(screen.getByRole("button", { name: /^JSON/ }).getAttribute("aria-pressed")).toBe("true");
    expect(
      view.runtime.queryClient.getQueryData<SettingsView>(["settings", "detail"])?.revision,
    ).toBe(22);
  });

  it("only picks Soul writer prompts and keeps the native built-in fallback visible", async () => {
    const view = await mount({ value: { prompt_id: "removed" } });
    expect(await screen.findByText("Selected prompt unavailable · Built in")).toBeTruthy();
    const picker = await open("Soul writer prompt");
    fireEvent.click(picker.getByRole("button", { name: "My writer" }));
    await waitFor(() =>
      expect(view.current().global.companion_soul_writer.prompt_id).toBe("My writer"),
    );
    const again = await open("Soul writer prompt");
    fireEvent.click(again.getByRole("button", { name: /^System prompts/ }));
    await waitFor(() =>
      expect(view.runtime.router.state.location.pathname).toBe("/settings/prompts"),
    );
    expect(view.runtime.router.state.location.search.compPicker).toBeUndefined();
  });

  it("uses the device revision for Thymos retention and preserves other model flags", async () => {
    const view = await mount({
      tab: "emotions",
      status: { type: "installed", source_revision: "revision" },
    });
    fireEvent.click(await screen.findByRole("switch", { name: "Keep Thymos in memory" }));
    await waitFor(() => expect(view.current().device.thymos_keep_model_loaded).toBe(true));
    expect(view.requests[0]).toEqual({
      expected_revision: 12,
      patch: { section: "device_onnx", model: "thymos", keep_model_loaded: true },
    });
    expect(view.current().device.embedding_keep_model_loaded).toBe(true);
    expect(view.current().device.kokoro_keep_model_loaded).toBe(true);
  });

  it("restores a running Thymos download in both page and shell and shares cancellation", async () => {
    const view = await mount({ tab: "emotions", platform: "other", jobs: [running()] });
    await screen.findByText("20%");
    expect(screen.queryByRole("button", { name: "Download" })).toBeNull();
    expect(view.install).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: /Downloads/ }));
    await waitFor(() => expect(screen.getAllByText("20%")).toHaveLength(2));
    expect(screen.getAllByText(/B\/s/)).toHaveLength(2);
    expect(screen.getAllByText(/left/)).toHaveLength(2);
    const cancel = screen.getAllByRole("button", { name: /Cancel/ });
    act(() => {
      cancel[0]!.click();
      cancel[1]!.click();
    });
    await waitFor(() => expect(view.cancel).toHaveBeenCalledTimes(1));
  });

  it("locks installation during admission and updates status when the shared job finishes", async () => {
    let finish!: (value: ReturnType<typeof ok<{ job_id: string }>>) => void;
    const install = vi.fn(
      () =>
        new Promise<ReturnType<typeof ok<{ job_id: string }>>>((resolve) => {
          finish = resolve;
        }),
    );
    const view = await mount({ tab: "emotions", calls: { companionEmotionInstall: install } });
    const button = await screen.findByRole("button", { name: "Download" });
    await waitFor(() => expect((button as HTMLButtonElement).disabled).toBe(false));
    act(() => {
      button.click();
      button.click();
    });
    await waitFor(() => expect(install).toHaveBeenCalledOnce());
    expect(screen.queryByRole("button", { name: "Download" })).toBeNull();
    await act(async () => finish(ok({ job_id: "new-job" })));
    view.setStatus({ type: "installed", source_revision: "revision" });
    await act(async () =>
      view.transport.emitAppEvent({
        type: "job_updated",
        job: running({ id: "new-job", state: "succeeded", updated_at: 2 }),
      }),
    );
    expect(await screen.findByRole("switch", { name: "Keep Thymos in memory" })).toBeTruthy();
  });

  it("keeps a restored download's failure visible after it leaves the active list", async () => {
    const view = await mount({ tab: "emotions", jobs: [running()] });
    await screen.findByText("20%");
    await act(async () =>
      view.transport.emitAppEvent({
        type: "job_updated",
        job: running({
          state: "failed",
          updated_at: 2,
          failure: {
            code: "resource_unavailable",
            retryable: true,
            entity_import: null,
            reason: null,
            model: null,
            hugging_face: null,
            ollama: null,
            image: null,
            speech: null,
          },
        }),
      }),
    );
    const retry = await screen.findByRole("button", { name: "Try again" });
    expect(screen.getByText("It isn't available right now.")).toBeTruthy();
    fireEvent.click(retry);
    await waitFor(() => expect(view.install).toHaveBeenCalledOnce());
  });

  it("repairs damaged files through the shared install flow", async () => {
    const view = await mount({ tab: "emotions", status: { type: "damaged" } });
    expect(await screen.findByText("Thymos can't run")).toBeTruthy();
    const button = screen.getByRole("button", { name: "Repair" });
    await waitFor(() => expect((button as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(button);
    await waitFor(() => expect(view.install).toHaveBeenCalledOnce());
    expect(await screen.findByText("20%")).toBeTruthy();
  });

  it("confirms native deletion and keeps retention as a separate preference", async () => {
    const view = await mount({
      tab: "emotions",
      status: { type: "installed", source_revision: "revision" },
    });
    fireEvent.click(await screen.findByRole("switch", { name: "Keep Thymos in memory" }));
    await waitFor(() => expect(view.current().device.thymos_keep_model_loaded).toBe(true));
    fireEvent.click(screen.getByRole("button", { name: /^Delete Thymos/ }));
    const dialog = within(await screen.findByRole("dialog"));
    fireEvent.click(dialog.getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(view.remove).toHaveBeenCalledOnce());
    expect(await screen.findByText("Not downloaded")).toBeTruthy();
    expect(view.current().device.thymos_keep_model_loaded).toBe(true);
  });
});
