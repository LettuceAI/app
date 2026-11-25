import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createBrowserHistory, createMemoryHistory, type RouterHistory } from "@tanstack/react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createApiClient } from "@/api/client";
import type {
  CharacterSummary,
  CharacterView_Serialize,
  CreatorView,
  LorebookSummary_Serialize,
  LorebookProjectView_Serialize,
  MediaLibraryItem,
  PersonaPage_Serialize,
  PersonaView_Serialize,
} from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";
import { newCharacterDraft, newPersonaDraft } from "./data/defaults";
import { exportEntry, importEntry } from "./data/actions";
import { mediaQuery } from "@/api/media-library-query";

vi.mock("@/api/window-controls", () => ({
  createWindowController: vi.fn().mockResolvedValue(null),
}));
beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(600);
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(600);
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockImplementation(function (this: HTMLElement) {
    return this.hasAttribute("data-index") ? 92 : 600;
  });
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});
const character: CharacterSummary = {
  id: "c1",
  name: "Cassian",
  description: "King of Welan",
  avatar: null,
  banner: null,
  card_style: "circle",
  gradient: { type: "none" },
  revision: 7,
  created_at: 1,
  updated_at: 1,
};
const personas: PersonaPage_Serialize = {
  items: [
    { id: "p1", title: "Hunter", description: "A quiet observer", nickname: null, avatar: null },
  ],
  next_cursor: null,
  default_persona: { persona_id: "p1", revision: 9, created_at: 1, updated_at: 1 },
};
const image: MediaLibraryItem = {
  asset: { asset_id: "img1", url: "https://example.test/image.png" },
  role: "image",
  asset_kind: "generated_image",
  retention: "library",
  expires_at: null,
  mime_type: "image/png",
  byte_size: 1024,
  width: 512,
  height: 512,
  duration_ms: null,
  source_label: "Throne room",
  created_at: 1,
  updated_at: 1,
  references: [],
};
const ok = <T,>(data: T) => ({ status: "ok" as const, data });
async function setup(calls: MockCallHandlers = {}, initial: string | RouterHistory = "/library") {
  const transport = createMockTransport({
    calls: {
      appStatus: () => ok({ ...mockAppStatus, ui_state: { onboarding: { completed: true } } }),
      libraryCounts: () => ok({ characters: 1, personas: 1, lorebooks: 0, images: 1, audio: 0 }),
      characterDraftDefault: () => ok(newCharacterDraft("", "")),
      settingsGet: () => ok({ global: { dynamic_memory: { enabled: false } } } as never),
      charactersList: (request) =>
        ok({
          items:
            !request.query ||
            `${character.name} ${character.description}`
              .toLowerCase()
              .includes(request.query.toLowerCase())
              ? [character]
              : [],
          next_cursor: null,
        }),
      personasList: (request) =>
        ok({
          ...personas,
          items: personas.items.filter(
            (item) =>
              !request.query ||
              `${item.title} ${item.description}`
                .toLowerCase()
                .includes(request.query.toLowerCase()),
          ),
        }),
      lorebooksList: () => ok({ items: [], next_cursor: null }),
      lorebookProjectsList: () => ok({ items: [], next_cursor: null }),
      mediaLibraryList: (request) =>
        ok({ items: request.kind === "audio" ? [] : [image], next_cursor: null }),
      mediaReferences: () => ok([]),
      conversationsLatestByCharacter: () => ok({ items: [], next_cursor: null }),
      conversationsList: () => ok({ items: [], next_cursor: null }),
      ...calls,
    },
  });
  const runtime = await createAppRuntime({
    transport,
    history: typeof initial === "string" ? createMemoryHistory({ initialEntries: [initial] }) : initial,
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return { runtime, transport };
}

async function chooseLorebookCover() {
  fireEvent.click(await screen.findByRole("button", { name: "Choose from library" }));
  const picker = await screen.findByRole("dialog", { name: "Choose from library" });
  fireEvent.click(await within(picker).findByRole("button", { name: "Throne room" }));
  expect(await screen.findByRole("img", { name: "Cover image" })).toBeTruthy();
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "Choose from library" })).toBeNull());
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
    fireEvent.change(screen.getAllByPlaceholderText("Search library")[0]!, {
      target: { value: "quiet" },
    });
    await waitFor(() => expect(screen.queryByText("Cassian")).toBeNull());
    expect(await screen.findByText("Hunter")).toBeTruthy();
    fireEvent.change(screen.getAllByPlaceholderText("Search library")[0]!, {
      target: { value: "" },
    });
    fireEvent.click(within(filters).getByRole("radio", { name: /Images/ }));
    expect(await screen.findByRole("button", { name: /Throne room/ })).toBeTruthy();
    await waitFor(() => expect(screen.queryByText("Hunter")).toBeNull());
  });
  it("waits for demand before following a cursor and refreshes personas on events", async () => {
    const list = vi.fn<NonNullable<MockCallHandlers["charactersList"]>>((request) =>
      ok({
        items: [
          {
            ...character,
            id: request.cursor ? "c2" : "c1",
            name: request.cursor ? "Mireille" : "Cassian",
          },
        ],
        next_cursor: request.cursor ? null : "next",
      }),
    );
    let name = "Hunter";
    const { transport } = await setup({
      charactersList: list,
      personasList: () => ok({ ...personas, items: [{ ...personas.items[0]!, title: name }] }),
    });
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
    await setup({
      charactersList: () =>
        failed
          ? {
              status: "error",
              error: { code: "unavailable", message: "private diagnostic", details: null },
            }
          : ok({ items: [], next_cursor: null }),
    });
    expect(await screen.findByText("It isn't available right now.")).toBeTruthy();
    expect(screen.queryByText("Your library is empty")).toBeNull();
    expect(screen.queryByText("private diagnostic")).toBeNull();
    failed = false;
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Hunter")).toBeTruthy();
  });
  it("returns to the Characters filter after editing a character", async () => {
    const view: CharacterView_Serialize = {
      id: "c1",
      archived: false,
      draft: {
        ...newCharacterDraft("Cassian", "King of Welan"),
        profile: {
          ...newCharacterDraft("Cassian", "").profile,
          definition: "Keep the full definition",
        },
      },
      assets: [],
      revision: 7,
      created_at: 1,
      updated_at: 1,
    };
    const { runtime } = await setup({
      characterGet: () => ok(view),
      characterLorebooksList: () => ok([]),
    });
    const filter = screen.getByRole("radio", { name: /^Characters/ });
    fireEvent.click(filter);
    await waitFor(() => expect(filter.getAttribute("aria-checked")).toBe("true"));
    fireEvent.click(await screen.findByRole("button", { name: /Cassian/ }));
    await waitFor(() => expect(runtime.router.state.resolvedLocation?.search.item).toBe("c1"));
    fireEvent.click(await screen.findByRole("button", { name: "Edit character" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/characters/c1/edit"));
    expect(await screen.findByDisplayValue("Cassian")).toBeTruthy();
    await act(async () => runtime.router.history.back());
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/library"));
    expect(runtime.router.state.location.search.item).toBeUndefined();
    expect((await screen.findByRole("radio", { name: /^Characters/ })).getAttribute("aria-checked")).toBe("true");
  });
  it("opens Edit with AI from the character context menu and returns to the Library filter", async () => {
    const proposal: CreatorView["proposal"] = {
      id: "first",
      parent_id: null,
      ordinal: 0,
      source: "start",
      draft: { name: character.name, definition: character.description ?? "", scenes: [], avatar: null },
      created_at: 1,
    };
    const work: CreatorView = {
      summary: { id: "work", kind: "character", name: character.name, target_id: character.id, applied_id: null, updated_at: 1 },
      revision: 1, proposal, history: [proposal], turns: [], active_job_id: null, reviewed: false,
      target_changed: false, model_profile_id: null, images: [],
    };
    const start = vi.fn<NonNullable<MockCallHandlers["creatorStart"]>>(() => ok(work));
    const initialUrl = window.location.href;
    const initialState: unknown = window.history.state;
    window.history.replaceState(null, "", "/");
    const history = createBrowserHistory();
    try {
      const { runtime } = await setup({
        creatorStart: start,
        creatorGet: () => ok(work),
        settingsGet: () => ok({ global: { dynamic_memory: { enabled: false }, creation_helper: { model_profile_id: null } } } as never),
      }, history);
      await act(async () => runtime.router.navigate({ to: "/library" }));
      const filter = screen.getByRole("radio", { name: /^Characters/ });
      fireEvent.click(filter);
      await waitFor(() => expect(filter.getAttribute("aria-checked")).toBe("true"));
      fireEvent.contextMenu(await screen.findByRole("button", { name: /Cassian/ }));
      await waitFor(() => expect(runtime.router.state.resolvedLocation?.search.item).toBe(character.id));
      fireEvent.click(await screen.findByRole("button", { name: /Edit with AI/ }));
      await waitFor(() => expect(start).toHaveBeenCalledExactlyOnceWith(
        expect.objectContaining({ kind: "character", target_id: character.id }),
      ));
      expect(await screen.findByDisplayValue(character.name)).toBeTruthy();
      expect(runtime.router.state.location.pathname).toBe("/creator/work");
      await act(async () => history.back());
      await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/library"));
      expect(runtime.router.state.location.search.item).toBeUndefined();
      expect((await screen.findByRole("radio", { name: /^Characters/ })).getAttribute("aria-checked")).toBe("true");
    } finally {
      cleanup();
      history.destroy();
      window.history.replaceState(initialState, "", initialUrl);
    }
  });
});

describe("library operations", () => {
  it.each(["all", "characters"])(
    "opens the character wizard from the empty %s filter",
    async (filter) => {
      const { runtime } = await setup({
        charactersList: () => ok({ items: [], next_cursor: null }),
        personasList: () => ok({ ...personas, items: [] }),
      });
      if (filter === "characters")
        fireEvent.click(await screen.findByRole("radio", { name: /Characters/ }));
      fireEvent.click(await screen.findByRole("button", { name: "Create character" }));
      await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/characters/new"));
      expect(await screen.findByPlaceholderText("Character name")).toBeTruthy();
      expect(screen.queryByRole("dialog")).toBeNull();
    },
  );

  it("opens the character wizard from the library Create menu", async () => {
    const { runtime } = await setup();
    fireEvent.click(await screen.findByRole("button", { name: "Create" }));
    const menu = await screen.findByRole("dialog");
    fireEvent.click(within(menu).getByRole("button", { name: /^Character/ }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/characters/new"));
    expect(await screen.findByPlaceholderText("Character name")).toBeTruthy();
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("creates an empty lorebook with its chosen cover and opens the editor", async () => {
    const create = vi.fn<NonNullable<MockCallHandlers["lorebookCreate"]>>(({ metadata }) =>
      ok({
        lorebook: {
          id: "new-book",
          name: metadata.name,
          detection: metadata.detection,
          icon: metadata.icon_asset_id === image.asset.asset_id ? image.asset : null,
          status: "active",
          revision: 1,
          created_at: 1,
          updated_at: 1,
        },
        entries: [],
      }),
    );
    const { runtime } = await setup({
      lorebookCreate: create,
      lorebookGeneratorDefaults: () => ok({
        target_count: 5, min_target_count: 5, max_target_count: 500, output_tokens: 4096,
        model_problem: "no_text_model",
      }),
      lorebookGet: () => ok({
        lorebook: {
          id: "new-book",
          name: "Welan",
          detection: "recent_messages",
          icon: image.asset,
          status: "active",
          revision: 1,
          created_at: 1,
          updated_at: 1,
        },
        entries: [],
      }),
    });
    fireEvent.click(await screen.findByRole("button", { name: "Create" }));
    const menu = await screen.findByRole("dialog");
    fireEvent.click(within(menu).getByRole("button", { name: /^Lorebook/ }));
    fireEvent.change(await screen.findByPlaceholderText("e.g. World of Welan"), {
      target: { value: "Welan" },
    });
    expect(await screen.findByText("Add a text model in Settings, then choose it for lorebooks.")).toBeTruthy();
    expect(screen.getByRole("button", { name: /^Write it with AI/ })).toHaveProperty("disabled", true);
    await chooseLorebookCover();
    expect(create).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Create lorebook" }));

    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/library/lorebooks/new-book"));
    expect(create).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({
      metadata: { name: "Welan", detection: "recent_messages", icon_asset_id: image.asset.asset_id },
      entries: [],
    }));
    expect(await screen.findByText("No entries yet")).toBeTruthy();
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(within(document.querySelector("header")!).getByRole("button", { name: "Library" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/library"));
  });

  it("keeps the cover across AI setup and sends it with the project", async () => {
    const project: LorebookProjectView_Serialize = {
      project_id: "ai-book", brief: "A kingdom", lorebook_name: "Welan", cover: image.asset,
      target_count: 5, stage: "created", sources: [], outline: [], drafts: [],
      coherence_changes: [], commit: null, active_job_ids: [], active_phase: null,
      plan_failure: null, revision: 1, created_at: 1, updated_at: 1,
    };
    const create = vi.fn<NonNullable<MockCallHandlers["lorebookProjectCreate"]>>(() => ok(project));
    await setup({
      lorebookGeneratorDefaults: () => ok({
        target_count: 5, min_target_count: 5, max_target_count: 500, output_tokens: 4096,
        model_problem: null,
      }),
      lorebookProjectCreate: create,
      lorebookProjectGet: () => ok(project),
      lorebookProjectPlan: () => ok({ job_id: "plan" }),
      settingsGet: () => ok({
        global: {
          dynamic_memory: { enabled: false },
          lorebook_generator: { selection: { model_profile_id: null } },
        },
      } as never),
      modelsList: () => ok({ models: [], default_model_id: null, revision: 1 }),
      providerAccountsList: () => ok([]),
    }, "/library/lorebooks/new");
    fireEvent.change(await screen.findByPlaceholderText("e.g. World of Welan"), {
      target: { value: "Welan" },
    });
    const ai = screen.getByRole("button", { name: /^Write it with AI/ });
    await waitFor(() => expect(ai).toHaveProperty("disabled", false));
    fireEvent.click(ai);
    await chooseLorebookCover();
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    fireEvent.change(await screen.findByPlaceholderText(/^A kingdom after a civil war/), {
      target: { value: "A kingdom" },
    });
    expect(screen.getByRole("img", { name: "Cover image" }).getAttribute("src")).toBe(image.asset.url);
    fireEvent.click(screen.getByRole("button", { name: "Plan entries" }));
    await waitFor(() => expect(create).toHaveBeenCalledWith(expect.objectContaining({
      brief: "A kingdom",
      lorebook_name: "Welan",
      cover_asset_id: image.asset.asset_id,
    })));
  });

  it("passes the selected cover when importing a lorebook file", async () => {
    const source = { uri: "lorebook.uec" };
    const commit = vi.fn<NonNullable<MockCallHandlers["importCommit"]>>(() =>
      ok({ kind: "imported", file_kind: "lorebook", id: "imported-book", warnings: [] }),
    );
    await setup({
      filesPickOpen: () => ok({ sources: [source] }),
      importPreview: () => ok({
        kind: "lorebook", name: "Welan", digest: "digest",
        content: { kind: "lorebook", name: "Welan", keyword_detection_mode: "recentMessageWindow", entries: [] },
        uec3: null, images: [], warnings: [],
      }),
      importCommit: commit,
    }, "/library/lorebooks/new");
    fireEvent.click(await screen.findByRole("button", { name: /^From a file/ }));
    fireEvent.change(screen.getByPlaceholderText("e.g. World of Welan"), {
      target: { value: "Welan" },
    });
    await chooseLorebookCover();
    fireEvent.click(screen.getByRole("button", { name: "Choose file" }));
    const sheet = await screen.findByRole("dialog", { name: "Import lorebook" });
    fireEvent.click(within(sheet).getByRole("button", { name: "Import" }));
    await waitFor(() => expect(commit).toHaveBeenCalledWith(expect.objectContaining({
      source,
      digest: "digest",
      options: { lorebook_cover_asset_id: image.asset.asset_id },
    })));
  });

  it("redirects old character-create links to the wizard", async () => {
    const { runtime } = await setup({}, "/library?create=character");
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/characters/new"));
    expect(await screen.findByPlaceholderText("Character name")).toBeTruthy();
  });

  it("creates a persona through the wizard and returns to its filter", async () => {
    const save = vi.fn<NonNullable<MockCallHandlers["personaSave"]>>(({ draft: { intro, ...draft } }) =>
      ok({
        id: "new",
        archived: false,
        draft: { ...draft, ...(intro == null ? {} : { intro }) },
        assets: [],
        revision: 1,
        created_at: 1,
        updated_at: 1,
      }),
    );
    const { runtime } = await setup({ personaSave: save }, "/library?create=persona");
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/personas/new"));
    fireEvent.change(await screen.findByRole("textbox", { name: "Name" }), {
      target: { value: "Wren" },
    });
    fireEvent.change(screen.getByRole("textbox", { name: "Description" }), {
      target: { value: "Travelling scribe" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Continue to extras" }));
    fireEvent.click(await screen.findByRole("button", { name: "Create persona" }));
    await waitFor(() =>
      expect(save).toHaveBeenCalledWith(
        expect.objectContaining({
          persona_id: null,
          expected_revision: null,
          draft: expect.objectContaining({ title: "Wren", description: "Travelling scribe" }),
        }),
      ),
    );
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/library"));
    expect(
      (await screen.findByRole("radio", { name: /Personas/ })).getAttribute("aria-checked"),
    ).toBe("true");
  });

  it("sets the default persona with the default-record revision", async () => {
    const setDefault = vi.fn<NonNullable<MockCallHandlers["personaDefaultSet"]>>(() =>
      ok({ ...personas.default_persona, persona_id: "p1", revision: 10 }),
    );
    await setup({
      personasList: () =>
        ok({ ...personas, default_persona: { ...personas.default_persona, persona_id: null } }),
      personaDefaultSet: setDefault,
    });
    fireEvent.click(await screen.findByRole("button", { name: /Hunter/ }));
    fireEvent.click(await screen.findByRole("button", { name: /Use as default persona/ }));
    await waitFor(() =>
      expect(setDefault).toHaveBeenCalledWith(
        expect.objectContaining({ persona_id: "p1", expected_revision: 9 }),
      ),
    );
  });

  it("opens the Android save picker from the persona menu", async () => {
    const target = { uri: "content://documents/Hunter.uec" };
    const pick = vi.fn(() => ok({ target }));
    const write = vi.fn(() => ok({ uri: target.uri, suggested_name: "Hunter.uec", warnings: [] }));
    const view: PersonaView_Serialize = {
      id: "p1",
      archived: false,
      draft: newPersonaDraft("Hunter", "A quiet observer"),
      assets: [],
      revision: 7,
      created_at: 1,
      updated_at: 1,
    };
    await setup({
      appStatus: () => ok({ ...mockAppStatus, platform: "android", ui_state: { onboarding: { completed: true } } }),
      personaGet: () => ok(view),
      filesPickSave: pick,
      personaExport: write,
    }, "/library?kind=persona&item=p1");
    const menu = within(await screen.findByRole("dialog", { name: "Hunter" }));
    await waitFor(() => expect((menu.getByRole("button", { name: "Delete persona" }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(menu.getByRole("button", { name: /^Export/ }));
    await waitFor(() => expect(write).toHaveBeenCalledExactlyOnceWith({ id: "p1", target }));
    expect(pick).toHaveBeenCalledExactlyOnceWith({ suggested_name: "Hunter.uec", kind: "any" });
  });

  it("keeps referenced media visible after a rejected delete and retries explicitly", async () => {
    const remove = vi.fn<NonNullable<MockCallHandlers["mediaLibraryRemove"]>>(() => ({
      status: "error",
      error: { code: "in_use", message: "private owner details", details: null },
    }));
    const { runtime } = await setup({ mediaLibraryRemove: remove });
    fireEvent.click(await screen.findByRole("radio", { name: /Images/ }));
    fireEvent.click(await screen.findByRole("button", { name: /Throne room/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Delete image" }));
    fireEvent.click(await screen.findByRole("button", { name: /^Delete$/ }));
    expect(
      await screen.findByText("It is still in use. Remove its references and try again."),
    ).toBeTruthy();
    expect(runtime.queryClient.getQueryData(mediaQuery(runtime.api, "image", "").queryKey)).toEqual(
      expect.objectContaining({ pages: [expect.objectContaining({ items: [image] })] }),
    );
    expect(remove).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(runtime.router.state.location.search.media).toBeUndefined());
  });

  it("returns to the Lorebooks filter after editing a lorebook", async () => {
    const lorebook: LorebookSummary_Serialize = {
      id: "l1",
      name: "Atlas",
      status: "active",
      detection: "recent_messages",
      icon: null,
      revision: 3,
      created_at: 1,
      updated_at: 1,
    };
    const { runtime } = await setup({
      lorebooksList: () => ok({ items: [lorebook], next_cursor: null }),
      lorebookGet: () => ok({ lorebook, entries: [] }),
    });
    const filter = screen.getByRole("radio", { name: /^Lorebooks/ });
    fireEvent.click(filter);
    await waitFor(() => expect(filter.getAttribute("aria-checked")).toBe("true"));
    fireEvent.click(await screen.findByRole("button", { name: /Atlas/ }));
    await waitFor(() => expect(runtime.router.state.resolvedLocation?.search.item).toBe("l1"));
    fireEvent.click(await screen.findByRole("button", { name: /^Edit lorebook/ }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/library/lorebooks/l1"));
    expect(await screen.findByText("No entries yet")).toBeTruthy();
    fireEvent.click(screen.getByTitle("Library"));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/library"));
    expect((await screen.findByRole("radio", { name: /^Lorebooks/ })).getAttribute("aria-checked")).toBe("true");
  });

  it("asks for the lorebook export format and sends it from the entry menu", async () => {
    const lorebook: LorebookSummary_Serialize = {
      id: "l1",
      name: "Atlas",
      status: "active",
      detection: "recent_messages",
      icon: null,
      revision: 3,
      created_at: 1,
      updated_at: 1,
    };
    const exported = vi.fn<NonNullable<MockCallHandlers["lorebookExport"]>>(() =>
      ok({ warnings: [] } as never),
    );
    const pick = vi.fn<NonNullable<MockCallHandlers["filesPickSave"]>>(() =>
      ok({ target: { uri: "/tmp/Atlas" } } as never),
    );
    await setup({
      lorebooksList: () => ok({ items: [lorebook], next_cursor: null }),
      lorebookGet: () => ok({ lorebook, entries: [] }),
      filesPickSave: pick,
      lorebookExport: exported,
    });
    fireEvent.click(await screen.findByRole("button", { name: /Atlas/ }));
    fireEvent.click(await screen.findByRole("button", { name: /^Export/ }));
    expect(exported).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole("button", { name: /^UEC/ }));
    await waitFor(() => expect(exported).toHaveBeenCalledTimes(1));
    expect(exported.mock.calls[0]![0]).toMatchObject({ id: "l1", format: "uec" });
    expect(pick.mock.calls[0]![0]).toMatchObject({ suggested_name: "Atlas.uec", kind: "any" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(screen.getByRole("button", { name: /Atlas/ }));
    fireEvent.click(await screen.findByRole("button", { name: /^Export/ }));
    fireEvent.click(await screen.findByRole("button", { name: /World Info JSON/ }));
    await waitFor(() => expect(exported).toHaveBeenCalledTimes(2));
    expect(exported.mock.calls[1]![0]).toMatchObject({ id: "l1", format: "world_info" });
    expect(pick.mock.calls[1]![0]).toMatchObject({ suggested_name: "Atlas.json", kind: "json" });
  });

  it.each(["All", "Personas"])("returns to the %s filter after editing a persona", async (filter) => {
    const view: PersonaView_Serialize = {
      id: "p1",
      archived: false,
      draft: newPersonaDraft("Hunter", "A quiet observer"),
      assets: [],
      revision: 7,
      created_at: 1,
      updated_at: 1,
    };
    const { runtime } = await setup({
      personaGet: () => ok(view),
      personaLorebooksList: () => ok([]),
    });
    const filterName = new RegExp(`^${filter}`);
    const radio = screen.getByRole("radio", { name: filterName });
    fireEvent.click(radio);
    await waitFor(() => expect(radio.getAttribute("aria-checked")).toBe("true"));
    fireEvent.click(await screen.findByRole("button", { name: /Hunter/ }));
    await waitFor(() => expect(runtime.router.state.resolvedLocation?.search.item).toBe("p1"));
    fireEvent.click(await screen.findByRole("button", { name: "Edit persona" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/personas/p1/edit"));
    expect(((await screen.findByRole("textbox", { name: "Name" })) as HTMLInputElement).value).toBe("Hunter");
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/library"));
    expect((await screen.findByRole("radio", { name: filterName })).getAttribute("aria-checked")).toBe("true");
  });
});

describe("library file transfer", () => {
  it("passes the inspected digest and native source through unchanged", async () => {
    const source = { uri: "content://picked/card.uec" };
    const commit = vi.fn<NonNullable<MockCallHandlers["importCommit"]>>(() =>
      ok({ kind: "imported", file_kind: "persona_file", id: "p2", warnings: [] }),
    );
    const api = createApiClient(
      createMockTransport({
        calls: {
          filesPickOpen: () => ok({ sources: [source] }),
          importPreview: () =>
            ok({
              kind: "persona_file",
              name: "Wren",
              digest: "inspected-digest",
              content: {
                kind: "persona",
                content: {
                  title: "Wren",
                  description: "Scribe",
                  nickname: null,
                  isDefault: false,
                  avatarCrop: null,
                  activeLorebookIds: [],
                },
              },
              uec3: null,
              images: [],
              warnings: [],
            }),
          importCommit: commit,
        },
      }),
    );
    expect(await importEntry(api)).toEqual({
      kind: "imported",
      file_kind: "persona_file",
      id: "p2",
      warnings: [],
    });
    expect(commit).toHaveBeenCalledWith(
      expect.objectContaining({ source, digest: "inspected-digest", options: {} }),
    );
  });
  it("does not export after the save picker is cancelled", async () => {
    const write = vi.fn<NonNullable<MockCallHandlers["characterExport"]>>(() =>
      ok({ uri: "unused", suggested_name: "unused", warnings: [] }),
    );
    const api = createApiClient(
      createMockTransport({
        calls: { filesPickSave: () => ok({ target: null }), characterExport: write },
      }),
    );
    expect(
      await exportEntry(api, {
        kind: "character",
        id: "c1",
        name: "Cassian",
        description: "",
        image: null,
      }),
    ).toBeNull();
    expect(write).not.toHaveBeenCalled();
  });
  it("refreshes media when another screen updates a character's assets", async () => {
    let items = [image];
    const { transport } = await setup({ mediaLibraryList: () => ok({ items, next_cursor: null }) });
    await screen.findByText("Cassian");
    fireEvent.click(screen.getByRole("radio", { name: /Images/ }));
    await screen.findByRole("button", { name: /Throne room/ });
    items = [{ ...image, asset: { ...image.asset, asset_id: "img2" }, source_label: "New image" }];
    await act(async () =>
      transport.emitAppEvent({ type: "character_changed", character_id: "c1" }),
    );
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
    const charactersList = vi.fn<NonNullable<MockCallHandlers["charactersList"]>>(() =>
      ok({ items: [character], next_cursor: "more" }),
    );
    const mediaList = vi.fn<NonNullable<MockCallHandlers["mediaLibraryList"]>>(() =>
      ok({ items: [image], next_cursor: null }),
    );
    await setup({ charactersList, mediaLibraryList: mediaList });
    await screen.findByText("Cassian");
    expect(charactersList).toHaveBeenCalledTimes(1);
    expect(charactersList).toHaveBeenCalledWith(
      expect.objectContaining({ limit: 24, cursor: null }),
    );
    expect(mediaList).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("radio", { name: /Images/ }));
    await screen.findByRole("button", { name: /Throne room/ });
    expect(mediaList).toHaveBeenCalledTimes(1);
    expect(mediaList).toHaveBeenCalledWith(
      expect.objectContaining({
        limit: 32,
        kind: "image",
        cursor: null,
        include_references: false,
      }),
    );
  });

  it("fetches media usage only when its menu opens", async () => {
    const references = vi.fn<NonNullable<MockCallHandlers["mediaReferences"]>>(() =>
      ok([{ kind: "character", owner_id: character.id }]),
    );
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
    const items = Array.from({ length: 5000 }, (_, index) => ({
      kind: "character",
      id: `many-${index}`,
      name: `Person ${index}`,
      description: "",
      image: null,
    }));
    await act(async () => {
      runtime.queryClient.setQueryData(["library", "entries", "character", ""], {
        pages: [{ items, next: null, defaultPersona: undefined }],
        pageParams: [null],
      });
    });
    expect(await screen.findByText("Person 0")).toBeTruthy();
    expect(screen.queryByText("Person 4999")).toBeNull();
    expect(screen.getAllByRole("button", { name: /Person \d/ }).length).toBeLessThan(30);
  });

  it("searches unseen records in the backend instead of consuming every cursor", async () => {
    const list = vi.fn<NonNullable<MockCallHandlers["charactersList"]>>((request) =>
      ok({
        items:
          request.query === "distant" ? [{ ...character, name: "Distant character" }] : [character],
        next_cursor: request.query ? null : "many-more",
      }),
    );
    await setup({ charactersList: list });
    await screen.findByText("Cassian");
    fireEvent.change(screen.getAllByPlaceholderText("Search library")[0]!, {
      target: { value: "distant" },
    });
    expect(await screen.findByText("Distant character")).toBeTruthy();
    expect(list).toHaveBeenCalledWith(
      expect.objectContaining({
        query: "distant",
        cursor: null,
        search_description: true,
        limit: 24,
      }),
    );
    expect(list.mock.calls.every(([request]) => request.cursor === null)).toBe(true);
  });
});

describe("library totals", () => {
  it("keeps every filter count visible without loading its items", async () => {
    const mediaList = vi.fn<NonNullable<MockCallHandlers["mediaLibraryList"]>>(() =>
      ok({ items: [], next_cursor: null }),
    );
    await setup({
      libraryCounts: () =>
        ok({ characters: 5000, personas: 7, lorebooks: 12, images: 55, audio: 80 }),
      mediaLibraryList: mediaList,
    });
    await screen.findByRole("radio", { name: /^All\s*5019$/ });
    for (const name of [
      /^Characters\s*5000$/,
      /^Personas\s*7$/,
      /^Lorebooks\s*12$/,
      /^Images\s*55$/,
      /^Audio\s*80$/,
    ])
      expect(screen.getByRole("radio", { name })).toBeTruthy();
    expect(mediaList).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("radio", { name: /^Characters\s*5000$/ }));
    await screen.findByText("Cassian");
    expect(screen.getByRole("radio", { name: /^Images\s*55$/ })).toBeTruthy();
    expect(screen.getByRole("radio", { name: /^Audio\s*80$/ })).toBeTruthy();
    expect(mediaList).not.toHaveBeenCalled();
  });

  it("reuses cached kind queries when switching from All to Characters, Personas, or Lorebooks", async () => {
    const charactersList = vi.fn<NonNullable<MockCallHandlers["charactersList"]>>(() =>
      ok({ items: [character], next_cursor: null }),
    );
    const personasList = vi.fn<NonNullable<MockCallHandlers["personasList"]>>(() => ok(personas));
    const lorebooksList = vi.fn<NonNullable<MockCallHandlers["lorebooksList"]>>(() =>
      ok({ items: [], next_cursor: null }),
    );
    await setup({ charactersList, personasList, lorebooksList });
    expect(await screen.findByText("Cassian")).toBeTruthy();
    expect(charactersList).toHaveBeenCalledTimes(1);
    expect(personasList).toHaveBeenCalledTimes(1);
    expect(lorebooksList).toHaveBeenCalledTimes(1);

    const filters = screen.getByRole("radiogroup", { name: "Show" });
    fireEvent.click(within(filters).getByRole("radio", { name: /^Characters/ }));
    expect(await screen.findByText("Cassian")).toBeTruthy();
    expect(charactersList).toHaveBeenCalledTimes(1);

    fireEvent.click(within(filters).getByRole("radio", { name: /^Personas/ }));
    expect(await screen.findByText("Hunter")).toBeTruthy();
    expect(personasList).toHaveBeenCalledTimes(1);

    fireEvent.click(within(filters).getByRole("radio", { name: /^Lorebooks/ }));
    expect(lorebooksList).toHaveBeenCalledTimes(1);
  });
});
