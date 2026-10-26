import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, expect, it, vi } from "vitest";
import { toast } from "@lettuceai/crisp";
import type { LogLineView, LogQueryRequest, LogQueryView } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";
import { matcherFor, parseLine, rawText } from "./data";

vi.mock("@/api/window-controls", () => ({
  createWindowController: vi.fn().mockResolvedValue(null),
}));
afterEach(() => {
  toast.dismiss();
  cleanup();
  vi.restoreAllMocks();
});
const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const day = "app-2026-10-26.log";
const raw = [
  "[2026-10-26T09:00:00Z] INFO app | started",
  "[2026-10-26T09:00:01Z] DEBUG app at=main.rs:1 | debug message",
  "[2026-10-26T09:00:02Z] WARN provider at=send.rs:2 | retry requested",
  "[2026-10-26T09:00:03Z] ERROR provider at=send.rs:3 | model failed",
  "unstructured continuation",
];
const view = (request: LogQueryRequest): LogQueryView => {
  const indices = request.filter.only_matches
    ? [3]
    : request.filter.levels.includes("debug")
      ? [0, 1, 2, 3, 4]
      : [0, 2, 3, 4];
  const positions = indices.map((index, position) => ({ index, position }));
  return {
    total: raw.length,
    filtered_total: indices.length,
    lines: positions
      .slice(request.offset, request.offset + request.limit)
      .map((entry) => ({ ...entry, raw: raw[entry.index]! })),
    counts: { info: 1, debug: 1, warn: 1, error: 1 },
    components: [
      { component: "app", count: 2 },
      { component: "provider", count: 2 },
    ],
    matches: request.filter.query ? positions.filter((entry) => entry.index === 3) : [],
    problems: positions.filter((entry) => entry.index === 2 || entry.index === 3),
  };
};
async function mount(
  options: {
    phone?: boolean;
    calls?: MockCallHandlers;
    files?: string[];
    language?: "en" | "cimode";
    path?: string;
  } = {},
) {
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockImplementation(function (
    this: HTMLElement,
  ) {
    return this.hasAttribute("data-index") ? 28 : 600;
  });
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(1000);
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (
    this: HTMLElement,
  ) {
    const scroller = this.closest("main");
    const top = this === scroller ? 0 : -(scroller?.scrollTop ?? 0);
    return new DOMRect(0, top, 1000, this.hasAttribute("data-index") ? 28 : 600);
  });
  let files = options.files ?? [day];
  const query = vi.fn<NonNullable<MockCallHandlers["logQuery"]>>((request) => ok(view(request)));
  const remove = vi.fn(() => {
    files = [];
    return ok(null);
  });
  const save = vi.fn(() => ok(null));
  const pick = vi.fn(() => ok({ target: { uri: "/exports/log.txt" } }));
  const report = vi.fn(() => ok("Actual native diagnostics"));
  const transport = createMockTransport({
    calls: {
      appStatus: () =>
        ok({
          ...mockAppStatus,
          platform: options.phone ? "android" : "other",
          ui_state: { onboarding: { completed: true } },
        }),
      settingsGet: () =>
        ok({ global: { ui_preferences: {} }, device: { revision: 1 }, revision: 1 } as never),
      jobsList: () => ok({ items: [], next_cursor: null }),
      logsList: () => ok({ files }),
      logQuery: query,
      logReadPage: ({ offset }) =>
        ok({ total: raw.length, lines: raw[offset] === undefined ? [] : [raw[offset]!] }),
      logRelevantLines: () => ok({ total: raw.length, matches: [2, 3] }),
      logDelete: remove,
      logsClear: remove,
      logExport: save,
      logExportSelection: save,
      filesPickSave: pick,
      logsDiagnosticsReport: report,
      ...options.calls,
    },
  });
  const history = createMemoryHistory({
    initialEntries: ["/settings", options.path ?? "/settings/logs"],
  });
  const runtime = await createAppRuntime({
    transport,
    history,
    i18n: await createTestI18n(options.language),
  });
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return { query, remove, save, pick, report, transport, history, runtime, writeText };
}
const openMore = async () => {
  fireEvent.click(await screen.findByRole("button", { name: "More" }));
};
const row = async (message: string) => (await screen.findByText(message)).closest("button")!;

