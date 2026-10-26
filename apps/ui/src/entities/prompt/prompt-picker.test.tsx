import { useState } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query";
import { I18nextProvider } from "react-i18next";
import { afterEach, expect, it, vi } from "vitest";
import { createApiClient } from "@/api/client";
import type { PromptKind, PromptSummary } from "@/api/generated/bindings";
import { createMockTransport, type MockCallHandlers } from "@/api/mock-transport";
import { createTestI18n } from "@/shared/testing/i18n";
import { PromptPickerSheet, promptsQuery } from "./index";

afterEach(cleanup);
const kind: PromptKind = "dynamic_memory_summarizer";
const otherKind: PromptKind = "dynamic_memory_manager";
const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const prompt = (id: string, patch: Partial<PromptSummary> = {}): PromptSummary => ({
  id,
  name: id,
  kind,
  archived: false,
  app_default: false,
  origin: { type: "user" },
  revision: 1,
  created_at: 1,
  updated_at: 1,
  ...patch,
});
const builtin = prompt("Bundled summary", {
  origin: { type: "built_in", key: "summary", protected: true, required: true, edited: false },
});

async function mount(
  options: {
    calls?: MockCallHandlers;
    items?: PromptSummary[];
    value?: string | null;
    inherited?: boolean;
    language?: "en" | "cimode";
  } = {},
) {
  const list = vi.fn<NonNullable<MockCallHandlers["promptsList"]>>(() =>
    ok({ items: options.items ?? [builtin, prompt("My summary")], next_cursor: null }),
  );
  const api = createApiClient(
    createMockTransport({ calls: { promptsList: list, ...options.calls } }),
  );
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: Infinity } },
  });
  const choose = vi.fn();
  const manage = vi.fn();
  const reopenLabel = "Reopen";
  const kindLabel = "Change kind";
  function Consumer() {
    const [open, setOpen] = useState(true);
    const [selectedKind, setKind] = useState(kind);
    const [value, setValue] = useState(options.value ?? null);
    const query = useQuery(promptsQuery(api, selectedKind));
    return (
      <>
        <button onClick={() => setOpen(true)}>{reopenLabel}</button>
        <button onClick={() => setKind(otherKind)}>{kindLabel}</button>
        <PromptPickerSheet
          open={open}
          kind={selectedKind}
          title={selectedKind}
          prompts={query}
          value={value}
          {...(options.inherited
            ? { defaultChoice: { title: "App default", description: "Inherited summary" } }
            : {})}
          onChoose={(id) => {
            choose(id);
            setValue(id);
          }}
          onClose={() => setOpen(false)}
          onManage={manage}
        />
      </>
    );
  }
  const i18n = await createTestI18n(options.language);
  await act(async () => {
    render(
      <QueryClientProvider client={client}>
        <I18nextProvider i18n={i18n}>
          <Consumer />
        </I18nextProvider>
      </QueryClientProvider>,
    );
  });
  return { choose, manage, list, client };
}

it("loads every native page, filters kind and archive state, and resets to built-in", async () => {
  const list = vi.fn<NonNullable<MockCallHandlers["promptsList"]>>(({ cursor }) =>
    ok({
      items: cursor
        ? [
            prompt("Last page"),
            prompt("Other kind", { kind: otherKind }),
            prompt("Archived", { archived: true }),
          ]
        : [builtin, prompt("First page")],
      next_cursor: cursor ? null : "next-page",
    }),
  );
  const { choose } = await mount({ calls: { promptsList: list }, value: builtin.id });
  const last = await screen.findByRole("button", { name: "Last page" });
  expect(list).toHaveBeenCalledWith({
    kind,
    lifecycle: "active",
    cursor: "next-page",
    limit: null,
  });
  expect(screen.queryByText("Other kind")).toBeNull();
  expect(screen.queryByText("Archived")).toBeNull();
  expect(screen.queryByRole("button", { name: "Bundled summary" })).toBeNull();
  const reset = screen.getByRole("button", { name: /^Built-in/ });
  expect(reset.getAttribute("aria-pressed")).toBe("true");
  fireEvent.click(last);
  expect(choose).toHaveBeenLastCalledWith("Last page");
  expect(reset.getAttribute("aria-pressed")).toBe("false");
  fireEvent.click(reset);
  expect(choose).toHaveBeenLastCalledWith(null);
  expect(reset.getAttribute("aria-pressed")).toBe("true");
});

