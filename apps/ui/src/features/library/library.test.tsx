import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createApiClient } from "@/api/client";
import type { CharacterSummary, CharacterView_Serialize, MediaLibraryItem, PersonaPage } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";
import { newCharacterDraft } from "./data/defaults";
import { exportEntry, importEntry, saveEntry } from "./data/actions";
import { mediaQuery } from "./data/queries";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));
beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(600);
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(600);
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });
const character: CharacterSummary = { id: "c1", name: "Cassian", description: "King of Welan", avatar: null, banner: null, card_style: "circle", gradient: { type: "none" }, revision: 7, created_at: 1, updated_at: 1 };
const personas: PersonaPage = { items: [{ id: "p1", title: "Hunter", description: "A quiet observer", nickname: null, avatar: null }], next_cursor: null, default_persona: { persona_id: "p1", revision: 9, created_at: 1, updated_at: 1 } };
const image: MediaLibraryItem = { asset: { asset_id: "img1", url: "https://example.test/image.png" }, role: "image", asset_kind: "generated_image", retention: "library", expires_at: null, mime_type: "image/png", byte_size: 1024, width: 512, height: 512, duration_ms: null, source_label: "Throne room", created_at: 1, updated_at: 1, references: [] };
const ok = <T,>(data: T) => ({ status: "ok" as const, data });
async function setup(calls: MockCallHandlers = {}, path = "/library") {
  const transport = createMockTransport({ calls: {
    appStatus: () => ok({ ...mockAppStatus, ui_state: { onboarding: { completed: true } } }),
    libraryCounts: () => ok({ characters: 1, personas: 1, lorebooks: 0, images: 1, audio: 0 }),
    characterDraftDefault: () => ok(newCharacterDraft("", "")),
    settingsGet: () => ok({ global: { dynamic_memory: { enabled: false } } } as never),
    charactersList: (request) => ok({ items: !request.query || `${character.name} ${character.description}`.toLowerCase().includes(request.query.toLowerCase()) ? [character] : [], next_cursor: null }),
    personasList: (request) => ok({ ...personas, items: personas.items.filter((item) => !request.query || `${item.title} ${item.description}`.toLowerCase().includes(request.query.toLowerCase())) }),
    lorebooksList: () => ok({ items: [], next_cursor: null }),
    mediaLibraryList: (request) => ok({ items: request.kind === "audio" ? [] : [image], next_cursor: null }),
    mediaReferences: () => ok([]),
    conversationsLatestByCharacter: () => ok({ items: [], next_cursor: null }),
    conversationsList: () => ok({ items: [], next_cursor: null }),
    ...calls,
  } });
  const runtime = await createAppRuntime({ transport, history: createMemoryHistory({ initialEntries: [path] }), i18n: await createTestI18n("en") });
  await act(async () => { render(<App runtime={runtime} />); });
  return { runtime, transport };
}

