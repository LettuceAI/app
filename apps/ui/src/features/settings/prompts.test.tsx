import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createBrowserHistory, createMemoryHistory } from "@tanstack/react-router";
import { toast } from "@lettuceai/crisp";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PromptCondition, PromptCreateRequest, PromptView } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));
beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(800);
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(1200);
});
afterEach(() => { toast.dismiss(); cleanup(); vi.restoreAllMocks(); });
const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const condition: PromptCondition = { type: "all", conditions: [{ type: "message_count_at_least", value: 6 }, { type: "message_count_at_least", value: 12 }, { type: "any", conditions: [{ type: "has_persona", value: true }, { type: "has_scene", value: true }] }, { type: "not", condition: { type: "reasoning_enabled", value: true } }] };
function document(patch: Partial<PromptView["prompt"]> = {}): PromptView {
  return {
    prompt: { id: "p1", name: "Story", kind: "direct_chat", origin: { type: "user" }, app_default: false, archived: false, revision: 7, created_at: 1, updated_at: 1, ...patch }, condense: false, behavior: "legacy_v1",
    entries: [{ id: "e1", built_in_key: null, name: "Direction", content: "Continue {{character}}", role: "system", enabled: true, position: "relative", depth: 0, conditional_min_messages: 8, interval_turns: 4, system_prompt: false, condition, image_slot: null }, { id: "e2", built_in_key: null, name: "Reminder", content: "Keep the scene moving", role: "user", enabled: true, position: "in_chat", depth: 2, conditional_min_messages: null, interval_turns: null, system_prompt: false, condition: null, image_slot: null }],
  };
}
async function openPage(calls: MockCallHandlers = {}, path = "/settings/prompts", browser = false) {
  const view = document();
  if (browser) window.history.replaceState(null, "", path);
  const history = browser ? createBrowserHistory() : createMemoryHistory({ initialEntries: ["/settings", path] });
  const runtime = await createAppRuntime({ transport: createMockTransport({ calls: {
    appStatus: () => ok({ ...mockAppStatus, ui_state: { onboarding: { completed: true } } }),
    promptsList: () => ok({ items: [view.prompt], next_cursor: null }), promptGet: () => ok(view),
    promptPlaceholders: ({ kind }) => ok({ kind, allowed: ["{{character}}"], required: [], image_slots: [] }),
    promptValidate: () => ok({ missing_placeholders: [] }),
    charactersList: () => ok({ items: [], next_cursor: null }),
    personasList: () => ok({ items: [], next_cursor: null, default_persona: { persona_id: null, revision: 1, created_at: 1, updated_at: 1 } }),
    jobsList: () => ok({ items: [], next_cursor: null }),
    ...calls,
  } }), history, i18n: await createTestI18n("en") });
  await act(async () => { render(<App runtime={runtime} />); });
  return { history, runtime };
}
const sheet = () => within(screen.getAllByRole("dialog").at(-1)!);

