import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { toast } from "@lettuceai/crisp";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  LorebookEntriesMutateRequest,
  LorebookEntryView,
  LorebookView_Serialize,
} from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({
  createWindowController: vi.fn().mockResolvedValue(null),
}));
beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(600);
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(1000);
});
afterEach(() => {
  toast.dismiss();
  cleanup();
  vi.restoreAllMocks();
});

const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const failure = (code: "conflict" | "unavailable") => ({
  status: "error" as const,
  error: { code, message: "diagnostic", details: null },
});
const entry = (id: string, patch: Partial<LorebookEntryView> = {}): LorebookEntryView => ({
  id,
  title: id,
  content: `${id} content`,
  keywords: [`${id.toLowerCase()} key`],
  enabled: true,
  always_active: false,
  case_sensitive: false,
  keyword_mode: "literal",
  priority: 3,
  ordinal: 0,
  revision: 1,
  created_at: 1,
  updated_at: 1,
  ...patch,
});
const initial = (): LorebookView_Serialize => ({
  lorebook: {
    id: "book",
    name: "World of Welan",
    status: "active",
    detection: "recent_messages",
    icon: { asset_id: "icon-1", url: "https://example.test/icon.png" } as never,
    revision: 4,
    created_at: 1,
    updated_at: 1,
  },
  entries: [
    entry("Gate"),
    entry("Crown", { always_active: true, keywords: [] }),
    entry("Vials", { enabled: false }),
  ],
});

function backend() {
  let view = initial();
  const writes: LorebookEntriesMutateRequest[] = [];
  const apply = (request: LorebookEntriesMutateRequest) => {
    writes.push(structuredClone(request));
    let entries = [...view.entries];
    for (const mutation of request.mutations) {
      if (mutation.type === "update")
        entries = entries.map((item) =>
          item.id === mutation.entry_id ? { ...item, ...mutation.entry } : item,
        );
      else if (mutation.type === "remove")
        entries = entries.filter((item) => item.id !== mutation.entry_id);
      else if (mutation.type === "add")
        entries.push(entry(`added-${writes.length}`, { ...mutation.entry }));
      else {
        const from = entries.findIndex((item) => item.id === mutation.entry_id);
        const [moved] = entries.splice(from, 1);
        entries.splice(mutation.index, 0, moved!);
      }
    }
    view = {
      lorebook: { ...view.lorebook, revision: view.lorebook.revision + 1 },
      entries,
    };
    return view;
  };
  return {
    writes,
    current: () => view,
    apply,
    replace: (next: LorebookView_Serialize) => {
      view = next;
    },
  };
}

