import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { toast } from "@lettuceai/crisp";
import { createBrowserHistory } from "@tanstack/react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { LibraryLorebookBinding, PersonaDefaultView, PersonaSaveRequest_Deserialize, PersonaView_Serialize } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";
import { newPersonaDraft } from "@/features/library";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(600);
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(600);
});
afterEach(() => {
  toast.dismiss();
  cleanup();
  vi.restoreAllMocks();
});

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const slot = (personaId: string | null, revision = 7): PersonaDefaultView => ({ persona_id: personaId, revision, created_at: 1, updated_at: 1 });

function persona(patch: Partial<PersonaView_Serialize> = {}): PersonaView_Serialize {
  return { id: "p1", archived: false, draft: newPersonaDraft("Hunter", "A quiet observer"), assets: [], revision: 5, created_at: 1, updated_at: 1, ...patch };
}

function binding(id: string, ordinal: number): LibraryLorebookBinding {
  return { lorebook_id: id, enabled: true, ordinal, revision: 1, created_at: 1, updated_at: 1 };
}

const books = ["a", "b", "c"].map((id) => ({ id, name: `Book ${id}`, status: "active" as const, detection: "recent_messages" as const, icon: null, revision: 1, created_at: 1, updated_at: 1 }));

function lorebookBackend(initial: string[], start = 5) {
  let ids = [...initial];
  let revision = start;
  const log: string[] = [];
  const result = () => ({ bindings: ids.map((id, index) => binding(id, index)), owner_revision: ++revision });
  const bind = vi.fn<NonNullable<MockCallHandlers["personaLorebooksBind"]>>((request) => {
    log.push(`bind ${request.lorebook_id}@${request.target_ordinal} rev ${request.expected_revision}`);
    ids.splice(request.target_ordinal ?? ids.length, 0, request.lorebook_id);
    return ok(result());
  });
  const unbind = vi.fn<NonNullable<MockCallHandlers["personaLorebooksUnbind"]>>((request) => {
    log.push(`unbind ${request.lorebook_id} rev ${request.expected_revision}`);
    ids = ids.filter((id) => id !== request.lorebook_id);
    return ok(result());
  });
  const reorder = vi.fn<NonNullable<MockCallHandlers["personaLorebooksReorder"]>>((request) => {
    log.push(`reorder ${request.lorebook_id}@${request.target_ordinal} rev ${request.expected_revision}`);
    ids = ids.filter((id) => id !== request.lorebook_id);
    ids.splice(request.target_ordinal, 0, request.lorebook_id);
    return ok(result());
  });
  return { log, order: () => ids, list: () => ok(ids.map((id, index) => binding(id, index))), calls: { personaLorebooksBind: bind, personaLorebooksUnbind: unbind, personaLorebooksReorder: reorder } };
}

async function renderEdit(view: PersonaView_Serialize, calls: MockCallHandlers = {}) {
  window.history.replaceState(null, "", "/");
  const history = createBrowserHistory();
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: () => ok({ ...mockAppStatus, ui_state: { onboarding: { completed: true } } }),
        personaGet: () => ok(view),
        personaLorebooksList: () => ok([]),
        personasList: () => ok({ items: [], next_cursor: null, default_persona: slot(null) }),
        charactersList: () => ok({ items: [], next_cursor: null }),
        conversationsList: () => ok({ items: [], next_cursor: null }),
        conversationsLatestByCharacter: () => ok({ items: [], next_cursor: null }),
        libraryCounts: () => ok({ characters: 0, personas: 1, lorebooks: 3, images: 0, audio: 0 }),
        lorebooksList: () => ok({ items: books, next_cursor: null }),
        ...calls,
      },
    }),
    history,
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  await act(async () => {
    history.push("/personas/p1/edit");
  });
  await screen.findByRole("textbox", { name: "Name" });
  return { history, runtime };
}