describe("system prompt management", () => {
  it("loads every cursor, filters by purpose and hides runtime fragments", async () => {
    const list = vi.fn(({ cursor }) => ok({ items: cursor ? [document({ id: "memory", name: "Memory work", kind: "dynamic_memory_manager" }).prompt, document({ id: "runtime", name: "Internal fragment", kind: "runtime_text" }).prompt] : [document().prompt], next_cursor: cursor ? null : "next" }));
    await openPage({ promptsList: list });
    expect(await screen.findByRole("button", { name: /Memory work/ })).toBeTruthy();
    expect(list).toHaveBeenCalledTimes(2);
    expect(screen.queryByText("Internal fragment")).toBeNull();
    fireEvent.click(screen.getByRole("radio", { name: "Memory" }));
    expect(screen.queryByRole("button", { name: /Story/ })).toBeNull();
    expect(screen.getByRole("button", { name: /Memory work/ })).toBeTruthy();
  });
  it("preserves nested conditions and placement fields while editing and reordering", async () => {
    const update = vi.fn(() => ok(document({ revision: 8 })));
    await openPage({ promptUpdate: update }, "/settings/prompts?prompt=p1");
    fireEvent.click(await screen.findByRole("button", { name: /Direction/ }));
    fireEvent.change(sheet().getByLabelText("Content"), { target: { value: "Changed {{character}}" } });
    fireEvent.click(sheet().getByRole("button", { name: "Done" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.keyDown(screen.getByRole("button", { name: /Reorder, item 1/ }), { key: "ArrowDown", altKey: true });
    const save = screen.getByRole("button", { name: "Save" });
    await waitFor(() => expect((save as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(save);
    await waitFor(() => expect(update).toHaveBeenCalledTimes(1));
    const request = (update.mock.calls[0] as unknown as [{ expected_revision: number; prompt: { entries: Array<{ entry_id: string }> } }])[0];
    expect(request.expected_revision).toBe(7);
    expect(request.prompt.entries.map((entry) => entry.entry_id)).toEqual(["e2", "e1"]);
    expect(request.prompt.entries[1]).toMatchObject({ content: "Changed {{character}}", condition, conditional_min_messages: 8, interval_turns: 4 });
  });
  it("copies a built-in into an unsaved draft and saves its source with fresh entries", async () => {
    const source = document({ origin: { type: "built_in", key: "chat", protected: true, required: true, edited: false } });
    source.entries[0]!.system_prompt = true;
    const create = vi.fn((_request: PromptCreateRequest) => ok(document({ id: "copy", name: "Story (copy)" })));
    const { runtime } = await openPage({ promptsList: () => ok({ items: [source.prompt], next_cursor: null }), promptGet: () => ok(source), promptCreate: create });
    fireEvent.click((await screen.findAllByRole("button", { name: "New prompt" }))[0]!);
    fireEvent.click(sheet().getByRole("button", { name: /Story/ }));
    expect((await screen.findByLabelText(/^Name/) as HTMLInputElement).value).toBe("Story (copy)");
    expect(create).not.toHaveBeenCalled();
    const save = screen.getByRole("button", { name: "Create prompt" });
    await waitFor(() => expect((save as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(save);
    await waitFor(() => expect(create).toHaveBeenCalledTimes(1));
    expect(create.mock.calls[0]![0]).toMatchObject({ source_prompt_id: "p1", prompt: { name: "Story (copy)", entries: [{ entry_id: null, system_prompt: false }, { entry_id: null, system_prompt: false }] } });
    await waitFor(() => expect(runtime.router.state.location.search).toMatchObject({ prompt: "copy" }));
    await waitFor(() => expect(screen.queryByRole("button", { name: "Create prompt" })).toBeNull());
  });
  it("creates from blank after naming and writing its entry", async () => {
    const create = vi.fn((_request: PromptCreateRequest) => ok(document({ id: "created", name: "New story" })));
    await openPage({ promptCreate: create });
    fireEvent.click((await screen.findAllByRole("button", { name: "New prompt" }))[0]!);
    fireEvent.click(sheet().getByRole("button", { name: /Blank/ }));
    fireEvent.change(await screen.findByLabelText(/^Name/), { target: { value: "New story" } });
    fireEvent.click(screen.getByRole("button", { name: /Untitled entry/ }));
    fireEvent.change(sheet().getByLabelText("Content"), { target: { value: "Write the next beat" } });
    expect((sheet().getByRole("button", { name: "Done" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(sheet().getByLabelText(/^Name/), { target: { value: "Direction" } });
    fireEvent.click(sheet().getByRole("button", { name: "Done" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    const save = screen.getByRole("button", { name: "Create prompt" });
    await waitFor(() => expect((save as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(save);
    await waitFor(() => expect(create).toHaveBeenCalledTimes(1));
    expect(create.mock.calls[0]![0]).toMatchObject({ source_prompt_id: null, prompt: { name: "New story", entries: [{ entry_id: null, name: "Direction", content: "Write the next beat" }] } });
  });
  it("previews unsaved text through Rust without a write", async () => {
    const preview = vi.fn(({ draft }) => ok({ prompt_id: "p1", entries: [{ entry_id: "e1", name: "Direction", role: "system" as const, position: "relative" as const, depth: 0, text: draft!.entries[0]!.content }], skipped_entry_ids: ["e2"] }));
    const update = vi.fn(); const create = vi.fn();
    await openPage({ promptPreview: preview, promptUpdate: update, promptCreate: create }, "/settings/prompts?prompt=p1");
    fireEvent.click(await screen.findByRole("button", { name: /Direction/ }));
    fireEvent.change(sheet().getByLabelText("Content"), { target: { value: "Unsaved wording" } });
    fireEvent.click(sheet().getByRole("button", { name: "Done" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(screen.getByRole("tab", { name: "Preview" }));
    expect(await screen.findByText("Unsaved wording")).toBeTruthy();
    await waitFor(() => expect(preview).toHaveBeenCalled());
    expect(preview.mock.calls.at(-1)![0]).toMatchObject({ draft: { entries: [{ entry_id: "e1", content: "Unsaved wording" }, { entry_id: "e2" }] }, character_id: null, persona_id: null });
    expect(update).not.toHaveBeenCalled(); expect(create).not.toHaveBeenCalled();
  });
  it("keeps a failed draft and reuses its operation id on retry", async () => {
    let tries = 0;
    const update = vi.fn(() => ++tries === 1 ? { status: "error" as const, error: { code: "unavailable" as const, message: "private diagnostics", details: null } } : ok(document({ name: "Retry me", revision: 8 })));
    await openPage({ promptUpdate: update }, "/settings/prompts?prompt=p1");
    fireEvent.change(await screen.findByLabelText(/^Name/), { target: { value: "Retry me" } });
    const save = screen.getByRole("button", { name: "Save" });
    await waitFor(() => expect((save as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(save);
    expect(await screen.findByText("It isn't available right now.")).toBeTruthy();
    expect(screen.queryByText("private diagnostics")).toBeNull();
    expect((screen.getByLabelText(/^Name/) as HTMLInputElement).value).toBe("Retry me");
    fireEvent.click(save);
    await waitFor(() => expect(update).toHaveBeenCalledTimes(2));
    expect(update.mock.calls[0]).toEqual(update.mock.calls[1]);
  });
  it("closes child views on Back and protects the editor's unsaved changes", async () => {
    const { history } = await openPage({}, "/settings/prompts", true);
    fireEvent.change(await screen.findByRole("textbox", { name: /Search/ }), { target: { value: "Story" } });
    fireEvent.click(screen.getByRole("button", { name: /Story/ }));
    fireEvent.click(await screen.findByRole("button", { name: /Direction/ }));
    await screen.findByRole("dialog");
    await act(async () => { history.back(); });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(history.location.search).toContain("prompt=p1");
    fireEvent.change(screen.getByLabelText(/^Name/), { target: { value: "Unsaved name" } });
    await act(async () => { history.back(); });
    fireEvent.click(await screen.findByRole("button", { name: "Keep editing" }));
    expect((screen.getByLabelText(/^Name/) as HTMLInputElement).value).toBe("Unsaved name");
    await act(async () => { history.back(); });
    fireEvent.click(sheet().getByRole("button", { name: "Discard" }));
    await waitFor(() => expect(screen.getByRole("textbox", { name: /Search/ })).toBeTruthy());
    expect((screen.getByRole("textbox", { name: /Search/ }) as HTMLInputElement).value).toBe("Story");
    history.destroy();
  });
});