it("reads native lines and sends level and whole-day search filters", async () => {
  const { query } = await mount();
  expect(await screen.findByText("model failed")).toBeTruthy();
  expect(screen.getByText("unstructured continuation")).toBeTruthy();
  expect(screen.queryByText("debug message")).toBeNull();
  expect(query.mock.calls[0]![0]).toMatchObject({
    name: day,
    offset: 0,
    filter: { levels: ["error", "warn", "info"], query: "" },
  });
  fireEvent.click(screen.getByRole("button", { name: /^Debug/ }));
  expect(await screen.findByText("debug message")).toBeTruthy();
  fireEvent.change(screen.getByRole("searchbox", { name: "Search logs" }), {
    target: { value: "failed" },
  });
  await waitFor(() =>
    expect(query).toHaveBeenLastCalledWith(
      expect.objectContaining({ filter: expect.objectContaining({ query: "failed" }) }),
    ),
  );
  await waitFor(() =>
    expect(
      (screen.getByRole("button", { name: "Only matches" }) as HTMLButtonElement).disabled,
    ).toBe(false),
  );
  fireEvent.click(screen.getByRole("button", { name: "Only matches" }));
  await waitFor(() =>
    expect(query).toHaveBeenLastCalledWith(
      expect.objectContaining({
        filter: expect.objectContaining({ query: "failed", only_matches: true }),
      }),
    ),
  );
});

it("opens the phone line sheet, copies the original line and keeps Back on Logs", async () => {
  const { writeText, history, runtime } = await mount({ phone: true });
  fireEvent.click(await row("model failed"));
  const dialog = within(await screen.findByRole("dialog", { name: "Line 4" }));
  fireEvent.click(dialog.getByRole("button", { name: "Copy line" }));
  await waitFor(() => expect(writeText).toHaveBeenCalledWith(raw[3]));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  fireEvent.click(await row("model failed"));
  await screen.findByRole("dialog");
  await act(async () => history.back());
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(runtime.router.state.location.pathname).toBe("/settings/logs");
});

it("exports selected zero-based indices through the native target picker", async () => {
  const { save, pick } = await mount();
  fireEvent.click(await row("started"));
  fireEvent.click(await row("model failed"), { ctrlKey: true });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
  expect(pick).toHaveBeenCalledWith({
    suggested_name: "app-2026-10-26-selection.log",
    kind: "any",
  });
  expect(save).toHaveBeenCalledWith({
    name: day,
    target: { uri: "/exports/log.txt" },
    selection: { type: "indices", indices: [0, 3] },
  });
});

it("exports the actual filter and confirms deletion", async () => {
  const { save, remove } = await mount();
  await row("started");
  await openMore();
  fireEvent.click(screen.getByRole("menuitem", { name: /Save the 4 shown lines/ }));
  await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
  expect(save).toHaveBeenCalledWith(
    expect.objectContaining({
      name: day,
      selection: {
        type: "filter",
        filter: expect.objectContaining({ levels: ["error", "warn", "info"] }),
      },
    }),
  );
  await openMore();
  fireEvent.click(screen.getByRole("menuitem", { name: "Delete this day" }));
  expect(remove).not.toHaveBeenCalled();
  fireEvent.click(
    within(await screen.findByRole("dialog")).getByRole("button", { name: "Delete" }),
  );
  await waitFor(() => expect(remove).toHaveBeenCalledWith({ name: day }));
  expect(await screen.findByText("No logs yet")).toBeTruthy();
});

it("uses payload-free change events only while following and does not regenerate diagnostics", async () => {
  const { query, transport, report } = await mount();
  await row("started");
  const before = query.mock.calls.length;
  await act(async () => transport.emitAppEvent({ type: "logs_changed" }));
  expect(query).toHaveBeenCalledTimes(before);
  fireEvent.click(screen.getByRole("button", { name: "Follow" }));
  await waitFor(() => expect(query.mock.calls.length).toBeGreaterThan(before));
  const following = query.mock.calls.length;
  await act(async () => {
    transport.emitAppEvent({ type: "logs_changed" });
    transport.emitAppEvent({ type: "logs_changed" });
  });
  await waitFor(() => expect(query.mock.calls.length).toBeGreaterThan(following));
  expect(report).not.toHaveBeenCalled();
  await openMore();
  fireEvent.click(screen.getByRole("menuitem", { name: "Diagnostics report" }));
  await screen.findByText("Actual native diagnostics");
  expect(report).toHaveBeenCalledTimes(1);
  const beforeReportEvent = query.mock.calls.length;
  await act(async () => transport.emitAppEvent({ type: "logs_changed" }));
  await waitFor(() => expect(query.mock.calls.length).toBeGreaterThan(beforeReportEvent));
  expect(report).toHaveBeenCalledTimes(1);
});

