import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { toast } from "@lettuceai/crisp";
import { createBrowserHistory } from "@tanstack/react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CharacterSaveRequest_Deserialize, CharacterView_Serialize, LibraryLorebookBinding } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers, type MockStreamHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";
import defaultDraft from "./fixtures/default-draft.json";
import companionDraft from "./fixtures/default-companion-draft.json";
import type { CharacterDraft } from "./data/form";
import type { CompanionNoteView, CompanionSoulDraft, JobView } from "@/api/generated/bindings";
import { draftToUpsert, newNoteDraft, noteProblem, noteToDraft } from "./data/notes";

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

function viewOf(base: unknown, patch: Partial<CharacterView_Serialize> = {}): CharacterView_Serialize {
  const draft = structuredClone(base as CharacterDraft);
  draft.profile.name = "Cassian";
  draft.profile.definition = "A quiet king";
  return { id: "c1", archived: false, draft, assets: [], revision: 5, created_at: 1, updated_at: 1, ...patch };
}

const roleplay = () => viewOf(defaultDraft);
const companion = () => viewOf(companionDraft);

function binding(id: string, ordinal: number): LibraryLorebookBinding {
  return { lorebook_id: id, enabled: true, ordinal, revision: 1, created_at: 1, updated_at: 1 };
}

const books = ["a", "b", "c"].map((id) => ({ id, name: `Book ${id}`, status: "active" as const, detection: "recent_messages" as const, icon: null, revision: 1, created_at: 1, updated_at: 1 }));

function lorebookBackend(initial: string[]) {
  let ids = [...initial];
  let revision = 5;
  const log: string[] = [];
  const result = () => ({ bindings: ids.map((id, index) => binding(id, index)), owner_revision: ++revision });
  const bind = vi.fn<NonNullable<MockCallHandlers["characterLorebooksBind"]>>((request) => {
    log.push(`bind ${request.lorebook_id}@${request.target_ordinal} rev ${request.expected_revision}`);
    ids.splice(request.target_ordinal ?? ids.length, 0, request.lorebook_id);
    return ok(result());
  });
  const unbind = vi.fn<NonNullable<MockCallHandlers["characterLorebooksUnbind"]>>((request) => {
    log.push(`unbind ${request.lorebook_id} rev ${request.expected_revision}`);
    ids = ids.filter((id) => id !== request.lorebook_id);
    return ok(result());
  });
  const reorder = vi.fn<NonNullable<MockCallHandlers["characterLorebooksReorder"]>>((request) => {
    log.push(`reorder ${request.lorebook_id}@${request.target_ordinal} rev ${request.expected_revision}`);
    ids = ids.filter((id) => id !== request.lorebook_id);
    ids.splice(request.target_ordinal, 0, request.lorebook_id);
    return ok(result());
  });
  return { bind, unbind, reorder, log, order: () => ids, list: () => ok(ids.map((id, index) => binding(id, index))), calls: { characterLorebooksBind: bind, characterLorebooksUnbind: unbind, characterLorebooksReorder: reorder } };
}

async function renderEdit(view: CharacterView_Serialize, calls: MockCallHandlers = {}, streams: MockStreamHandlers = {}) {
  window.history.replaceState(null, "", "/");
  const history = createBrowserHistory();
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: () => ok({ ...mockAppStatus, ui_state: { onboarding: { completed: true } } }),
        characterGet: () => ok(view),
        characterLorebooksList: () => ok([]),
        charactersList: () => ok({ items: [], next_cursor: null }),
        conversationsList: () => ok({ items: [], next_cursor: null }),
        conversationsLatestByCharacter: () => ok({ items: [], next_cursor: null }),
        modelsList: () => ok({ models: [], default_model_id: null, revision: 1 }),
        providerAccountsList: () => ok([]),
        promptsList: () => ok({ items: [], next_cursor: null }),
        settingsGet: () => ok({ global: { dynamic_memory: { enabled: true } } } as never),
        lorebooksList: () => ok({ items: books, next_cursor: null }),
        audioProvidersList: () => ok([]),
        characterDraftDefault: (mode) => ok((mode === "companion" ? companionDraft : defaultDraft) as CharacterDraft),
        companionNotesList: () => ok([]),
        ...calls,
      },
      streams,
    }),
    history,
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  await act(async () => {
    history.push("/characters/c1/edit");
  });
  await screen.findByPlaceholderText("Character name");
  return { history, runtime };
}

const nameInput = () => screen.getByPlaceholderText("Character name") as HTMLInputElement;
const saveButton = () => screen.queryByRole("button", { name: "Save" }) as HTMLButtonElement | null;
const tab = (name: string) => screen.getByRole("tab", { name: new RegExp(`^${name}`) });

function saved(request: CharacterSaveRequest_Deserialize, revision = 6): CharacterView_Serialize {
  return { id: "c1", archived: false, draft: request.draft as CharacterView_Serialize["draft"], assets: [], revision, created_at: 1, updated_at: 2 };
}

