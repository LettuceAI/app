import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { toast } from "@lettuceai/crisp";
import type {
  JobEvent,
  UsageExportCsvRequest_Deserialize,
  UsageClearBeforeRequest,
  UsageRecalculateCostsRequest,
  SettingsView,
  UsageStatsRequest,
} from "@/api/generated/bindings";
import {
  createMockTransport,
  mockAppStatus,
  type MockCallHandlers,
  type MockStreamHandlers,
} from "@/api/mock-transport";
import { invalidationsFor, queryKeys } from "@/api/query-keys";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";
import { calendarDays, dayKey, rangeDates, rowTokens, timeSummary } from "./usage/data";
import { quotaDate } from "./usage/format";
import { nanoAccount, nanoUsage, priceJob, usageRow, usageTotals } from "./usage.test-fixtures";

vi.mock("@/api/window-controls", () => ({
  createWindowController: vi.fn().mockResolvedValue(null),
}));
const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const failure = () => ({
  status: "error" as const,
  error: { code: "unavailable" as const, message: "private diagnostic", details: null },
});
afterEach(() => {
  toast.dismiss();
  cleanup();
  vi.restoreAllMocks();
});
const stats = (request: UsageStatsRequest) => ({
  totals: usageTotals(),
  groups: [
    {
      key:
        request.group_by === "day"
          ? dayKey(new Date())
          : request.group_by === "model"
            ? "model-1"
            : request.group_by === "character"
              ? "char-1"
              : request.group_by === "provider"
                ? "openrouter"
                : "chat",
      label:
        request.group_by === "model"
          ? "Sonnet"
          : request.group_by === "character"
            ? "Alice"
            : request.group_by === "provider"
              ? "OpenRouter"
              : null,
      totals: usageTotals(),
    },
  ],
});
async function mount(
  options: {
    path?: string;
    platform?: "android" | "other";
    calls?: MockCallHandlers;
    streams?: MockStreamHandlers;
  } = {},
) {
  const transport = createMockTransport({
    calls: {
      appStatus: () =>
        ok({
          ...mockAppStatus,
          platform: options.platform ?? "other",
          ui_state: { onboarding: { completed: true } },
        }),
      settingsGet: () =>
        ok({
          global: { ui_preferences: {} },
          device: { revision: 1 },
          revision: 1,
        } as unknown as SettingsView),
      providerAccountsList: () => ok([]),
      jobsList: () => ok({ items: [], next_cursor: null }),
      usageStats: (request) => ok(stats(request)),
      usageQuery: () => ok({ items: [usageRow()], next_cursor: null }),
      usageGet: () => ok(usageRow()),
      appUsageDays: () => ok({ days: [{ day: dayKey(new Date()), active_ms: 3600000 }] }),
      ...options.calls,
    },
    streams: options.streams ?? {},
  });
  const history = createMemoryHistory({
    initialEntries: ["/settings", options.path ?? "/settings/usage"],
  });
  const runtime = await createAppRuntime({ transport, history, i18n: await createTestI18n() });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return { transport, runtime, history };
}
async function more(name: string) {
  fireEvent.click(await screen.findByRole("button", { name: "More" }));
  fireEvent.click(await screen.findByRole("menuitem", { name }));
}