describe("library design port", () => {
  it("uses the demo filters and cards, searches descriptions and isolates media", async () => {
    await setup();
    expect(await screen.findByText("Cassian")).toBeTruthy();
    const filters = screen.getByRole("radiogroup", { name: "Show" });
    expect(within(filters).getAllByRole("radio")).toHaveLength(6);
    expect(within(filters).getByRole("radio", { name: /^All\s*2$/ })).toBeTruthy();
    expect(within(filters).getByRole("radio", { name: /^Images\s*1$/ })).toBeTruthy();
    expect(within(filters).getByRole("radio", { name: /^Audio\s*0$/ })).toBeTruthy();
    expect(await screen.findByText("Hunter")).toBeTruthy();
    expect(screen.getByText("You")).toBeTruthy();
    fireEvent.change(screen.getAllByPlaceholderText("Search library")[0]!, { target: { value: "quiet" } });
    await waitFor(() => expect(screen.queryByText("Cassian")).toBeNull());
    expect(await screen.findByText("Hunter")).toBeTruthy();
    fireEvent.change(screen.getAllByPlaceholderText("Search library")[0]!, { target: { value: "" } });
    fireEvent.click(within(filters).getByRole("radio", { name: /Images/ }));
    expect(await screen.findByRole("button", { name: /Throne room/ })).toBeTruthy();
    await waitFor(() => expect(screen.queryByText("Hunter")).toBeNull());
  });
  it("waits for demand before following a cursor and refreshes personas on events", async () => {
    const list = vi.fn<NonNullable<MockCallHandlers["charactersList"]>>((request) => ok({ items: [{ ...character, id: request.cursor ? "c2" : "c1", name: request.cursor ? "Mireille" : "Cassian" }], next_cursor: request.cursor ? null : "next" }));
    let name = "Hunter";
    const { transport } = await setup({ charactersList: list, personasList: () => ok({ ...personas, items: [{ ...personas.items[0]!, title: name }] }) });
    expect(await screen.findByText("Cassian")).toBeTruthy();
    expect(list).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("Mireille")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Load more" }));
    expect(await screen.findByText("Mireille")).toBeTruthy();
    expect(list).toHaveBeenCalledWith(expect.objectContaining({ cursor: "next" }));
    name = "Wren";
    await act(async () => transport.emitAppEvent({ type: "persona_changed", persona_id: "p1" }));
    expect(await screen.findByText("Wren")).toBeTruthy();
  });
  it("shows a typed retry instead of a false empty state", async () => {
    let failed = true;
    await setup({ charactersList: () => failed ? { status: "error", error: { code: "unavailable", message: "private diagnostic", details: null } } : ok({ items: [], next_cursor: null }) });
    expect(await screen.findByText("It isn't available right now.")).toBeTruthy();
    expect(screen.queryByText("Your library is empty")).toBeNull();
    expect(screen.queryByText("private diagnostic")).toBeNull();
    failed = false;
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Hunter")).toBeTruthy();
  });
  it("keeps editor input after failure and fences its original revision", async () => {
    const save = vi.fn<NonNullable<MockCallHandlers["characterSave"]>>(() => ({ status: "error", error: { code: "conflict", message: "stale", details: null } }));
    const view: CharacterView_Serialize = { id: "c1", archived: false, draft: { ...newCharacterDraft("Cassian", "King of Welan"), profile: { ...newCharacterDraft("Cassian", "").profile, definition: "Keep the full definition" } }, assets: [], revision: 7, created_at: 1, updated_at: 1 };
    await setup({ characterGet: () => ok(view), characterSave: save });
    fireEvent.click(await screen.findByRole("button", { name: /Cassian/ }));
    const edit = await screen.findByRole("button", { name: "Edit character" });
    await waitFor(() => expect((edit as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(edit);
    const input = await screen.findByRole("textbox", { name: "Name" });
    fireEvent.change(input, { target: { value: "Changed" } });
    fireEvent.click(screen.getByRole("button", { name: /^Save$/ }));
    expect(await screen.findByText("It changed somewhere else. Reload and try again.")).toBeTruthy();
    expect((input as HTMLInputElement).value).toBe("Changed");
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ expected_revision: 7, draft: expect.objectContaining({ profile: expect.objectContaining({ name: "Changed", definition: "Keep the full definition" }) }) }));
  });
  it("preserves every unedited character setting", async () => {
    const draft = newCharacterDraft("Before", "Old description");
    draft.provenance.tags = ["Keep this"];
    draft.defaults.voice_autoplay = true;
    const view: CharacterView_Serialize = { id: "c1", archived: false, draft, assets: [], revision: 12, created_at: 1, updated_at: 1 };
    const save = vi.fn<NonNullable<MockCallHandlers["characterSave"]>>(() => ok(view));
    const api = createApiClient(createMockTransport({ calls: { characterSave: save } }));
    await saveEntry(api, { kind: "character", view }, "After", "New description", "operation");
    expect(save).toHaveBeenCalledWith({ character_id: "c1", expected_revision: 12, client_operation_id: "operation", draft: { ...draft, profile: { ...draft.profile, name: "After", description: "New description" } } });
  });
});