it("keeps inherited defaults distinct from an explicitly chosen built-in", async () => {
  const { choose } = await mount({ inherited: true });
  const bundled = await screen.findByRole("button", { name: "Bundled summary" });
  const inherited = screen.getByRole("button", { name: /^App default/ });
  expect(inherited.getAttribute("aria-pressed")).toBe("true");
  fireEvent.click(bundled);
  expect(choose).toHaveBeenLastCalledWith(builtin.id);
  expect(inherited.getAttribute("aria-pressed")).toBe("false");
  expect(bundled.getAttribute("aria-pressed")).toBe("true");
});

it("keeps System prompts last and available while loading and after a typed failure", async () => {
  let fail = true;
  let finish!: () => void;
  const gate = new Promise<void>((resolve) => {
    finish = resolve;
  });
  const { manage } = await mount({
    calls: {
      promptsList: async () => {
        await gate;
        return fail
          ? {
              status: "error",
              error: { code: "unavailable", message: "private diagnostic", details: null },
            }
          : ok({ items: [], next_cursor: null });
      },
    },
  });
  expect(screen.queryByRole("button", { name: /^Built-in/ })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: /^System prompts/ }));
  expect(manage).toHaveBeenCalledTimes(1);
  await act(async () => {
    finish();
    await gate;
  });
  expect(await screen.findByText("It isn't available right now.")).toBeTruthy();
  expect(screen.queryByText("private diagnostic")).toBeNull();
  expect(screen.getByRole("button", { name: /^System prompts/ })).toBeTruthy();
  fail = false;
  fireEvent.click(screen.getByRole("button", { name: "Try again" }));
  await screen.findByRole("button", { name: /^Built-in/ });
  expect(screen.getByText("Built-in prompt is unavailable")).toBeTruthy();
  const rows = within(screen.getByRole("dialog")).getAllByRole("button");
  expect(rows.at(-1)?.textContent).toContain("System prompts");
});

it("searches large lists and clears search when reopened", async () => {
  await mount({ items: Array.from({ length: 10 }, (_, index) => prompt(`Summary ${index}`)) });
  const search = await screen.findByRole("searchbox", { name: "Search prompts" });
  fireEvent.change(search, { target: { value: "Summary 9" } });
  expect(screen.getByRole("button", { name: "Summary 9" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Summary 0" })).toBeNull();
  fireEvent.change(search, { target: { value: "missing" } });
  expect(screen.getByText("No matching prompts")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Close" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  fireEvent.click(screen.getByRole("button", { name: "Reopen" }));
  expect(await screen.findByRole("searchbox", { name: "Search prompts" })).toHaveProperty(
    "value",
    "",
  );
  expect(screen.getByRole("button", { name: "Summary 0" })).toBeTruthy();
});

it("clears the previous search when changing prompt kinds", async () => {
  const list = vi.fn<NonNullable<MockCallHandlers["promptsList"]>>(({ kind: requested }) =>
    ok({
      items: Array.from({ length: 10 }, (_, index) =>
        prompt(`Prompt ${index}`, { kind: requested ?? kind }),
      ),
      next_cursor: null,
    }),
  );
  await mount({ calls: { promptsList: list } });
  fireEvent.change(await screen.findByRole("searchbox", { name: "Search prompts" }), {
    target: { value: "previous" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Change kind" }));
  await waitFor(() =>
    expect(list).toHaveBeenCalledWith(expect.objectContaining({ kind: otherKind })),
  );
  expect(await screen.findByRole("searchbox", { name: "Search prompts" })).toHaveProperty(
    "value",
    "",
  );
  expect(await screen.findByRole("button", { name: "Prompt 0" })).toBeTruthy();
});

it("localizes the shared controls", async () => {
  await mount({ language: "cimode" });
  expect(await screen.findByRole("button", { name: /^promptPicker.builtIn/ })).toBeTruthy();
  expect(screen.getByRole("button", { name: /^promptPicker.manage/ })).toBeTruthy();
});