describe("usage calendar and accounting", () => {
  it("uses local calendar boundaries and all seven days across DST", () => {
    const now = new Date(2026, 9, 25, 15).getTime();
    const range = rangeDates("7d", now);
    expect(new Date(range.start!).getHours()).toBe(0);
    expect(dayKey(new Date(range.start!))).toBe("2026-10-19");
    expect(new Date(range.end!).getHours()).toBe(23);
    expect(calendarDays("7d", now)).toHaveLength(7);
    expect(rangeDates("all", now)).toEqual({ start: null, end: null });
  });
  it("keeps more than thirty days of app time and includes days with no activity", () => {
    const now = new Date(2026, 9, 25, 15).getTime();
    const result = timeSummary(
      [
        { day: "2026-08-01", active_ms: 120000 },
        { day: "2026-10-25", active_ms: 60000 },
      ],
      "all",
      now,
    );
    expect(result.keys.length).toBe(86);
    expect(result.total).toBe(180000);
    expect(result.average).toBe(180000 / 86);
    expect(result.previous).toBeNull();
    expect(result.monthAverage).toBe(2000);
  });
  it("compares the adjacent period and preserves missing token counts", () => {
    const now = new Date(2026, 9, 25, 15).getTime();
    expect(
      timeSummary(
        [
          { day: "2026-10-18", active_ms: 100 },
          { day: "2026-10-25", active_ms: 200 },
        ],
        "7d",
        now,
      ).previous,
    ).toBe(100);
    expect(rowTokens(usageRow({ total_tokens: null, prompt_tokens: null }))).toBeNull();
    expect(rowTokens(usageRow({ total_tokens: null }))).toBe(120);
    expect(rowTokens(usageRow({ total_tokens: 999 }))).toBe(999);
  });
  it("parses quota timestamps without inventing dates", () => {
    expect(quotaDate("1700000000", "en")).toBe(quotaDate("1700000000000", "en"));
    expect(quotaDate("nonsense", "en")).toBeNull();
    expect(quotaDate(null, "en")).toBeNull();
  });
  it("invalidates time and quota independently from request reports", () => {
    expect(invalidationsFor({ type: "app_usage_changed" })).toEqual([
      queryKeys.usage.time(),
      queryKeys.usage.clock(),
    ]);
    expect(
      invalidationsFor({ type: "provider_quota", account_id: "nano", level: "near_limit" }),
    ).toContainEqual(queryKeys.usage.nano("nano"));
    expect(
      invalidationsFor({ type: "generation_settled", conversation_id: "chat" } as never),
    ).toContainEqual(queryKeys.usage.all);
  });
});
describe("Usage settings", () => {
  it("opens real summaries with cached token coverage and historical groups", async () => {
    await mount();
    expect(await screen.findByText("1/1 priced")).toBeTruthy();
    expect(screen.getByText("1/1 reported")).toBeTruthy();
    expect(screen.getByText("By model")).toBeTruthy();
    expect(screen.queryByRole("tab", { name: "NanoGPT" })).toBeNull();
  });
  it("keeps native cursor pagination and filter state", async () => {
    const query = vi.fn((request) =>
      ok({
        items: [
          usageRow({
            id: request.cursor ? "usage-2" : "usage-1",
            character_name: request.cursor ? "Bob" : "Alice",
          }),
        ],
        next_cursor: request.cursor ? null : "next",
      }),
    );
    const view = await mount({
      path: "/settings/usage?usageTab=requests&usageStatus=failed&usageOperation=chat&usageModel=model-1",
      calls: { usageQuery: query },
    });
    fireEvent.click(await screen.findByRole("button", { name: "Show more" }));
    expect(await screen.findByRole("button", { name: /Chat · Bob/ })).toBeTruthy();
    expect(
      query.mock.calls.some(
        ([request]) =>
          request.cursor === "next" &&
          request.filters.status === "failed" &&
          request.filters.model_id === "model-1",
      ),
    ).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Clear filters" }));
    await waitFor(() => expect(view.runtime.router.state.location.search.usageStatus).toBe("all"));
  });
  it("opens details over the list and closes with browser Back", async () => {
    const view = await mount({ path: "/settings/usage?usageTab=requests" });
    fireEvent.click(await screen.findByRole("button", { name: /Chat · Alice/ }));
    const detail = within(await screen.findByRole("dialog"));
    expect(detail.getByLabelText("Where the tokens went")).toBeTruthy();
    expect(detail.getByText("40 from cache · 10 written to cache")).toBeTruthy();
    expect(
      detail.getByText("$0.0001 cache reads, included · $0.0002 cache writes, included"),
    ).toBeTruthy();
    await act(async () => view.history.back());
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(view.runtime.router.state.location.search.usageTab).toBe("requests");
  });
  it("loads a detail deep link and keeps unknown local costs unknown", async () => {
    const get = vi.fn(() =>
      ok(
        usageRow({
          provider_kind: "llamacpp",
          total_cost: null,
          prompt_cost: null,
          completion_cost: null,
          prompt_tokens: null,
          total_tokens: null,
        }),
      ),
    );
    const view = await mount({
      path: "/settings/usage?usageTab=requests&usagePanel=request&usageRequest=usage-1",
      calls: { usageGet: get },
    });
    const detail = within(await screen.findByRole("dialog"));
    expect(await detail.findByText("No price")).toBeTruthy();
    expect(detail.queryByText("Free")).toBeNull();
    expect(detail.queryByLabelText("Where the tokens went")).toBeNull();
    fireEvent.click(detail.getByRole("button", { name: "Close" }));
    await waitFor(() =>
      expect(view.runtime.router.state.location.search.usagePanel).toBeUndefined(),
    );
    expect(view.history.location.pathname).toBe("/settings/usage");
  });
  it("shows retryable typed errors without native diagnostics", async () => {
    await mount({ calls: { usageStats: () => failure() } });
    expect((await screen.findAllByText("It isn't available right now.")).length).toBeGreaterThan(0);
    expect(screen.queryByText("private diagnostic")).toBeNull();
  });
  it("exports the report filters after a desktop file choice", async () => {
    const save = vi.fn(() => ok({ target: { uri: "/tmp/usage.csv" } }));
    const exportCsv = vi.fn((_request: UsageExportCsvRequest_Deserialize) =>
      ok({ uri: "/tmp/usage.csv", suggested_name: "usage.csv", warnings: [] }),
    );
    await mount({
      path: "/settings/usage?usageRange=7d&usageProvider=openrouter",
      calls: { filesPickSave: save, usageExportCsv: exportCsv },
    });
    fireEvent.click(await screen.findByRole("button", { name: "Export as CSV" }));
    await waitFor(() => expect(exportCsv).toHaveBeenCalledTimes(1));
    expect(exportCsv.mock.calls[0]?.[0]).toMatchObject({
      target: { uri: "/tmp/usage.csv" },
      filters: { provider_kind: "openrouter" },
    });
  });
  it("does not export when the save picker is cancelled", async () => {
    const exportCsv = vi.fn(() => ok({ uri: "unused", suggested_name: "usage.csv", warnings: [] }));
    const save = vi.fn(() => ok({ target: null }));
    await mount({ calls: { filesPickSave: save, usageExportCsv: exportCsv } });
    fireEvent.click(await screen.findByRole("button", { name: "Export as CSV" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(exportCsv).not.toHaveBeenCalled();
  });
  it("publishes Android CSV without a desktop picker", async () => {
    const save = vi.fn(() => ok({ target: null }));
    const exportCsv = vi.fn((_request: UsageExportCsvRequest_Deserialize) =>
      ok({ uri: "content://usage", suggested_name: "usage.csv", warnings: [] }),
    );
    await mount({
      platform: "android",
      path: "/settings/usage?usageTab=requests&usageStatus=failed",
      calls: { filesPickSave: save, usageExportCsv: exportCsv },
    });
    await more("Export as CSV");
    await waitFor(() => expect(exportCsv).toHaveBeenCalledTimes(1));
    expect(save).not.toHaveBeenCalled();
    expect(exportCsv.mock.calls[0]?.[0]).toMatchObject({
      target: null,
      filters: { status: "failed" },
    });
  });
  it("retries clearing with the same cutoff and operation identity", async () => {
    const clear = vi.fn((_request: UsageClearBeforeRequest) =>
      clear.mock.calls.length === 1 ? failure() : ok({ removed: 2 }),
    );
    await mount({ calls: { usageClearBefore: clear } });
    await more("Delete old records");
    const detail = within(await screen.findByRole("dialog"));
    fireEvent.click(detail.getByRole("button", { name: "Delete" }));
    fireEvent.click(await detail.findByRole("button", { name: "Try again" }));
    await waitFor(() => expect(clear).toHaveBeenCalledTimes(2));
    expect(clear.mock.calls[0]).toEqual(clear.mock.calls[1]);
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });
  it("recovers a price job, streams progress and cancels through the native job", async () => {
    let emit: ((event: JobEvent) => void) | undefined;
    const cancel = vi.fn(() => ok(null));
    const view = await mount({
      calls: {
        jobsList: (request) =>
          ok({
            items: request.subject?.id === "usage-recalculate-costs" ? [priceJob()] : [],
            next_cursor: null,
          }),
        jobCancel: cancel,
      },
      streams: {
        jobWatch: (_args, onEvent) => {
          emit = onEvent;
          return ok(priceJob());
        },
      },
    });
    expect(await screen.findByText("1 of 4 requests checked")).toBeTruthy();
    await act(async () =>
      emit?.({
        type: "progress",
        job: priceJob({
          progress: {
            current: 3,
            total: 4,
            unit: "items",
            label_code: null,
            bytes_per_second: null,
          },
        }),
      }),
    );
    expect(await screen.findByText("3 of 4 requests checked")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(cancel).toHaveBeenCalledWith({ job_id: "prices-job" }));
    await act(async () => emit?.({ type: "progress", job: priceJob({ state: "cancelled" }) }));
    await waitFor(() => expect(screen.queryByText("3 of 4 requests checked")).toBeNull());
    expect(view.history.location.pathname).toBe("/settings/usage");
  });
  it("retries a lost price-start response with the same operation identity", async () => {
    const start = vi.fn((_request: UsageRecalculateCostsRequest) =>
      start.mock.calls.length === 1 ? failure() : ok({ job_id: "prices-job" }),
    );
    await mount({
      calls: { usageRecalculateCosts: start, jobGet: () => ok(priceJob()) },
      streams: { jobWatch: () => ok(priceJob()) },
    });
    await more("Fetch missing OpenRouter prices");
    fireEvent.click(await screen.findByRole("button", { name: "Try again" }));
    await waitFor(() => expect(start).toHaveBeenCalledTimes(2));
    expect(start.mock.calls[0]).toEqual(start.mock.calls[1]);
    expect(await screen.findByText("1 of 4 requests checked")).toBeTruthy();
  });
  it("selects configured NanoGPT accounts and explicitly refreshes server quotas", async () => {
    const quota = vi.fn((request) => ok(nanoUsage(request.account_id)));
    await mount({
      path: "/settings/usage?usageTab=nanogpt&usageAccount=Second",
      calls: {
        providerAccountsList: () => ok([nanoAccount(), nanoAccount("Second")]),
        providerNanogptUsage: quota,
      },
    });
    expect(await screen.findByText("200 of 1,000 tokens")).toBeTruthy();
    expect(quota.mock.calls[0]?.[0]).toMatchObject({ account_id: "Second", refresh: false });
    const refresh = screen.getAllByRole("button", { name: "Refresh" });
    fireEvent.click(refresh[refresh.length - 1]!);
    await waitFor(() =>
      expect(quota.mock.calls.some(([request]) => request.refresh === true)).toBe(true),
    );
    expect(screen.queryByText("200 of 1,000 requests")).toBeNull();
  });
  it("explains missing NanoGPT credentials without requesting quotas", async () => {
    const quota = vi.fn(() => ok(nanoUsage()));
    await mount({
      path: "/settings/usage?usageTab=nanogpt",
      calls: {
        providerAccountsList: () => ok([nanoAccount("No key", { api_key_set: false })]),
        providerNanogptUsage: quota,
      },
    });
    expect(await screen.findByText("NanoGPT has no API key")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Open Providers" })).toBeTruthy();
    expect(quota).not.toHaveBeenCalled();
  });
  it("refreshes NanoGPT from the page header with an explicit server read", async () => {
    const quota = vi.fn((request) => ok(nanoUsage(request.account_id)));
    await mount({
      path: "/settings/usage?usageTab=nanogpt",
      calls: { providerAccountsList: () => ok([nanoAccount()]), providerNanogptUsage: quota },
    });
    await screen.findByText("200 of 1,000 tokens");
    fireEvent.click(screen.getAllByRole("button", { name: "Refresh" })[0]!);
    await waitFor(() => expect(quota.mock.calls.some(([request]) => request.refresh)).toBe(true));
  });
  it("clears old request panels and cached details when the database is replaced", async () => {
    let row = usageRow();
    const view = await mount({
      path: "/settings/usage?usageTab=requests&usagePanel=request&usageRequest=usage-1",
      calls: {
        usageGet: ({ id }) => ok(id === row.id ? row : null),
        usageQuery: () => ok({ items: [row], next_cursor: null }),
      },
    });
    await screen.findByRole("dialog");
    row = usageRow({ id: "usage-new", character_name: "New character" });
    await act(async () => view.transport.emitAppEvent({ type: "database_reloaded" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(view.runtime.router.state.location.search.usageRequest).toBeUndefined();
    expect(await screen.findByRole("button", { name: /Chat · New character/ })).toBeTruthy();
    expect(
      view.runtime.queryClient.getQueryData([...queryKeys.usage.all, "detail", "usage-1"]),
    ).not.toEqual(expect.objectContaining({ character_name: "Alice" }));
  });
  it("does not export an old database after its save picker resolves", async () => {
    let choose: ((value: ReturnType<typeof ok<{ target: { uri: string } }>>) => void) | undefined;
    const save = vi.fn(
      () =>
        new Promise<ReturnType<typeof ok<{ target: { uri: string } }>>>((resolve) => {
          choose = resolve;
        }),
    );
    const exportCsv = vi.fn(() => ok({ uri: "unused", suggested_name: "usage.csv", warnings: [] }));
    const view = await mount({ calls: { filesPickSave: save, usageExportCsv: exportCsv } });
    fireEvent.click(await screen.findByRole("button", { name: "Export as CSV" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    await act(async () => view.transport.emitAppEvent({ type: "database_reloaded" }));
    await act(async () => choose?.(ok({ target: { uri: "/tmp/old.csv" } })));
    expect(exportCsv).not.toHaveBeenCalled();
  });
});