describe("library operations", () => {
  it.each(["all", "characters"])("opens the character wizard from the empty %s filter", async (filter) => {
    const { runtime } = await setup({
      charactersList: () => ok({ items: [], next_cursor: null }),
      personasList: () => ok({ ...personas, items: [] }),
    });
    if (filter === "characters") fireEvent.click(await screen.findByRole("radio", { name: /Characters/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Create character" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/characters/new"));
    expect(await screen.findByPlaceholderText("Character name")).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("opens the character wizard from the library Create menu", async () => {
    const { runtime } = await setup();
    fireEvent.click(await screen.findByRole("button", { name: "Create" }));
    const menu = await screen.findByRole("dialog");
    fireEvent.click(within(menu).getByRole("button", { name: /^Character/ }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/characters/new"));
    expect(await screen.findByPlaceholderText("Character name")).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("redirects old character-create links to the wizard", async () => {
    const { runtime } = await setup({}, "/library?create=character");
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/characters/new"));
    expect(await screen.findByPlaceholderText("Character name")).toBeTruthy();
  });

  it("creates a persona through the wizard and returns to its filter", async () => {
    const save = vi.fn<NonNullable<MockCallHandlers["personaSave"]>>((request) => ok({ id: "new", archived: false, draft: { ...request.draft }, assets: [], revision: 1, created_at: 1, updated_at: 1 }));
    const { runtime } = await setup({ personaSave: save }, "/library?create=persona");
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/personas/new"));
    fireEvent.change(await screen.findByRole("textbox", { name: "Name" }), { target: { value: "Wren" } });
    fireEvent.change(screen.getByRole("textbox", { name: "Description" }), { target: { value: "Travelling scribe" } });
    fireEvent.click(screen.getByRole("button", { name: "Continue to extras" }));
    fireEvent.click(await screen.findByRole("button", { name: "Create persona" }));
    await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ persona_id: null, expected_revision: null, draft: expect.objectContaining({ title: "Wren", description: "Travelling scribe" }) })));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/library"));
    expect((await screen.findByRole("radio", { name: /Personas/ })).getAttribute("aria-checked")).toBe("true");
  });

  it("sets the default persona with the default-record revision", async () => {
    const setDefault = vi.fn<NonNullable<MockCallHandlers["personaDefaultSet"]>>(() => ok({ ...personas.default_persona, persona_id: "p1", revision: 10 }));
    await setup({ personasList: () => ok({ ...personas, default_persona: { ...personas.default_persona, persona_id: null } }), personaDefaultSet: setDefault });
    fireEvent.click(await screen.findByRole("button", { name: /Hunter/ }));
    fireEvent.click(await screen.findByRole("button", { name: /Use as default persona/ }));
    await waitFor(() => expect(setDefault).toHaveBeenCalledWith(expect.objectContaining({ persona_id: "p1", expected_revision: 9 })));
  });

  it("keeps referenced media visible after a rejected delete and retries explicitly", async () => {
    const remove = vi.fn<NonNullable<MockCallHandlers["mediaLibraryRemove"]>>(() => ({ status: "error", error: { code: "in_use", message: "private owner details", details: null } }));
    const { runtime } = await setup({ mediaLibraryRemove: remove });
    fireEvent.click(await screen.findByRole("radio", { name: /Images/ }));
    fireEvent.click(await screen.findByRole("button", { name: /Throne room/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Delete image" }));
    fireEvent.click(await screen.findByRole("button", { name: /^Delete$/ }));
    expect(await screen.findByText("It is still in use. Remove its references and try again.")).toBeTruthy();
    expect(runtime.queryClient.getQueryData(mediaQuery(runtime.api, "image", "").queryKey)).toEqual(expect.objectContaining({ pages: [expect.objectContaining({ items: [image] })] }));
    expect(remove).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(runtime.router.state.location.search.media).toBeUndefined());
  });

  it("browser back closes the menu and reopening resets its nested editor", async () => {
    const view: CharacterView_Serialize = { id: "c1", archived: false, draft: newCharacterDraft("Cassian", "King of Welan"), assets: [], revision: 7, created_at: 1, updated_at: 1 };
    const { runtime } = await setup({ characterGet: () => ok(view) });
    fireEvent.click(await screen.findByRole("button", { name: /Cassian/ }));
    const edit = await screen.findByRole("button", { name: "Edit character" });
    await waitFor(() => expect((edit as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(edit);
    expect(await screen.findByRole("textbox", { name: "Name" })).toBeTruthy();
    await act(async () => runtime.router.history.back());
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(screen.getByRole("button", { name: /Cassian/ }));
    expect(await screen.findByRole("button", { name: "Edit character" })).toBeTruthy();
    expect(screen.queryByRole("textbox", { name: "Name" })).toBeNull();
  });
});


describe("library file transfer", () => {
  it("passes the inspected digest and native source through unchanged", async () => {
    const source = { uri: "content://picked/card.uec" };
    const commit = vi.fn<NonNullable<MockCallHandlers["importCommit"]>>(() => ok({ kind: "imported", file_kind: "persona_file", id: "p2", warnings: [] }));
    const api = createApiClient(createMockTransport({ calls: {
      filesPickOpen: () => ok({ sources: [source] }),
      importPreview: () => ok({ kind: "persona_file", name: "Wren", digest: "inspected-digest", content: { kind: "persona", content: { title: "Wren", description: "Scribe", nickname: null, isDefault: false, avatarCrop: null, activeLorebookIds: [] } }, uec3: null, images: [], warnings: [] }),
      importCommit: commit,
    } }));
    expect(await importEntry(api)).toEqual({ kind: "imported", file_kind: "persona_file", id: "p2", warnings: [] });
    expect(commit).toHaveBeenCalledWith(expect.objectContaining({ source, digest: "inspected-digest", options: {} }));
  });
  it("does not export after the save picker is cancelled", async () => {
    const write = vi.fn<NonNullable<MockCallHandlers["characterExport"]>>(() => ok({ uri: "unused", suggested_name: "unused", warnings: [] }));
    const api = createApiClient(createMockTransport({ calls: { filesPickSave: () => ok({ target: null }), characterExport: write } }));
    expect(await exportEntry(api, { kind: "character", id: "c1", name: "Cassian", description: "", image: null })).toBeNull();
    expect(write).not.toHaveBeenCalled();
  });
  it("refreshes media when another screen updates a character's assets", async () => {
    let items = [image];
    const { transport } = await setup({ mediaLibraryList: () => ok({ items, next_cursor: null }) });
    await screen.findByText("Cassian");
    fireEvent.click(screen.getByRole("radio", { name: /Images/ }));
    await screen.findByRole("button", { name: /Throne room/ });
    items = [{ ...image, asset: { ...image.asset, asset_id: "img2" }, source_label: "New image" }];
    await act(async () => transport.emitAppEvent({ type: "character_changed", character_id: "c1" }));
    fireEvent.click(screen.getByRole("radio", { name: /Images/ }));
    expect(await screen.findByRole("button", { name: /New image/ })).toBeTruthy();
  });
});

describe("bounded library loading", () => {
  it("loads thumbnail URLs in the grid and originals only in image details", async () => {
    const asset = { ...image.asset, preview_url: "https://example.test/image-preview.webp" };
    await setup({
      charactersList: () => ok({ items: [{ ...character, avatar: asset }], next_cursor: null }),
      mediaLibraryList: () => ok({ items: [{ ...image, asset }], next_cursor: null }),
    });
    const card = await screen.findByRole("button", { name: /Cassian/ });
    expect(card.querySelector("img")?.getAttribute("src")).toBe(asset.preview_url);
    expect(document.querySelector(`img[src="${asset.url}"]`)).toBeNull();
    fireEvent.click(screen.getByRole("radio", { name: /Images/ }));
    const tile = await screen.findByRole("button", { name: /Throne room/ });
    expect(tile.querySelector("img")?.getAttribute("src")).toBe(asset.preview_url);
    expect(document.querySelector(`img[src="${asset.url}"]`)).toBeNull();
    fireEvent.click(tile);
    await waitFor(() => expect(document.querySelector(`img[src="${asset.url}"]`)).toBeTruthy());
  });

  it("does not load media until selected and requests only a small entry batch", async () => {
    const charactersList = vi.fn<NonNullable<MockCallHandlers["charactersList"]>>(() => ok({ items: [character], next_cursor: "more" }));
    const mediaList = vi.fn<NonNullable<MockCallHandlers["mediaLibraryList"]>>(() => ok({ items: [image], next_cursor: null }));
    await setup({ charactersList, mediaLibraryList: mediaList });
    await screen.findByText("Cassian");
    expect(charactersList).toHaveBeenCalledTimes(1);
    expect(charactersList).toHaveBeenCalledWith(expect.objectContaining({ limit: 8, cursor: null }));
    expect(mediaList).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("radio", { name: /Images/ }));
    await screen.findByRole("button", { name: /Throne room/ });
    expect(mediaList).toHaveBeenCalledTimes(1);
    expect(mediaList).toHaveBeenCalledWith(expect.objectContaining({ limit: 32, kind: "image", cursor: null, include_references: false }));
  });

  it("fetches media usage only when its menu opens", async () => {
    const references = vi.fn<NonNullable<MockCallHandlers["mediaReferences"]>>(() => ok([{ kind: "character", owner_id: character.id }]));
    await setup({ mediaReferences: references });
    await screen.findByText("Cassian");
    fireEvent.click(screen.getByRole("radio", { name: /Images/ }));
    const tile = await screen.findByRole("button", { name: /Throne room/ });
    expect(references).not.toHaveBeenCalled();
    fireEvent.click(tile);
    expect(await screen.findByRole("button", { name: "Use as Cassian's avatar" })).toBeTruthy();
    expect(references).toHaveBeenCalledExactlyOnceWith({ asset_id: image.asset.asset_id });
  });

  it("mounts only visible rows even with thousands of already cached entries", async () => {
    const { runtime } = await setup();
    await screen.findByText("Cassian");
    const items = Array.from({ length: 5000 }, (_, index) => ({ kind: "character", id: `many-${index}`, name: `Person ${index}`, description: "", image: null }));
    await act(async () => { runtime.queryClient.setQueryData(["library", "entries", "all", ""], { pages: [{ items, next: null, defaultPersona: undefined }], pageParams: [{ characters: null, personas: null, lorebooks: null }] }); });
    expect(await screen.findByText("Person 0")).toBeTruthy();
    expect(screen.queryByText("Person 4999")).toBeNull();
    expect(screen.getAllByRole("button", { name: /Person \d/ }).length).toBeLessThan(30);
  });

  it("searches unseen records in the backend instead of consuming every cursor", async () => {
    const list = vi.fn<NonNullable<MockCallHandlers["charactersList"]>>((request) => ok({ items: request.query === "distant" ? [{ ...character, name: "Distant character" }] : [character], next_cursor: request.query ? null : "many-more" }));
    await setup({ charactersList: list });
    await screen.findByText("Cassian");
    fireEvent.change(screen.getAllByPlaceholderText("Search library")[0]!, { target: { value: "distant" } });
    expect(await screen.findByText("Distant character")).toBeTruthy();
    expect(list).toHaveBeenCalledWith(expect.objectContaining({ query: "distant", cursor: null, search_description: true, limit: 8 }));
    expect(list.mock.calls.every(([request]) => request.cursor === null)).toBe(true);
  });
});

describe("library totals", () => {
  it("keeps every filter count visible without loading its items", async () => {
    const mediaList = vi.fn<NonNullable<MockCallHandlers["mediaLibraryList"]>>(() => ok({ items: [], next_cursor: null }));
    await setup({ libraryCounts: () => ok({ characters: 5000, personas: 7, lorebooks: 12, images: 55, audio: 80 }), mediaLibraryList: mediaList });
    await screen.findByRole("radio", { name: /^All\s*5019$/ });
    for (const name of [/^Characters\s*5000$/, /^Personas\s*7$/, /^Lorebooks\s*12$/, /^Images\s*55$/, /^Audio\s*80$/]) expect(screen.getByRole("radio", { name })).toBeTruthy();
    expect(mediaList).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("radio", { name: /^Characters\s*5000$/ }));
    await screen.findByText("Cassian");
    expect(screen.getByRole("radio", { name: /^Images\s*55$/ })).toBeTruthy();
    expect(screen.getByRole("radio", { name: /^Audio\s*80$/ })).toBeTruthy();
    expect(mediaList).not.toHaveBeenCalled();
  });
});
