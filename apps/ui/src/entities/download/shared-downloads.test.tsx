import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { I18nextProvider, useTranslation } from "react-i18next";
import { afterEach, expect, it, vi } from "vitest";
import { createApiClient } from "@/api/client";
import type { JobView } from "@/api/generated/bindings";
import { createMockTransport, type MockTransport } from "@/api/mock-transport";
import { createTestI18n } from "@/shared/testing/i18n";
import { DownloadCard } from "./DownloadCard";
import { DownloadsProvider } from "./DownloadsProvider";
import { useDownloads, useDownloadView } from "./downloads-context";

afterEach(() => { cleanup(); vi.restoreAllMocks(); });

const running: JobView = { id: "shared", kind: "artifact_install", subject: { kind: "artifact_install", id: "asset" }, subject_detail: { type: "whisper_model_install", model_id: "small", filename: "ggml-small.bin" }, state: "running", progress: { current: 100, total: 1000, unit: "bytes", label_code: "download", bytes_per_second: null }, created_at: 1, updated_at: 1, failure: null, result: null };
const ok = <T,>(data: T) => ({ status: "ok" as const, data });

function Consumers() {
  const { t } = useTranslation();
  const { ready, downloads, error, retry } = useDownloads();
  const view = useDownloadView(running);
  return <>
    <output data-testid="shared-state">{JSON.stringify({ ready, active: downloads.length, state: view?.state, speed: view?.progress.bytes_per_second, error: error !== null })}</output>
    <button onClick={retry}>{t("actions.retry")}</button>
    <DownloadCard job={running} />
    <DownloadCard job={running} />
  </>;
}

async function mount(transport: MockTransport) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  const api = createApiClient(transport);
  const i18n = await createTestI18n();
  await act(async () => { render(<QueryClientProvider client={client}><I18nextProvider i18n={i18n}><DownloadsProvider api={api}><Consumers /></DownloadsProvider></I18nextProvider></QueryClientProvider>); });
}

function state() { return JSON.parse(screen.getByTestId("shared-state").textContent!) as { ready: boolean; active: number; state: string; speed: number | null; error: boolean }; }

it("shares measured progress and deduplicates cancellation across mounted consumers", async () => {
  vi.spyOn(performance, "now").mockReturnValue(0);
  let finish!: (value: ReturnType<typeof ok<null>>) => void;
  const cancel = vi.fn(() => new Promise<ReturnType<typeof ok<null>>>((resolve) => { finish = resolve; }));
  const transport = createMockTransport({ calls: { jobsList: () => ok({ items: [running], next_cursor: null }), jobCancel: cancel } });
  await mount(transport);
  await waitFor(() => expect(state().ready).toBe(true));
  vi.mocked(performance.now).mockReturnValue(1000);
  await act(async () => { transport.emitAppEvent({ type: "job_updated", job: { ...running, progress: { ...running.progress, current: 200 } } }); });
  vi.mocked(performance.now).mockReturnValue(2000);
  await act(async () => { transport.emitAppEvent({ type: "job_updated", job: { ...running, progress: { ...running.progress, current: 400 } } }); });
  expect(state().speed).toBe(200);
  expect(screen.getAllByText("40%")).toHaveLength(2);
  const buttons = screen.getAllByRole("button", { name: /Cancel/ });
  act(() => { buttons[0]!.click(); buttons[1]!.click(); });
  await waitFor(() => expect(cancel).toHaveBeenCalledTimes(1));
  expect(screen.queryAllByRole("button", { name: /Cancel/ })).toHaveLength(0);
  await act(async () => { finish(ok(null)); });
});

it("retains a terminal shared view after the download leaves the active menu", async () => {
  const transport = createMockTransport({ calls: { jobsList: () => ok({ items: [running], next_cursor: null }) } });
  await mount(transport);
  await waitFor(() => expect(state().active).toBe(1));
  await act(async () => { transport.emitAppEvent({ type: "job_updated", job: { ...running, state: "succeeded" } }); });
  expect(state()).toMatchObject({ active: 0, state: "succeeded", speed: null });
  expect(screen.queryAllByRole("button", { name: /Cancel/ })).toHaveLength(0);
});

it("shows a typed cancellation failure and retries through the same provider", async () => {
  const cancel = vi.fn().mockReturnValueOnce({ status: "error", error: { code: "unavailable", message: "offline", details: null } }).mockReturnValue(ok(null));
  const transport = createMockTransport({ calls: { jobsList: () => ok({ items: [running], next_cursor: null }), jobCancel: cancel } });
  await mount(transport);
  await waitFor(() => expect(state().ready).toBe(true));
  fireEvent.click(screen.getAllByRole("button", { name: /Cancel/ })[0]!);
  expect(await screen.findByText("It isn't available right now.")).toBeTruthy();
  fireEvent.click(screen.getAllByRole("button", { name: "Try again" })[1]!);
  await waitFor(() => expect(cancel).toHaveBeenCalledTimes(2));
  await waitFor(() => expect(screen.queryByText("It isn't available right now.")).toBeNull());
});

it("reloads inventory and clears throughput samples when the database changes", async () => {
  vi.spyOn(performance, "now").mockReturnValue(0);
  let items = [running];
  const list = vi.fn(() => ok({ items, next_cursor: null }));
  const transport = createMockTransport({ calls: { jobsList: list } });
  await mount(transport);
  await waitFor(() => expect(state().ready).toBe(true));
  vi.mocked(performance.now).mockReturnValue(1000);
  await act(async () => { transport.emitAppEvent({ type: "job_updated", job: { ...running, progress: { ...running.progress, current: 200 } } }); });
  vi.mocked(performance.now).mockReturnValue(2000);
  await act(async () => { transport.emitAppEvent({ type: "job_updated", job: { ...running, progress: { ...running.progress, current: 400 } } }); });
  expect(state().speed).toBe(200);
  items = [];
  await act(async () => { transport.emitAppEvent({ type: "database_reloaded" }); });
  await waitFor(() => expect(state().active).toBe(0));
  expect(list).toHaveBeenCalledTimes(2);
  vi.mocked(performance.now).mockReturnValue(3000);
  await act(async () => { transport.emitAppEvent({ type: "job_updated", job: { ...running, progress: { ...running.progress, current: 500 } } }); });
  expect(state().speed).toBeNull();
});

it("surfaces subscription failure and reconnects through the shared retry action", async () => {
  const transport = createMockTransport({ calls: { jobsList: () => ok({ items: [], next_cursor: null }) } });
  const subscribe = vi.spyOn(transport, "subscribe").mockRejectedValueOnce({ code: "unavailable", message: "offline", details: null });
  await mount(transport);
  await waitFor(() => expect(state().error).toBe(true));
  expect(state().ready).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Try again" }));
  await waitFor(() => expect(subscribe).toHaveBeenCalledTimes(2));
  await waitFor(() => expect(state().ready).toBe(true));
  await act(async () => { transport.emitAppEvent({ type: "job_updated", job: running }); });
  expect(state().active).toBe(1);
});