const nameInput = () => screen.getByRole("textbox", { name: "Name" }) as HTMLInputElement;
const descriptionInput = () => screen.getByRole("textbox", { name: "Description" }) as HTMLTextAreaElement;
const saveButton = () => screen.queryByRole("button", { name: "Save" }) as HTMLButtonElement | null;
const defaultSwitch = () => screen.getByRole("switch", { name: "Use as default" }) as HTMLButtonElement;

function saved(request: PersonaSaveRequest_Deserialize, revision = 6): PersonaView_Serialize {
  return { id: "p1", archived: false, draft: request.draft as PersonaView_Serialize["draft"], assets: [], revision, created_at: 1, updated_at: 2 };
}

async function pickBooks(...names: string[]) {
  fireEvent.click(await screen.findByRole("button", { name: /^Lorebooks/ }));
  const sheet = await screen.findByRole("dialog");
  for (const name of names) fireEvent.click(await within(sheet).findByRole("checkbox", { name }));
  fireEvent.click(within(sheet).getByRole("button", { name: "Done" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
}

describe("persona edit page", () => {
  it("shows the stored fields and lorebooks and the persona name as the title", async () => {
    const backend = lorebookBackend(["a", "b"]);
    const view = persona();
    view.draft.nickname = "Hunt";
    view.draft.design_description = "Grey coat";
    await renderEdit(view, { personaLorebooksList: backend.list });
    expect(nameInput().value).toBe("Hunter");
    expect((screen.getByRole("textbox", { name: "Nickname" }) as HTMLInputElement).value).toBe("Hunt");
    expect(descriptionInput().value).toBe("A quiet observer");
    expect((screen.getByRole("textbox", { name: "Design note" }) as HTMLTextAreaElement).value).toBe("Grey coat");
    expect((await screen.findByRole("button", { name: /^Lorebooks/ })).textContent).toContain("2 lorebooks");
    expect(screen.getByRole("heading", { name: "Hunter" })).toBeTruthy();
    expect(defaultSwitch().getAttribute("aria-checked")).toBe("false");
  });

  it("offers saving only after a change and discards back to the stored values", async () => {
    await renderEdit(persona());
    expect(saveButton()).toBeNull();
    fireEvent.change(nameInput(), { target: { value: "Hunt" } });
    expect(await screen.findByRole("button", { name: "Save" })).toBeTruthy();
    expect(screen.getByText("Unsaved changes")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Discard" }));
    await waitFor(() => expect(saveButton()).toBeNull());
    expect(nameInput().value).toBe("Hunter");
  });

  it("cannot save without a name or a description", async () => {
    await renderEdit(persona());
    fireEvent.change(nameInput(), { target: { value: " " } });
    expect((await screen.findByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(nameInput(), { target: { value: "Hunt" } });
    expect(saveButton()!.disabled).toBe(false);
    fireEvent.change(descriptionInput(), { target: { value: "  " } });
    expect(saveButton()!.disabled).toBe(true);
  });

  it("saves against the loaded revision, keeps the operation id until the payload changes and refreshes the baseline", async () => {
    let fail = true;
    const save = vi.fn((request: PersonaSaveRequest_Deserialize) => fail ? { status: "error" as const, error: { code: "unavailable" as const, message: "down", details: null } } : ok(saved(request)));
    await renderEdit(persona(), { personaSave: save });
    fireEvent.change(nameInput(), { target: { value: "  Hunt  " } });
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0]![0]).toMatchObject({ persona_id: "p1", expected_revision: 5, draft: { title: "Hunt", description: "A quiet observer" } });
    await waitFor(() => expect(saveButton()!.disabled).toBe(false));
    fireEvent.click(saveButton()!);
    await waitFor(() => expect(save).toHaveBeenCalledTimes(2));
    expect(save.mock.calls[1]![0].client_operation_id).toBe(save.mock.calls[0]![0].client_operation_id);
    await waitFor(() => expect(saveButton()!.disabled).toBe(false));
    fireEvent.change(nameInput(), { target: { value: "Hunter Two" } });
    fail = false;
    fireEvent.click(saveButton()!);
    await waitFor(() => expect(save).toHaveBeenCalledTimes(3));
    expect(save.mock.calls[2]![0].client_operation_id).not.toBe(save.mock.calls[0]![0].client_operation_id);
    expect(await screen.findByText("Changes saved")).toBeTruthy();
    await waitFor(() => expect(saveButton()).toBeNull());
    expect(nameInput().value).toBe("Hunter Two");
    fireEvent.change(nameInput(), { target: { value: "Hunter Three" } });
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(4));
    expect(save.mock.calls[3]![0].expected_revision).toBe(6);
  });

  it("saves the persona first, then binds, unbinds and reorders lorebooks on the returned revision", async () => {
    const backend = lorebookBackend(["a", "b"], 6);
    const save = vi.fn((request: PersonaSaveRequest_Deserialize) => {
      backend.log.push(`save rev ${request.expected_revision}`);
      return ok(saved(request, 6));
    });
    await renderEdit(persona(), { ...backend.calls, personaLorebooksList: backend.list, personaSave: save });
    fireEvent.change(nameInput(), { target: { value: "Hunt" } });
    await pickBooks("Book a", "Book c");
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    await waitFor(() => expect(saveButton()).toBeNull());
    expect(backend.log).toEqual(["save rev 5", "unbind a rev 6", "bind c@1 rev 7"]);
    expect(backend.order()).toEqual(["b", "c"]);
    fireEvent.change(nameInput(), { target: { value: "Hunter Two" } });
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(2));
    expect(backend.log.at(-1)).toBe("save rev 8");
  });

  it("only changes lorebooks without saving the persona and reorders", async () => {
    const backend = lorebookBackend(["a", "b"]);
    const save = vi.fn();
    await renderEdit(persona(), { ...backend.calls, personaLorebooksList: backend.list, personaSave: save });
    await pickBooks("Book a", "Book c", "Book a");
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    await waitFor(() => expect(saveButton()).toBeNull());
    expect(save).not.toHaveBeenCalled();
    expect(backend.log).toEqual(["bind c@1 rev 5", "reorder b@0 rev 6", "reorder c@1 rev 7"]);
    expect(backend.order()).toEqual(["b", "c", "a"]);
  });

  it("keeps the picked lorebooks and the bar when a lorebook step fails", async () => {
    let server = persona();
    const bind = vi.fn<NonNullable<MockCallHandlers["personaLorebooksBind"]>>(() => ({ status: "error" as const, error: { code: "unavailable" as const, message: "down", details: null } }));
    const save = vi.fn((request: PersonaSaveRequest_Deserialize) => {
      server = saved(request, 6);
      return ok(server);
    });
    await renderEdit(persona(), { personaSave: save, personaLorebooksBind: bind, personaGet: () => ok(save.mock.calls.length > 0 ? { ...server, revision: 7 } : server) });
    fireEvent.change(nameInput(), { target: { value: "Hunt" } });
    await pickBooks("Book a");
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    expect(await screen.findByText("Saved, but the lorebooks could not be updated. Try saving again.")).toBeTruthy();
    expect(bind.mock.calls[0]![0]).toMatchObject({ expected_revision: 6 });
    expect(screen.getByRole("button", { name: /^Lorebooks/ }).textContent).toContain("1 lorebook");
    expect(saveButton()).toBeTruthy();
  });

  it("keeps the edits, says so and reloads the persona when it changed elsewhere", async () => {
    let revision = 5;
    const save = vi.fn((request: PersonaSaveRequest_Deserialize) => request.expected_revision === 5
      ? { status: "error" as const, error: { code: "conflict" as const, message: "stale revision", details: null } }
      : ok(saved(request, 9)));
    const get = vi.fn(() => ok(persona({ revision })));
    await renderEdit(persona(), { personaSave: save, personaGet: get });
    const loads = get.mock.calls.length;
    revision = 8;
    fireEvent.change(nameInput(), { target: { value: "Hunt" } });
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    expect(await screen.findByText("It changed somewhere else. Reload and try again.")).toBeTruthy();
    expect(screen.queryByText("stale revision")).toBeNull();
    await waitFor(() => expect(get.mock.calls.length).toBe(loads + 1));
    expect(nameInput().value).toBe("Hunt");
    await waitFor(() => expect(saveButton()!.disabled).toBe(false));
    fireEvent.click(saveButton()!);
    await waitFor(() => expect(save).toHaveBeenCalledTimes(2));
    expect(save.mock.calls[1]![0].expected_revision).toBe(8);
  });

  it("rebases onto the fresh persona after a conflict so other changes are not overwritten", async () => {
    let fresh = false;
    const save = vi.fn((request: PersonaSaveRequest_Deserialize) => request.expected_revision === 5
      ? { status: "error" as const, error: { code: "conflict" as const, message: "stale revision", details: null } }
      : ok(saved(request, 9)));
    const get = vi.fn(() => {
      if (!fresh) return ok(persona());
      const view = persona({ revision: 8 });
      view.draft.nickname = "Fresh";
      return ok(view);
    });
    await renderEdit(persona(), { personaSave: save, personaGet: get });
    fresh = true;
    fireEvent.change(descriptionInput(), { target: { value: "Mine" } });
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    await waitFor(() => expect((screen.getByRole("textbox", { name: "Nickname" }) as HTMLInputElement).value).toBe("Fresh"));
    expect(descriptionInput().value).toBe("Mine");
    await waitFor(() => expect(saveButton()!.disabled).toBe(false));
    fireEvent.click(saveButton()!);
    await waitFor(() => expect(save).toHaveBeenCalledTimes(2));
    expect(save.mock.calls[1]![0]).toMatchObject({ expected_revision: 8, draft: { nickname: "Fresh", description: "Mine" } });
  });

  it("turns the default on with the slot revision and clears it again", async () => {
    let current = slot(null);
    const set = vi.fn<NonNullable<MockCallHandlers["personaDefaultSet"]>>((request) => {
      current = slot(request.persona_id, request.expected_revision + 1);
      return ok(current);
    });
    const clear = vi.fn<NonNullable<MockCallHandlers["personaDefaultClear"]>>((request) => {
      current = slot(null, request.expected_revision + 1);
      return ok(current);
    });
    await renderEdit(persona(), {
      personasList: () => ok({ items: [], next_cursor: null, default_persona: current }),
      personaDefaultSet: set,
      personaDefaultClear: clear,
    });
    fireEvent.click(defaultSwitch());
    await waitFor(() => expect(set).toHaveBeenCalledTimes(1));
    expect(set.mock.calls[0]![0]).toMatchObject({ persona_id: "p1", expected_revision: 7 });
    expect(saveButton()).toBeNull();
    await waitFor(() => expect(defaultSwitch().disabled).toBe(false));
    expect(defaultSwitch().getAttribute("aria-checked")).toBe("true");
    fireEvent.click(defaultSwitch());
    await waitFor(() => expect(clear).toHaveBeenCalledTimes(1));
    expect(clear.mock.calls[0]![0].expected_revision).toBe(8);
    expect(clear.mock.calls[0]![0].client_operation_id).not.toBe(set.mock.calls[0]![0].client_operation_id);
    await waitFor(() => expect(defaultSwitch().getAttribute("aria-checked")).toBe("false"));
  });

  it("starts on when the persona is already the default", async () => {
    await renderEdit(persona(), { personasList: () => ok({ items: [], next_cursor: null, default_persona: slot("p1") }) });
    expect(defaultSwitch().getAttribute("aria-checked")).toBe("true");
  });

  it("rolls the default switch back and says why when the change fails", async () => {
    const set = vi.fn(() => ({ status: "error" as const, error: { code: "conflict" as const, message: "stale", details: null } }));
    await renderEdit(persona(), { personaDefaultSet: set });
    fireEvent.click(defaultSwitch());
    expect(await screen.findByText("It changed somewhere else. Reload and try again.")).toBeTruthy();
    await waitFor(() => expect(defaultSwitch().getAttribute("aria-checked")).toBe("false"));
    await waitFor(() => expect(defaultSwitch().disabled).toBe(false));
  });

  it("deletes after confirming and lands on the library without a history entry for the persona", async () => {
    const remove = vi.fn<NonNullable<MockCallHandlers["personaDelete"]>>(() => ok(null));
    const { history, runtime } = await renderEdit(persona(), { personaDelete: remove });
    fireEvent.click(screen.getByRole("button", { name: "More actions" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Delete persona" }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("The persona is removed from chats that use it.")).toBeTruthy();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(remove).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "More actions" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Delete persona" }));
    fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(remove).toHaveBeenCalledTimes(1));
    expect(remove.mock.calls[0]![0]).toMatchObject({ persona_id: "p1", expected_revision: 5 });
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/library"));
    await act(async () => history.back());
    await waitFor(() => expect(history.location.pathname).toBe("/"));
  });

  it("deletes without asking about unsaved edits", async () => {
    const remove = vi.fn(() => ok(null));
    const { runtime } = await renderEdit(persona(), { personaDelete: remove });
    fireEvent.change(nameInput(), { target: { value: "Hunt" } });
    fireEvent.click(screen.getByRole("button", { name: "More actions" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Delete persona" }));
    fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/library"));
    expect(screen.queryByText("Discard your changes?")).toBeNull();
  });

  it("exports the persona and reports warnings", async () => {
    const exported = vi.fn(() => ok({ warnings: ["x"] } as never));
    await renderEdit(persona(), {
      filesPickSave: () => ok({ target: { uri: "/tmp/Hunter.uec" } } as never),
      personaExport: exported,
    });
    fireEvent.click(screen.getByRole("button", { name: "More actions" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Export" }));
    await waitFor(() => expect(exported).toHaveBeenCalledWith(expect.objectContaining({ id: "p1" })));
    expect(await screen.findByText("Exported with 1 warning")).toBeTruthy();
  });

  it("asks before leaving with unsaved changes and leaves on discard", async () => {
    const { history } = await renderEdit(persona());
    fireEvent.change(nameInput(), { target: { value: "Hunt" } });
    await screen.findByRole("button", { name: "Save" });
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("Discard your changes?")).toBeTruthy();
    fireEvent.click(within(dialog).getByRole("button", { name: "Keep editing" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(history.location.pathname).toBe("/personas/p1/edit");
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Discard" }));
    await waitFor(() => expect(history.location.pathname).toBe("/"));
  });

  it("leaves without asking when nothing changed", async () => {
    const { history } = await renderEdit(persona());
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    await waitFor(() => expect(history.location.pathname).toBe("/"));
  });
});

describe("persona edit loading", () => {
  it("offers a way back to the library when the persona is gone", async () => {
    window.history.replaceState(null, "", "/");
    const history = createBrowserHistory();
    const runtime = await createAppRuntime({
      transport: createMockTransport({
        calls: {
          appStatus: () => ok({ ...mockAppStatus, ui_state: { onboarding: { completed: true } } }),
          personaGet: () => ({ status: "error" as const, error: { code: "not_found" as const, message: "gone", details: null } }),
          personaLorebooksList: () => ok([]),
          personasList: () => ok({ items: [], next_cursor: null, default_persona: slot(null) }),
          charactersList: () => ok({ items: [], next_cursor: null }),
          conversationsList: () => ok({ items: [], next_cursor: null }),
          libraryCounts: () => ok({ characters: 0, personas: 0, lorebooks: 0, images: 0, audio: 0 }),
          lorebooksList: () => ok({ items: [], next_cursor: null }),
        },
      }),
      history,
      i18n: await createTestI18n("en"),
    });
    await act(async () => {
      render(<App runtime={runtime} />);
    });
    await act(async () => {
      history.push("/personas/missing/edit");
    });
    expect(await screen.findByText("It no longer exists.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Back to library" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/library"));
  });
});