describe("character edit page", () => {
  it("shows the stored values, offers saving only after a change and discards back to them", async () => {
    await renderEdit(roleplay());
    expect(nameInput().value).toBe("Cassian");
    expect(saveButton()).toBeNull();
    fireEvent.change(nameInput(), { target: { value: "Cass" } });
    expect(await screen.findByRole("button", { name: "Save" })).toBeTruthy();
    expect(screen.getByText("Unsaved changes")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Discard" }));
    await waitFor(() => expect(saveButton()).toBeNull());
    expect(nameInput().value).toBe("Cassian");
  });

  it("saves against the loaded revision and keeps the operation id until the payload changes", async () => {
    let fail = true;
    const save = vi.fn((request: CharacterSaveRequest_Deserialize) => fail ? { status: "error" as const, error: { code: "unavailable" as const, message: "down", details: null } } : ok(saved(request)));
    await renderEdit(roleplay(), { characterSave: save });
    fireEvent.change(nameInput(), { target: { value: "  Cass  " } });
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0]![0]).toMatchObject({ character_id: "c1", expected_revision: 5, draft: { profile: { name: "Cass", definition: "A quiet king" } } });
    await waitFor(() => expect(saveButton()!.disabled).toBe(false));
    fireEvent.click(saveButton()!);
    await waitFor(() => expect(save).toHaveBeenCalledTimes(2));
    expect(save.mock.calls[1]![0].client_operation_id).toBe(save.mock.calls[0]![0].client_operation_id);
    await waitFor(() => expect(saveButton()!.disabled).toBe(false));
    fireEvent.change(nameInput(), { target: { value: "Cassie" } });
    fail = false;
    fireEvent.click(saveButton()!);
    await waitFor(() => expect(save).toHaveBeenCalledTimes(3));
    expect(save.mock.calls[2]![0].client_operation_id).not.toBe(save.mock.calls[0]![0].client_operation_id);
    expect(await screen.findByText("Changes saved")).toBeTruthy();
    await waitFor(() => expect(saveButton()).toBeNull());
    expect(nameInput().value).toBe("Cassie");
  });

  it("keeps the draft and says so when the character changed elsewhere", async () => {
    const save = vi.fn(() => ({ status: "error" as const, error: { code: "conflict" as const, message: "stale revision", details: null } }));
    await renderEdit(roleplay(), { characterSave: save });
    fireEvent.change(nameInput(), { target: { value: "Cass" } });
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    expect(await screen.findByText("It changed somewhere else. Reload and try again.")).toBeTruthy();
    expect(screen.queryByText("stale revision")).toBeNull();
    expect(nameInput().value).toBe("Cass");
    expect(saveButton()).toBeTruthy();
  });

  it("cannot save without a name or a definition", async () => {
    await renderEdit(roleplay());
    fireEvent.change(nameInput(), { target: { value: " " } });
    expect((await screen.findByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(nameInput(), { target: { value: "Cass" } });
    expect(saveButton()!.disabled).toBe(false);
    fireEvent.change(screen.getByPlaceholderText("Personality, speaking style, background, what they know"), { target: { value: "" } });
    expect(saveButton()!.disabled).toBe(true);
  });

  it("binds an added lorebook and reorders, chaining every revision", async () => {
    const backend = lorebookBackend(["a", "b"]);
    const save = vi.fn();
    await renderEdit(roleplay(), { ...backend.calls, characterLorebooksList: backend.list, characterSave: save });
    fireEvent.click(tab("Chat"));
    fireEvent.click(await screen.findByRole("button", { name: /Lorebooks/ }));
    const sheet = await screen.findByRole("dialog");
    fireEvent.click(await within(sheet).findByRole("checkbox", { name: "Book a" }));
    fireEvent.click(within(sheet).getByRole("checkbox", { name: "Book c" }));
    fireEvent.click(within(sheet).getByRole("checkbox", { name: "Book a" }));
    fireEvent.click(within(sheet).getByRole("button", { name: "Done" }));
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    await waitFor(() => expect(saveButton()).toBeNull());
    expect(save).not.toHaveBeenCalled();
    expect(backend.log).toEqual(["bind c@1 rev 5", "reorder b@0 rev 6", "reorder c@1 rev 7"]);
    expect(backend.order()).toEqual(["b", "c", "a"]);
  });

  it("unbinds a lorebook that was taken off and binds a new one after it", async () => {
    const backend = lorebookBackend(["a", "b"]);
    await renderEdit(roleplay(), { ...backend.calls, characterLorebooksList: backend.list });
    fireEvent.click(tab("Chat"));
    fireEvent.click(await screen.findByRole("button", { name: /Lorebooks/ }));
    const sheet = await screen.findByRole("dialog");
    fireEvent.click(await within(sheet).findByRole("checkbox", { name: "Book a" }));
    fireEvent.click(within(sheet).getByRole("checkbox", { name: "Book c" }));
    fireEvent.click(within(sheet).getByRole("button", { name: "Done" }));
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    await waitFor(() => expect(saveButton()).toBeNull());
    expect(backend.log).toEqual(["unbind a rev 5", "bind c@1 rev 6"]);
    expect(backend.order()).toEqual(["b", "c"]);
  });

  it("keeps the saved character and the picked lorebooks when a lorebook step fails, then retries on the refreshed revision", async () => {
    let server = roleplay();
    let failBind = true;
    const bind = vi.fn((request: { expected_revision: number; client_operation_id: string }) => {
      void request;
      return failBind ? { status: "error" as const, error: { code: "unavailable" as const, message: "down", details: null } } : ok({ bindings: [binding("a", 0)], owner_revision: 9 });
    });
    const save = vi.fn((request: CharacterSaveRequest_Deserialize) => {
      server = saved(request, 6);
      return ok(server);
    });
    await renderEdit(roleplay(), {
      characterSave: save,
      characterLorebooksBind: bind,
      characterGet: () => ok(save.mock.calls.length > 0 ? { ...server, revision: 7 } : server),
    });
    fireEvent.change(nameInput(), { target: { value: "Cass" } });
    fireEvent.click(tab("Chat"));
    fireEvent.click(await screen.findByRole("button", { name: /Lorebooks/ }));
    fireEvent.click(await within(await screen.findByRole("dialog")).findByRole("checkbox", { name: "Book a" }));
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Done" }));
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    expect(await screen.findByText("Saved, but the lorebooks could not be updated. Try saving again.")).toBeTruthy();
    expect(save).toHaveBeenCalledTimes(1);
    expect(bind.mock.calls[0]![0].expected_revision).toBe(6);
    expect(screen.getByRole("button", { name: /Lorebooks/ }).textContent).toContain("1 lorebook");
    expect(saveButton()).toBeTruthy();
    failBind = false;
    await waitFor(() => expect(saveButton()!.disabled).toBe(false));
    fireEvent.click(saveButton()!);
    await waitFor(() => expect(bind).toHaveBeenCalledTimes(2));
    expect(bind.mock.calls[1]![0].expected_revision).toBe(7);
    expect(bind.mock.calls[1]![0].client_operation_id).not.toBe(bind.mock.calls[0]![0].client_operation_id);
    expect(save).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(saveButton()).toBeNull());
  });

  it("asks before leaving when only a scene direction was typed", async () => {
    const { history } = await renderEdit(roleplay());
    fireEvent.click(tab("Scenes"));
    fireEvent.click(await screen.findByRole("button", { name: "Write the first scene" }));
    fireEvent.change(await screen.findByPlaceholderText("Keep the tension building"), { target: { value: "Slow burn" } });
    await screen.findByRole("button", { name: "Save" });
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("Discard your changes?")).toBeTruthy();
    fireEvent.click(within(dialog).getByRole("button", { name: "Keep editing" }));
    expect(history.location.pathname).toBe("/characters/c1/edit");
  });

  it("changes only the edited field of a fully populated character", async () => {
    const view = companion();
    const draft = view.draft;
    draft.profile.description = "A king";
    draft.profile.nickname = "Cass";
    draft.provenance.creator = "me";
    draft.provenance.tags = ["fantasy"];
    draft.defaults.voice = { kind: "voice_profile", value: "vp1" };
    draft.defaults.voice_autoplay = true;
    draft.defaults.model_profile_id = "m1";
    const soul = draft.defaults.companion_soul!;
    soul.soul.baseline_affect.trust = null;
    soul.soul.regulation_style.pride = null;
    soul.relationship_defaults.tension = null;
    draft.media = [
      { asset_id: "av", slot: "avatar_original", ordinal: 0 },
      { asset_id: "bn", slot: "banner", ordinal: 0 },
      { asset_id: "bg", slot: "background", ordinal: 0 },
      { asset_id: "r1", slot: "design_reference", ordinal: 0 },
      { asset_id: "r2", slot: "design_reference", ordinal: 1 },
    ];
    draft.scenes = [{ id: "s1", ordinal: 0, content: { format_version: 1, parts: [{ kind: "text", text: "The throne room" }] }, direction: "Late evening", selected_variant_id: "v1", assets: [] }];
    draft.variants = [{ id: "v1", scene_id: "s1", ordinal: 0, content: { format_version: 1, parts: [{ kind: "text", text: "An alternate" }] }, direction: null }];
    draft.starters = [{ id: "st1", name: "Meeting", ordinal: 0, messages: [{ id: "sm1", role: "assistant", content: "Sit." }], scene_id: "s1", prompt_id: null, lorebooks: { kind: "inherit" } }];
    draft.defaults.default_scene_id = "s1";
    view.assets = ["av", "bn", "bg", "r1", "r2"].map((id) => ({ asset_id: id, url: `https://example.test/${id}.png` }));
    const save = vi.fn((request: CharacterSaveRequest_Deserialize) => ok(saved(request)));
    await renderEdit(view, { characterSave: save });
    fireEvent.change(nameInput(), { target: { value: "Cass" } });
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0]![0].draft).toEqual({ ...draft, profile: { ...draft.profile, name: "Cass" } });
  });

  it("hides the Soul tab for roleplay characters", async () => {
    await renderEdit(roleplay());
    expect(screen.getByRole("tab", { name: /^Profile/ })).toBeTruthy();
    expect(screen.queryByRole("tab", { name: "Soul" })).toBeNull();
  });

  it("shows the Soul tab for companions and saves slider and switch changes", async () => {
    const save = vi.fn((request: CharacterSaveRequest_Deserialize) => ok(saved(request)));
    await renderEdit(companion(), { characterSave: save });
    fireEvent.click(tab("Soul"));
    fireEvent.click(await screen.findByRole("button", { name: /Baseline affect/ }));
    const warmth = await screen.findByRole("slider", { name: "Warmth" });
    fireEvent.change(warmth, { target: { value: "70" } });
    fireEvent.click(screen.getByRole("switch", { name: "Time awareness" }));
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    const soul = save.mock.calls[0]![0].draft.defaults.companion_soul!;
    expect(soul.soul.baseline_affect.warmth).toBe(0.7);
    expect(soul.time_awareness).toBe(true);
  });

  it("writes a provider voice and clears it with None", async () => {
    const save = vi.fn((request: CharacterSaveRequest_Deserialize) => ok(saved(request)));
    await renderEdit(roleplay(), {
      characterSave: save,
      audioProvidersList: () => ok([{ id: "p1", label: "Kokoro", configuration: { type: "kokoro", variant: null }, has_api_key: false, revision: 1 }]),
      audioProviderVoices: () => ok([{ voice_id: "v1", name: "Adam", preview_url: null, labels: {} }]),
    });
    fireEvent.click(tab("Chat"));
    fireEvent.click(await screen.findByRole("button", { name: /^Voice/ }));
    fireEvent.click(await screen.findByRole("button", { name: /Adam/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0]![0].draft.defaults.voice).toEqual({ kind: "provider", value: { provider_id: "p1", voice_id: "v1", model_id: null, voice_name: "Adam" } });
    await waitFor(() => expect(saveButton()).toBeNull());
    fireEvent.click(screen.getByRole("button", { name: /^Voice/ }));
    fireEvent.click(await screen.findByRole("button", { name: /^None/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(2));
    expect(save.mock.calls[1]![0].draft.defaults.voice).toBeNull();
  });

  it("archives with an undo that restores", async () => {
    const archive = vi.fn(() => ok({ ...roleplay(), archived: true, revision: 6 }));
    const restore = vi.fn(() => ok({ ...roleplay(), revision: 7 }));
    await renderEdit(roleplay(), { characterArchive: archive, characterRestore: restore });
    fireEvent.click(screen.getByRole("button", { name: "More actions" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Archive" }));
    await waitFor(() => expect(archive).toHaveBeenCalledWith(expect.objectContaining({ character_id: "c1", expected_revision: 5 })));
    fireEvent.click(await screen.findByRole("button", { name: "Undo" }));
    await waitFor(() => expect(restore).toHaveBeenCalledWith(expect.objectContaining({ character_id: "c1", expected_revision: 6 })));
  });

  it("asks before leaving with unsaved changes and leaves on discard", async () => {
    const { history } = await renderEdit(roleplay());
    fireEvent.change(nameInput(), { target: { value: "Cass" } });
    await screen.findByRole("button", { name: "Save" });
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "Keep editing" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(history.location.pathname).toBe("/characters/c1/edit");
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Discard" }));
    await waitFor(() => expect(history.location.pathname).toBe("/"));
  });

  it("leaves without asking when nothing changed", async () => {
    const { history } = await renderEdit(roleplay());
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    await waitFor(() => expect(history.location.pathname).toBe("/"));
  });
});

function jobView(patch: Partial<JobView>): JobView {
  return {
    id: "j1", kind: "companion_soul_writer", subject: { kind: "character", id: "c1" } as unknown as JobView["subject"], subject_detail: null, state: "succeeded",
    progress: { current: 0, total: null, unit: null, label_code: null, bytes_per_second: null }, created_at: 1, updated_at: 1, failure: null, result: null, ...patch,
  };
}

function soulCompanion() {
  const view = companion();
  view.draft.defaults.companion_soul!.soul.essence = "Old essence";
  view.draft.scenes = [{ id: "s1", ordinal: 0, content: { format_version: 1, parts: [{ kind: "text", text: "The throne room" }] }, direction: "Late evening", selected_variant_id: null, assets: [] }];
  return view;
}

function proposalOf(view: CharacterView_Serialize): CompanionSoulDraft {
  const config = view.draft.defaults.companion_soul!;
  return {
    soul: { ...config.soul, essence: "Calm king", traits: "Patient", baseline_affect: { ...config.soul.baseline_affect, warmth: 0.8 } },
    authored_facts: [],
    relationship_defaults: config.relationship_defaults,
  };
}


function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

function draftWith(view: CharacterView_Serialize, essence: string): CompanionSoulDraft {
  const proposal = proposalOf(view);
  return { ...proposal, soul: { ...proposal.soul, essence } };
}

const withinSheet = () => within(screen.getByRole("dialog"));

async function openGenerate() {
  fireEvent.click(tab("Soul"));
  fireEvent.click(await screen.findByRole("button", { name: "Generate" }));
  await screen.findByRole("dialog");
}

describe("soul generation", () => {
  it("sends the draft, opening context, current soul and direction, then reviews and applies only the selected parts", async () => {
    const view = soulCompanion();
    const run = vi.fn(() => ok({ job_id: "j1" }));
    const save = vi.fn((request: CharacterSaveRequest_Deserialize) => ok(saved(request)));
    await renderEdit(view, { companionSoulWriterRun: run, characterSave: save }, {
      jobWatch: (_args, emit) => {
        emit({ type: "progress", job: jobView({ state: "running", progress: { current: 0, total: null, unit: null, label_code: "identity", bytes_per_second: null } }) });
        return ok(jobView({ result: { type: "companion_soul_draft", draft: proposalOf(view) } }));
      },
    });
    await openGenerate();
    fireEvent.change(withinSheet().getByPlaceholderText(/Lean tsundere/), { target: { value: "Guarded" } });
    fireEvent.click(withinSheet().getByRole("button", { name: "Generate" }));
    expect(await withinSheet().findByText("Review generated soul")).toBeTruthy();
    expect(await screen.findByRole("button", { name: "Apply" })).toBeTruthy();
    expect(run).toHaveBeenCalledTimes(1);
    const request = (run.mock.calls[0] as unknown as [Record<string, unknown>])[0];
    expect(request).toMatchObject({
      character_name: "Cassian",
      character_definition: "A quiet king",
      opening_context: "Scene 1\nThe throne room\nDirection: Late evening",
      user_notes: "Guarded",
      model_profile_id: null,
      current_soul: { soul: { essence: "Old essence" } },
    });
    expect(screen.getByText("Now: Old essence")).toBeTruthy();
    expect(screen.queryByText("Now: A quiet king")).toBeNull();
    fireEvent.click(screen.getByRole("checkbox", { name: "Use Traits" }));
    expect(saveButton()).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    const soul = save.mock.calls[0]![0].draft.defaults.companion_soul!;
    expect(soul.soul.essence).toBe("Calm king");
    expect(soul.soul.traits).toBe("");
    expect(soul.soul.baseline_affect.warmth).toBe(0.8);
  });

  it("leaves the draft untouched on discard", async () => {
    const view = soulCompanion();
    await renderEdit(view, { companionSoulWriterRun: () => ok({ job_id: "j1" }) }, {
      jobWatch: () => ok(jobView({ result: { type: "companion_soul_draft", draft: proposalOf(view) } })),
    });
    await openGenerate();
    fireEvent.click(withinSheet().getByRole("button", { name: "Generate" }));
    fireEvent.click(await screen.findByRole("button", { name: "Discard" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(saveButton()).toBeNull();
  });

  it("stops a running job", async () => {
    const cancel = vi.fn(() => ok(null));
    await renderEdit(soulCompanion(), { companionSoulWriterRun: () => ok({ job_id: "j1" }), jobCancel: cancel }, {
      jobWatch: () => new Promise(() => undefined),
    });
    await openGenerate();
    fireEvent.click(withinSheet().getByRole("button", { name: "Generate" }));
    fireEvent.click(await screen.findByRole("button", { name: "Stop" }));
    await waitFor(() => expect(cancel).toHaveBeenCalledWith({ job_id: "j1" }));
    expect(await withinSheet().findByRole("button", { name: "Generate" })).toBeTruthy();
  });

  it("cancels the job when the sheet closes while it runs", async () => {
    const cancel = vi.fn(() => ok(null));
    await renderEdit(soulCompanion(), { companionSoulWriterRun: () => ok({ job_id: "j1" }), jobCancel: cancel }, {
      jobWatch: () => new Promise(() => undefined),
    });
    await openGenerate();
    fireEvent.click(withinSheet().getByRole("button", { name: "Generate" }));
    await screen.findByRole("button", { name: "Stop" });
    fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });
    await waitFor(() => expect(cancel).toHaveBeenCalledWith({ job_id: "j1" }));
  });

  it("shows a failure with a retry", async () => {
    let fail = true;
    const run = vi.fn(() => ok({ job_id: "j1" }));
    const view = soulCompanion();
    await renderEdit(view, { companionSoulWriterRun: run }, {
      jobWatch: () => ok(fail ? jobView({ state: "failed" }) : jobView({ result: { type: "companion_soul_draft", draft: proposalOf(view) } })),
    });
    await openGenerate();
    fireEvent.click(withinSheet().getByRole("button", { name: "Generate" }));
    expect(await screen.findByText("The soul could not be generated.")).toBeTruthy();
    fail = false;
    fireEvent.click(withinSheet().getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("button", { name: "Apply" })).toBeTruthy();
    expect(run).toHaveBeenCalledTimes(2);
  });

  it("cannot generate without a definition", async () => {
    const view = soulCompanion();
    view.draft.profile.definition = null;
    await renderEdit(view);
    fireEvent.click(tab("Soul"));
    expect((await screen.findByRole("button", { name: "Generate" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText("Add a definition first")).toBeTruthy();
  });

  it("cancels the job once its id arrives when stopped before the run was accepted", async () => {
    const accepted = deferred<{ status: "ok"; data: { job_id: string } }>();
    const cancel = vi.fn(() => ok(null));
    const watch = vi.fn(() => new Promise<never>(() => undefined));
    await renderEdit(soulCompanion(), { companionSoulWriterRun: () => accepted.promise, jobCancel: cancel }, { jobWatch: watch });
    await openGenerate();
    fireEvent.click(withinSheet().getByRole("button", { name: "Generate" }));
    fireEvent.click(await screen.findByRole("button", { name: "Stop" }));
    expect(cancel).not.toHaveBeenCalled();
    await act(async () => accepted.resolve(ok({ job_id: "j1" })));
    await waitFor(() => expect(cancel).toHaveBeenCalledTimes(1));
    expect(cancel).toHaveBeenCalledWith({ job_id: "j1" });
    expect(watch).not.toHaveBeenCalled();
  });

  it("ignores the result of a run that was stopped before a newer one", async () => {
    const view = soulCompanion();
    const first = deferred<ReturnType<typeof ok<JobView>>>();
    let watches = 0;
    await renderEdit(view, { companionSoulWriterRun: () => ok({ job_id: "j1" }), jobCancel: () => ok(null) }, {
      jobWatch: () => {
        watches += 1;
        return watches === 1 ? first.promise : ok(jobView({ result: { type: "companion_soul_draft", draft: draftWith(view, "Fresh essence") } }));
      },
    });
    await openGenerate();
    fireEvent.click(withinSheet().getByRole("button", { name: "Generate" }));
    fireEvent.click(await screen.findByRole("button", { name: "Stop" }));
    fireEvent.click(await withinSheet().findByRole("button", { name: "Generate" }));
    const essence = await withinSheet().findByRole("textbox", { name: "Essence" });
    expect((essence as HTMLTextAreaElement).value).toBe("Fresh essence");
    await act(async () => first.resolve(ok(jobView({ result: { type: "companion_soul_draft", draft: draftWith(view, "Stale essence") } }))));
    expect((withinSheet().getByRole("textbox", { name: "Essence" }) as HTMLTextAreaElement).value).toBe("Fresh essence");
  });

  it("cancels the job when the page goes away while it runs", async () => {
    const cancel = vi.fn(() => ok(null));
    await renderEdit(soulCompanion(), { companionSoulWriterRun: () => ok({ job_id: "j1" }), jobCancel: cancel }, { jobWatch: () => new Promise(() => undefined) });
    await openGenerate();
    fireEvent.click(withinSheet().getByRole("button", { name: "Generate" }));
    await screen.findByRole("button", { name: "Stop" });
    cleanup();
    await waitFor(() => expect(cancel).toHaveBeenCalledWith({ job_id: "j1" }));
  });

  it("offers Apply for changed authored facts only", async () => {
    const view = soulCompanion();
    const same: CompanionSoulDraft = { soul: view.draft.defaults.companion_soul!.soul, authored_facts: [], relationship_defaults: view.draft.defaults.companion_soul!.relationship_defaults };
    const first = await renderEdit(view, { companionSoulWriterRun: () => ok({ job_id: "j1" }) }, { jobWatch: () => ok(jobView({ result: { type: "companion_soul_draft", draft: same } })) });
    await openGenerate();
    fireEvent.click(withinSheet().getByRole("button", { name: "Generate" }));
    expect(await screen.findByText("No differences from current.")).toBeTruthy();
    expect((screen.getByRole("button", { name: "Apply" }) as HTMLButtonElement).disabled).toBe(true);
    void first;
  });

  it("applies authored facts that differ even when nothing else changed", async () => {
    const view = soulCompanion();
    const config = view.draft.defaults.companion_soul!;
    const withFact: CompanionSoulDraft = { soul: config.soul, authored_facts: [{ id: "f1" } as never], relationship_defaults: config.relationship_defaults };
    await renderEdit(view, { companionSoulWriterRun: () => ok({ job_id: "j1" }) }, { jobWatch: () => ok(jobView({ result: { type: "companion_soul_draft", draft: withFact } })) });
    await openGenerate();
    fireEvent.click(withinSheet().getByRole("button", { name: "Generate" }));
    expect(await screen.findByText("1 authored fact comes with the identity.")).toBeTruthy();
    expect(screen.queryByText("No differences from current.")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    expect(await screen.findByRole("button", { name: "Save" })).toBeTruthy();
  });
});

function noteOf(patch: Partial<CompanionNoteView> = {}): CompanionNoteView {
  return { id: "n1", character_id: "c1", label: "Birthday", content: "It is your birthday.", available_at: Date.UTC(2030, 2, 14, 9, 0), expires_at: null, recurrence: "yearly", recurrence_window_ms: 24 * 3600 * 1000, enabled: true, created_at: 1, updated_at: 1, ...patch };
}

describe("scheduled notes", () => {
  it("lists notes and toggling upserts immediately without dirtying the page", async () => {
    const upsert = vi.fn((request: { enabled: boolean }) => ok(noteOf({ enabled: request.enabled })));
    await renderEdit(companion(), { companionNotesList: () => ok([noteOf()]), companionNotesUpsert: upsert });
    fireEvent.click(tab("Soul"));
    expect(await screen.findByText("Birthday")).toBeTruthy();
    fireEvent.click(screen.getByRole("switch", { name: "Birthday enabled" }));
    await waitFor(() => expect(upsert).toHaveBeenCalledTimes(1));
    expect(upsert.mock.calls[0]![0]).toMatchObject({ character_id: "c1", note_id: "n1", enabled: false, label: "Birthday", recurrence: "yearly", recurrence_window_ms: 24 * 3600 * 1000 });
    expect(saveButton()).toBeNull();
  });

  it("rolls the switch back when the update fails", async () => {
    const upsert = vi.fn(() => ({ status: "error" as const, error: { code: "unavailable" as const, message: "down", details: null } }));
    await renderEdit(companion(), { companionNotesList: () => ok([noteOf()]), companionNotesUpsert: upsert });
    fireEvent.click(tab("Soul"));
    const toggle = await screen.findByRole("switch", { name: "Birthday enabled" });
    fireEvent.click(toggle);
    expect((await screen.findAllByText("It isn't available right now.")).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.getByRole("switch", { name: "Birthday enabled" }).getAttribute("aria-checked")).toBe("true"));
  });

  it("creates a note with legacy defaults", async () => {
    const upsert = vi.fn((request: { content: string }) => ok(noteOf({ content: request.content })));
    await renderEdit(companion(), { companionNotesUpsert: upsert });
    fireEvent.click(tab("Soul"));
    fireEvent.click(await screen.findByRole("button", { name: /Add a note/ }));
    const sheet = within(await screen.findByRole("dialog"));
    expect((sheet.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(sheet.getByPlaceholderText(/Today is your birthday/), { target: { value: "  Remember the sea  " } });
    fireEvent.click(sheet.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(upsert).toHaveBeenCalledTimes(1));
    expect(upsert.mock.calls[0]![0]).toMatchObject({ character_id: "c1", note_id: null, label: "", content: "Remember the sea", expires_at: null, recurrence: "none", recurrence_window_ms: null, enabled: true });
    expect(saveButton()).toBeNull();
  });

  it("edits a note and deletes it after confirming", async () => {
    const upsert = vi.fn(() => ok(noteOf()));
    const remove = vi.fn(() => ok(null));
    await renderEdit(companion(), { companionNotesList: () => ok([noteOf()]), companionNotesUpsert: upsert, companionNotesDelete: remove });
    fireEvent.click(tab("Soul"));
    fireEvent.click(await screen.findByRole("button", { name: "Edit" }));
    let sheet = within(await screen.findByRole("dialog"));
    fireEvent.change(sheet.getByPlaceholderText("Birthday"), { target: { value: "Name day" } });
    fireEvent.change(sheet.getByPlaceholderText("24"), { target: { value: "48" } });
    fireEvent.click(sheet.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(upsert).toHaveBeenCalledTimes(1));
    expect((upsert.mock.calls[0] as unknown as [object])[0]).toMatchObject({ note_id: "n1", label: "Name day", recurrence_window_ms: 48 * 3600 * 1000 });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(await screen.findByRole("button", { name: "Edit" }));
    sheet = within(await screen.findByRole("dialog"));
    fireEvent.click(sheet.getByRole("button", { name: "Delete note" }));
    const confirmation = await screen.findAllByRole("dialog");
    fireEvent.click(within(confirmation.at(-1)!).getByRole("button", { name: "Delete note" }));
    await waitFor(() => expect(remove).toHaveBeenCalledTimes(1));
    expect((remove.mock.calls[0] as unknown as [object])[0]).toMatchObject({ note_id: "n1" });
  });

  it("blocks saving a note whose end is not after its start", async () => {
    const upsert = vi.fn();
    await renderEdit(companion(), { companionNotesList: () => ok([noteOf({ expires_at: Date.UTC(2030, 2, 1) })]), companionNotesUpsert: upsert });
    fireEvent.click(tab("Soul"));
    fireEvent.click(await screen.findByRole("button", { name: "Edit" }));
    const sheet = within(await screen.findByRole("dialog"));
    expect(sheet.getByText("The end must come after the start.")).toBeTruthy();
    expect((sheet.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
    expect(upsert).not.toHaveBeenCalled();
    const draft = { ...newNoteDraft(0), content: "x", availableAt: new Date(60000) };
    expect(noteProblem({ ...draft, expiresAt: new Date(60000) })).toBe("end");
    expect(noteProblem({ ...draft, expiresAt: new Date(120000) })).toBeNull();
  });

  it("sends a picked local date and time as exact epoch milliseconds and shows it back unchanged", () => {
    const picked = new Date(2030, 2, 14, 9, 30);
    const request = draftToUpsert("c1", null, { ...newNoteDraft(0), content: "x", availableAt: picked, expiresAt: new Date(2030, 2, 15, 23, 59) }, "op");
    expect(request.available_at).toBe(picked.getTime());
    expect(String(request.available_at)).toHaveLength(13);
    expect(request.expires_at).toBe(new Date(2030, 2, 15, 23, 59).getTime());
    const back = noteToDraft(noteOf({ available_at: request.available_at }));
    const format = (date: Date) => new Intl.DateTimeFormat("en", { dateStyle: "medium", timeStyle: "short" }).format(date);
    expect(format(back.availableAt)).toBe(format(picked));
    expect([back.availableAt.getFullYear(), back.availableAt.getMonth(), back.availableAt.getDate(), back.availableAt.getHours(), back.availableAt.getMinutes()]).toEqual([2030, 2, 14, 9, 30]);
  });

  it("keeps instants before 1970 exact and shows them in local time", async () => {
    const moment = new Date(1890, 2, 14, 10, 0).getTime();
    expect(moment).toBeLessThan(0);
    const upsert = vi.fn(() => ok(noteOf({ available_at: moment })));
    await renderEdit(companion(), { companionNotesList: () => ok([noteOf({ available_at: moment, recurrence: "none", recurrence_window_ms: null })]), companionNotesUpsert: upsert });
    fireEvent.click(tab("Soul"));
    expect((await screen.findByText(/Mar 14, 1890/)).textContent).toContain("10:00");
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    const sheet = within(await screen.findByRole("dialog"));
    expect(sheet.getByText(/Mar 14, 1890, 10:00/)).toBeTruthy();
    fireEvent.change(sheet.getByPlaceholderText("Birthday"), { target: { value: "Founding" } });
    fireEvent.click(sheet.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(upsert).toHaveBeenCalledTimes(1));
    const sent = (upsert.mock.calls[0] as unknown as [{ available_at: number; expires_at: number | null }])[0];
    expect(sent.available_at).toBe(moment);
    expect(sent.expires_at).toBeNull();
  });

  it("reuses the operation id of a failed toggle and drops it once a write succeeds", async () => {
    let calls = 0;
    const upsert = vi.fn(() => {
      calls += 1;
      return calls === 1 ? { status: "error" as const, error: { code: "unavailable" as const, message: "down", details: null } } : ok(noteOf());
    });
    await renderEdit(companion(), { companionNotesList: () => ok([noteOf()]), companionNotesUpsert: upsert });
    fireEvent.click(tab("Soul"));
    const toggle = () => screen.getByRole("switch", { name: "Birthday enabled" });
    fireEvent.click(await screen.findByRole("switch", { name: "Birthday enabled" }));
    await waitFor(() => expect(upsert).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(toggle().getAttribute("aria-checked")).toBe("true"));
    fireEvent.click(toggle());
    await waitFor(() => expect(upsert).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(toggle().getAttribute("aria-checked")).toBe("true"));
    fireEvent.click(toggle());
    await waitFor(() => expect(upsert).toHaveBeenCalledTimes(3));
    const ids = (upsert.mock.calls as unknown as Array<[{ client_operation_id: string }]>).map(([request]) => request.client_operation_id);
    expect(ids[1]).toBe(ids[0]);
    expect(ids[2]).not.toBe(ids[1]);
  });

  it("picks 1890 through the calendar's year view and sends the exact negative milliseconds", async () => {
    const upsert = vi.fn(() => ok(noteOf()));
    await renderEdit(companion(), { companionNotesUpsert: upsert });
    fireEvent.click(tab("Soul"));
    fireEvent.click(await screen.findByRole("button", { name: /Add a note/ }));
    const sheet = within(await screen.findByRole("dialog"));
    fireEvent.change(sheet.getByPlaceholderText(/Today is your birthday/), { target: { value: "Founding day" } });
    fireEvent.click(sheet.getByRole("button", { name: "Show on" }));
    fireEvent.click(await screen.findByRole("button", { name: "Choose month and year" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Year" }), { target: { value: "1890" } });
    fireEvent.click(screen.getByRole("button", { name: "Mar" }));
    fireEvent.click(screen.getByRole("button", { name: "14" }));
    fireEvent.change(screen.getByLabelText("Time"), { target: { value: "10:00" } });
    expect(screen.getByText(/Mar 14, 1890, 10:00/)).toBeTruthy();
    fireEvent.click(sheet.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(upsert).toHaveBeenCalledTimes(1));
    const sent = (upsert.mock.calls[0] as unknown as [{ available_at: number }])[0];
    expect(sent.available_at).toBe(new Date(1890, 2, 14, 10, 0).getTime());
    expect(sent.available_at).toBeLessThan(0);
  });

  it("ends a repeating note at 23:59 when a day is picked but keeps a typed time", async () => {
    const upsert = vi.fn(() => ok(noteOf()));
    const old = noteOf({ available_at: new Date(2020, 0, 1, 9, 0).getTime() });
    await renderEdit(companion(), { companionNotesList: () => ok([old]), companionNotesUpsert: upsert });
    fireEvent.click(tab("Soul"));
    fireEvent.click(await screen.findByRole("button", { name: "Edit" }));
    const sheet = within(await screen.findByRole("dialog"));
    fireEvent.click(sheet.getByRole("button", { name: "Stops after" }));
    fireEvent.click(await screen.findByRole("button", { name: "20" }));
    fireEvent.click(sheet.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(upsert).toHaveBeenCalledTimes(1));
    const ended = new Date((upsert.mock.calls[0] as unknown as [{ expires_at: number }])[0].expires_at);
    expect([ended.getDate(), ended.getHours(), ended.getMinutes()]).toEqual([20, 23, 59]);
  });

  it("keeps a time typed before any day was picked", async () => {
    const upsert = vi.fn(() => ok(noteOf()));
    const old = noteOf({ available_at: new Date(2020, 0, 1, 9, 0).getTime() });
    await renderEdit(companion(), { companionNotesList: () => ok([old]), companionNotesUpsert: upsert });
    fireEvent.click(tab("Soul"));
    fireEvent.click(await screen.findByRole("button", { name: "Edit" }));
    const sheet = within(await screen.findByRole("dialog"));
    fireEvent.click(sheet.getByRole("button", { name: "Stops after" }));
    fireEvent.change(await screen.findByLabelText("Time"), { target: { value: "08:00" } });
    fireEvent.click(sheet.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(upsert).toHaveBeenCalledTimes(1));
    const ended = new Date((upsert.mock.calls[0] as unknown as [{ expires_at: number }])[0].expires_at);
    expect([ended.getHours(), ended.getMinutes()]).toEqual([8, 0]);
  });

  it("asks for a valid repeat window instead of dropping it", async () => {
    const upsert = vi.fn();
    await renderEdit(companion(), { companionNotesList: () => ok([noteOf()]), companionNotesUpsert: upsert });
    fireEvent.click(tab("Soul"));
    fireEvent.click(await screen.findByRole("button", { name: "Edit" }));
    const sheet = within(await screen.findByRole("dialog"));
    fireEvent.change(sheet.getByPlaceholderText("24"), { target: { value: "0" } });
    expect(sheet.getByText("Enter a whole number of hours above zero, or leave it empty.")).toBeTruthy();
    expect((sheet.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(sheet.getByPlaceholderText("24"), { target: { value: "" } });
    expect((sheet.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(false);
    expect(upsert).not.toHaveBeenCalled();
  });
});