async function setup(
  calls: MockCallHandlers = {},
  platform: "other" | "android" = "other",
  store = backend(),
) {
  const requests = { get: 0 };
  const transport = createMockTransport({
    calls: {
      appStatus: () =>
        ok({ ...mockAppStatus, platform, ui_state: { onboarding: { completed: true } } }),
      settingsGet: () => ok({ global: { dynamic_memory: { enabled: false } } } as never),
      libraryCounts: () => ok({ characters: 0, personas: 0, lorebooks: 1, images: 0, audio: 0 }),
      charactersList: () => ok({ items: [], next_cursor: null }),
      personasList: () =>
        ok({
          items: [],
          next_cursor: null,
          default_persona: { persona_id: null, revision: 1, created_at: 1, updated_at: 1 },
        }),
      lorebooksList: () => ok({ items: [store.current().lorebook as never], next_cursor: null }),
      lorebookProjectsList: () => ok({ items: [], next_cursor: null }),
      mediaLibraryList: () => ok({ items: [], next_cursor: null }),
      conversationsLatestByCharacter: () => ok({ items: [], next_cursor: null }),
      conversationsList: () => ok({ items: [], next_cursor: null }),
      lorebookGet: () => {
        requests.get += 1;
        return ok(store.current());
      },
      lorebookEntriesMutate: (request) => ok(store.apply(request)),
      ...calls,
    },
  });
  const history = createMemoryHistory({ initialEntries: ["/library/lorebooks/book"] });
  const runtime = await createAppRuntime({
    transport,
    history,
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return { runtime, history, store, requests };
}

const rowSwitch = (title: string) => screen.getByRole("switch", { name: `${title} enabled` });
const titles = () =>
  screen
    .getAllByRole("switch")
    .map((item) => item.getAttribute("aria-label")!.replace(/ enabled$/, ""));

describe("lorebook editor page", () => {
  it("lists entries with their keywords and live count, and search filters them", async () => {
    await setup();
    expect(await screen.findByText("Gate content")).toBeTruthy();
    expect(screen.getByText("World of Welan")).toBeTruthy();
    expect(screen.getByText("2 of 3 live")).toBeTruthy();
    expect(screen.getByText("gate key")).toBeTruthy();
    expect(screen.getByText("In every prompt")).toBeTruthy();
    expect(screen.getByText("Always")).toBeTruthy();
    expect(
      screen.getByText("Drag to reorder. Earlier entries win when the prompt runs out of room."),
    ).toBeTruthy();
    expect(titles()).toEqual(["Gate", "Crown", "Vials"]);
    const search = screen.getByPlaceholderText("Search entries and keywords");
    fireEvent.change(search, { target: { value: "vials key" } });
    await waitFor(() => expect(titles()).toEqual(["Vials"]));
    expect(screen.queryByText(/Drag to reorder/)).toBeNull();
    fireEvent.change(search, { target: { value: "zzz" } });
    expect(await screen.findByText("No entries match")).toBeTruthy();
    fireEvent.change(search, { target: { value: "crown content" } });
    await waitFor(() => expect(titles()).toEqual(["Crown"]));
  });

  it("shows the empty state with a way to add the first entry", async () => {
    await setup({ lorebookGet: () => ok({ ...initial(), entries: [] }) });
    expect(await screen.findByText("No entries yet")).toBeTruthy();
    expect(screen.getAllByRole("button", { name: "New entry" }).length).toBeGreaterThan(1);
  });

  it("says the lorebook is gone and offers the way back", async () => {
    const { runtime } = await setup({ lorebookGet: () => failure("conflict") as never });
    expect(
      await screen.findByText("It changed somewhere else. Reload and try again."),
    ).toBeTruthy();
    fireEvent.click(
      within(document.querySelector("header")!).getByRole("button", { name: "Library" }),
    );
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/library"));
  });

  it("sends an update for the enable switch and applies the stored result", async () => {
    const { store } = await setup();
    await screen.findByText("Gate content");
    fireEvent.click(rowSwitch("Vials"));
    await waitFor(() => expect(store.writes).toHaveLength(1));
    expect(store.writes[0]).toMatchObject({
      lorebook_id: "book",
      expected_revision: 4,
      mutations: [{ type: "update", entry_id: "Vials", entry: { enabled: true, priority: 3 } }],
    });
    await waitFor(() => expect(rowSwitch("Vials").getAttribute("aria-checked")).toBe("true"));
    expect(screen.getByText("3 of 3 live")).toBeTruthy();
  });

  it("rolls the switch back and says why when the write fails", async () => {
    await setup({ lorebookEntriesMutate: () => failure("unavailable") });
    await screen.findByText("Gate content");
    fireEvent.click(rowSwitch("Gate"));
    expect(await screen.findByText("It isn't available right now.")).toBeTruthy();
    await waitFor(() => expect(rowSwitch("Gate").getAttribute("aria-checked")).toBe("true"));
  });

  it("refetches the lorebook and says so when a write conflicts", async () => {
    const { requests } = await setup({ lorebookEntriesMutate: () => failure("conflict") });
    await screen.findByText("Gate content");
    const before = requests.get;
    fireEvent.click(rowSwitch("Gate"));
    expect(
      await screen.findByText("It changed somewhere else. Reload and try again."),
    ).toBeTruthy();
    await waitFor(() => expect(requests.get).toBeGreaterThan(before));
    await waitFor(() => expect(rowSwitch("Gate").getAttribute("aria-checked")).toBe("true"));
  });

  it("chains the revisions of back to back writes", async () => {
    const { store } = await setup();
    await screen.findByText("Gate content");
    fireEvent.click(rowSwitch("Gate"));
    fireEvent.click(rowSwitch("Crown"));
    await waitFor(() => expect(store.writes).toHaveLength(2));
    expect(store.writes.map((write) => write.expected_revision)).toEqual([4, 5]);
    expect(rowSwitch("Gate").getAttribute("aria-checked")).toBe("false");
    await waitFor(() => expect(rowSwitch("Crown").getAttribute("aria-checked")).toBe("false"));
  });

  it("adds a new entry at the end", async () => {
    const { store } = await setup();
    await screen.findByText("Gate content");
    fireEvent.click(screen.getByRole("button", { name: "New entry" }));
    const dialog = await screen.findByRole("dialog");
    const save = within(dialog).getByRole("button", { name: "Save entry" }) as HTMLButtonElement;
    expect(save.disabled).toBe(true);
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Title" }), {
      target: { value: "The war" },
    });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Content" }), {
      target: { value: "Ended three months ago." },
    });
    expect(within(dialog).getByText("4 words. What the model learns when this fires")).toBeTruthy();
    fireEvent.click(save);
    await waitFor(() => expect(store.writes).toHaveLength(1));
    expect(store.writes[0]).toMatchObject({
      expected_revision: 4,
      mutations: [
        {
          type: "add",
          index: null,
          entry: {
            title: "The war",
            content: "Ended three months ago.",
            enabled: true,
            priority: 0,
          },
        },
      ],
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(await screen.findByText("Ended three months ago.")).toBeTruthy();
  });

  it("starts a second new entry empty", async () => {
    await setup();
    await screen.findByText("Gate content");
    fireEvent.click(screen.getByRole("button", { name: "New entry" }));
    fireEvent.change(await screen.findByRole("textbox", { name: "Content" }), {
      target: { value: "Half written" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(screen.getByRole("button", { name: "New entry" }));
    expect(
      ((await screen.findByRole("textbox", { name: "Content" })) as HTMLTextAreaElement).value,
    ).toBe("");
  });

  it("edits an entry with an update and keeps the draft when saving fails", async () => {
    const { store } = await setup();
    await screen.findByText("Gate content");
    fireEvent.click(screen.getByText("Gate content"));
    const content = await screen.findByRole("textbox", { name: "Content" });
    fireEvent.change(content, { target: { value: "Changed gate" } });
    fireEvent.click(screen.getByRole("button", { name: "Save entry" }));
    await waitFor(() => expect(store.writes).toHaveLength(1));
    expect(store.writes[0]).toMatchObject({
      expected_revision: 4,
      mutations: [
        { type: "update", entry_id: "Gate", entry: { content: "Changed gate", priority: 3 } },
      ],
    });
    expect(await screen.findByText("Changed gate")).toBeTruthy();
  });

  it("keeps the draft after a failed save", async () => {
    await setup({ lorebookEntriesMutate: () => failure("unavailable") });
    await screen.findByText("Gate content");
    fireEvent.click(screen.getByText("Gate content"));
    const content = await screen.findByRole("textbox", { name: "Content" });
    fireEvent.change(content, { target: { value: "Changed gate" } });
    fireEvent.click(screen.getByRole("button", { name: "Save entry" }));
    expect(await screen.findByRole("button", { name: "Try again" })).toBeTruthy();
    expect((content as HTMLTextAreaElement).value).toBe("Changed gate");
  });

  it("saves against the revision seen at open and offers a refresh when the book moved on", async () => {
    const store = backend();
    const { runtime } = await setup(
      {
        lorebookEntriesMutate: (request) =>
          request.expected_revision === store.current().lorebook.revision
            ? ok(store.apply(request))
            : failure("conflict"),
      },
      "other",
      store,
    );
    await screen.findByText("Gate content");
    fireEvent.click(screen.getByText("Gate content"));
    const content = await screen.findByRole("textbox", { name: "Content" });
    fireEvent.change(content, { target: { value: "Changed gate" } });
    store.replace({ ...store.current(), lorebook: { ...store.current().lorebook, revision: 7 } });
    await act(() => runtime.queryClient.refetchQueries());
    fireEvent.click(screen.getByRole("button", { name: "Save entry" }));
    const refresh = await screen.findByRole("button", { name: "Refresh revision" });
    expect(store.writes).toHaveLength(0);
    expect((content as HTMLTextAreaElement).value).toBe("Changed gate");
    fireEvent.click(refresh);
    await waitFor(() => expect(screen.queryByRole("button", { name: "Refresh revision" })).toBeNull());
    fireEvent.click(screen.getByRole("button", { name: "Save entry" }));
    await waitFor(() => expect(store.writes).toHaveLength(1));
    expect(store.writes[0]).toMatchObject({
      expected_revision: 7,
      mutations: [{ type: "update", entry_id: "Gate", entry: { content: "Changed gate" } }],
    });
  });

  it("opens the sheet with the stored entry while a switch write is still pending", async () => {
    await setup({ lorebookEntriesMutate: () => new Promise(() => undefined) });
    await screen.findByText("Gate content");
    fireEvent.click(rowSwitch("Gate"));
    expect(rowSwitch("Gate").getAttribute("aria-checked")).toBe("false");
    fireEvent.click(screen.getByText("Gate content"));
    const dialog = await screen.findByRole("dialog");
    expect(
      within(dialog).getByRole("switch", { name: "Enabled" }).getAttribute("aria-checked"),
    ).toBe("true");
  });

  it("cancels a running keyword suggestion when the sheet closes", async () => {
    let accept: (value: ReturnType<typeof ok<{ job_id: string }>>) => void = () => undefined;
    const cancel = vi.fn(() => ok(null));
    await setup({
      lorebookKeywordsDraft: () => new Promise((resolve) => (accept = resolve as never)),
      jobCancel: cancel,
    });
    await screen.findByText("Gate content");
    fireEvent.click(screen.getByText("Gate content"));
    fireEvent.click(await screen.findByRole("button", { name: "Suggest" }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await act(async () => accept(ok({ job_id: "job-1" })));
    await waitFor(() => expect(cancel).toHaveBeenCalledWith({ job_id: "job-1" }));
  });

  it("confirms before removing an entry from its sheet", async () => {
    const { store } = await setup();
    await screen.findByText("Gate content");
    fireEvent.click(screen.getByText("Gate content"));
    fireEvent.click(await screen.findByRole("button", { name: "Delete entry" }));
    await waitFor(() => expect(screen.getAllByRole("dialog").length).toBeGreaterThan(1));
    const confirmation = screen.getAllByRole("dialog").at(-1)!;
    expect(
      within(confirmation).getByText("It comes out of the lorebook. This cannot be undone."),
    ).toBeTruthy();
    fireEvent.click(within(confirmation).getByRole("button", { name: "Keep" }));
    await waitFor(() => expect(screen.getAllByRole("dialog")).toHaveLength(1));
    expect(store.writes).toHaveLength(0);
    fireEvent.click(screen.getByRole("button", { name: "Delete entry" }));
    await waitFor(() => expect(screen.getAllByRole("dialog").length).toBeGreaterThan(1));
    const again = screen.getAllByRole("dialog").at(-1)!;
    fireEvent.click(within(again).getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(store.writes).toHaveLength(1));
    expect(store.writes[0]).toMatchObject({
      expected_revision: 4,
      mutations: [{ type: "remove", entry_id: "Gate" }],
    });
    await waitFor(() => expect(titles()).toEqual(["Crown", "Vials"]));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("sends one reorder with the final index", async () => {
    const { store } = await setup();
    await screen.findByText("Gate content");
    fireEvent.keyDown(screen.getByRole("button", { name: /Reorder, item 1/ }), {
      key: "ArrowDown",
      altKey: true,
    });
    await waitFor(() => expect(store.writes).toHaveLength(1));
    expect(store.writes[0]).toMatchObject({
      expected_revision: 4,
      mutations: [{ type: "reorder", entry_id: "Crown", index: 0 }],
    });
    await waitFor(() => expect(titles()).toEqual(["Crown", "Gate", "Vials"]));
  });

  it("rolls the order back when a reorder fails", async () => {
    await setup({ lorebookEntriesMutate: () => failure("unavailable") });
    await screen.findByText("Gate content");
    fireEvent.keyDown(screen.getByRole("button", { name: /Reorder, item 1/ }), {
      key: "ArrowDown",
      altKey: true,
    });
    expect(await screen.findByText("It isn't available right now.")).toBeTruthy();
    await waitFor(() => expect(titles()).toEqual(["Gate", "Crown", "Vials"]));
  });

  it("saves the name on blur only when it changed and keeps the stored detection and icon", async () => {
    const store = backend();
    const update = vi.fn<NonNullable<MockCallHandlers["lorebookUpdateMetadata"]>>((request) => {
      const next = {
        lorebook: { ...store.current().lorebook, name: request.metadata.name, revision: 5 },
        entries: store.current().entries,
      };
      store.replace(next);
      return ok(next);
    });
    await setup({ lorebookUpdateMetadata: update }, "other", store);
    await screen.findByText("Gate content");
    fireEvent.click(screen.getByRole("button", { name: "Lorebook settings" }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).queryByRole("button", { name: "Done" })).toBeNull();
    const name = within(dialog).getByRole("textbox", { name: "Name" });
    fireEvent.focus(name);
    fireEvent.blur(name);
    expect(update).not.toHaveBeenCalled();
    fireEvent.change(name, { target: { value: "Welan atlas" } });
    fireEvent.blur(name);
    await waitFor(() => expect(update).toHaveBeenCalledTimes(1));
    expect(update.mock.calls[0]![0]).toMatchObject({
      lorebook_id: "book",
      expected_revision: 4,
      metadata: { name: "Welan atlas", detection: "recent_messages", icon_asset_id: "icon-1" },
    });
    expect(await screen.findByText("Welan atlas")).toBeTruthy();
  });

  it("reverts the name on Escape without writing", async () => {
    const update = vi.fn(() => ok(initial()));
    await setup({ lorebookUpdateMetadata: update });
    await screen.findByText("Gate content");
    fireEvent.click(screen.getByRole("button", { name: "Lorebook settings" }));
    const name = (await within(await screen.findByRole("dialog")).findByRole("textbox", {
      name: "Name",
    })) as HTMLInputElement;
    fireEvent.change(name, { target: { value: "Something else" } });
    fireEvent.keyDown(name, { key: "Escape" });
    expect(name.value).toBe("World of Welan");
    fireEvent.blur(name);
    expect(update).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog")).toBeTruthy();
  });

  it("deletes the lorebook after confirming and replaces the editor with the library", async () => {
    const remove = vi.fn<NonNullable<MockCallHandlers["lorebookDelete"]>>(() => ok(null as never));
    const { runtime, history } = await setup({ lorebookDelete: remove });
    await screen.findByText("Gate content");
    fireEvent.click(screen.getByRole("button", { name: "Lorebook settings" }));
    fireEvent.click(await screen.findByRole("button", { name: "Delete lorebook" }));
    await waitFor(() => expect(screen.getAllByRole("dialog").length).toBeGreaterThan(1));
    const confirmation = screen.getAllByRole("dialog").at(-1)!;
    expect(
      within(confirmation).getByText(
        "Characters and chats that use it lose access to its entries.",
      ),
    ).toBeTruthy();
    fireEvent.click(within(confirmation).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.getAllByRole("dialog")).toHaveLength(1));
    expect(remove).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Delete lorebook" }));
    await waitFor(() => expect(screen.getAllByRole("dialog").length).toBeGreaterThan(1));
    const again = screen.getAllByRole("dialog").at(-1)!;
    fireEvent.click(within(again).getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(remove).toHaveBeenCalledTimes(1));
    expect(remove.mock.calls[0]![0]).toMatchObject({ lorebook_id: "book", expected_revision: 4 });
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/library"));
    await act(async () => history.back());
    expect(history.location.pathname).not.toBe("/library/lorebooks/book");
  });

  it("opens the trigger preview as a pushed page", async () => {
    const { runtime } = await setup();
    await screen.findByText("Gate content");
    fireEvent.click(screen.getByRole("button", { name: "Trigger preview" }));
    await waitFor(() =>
      expect(runtime.router.state.location.pathname).toBe("/library/lorebooks/book/preview"),
    );
  });

  it("offers UEC first and sends the chosen export format from the desktop button", async () => {
    const exported = vi.fn<NonNullable<MockCallHandlers["lorebookExport"]>>(() =>
      ok({ warnings: [] } as never),
    );
    const pick = vi.fn<NonNullable<MockCallHandlers["filesPickSave"]>>(() =>
      ok({ target: { uri: "/tmp/World.uec" } } as never),
    );
    await setup({ filesPickSave: pick, lorebookExport: exported });
    await screen.findByText("Gate content");
    fireEvent.click(screen.getByRole("button", { name: "Export" }));
    const dialog = await screen.findByRole("dialog");
    const choices = within(dialog).getAllByRole("button", { name: /^(UEC|World Info JSON)/ });
    expect(choices[0]!.textContent).toContain("UEC");
    expect(choices[1]!.textContent).toContain("World Info JSON");
    fireEvent.click(choices[0]!);
    await waitFor(() => expect(exported).toHaveBeenCalledTimes(1));
    expect(exported.mock.calls[0]![0]).toMatchObject({ id: "book", format: "uec" });
    expect(pick.mock.calls[0]![0]).toMatchObject({
      suggested_name: "World_of_Welan.uec",
      kind: "any",
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(screen.getByRole("button", { name: "Export" }));
    fireEvent.click(await screen.findByRole("button", { name: /World Info JSON/ }));
    await waitFor(() => expect(exported).toHaveBeenCalledTimes(2));
    expect(exported.mock.calls[1]![0]).toMatchObject({ id: "book", format: "world_info" });
    expect(pick.mock.calls[1]![0]).toMatchObject({
      suggested_name: "World_of_Welan.json",
      kind: "json",
    });
  });

  it("keeps export in a menu on the phone and shows the floating add button", async () => {
    const exported = vi.fn<NonNullable<MockCallHandlers["lorebookExport"]>>(() =>
      ok({ warnings: [] } as never),
    );
    await setup(
      {
        filesPickSave: () => ok({ target: { uri: "/tmp/World.uec" } } as never),
        lorebookExport: exported,
      },
      "android",
    );
    await screen.findByText("Gate content");
    expect(screen.queryByRole("button", { name: "Export" })).toBeNull();
    expect(screen.queryByText("2 of 3 live")).toBeNull();
    expect(screen.getAllByRole("button", { name: "New entry" })).toHaveLength(1);
    fireEvent.click(screen.getByRole("button", { name: "More" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Export" }));
    fireEvent.click(await screen.findByRole("button", { name: /UEC/ }));
    await waitFor(() => expect(exported).toHaveBeenCalledTimes(1));
    expect(exported.mock.calls[0]![0]).toMatchObject({ id: "book", format: "uec" });
  });
});