it("reads real diagnostics in the empty state and exposes clipboard errors", async () => {
  const { report, writeText } = await mount({ files: [] });
  expect(await screen.findByText("No logs yet")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Diagnostics report" }));
  const dialog = within(await screen.findByRole("dialog"));
  expect(await dialog.findByText("Actual native diagnostics")).toBeTruthy();
  expect(report).toHaveBeenCalledTimes(1);
  writeText.mockRejectedValueOnce(new Error("private clipboard failure"));
  fireEvent.click(dialog.getAllByRole("button", { name: "Copy" }).at(-1)!);
  expect(await dialog.findByRole("button", { name: "Try again" })).toBeTruthy();
  expect(screen.queryByText("private clipboard failure")).toBeNull();
});

it("shows native read failures with retry instead of fake log lines", async () => {
  let fail = true;
  await mount({
    calls: {
      logQuery: (request) =>
        fail
          ? {
              status: "error",
              error: { code: "unavailable", message: "private native failure", details: null },
            }
          : ok(view(request)),
    },
  });
  expect(await screen.findByText("It isn't available right now.")).toBeTruthy();
  expect(screen.queryByText("private native failure")).toBeNull();
  fail = false;
  fireEvent.click(screen.getByRole("button", { name: "Try again" }));
  expect(await screen.findByText("started")).toBeTruthy();
});

it("parses location-free, full and malformed lines and copies in original order", () => {
  const rows: LogLineView[] = raw.map((text, index) => ({ index, position: index, raw: text }));
  expect(parseLine(rows[0]!)).toMatchObject({
    level: "info",
    component: "app",
    location: "",
    message: "started",
  });
  expect(parseLine(rows[3]!)).toMatchObject({
    level: "error",
    location: "send.rs:3",
    message: "model failed",
  });
  expect(parseLine(rows[4]!)).toMatchObject({ level: null, message: raw[4] });
  expect(rawText([rows[3]!, rows[0]!])).toBe(`${raw[0]}\n${raw[3]}`);
  expect(matcherFor("[", ["regex"])).toBeNull();
  expect(matcherFor("app", ["word"])?.test("application")).toBe(false);
});

it("jumps to problems beyond the first native page", async () => {
  vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockReturnValue(60000);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(600);
  vi.spyOn(HTMLElement.prototype, "scrollTo").mockImplementation(function (
    this: HTMLElement,
    options?: ScrollToOptions | number,
  ) {
    this.scrollTop = typeof options === "object" ? (options.top ?? 0) : (options ?? 0);
    this.dispatchEvent(new Event("scroll"));
  });
  const query = vi.fn<NonNullable<MockCallHandlers["logQuery"]>>((request) =>
    ok({
      total: 2001,
      filtered_total: 2001,
      lines: Array.from(
        { length: Math.min(request.limit, 2001 - request.offset) },
        (_, offset) => ({
          index: request.offset + offset,
          position: request.offset + offset,
          raw: `[2026-10-26T09:00:00Z] INFO app | entry ${request.offset + offset}`,
        }),
      ),
      counts: { debug: 0, info: 2000, warn: 0, error: 1 },
      components: [{ component: "app", count: 2001 }],
      matches: [],
      problems: [{ index: 1200, position: 1200 }],
    }),
  );
  await mount({ calls: { logQuery: query } });
  await screen.findByText("entry 0");
  fireEvent.click(screen.getByRole("button", { name: "Next problem" }));
  await waitFor(() =>
    expect(query).toHaveBeenCalledWith(expect.objectContaining({ offset: 1000 })),
  );
  expect(await screen.findByText("entry 1200")).toBeTruthy();
});

it("replaces the phone More sheet when opening diagnostics", async () => {
  const { history, runtime } = await mount({ phone: true });
  await row("started");
  await openMore();
  fireEvent.click(
    within(await screen.findByRole("dialog")).getByRole("button", { name: "Diagnostics report" }),
  );
  expect(await screen.findByText("Actual native diagnostics")).toBeTruthy();
  await act(async () => history.back());
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(runtime.router.state.location.pathname).toBe("/settings/logs");
});

it("keeps a newer day's content when the previous read finishes late", async () => {
  let finish!: (value: ReturnType<typeof ok<LogQueryView>>) => void;
  const gate = new Promise<ReturnType<typeof ok<LogQueryView>>>((resolve) => {
    finish = resolve;
  });
  const older = "app-2026-10-25.log";
  let previousRequest!: LogQueryRequest;
  await mount({
    phone: true,
    files: [day, older],
    calls: {
      logQuery: (request) => {
        if (request.name === day) {
          previousRequest = request;
          return gate;
        }
        const result = view(request);
        return ok({ ...result, lines: [{ index: 0, position: 0, raw: "new day contents" }] });
      },
    },
  });
  fireEvent.change(await screen.findByRole("combobox", { name: "Day" }), {
    target: { value: older },
  });
  expect(await screen.findByText("new day contents")).toBeTruthy();
  await act(async () => finish(ok(view(previousRequest))));
  expect(screen.getByText("new day contents")).toBeTruthy();
  expect(screen.queryByText("started")).toBeNull();
});

it("clears the old range selection when filters change", async () => {
  const { query } = await mount();
  fireEvent.click(await row("started"));
  fireEvent.click(await row("model failed"), { ctrlKey: true });
  expect(screen.getByRole("button", { name: "Save" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: /^Debug/ }));
  await screen.findByText("debug message");
  expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
  const reads = query.mock.calls.length;
  fireEvent.click(await row("model failed"), { shiftKey: true });
  expect(query).toHaveBeenCalledTimes(reads);
  expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
});

it("closes a replacement panel back to Logs when More was opened by a direct link", async () => {
  const { runtime } = await mount({ phone: true, path: "/settings/logs?logPanel=more" });
  fireEvent.click(
    within(await screen.findByRole("dialog")).getByRole("button", { name: "Diagnostics report" }),
  );
  await screen.findByText("Actual native diagnostics");
  fireEvent.click(
    within(screen.getByRole("dialog", { name: "Diagnostics report" })).getByRole("button", {
      name: "Close",
    }),
  );
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(runtime.router.state.location.pathname).toBe("/settings/logs");
  expect(runtime.router.state.location.search.logPanel).toBeUndefined();
});

it("keeps following the newest rows after refreshing a searched log", async () => {
  vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockReturnValue(60000);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(600);
  vi.spyOn(HTMLElement.prototype, "scrollTo").mockImplementation(function (
    this: HTMLElement,
    options?: ScrollToOptions | number,
  ) {
    this.scrollTop = typeof options === "object" ? (options.top ?? 0) : (options ?? 0);
    this.dispatchEvent(new Event("scroll"));
  });
  let total = 100;
  const query = vi.fn<NonNullable<MockCallHandlers["logQuery"]>>((request) =>
    ok({
      total,
      filtered_total: total,
      lines: Array.from({ length: total }, (_, index) => ({
        index,
        position: index,
        raw: `[2026-10-26T09:00:00Z] INFO app | entry ${index}`,
      })),
      counts: { debug: 0, info: total, warn: 0, error: 0 },
      components: [{ component: "app", count: total }],
      matches: request.filter.query ? [{ index: 30, position: 30 }] : [],
      problems: [],
    }),
  );
  const { transport } = await mount({ calls: { logQuery: query } });
  await screen.findByText("entry 0");
  fireEvent.change(screen.getByRole("searchbox", { name: "Search logs" }), {
    target: { value: "entry 30" },
  });
  await waitFor(() =>
    expect(query).toHaveBeenLastCalledWith(
      expect.objectContaining({ filter: expect.objectContaining({ query: "entry 30" }) }),
    ),
  );
  fireEvent.click(screen.getByRole("button", { name: "Follow" }));
  const main = screen.getByRole("searchbox", { name: "Search logs" }).closest("main")!;
  await waitFor(() => expect(main.scrollTop).toBeGreaterThan(1000));
  total++;
  const previous = query.mock.calls.length;
  await act(async () => transport.emitAppEvent({ type: "logs_changed" }));
  await waitFor(() => expect(query.mock.calls.length).toBeGreaterThan(previous));
  await waitFor(() => expect(main.scrollTop).toBeGreaterThan(1000));
  fireEvent.click(screen.getByRole("button", { name: "Next match" }));
  expect(await screen.findByRole("button", { name: "Follow" })).toBeTruthy();
  await waitFor(() => expect(main.scrollTop).toBeLessThan(1000));
});

it("discards a range response that completes after the filter changed", async () => {
  let finish!: (value: ReturnType<typeof ok<LogQueryView>>) => void;
  let captured!: LogQueryRequest;
  const gate = new Promise<ReturnType<typeof ok<LogQueryView>>>((resolve) => {
    finish = resolve;
  });
  const query = vi.fn<NonNullable<MockCallHandlers["logQuery"]>>((request) => {
    if (request.limit !== 500) {
      captured = request;
      return gate;
    }
    return ok(view(request));
  });
  await mount({ calls: { logQuery: query } });
  fireEvent.click(await row("started"));
  fireEvent.click(await row("model failed"), { shiftKey: true });
  await waitFor(() => expect(captured).toBeDefined());
  fireEvent.click(screen.getByRole("button", { name: /^Debug/ }));
  await screen.findByText("debug message");
  await act(async () => finish(ok(view(captured))));
  expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
});
