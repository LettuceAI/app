import type { ComponentProps } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, expect, it, vi } from "vitest";
import type { Sheet } from "@lettuceai/crisp";
import type { SettingsUpdateRequest_Deserialize, SettingsView } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

const sheetRender = vi.hoisted(() => vi.fn());
vi.mock("@lettuceai/crisp", async (original) => {
  const crisp = await original<typeof import("@lettuceai/crisp")>();
  return {
    ...crisp,
    Sheet: (props: ComponentProps<typeof Sheet>) => {
      sheetRender(props);
      return <crisp.Sheet {...props} />;
    },
  };
});
vi.mock("@/api/window-controls", () => ({
  createWindowController: vi.fn().mockResolvedValue(null),
}));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
const ok = <T,>(data: T) => ({ status: "ok" as const, data });

async function mount(tab = "lorebook", gate?: Promise<void>) {
  let current = {
    revision: 1,
    global: {
      ui_preferences: {},
      lorebook_scan_depth: 10,
      lorebook_generator: {
        default_target_count: null,
        max_output_tokens: null,
        structured_fallback_format: "json",
        selection: {
          model_profile_id: null,
          planner_prompt_id: null,
          writer_prompt_id: null,
          refine_prompt_id: null,
          coherence_prompt_id: null,
        },
      },
      lorebook_entry_generator: {
        model_profile_id: null,
        entry_prompt_id: null,
        keyword_prompt_id: null,
        structured_fallback_format: "json",
      },
    },
    device: { revision: 1 },
  } as unknown as SettingsView;
  const write = vi.fn(async (request: SettingsUpdateRequest_Deserialize) => {
    if (gate) await gate;
    const patch = request.patch;
    if (patch.section === "lorebook_generator")
      current = { ...current, global: { ...current.global, lorebook_generator: patch.value } };
    if (patch.section === "general")
      current = {
        ...current,
        global: { ...current.global, lorebook_scan_depth: patch.lorebook_scan_depth! },
      };
    current = { ...current, revision: current.revision + 1 };
    return ok(current);
  });
  const transport = createMockTransport({
    calls: {
      appStatus: () => ok({ ...mockAppStatus, ui_state: { onboarding: { completed: true } } }),
      settingsGet: () => ok(current),
      settingsUpdate: write,
      modelsList: () => ok({ models: [], default_model_id: null, revision: 1 }),
      providerAccountsList: () => ok([]),
      promptsList: () => ok({ items: [], next_cursor: null }),
      jobsList: () => ok({ items: [], next_cursor: null }),
      lorebookGeneratorDefaults: () =>
        ok({ target_count: 12, min_target_count: 1, max_target_count: 100, output_tokens: 4096 }),
    },
  });
  const history = createMemoryHistory({
    initialEntries: ["/settings", `/settings/lorebooks?loreTab=${tab}`],
  });
  const runtime = await createAppRuntime({ transport, history, i18n: await createTestI18n() });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return { history, runtime, write };
}

it.each([
  ["Model", "Search models"],
  ["Plan", "Search prompts"],
])("keeps the %s picker mounted with its content while closing", async (title, search) => {
  const { history, runtime } = await mount();
  fireEvent.click(
    (await screen.findByText(title, { exact: true, selector: "span" })).closest("button")!,
  );
  const dialog = within(await screen.findByRole("dialog"));
  fireEvent.change(dialog.getByRole("searchbox", { name: search }), { target: { value: "kept" } });
  await act(async () => history.back());
  await waitFor(() => expect(runtime.router.state.location.search.lorePicker).toBeUndefined());
  const props = sheetRender.mock.lastCall?.[0] as ComponentProps<typeof Sheet>;
  expect(props.open).toBe(false);
  expect(props.title).toBe(title);
  expect(props.children).toHaveProperty("props.children.0.props.value", "kept");
});

it.each([
  ["lorebook", "Entries to plan", 23, "lorebook_generator"],
  ["scanning", "Messages to check", 17, "general"],
])("edits the %s slider value directly and saves on Enter", async (tab, label, value, section) => {
  const { write } = await mount(tab);
  const input = await screen.findByRole("textbox", { name: label });
  act(() => input.focus());
  fireEvent.change(input, { target: { value: String(value) } });
  expect(write).not.toHaveBeenCalled();
  fireEvent.keyDown(input, { key: "Enter" });
  await waitFor(() => expect(write).toHaveBeenCalledTimes(1));
  const patch = write.mock.calls[0]![0].patch;
  expect(patch.section).toBe(section);
  expect(
    patch.section === "general"
      ? patch.lorebook_scan_depth
      : patch.section === "lorebook_generator"
        ? patch.value.default_target_count
        : null,
  ).toBe(value);
  expect((screen.getByRole("slider", { name: label }) as HTMLInputElement).value).toBe(
    String(value),
  );
});

it("cancels a typed value with Escape and bounds committed values", async () => {
  const { write } = await mount();
  const input = await screen.findByRole("textbox", { name: "Entries to plan" });
  act(() => input.focus());
  fireEvent.change(input, { target: { value: "35" } });
  fireEvent.keyDown(input, { key: "Escape" });
  expect(write).not.toHaveBeenCalled();
  expect((input as HTMLInputElement).value).toBe("12");
  act(() => input.focus());
  fireEvent.change(input, { target: { value: "200" } });
  fireEvent.blur(input);
  await waitFor(() => expect(write).toHaveBeenCalledTimes(1));
  const patch = write.mock.calls[0]![0].patch;
  expect(patch.section === "lorebook_generator" && patch.value.default_target_count).toBe(100);
});

it("does not navigate back twice when a selection saves after dismissal", async () => {
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  const { history, runtime, write } = await mount("lorebook", gate);
  fireEvent.click(
    (await screen.findByText("Model", { exact: true, selector: "span" })).closest("button")!,
  );
  const dialog = within(await screen.findByRole("dialog"));
  fireEvent.click(dialog.getByRole("button", { name: "Automatic" }));
  await waitFor(() => expect(write).toHaveBeenCalledTimes(1));
  await act(async () => history.back());
  await act(async () => {
    release();
    await gate;
  });
  expect(runtime.router.state.location.pathname).toBe("/settings/lorebooks");
  expect(runtime.router.state.location.search.lorePicker).toBeUndefined();
});
