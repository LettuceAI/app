import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createBrowserHistory, createMemoryHistory, type RouterHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CharacterSaveRequest_Deserialize, CharacterView_Serialize, PromptSummary, PromptKind } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers, type MockStreamHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";
import defaultDraft from "./fixtures/default-draft.json";
import companionDraft from "./fixtures/default-companion-draft.json";
import { newForm, withAddedScene, withoutScene, draftToSave, isTouched, withDefaultScene, type CharacterDraft } from "./data/form";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

const start = defaultDraft as CharacterDraft;

function saved(request: CharacterSaveRequest_Deserialize): CharacterView_Serialize {
  return { id: "c-new", archived: false, draft: request.draft, assets: [], revision: 1, created_at: 1, updated_at: 1 };
}

async function renderCreate(calls: MockCallHandlers = {}, language: "en" | "cimode" = "en", browser = false, streams: MockStreamHandlers = {}) {
  let history: RouterHistory;
  if (browser) {
    window.history.replaceState(null, "", "/");
    history = createBrowserHistory();
  } else {
    history = createMemoryHistory({ initialEntries: ["/", "/characters/new"] });
  }
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: () => ({ status: "ok", data: { ...mockAppStatus, ui_state: { onboarding: { completed: true } } } }),
        characterDraftDefault: (mode) => ({ status: "ok", data: mode === "companion" ? companionDraft as CharacterDraft : start }),
        companionEmotionStatus: () => ({ status: "ok", data: { type: "installed", source_revision: "r1" } }),
        charactersList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        conversationsList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        conversationsLatestByCharacter: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        modelsList: () => ({ status: "ok", data: { models: [], default_model_id: null, revision: 1 } }),
        providerAccountsList: () => ({ status: "ok", data: [] }),
        promptsList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        settingsGet: () => ({ status: "ok", data: { global: { dynamic_memory: { enabled: false } } } as never }),
        lorebooksList: () => ({
          status: "ok",
          data: { items: [{ id: "l1", name: "Harbor lore", status: "active", detection: "recent_messages", icon: null, revision: 1, created_at: 1, updated_at: 1 }], next_cursor: null },
        }),
        ...calls,
      },
      streams,
    }),
    history,
    i18n: await createTestI18n(language),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  if (browser) {
    await act(async () => {
      history.push("/characters/new");
    });
  }
  return history;
}

const continueButton = () => screen.getByRole("button", { name: /^(Continue to |Skip$)/ }) as HTMLButtonElement;

async function deleteScene() {
  fireEvent.click(screen.getByRole("button", { name: "Delete scene" }));
  fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Delete scene" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
}

async function fillRequired() {
  fireEvent.change(await screen.findByPlaceholderText("Character name"), { target: { value: "  Aria  " } });
  fireEvent.click(continueButton());
  fireEvent.click(continueButton());
  fireEvent.change(await screen.findByPlaceholderText("Personality, speaking style, background, what they know"), { target: { value: "A sea captain" } });
}

afterEach(() => {
  cleanup();
});

describe("character creation", () => {
  it("asks for a name, then a definition, before moving on", async () => {
    await renderCreate();
    expect(await screen.findByPlaceholderText("Character name")).toBeTruthy();
    expect(continueButton().disabled).toBe(true);
    fireEvent.change(screen.getByPlaceholderText("Character name"), { target: { value: "Aria" } });
    fireEvent.click(continueButton());
    expect(await screen.findByText("Each scene is a way a chat can begin. The default one is used when you don't pick.")).toBeTruthy();
    fireEvent.click(continueButton());
    await screen.findByPlaceholderText("Personality, speaking style, background, what they know");
    expect(continueButton().disabled).toBe(true);
    fireEvent.change(screen.getByPlaceholderText("Personality, speaking style, background, what they know"), { target: { value: "Brave and kind" } });
    expect(screen.getByText("3 words")).toBeTruthy();
    expect(continueButton().disabled).toBe(false);
  });

  it("goes back a step without losing what was typed", async () => {
    await renderCreate();
    fireEvent.change(await screen.findByPlaceholderText("Character name"), { target: { value: "Aria" } });
    fireEvent.click(continueButton());
    expect(await screen.findByText("Each scene is a way a chat can begin. The default one is used when you don't pick.")).toBeTruthy();
    fireEvent.click(screen.getAllByRole("button", { name: "Back" }).at(-1)!);
    expect(await screen.findByPlaceholderText("Character name")).toBeTruthy();
    expect((screen.getByPlaceholderText("Character name") as HTMLInputElement).value).toBe("Aria");
  });

  it("saves the default draft with what was entered, a default scene and the chosen lorebook", async () => {
    const save = vi.fn((request: CharacterSaveRequest_Deserialize) => ({ status: "ok" as const, data: saved(request) }));
    const bind = vi.fn(() => ({ status: "ok" as const, data: { bindings: [], owner_revision: 2 } }));
    const history = await renderCreate({ characterSave: save, characterLorebooksBind: bind });
    fireEvent.change(await screen.findByPlaceholderText("Character name"), { target: { value: "  Aria  " } });
    fireEvent.click(continueButton());

    fireEvent.change(await screen.findByRole("textbox", { name: "Opening message" }), { target: { value: "The docks at dawn" } });
    expect(await screen.findByRole("button", { name: "Scene 1" })).toBeTruthy();
    fireEvent.click(continueButton());
    fireEvent.change(await screen.findByPlaceholderText("Personality, speaking style, background, what they know"), { target: { value: "A sea captain" } });
    fireEvent.click(continueButton());

    fireEvent.click(await screen.findByRole("button", { name: /Lorebooks/ }));
    const lorebooks = await screen.findByRole("dialog");
    fireEvent.click(await within(lorebooks).findByRole("checkbox", { name: "Harbor lore" }));
    fireEvent.click(within(lorebooks).getByRole("button", { name: "Done" }));
    fireEvent.click(await screen.findByRole("button", { name: "Create character" }));

    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    const request = save.mock.calls[0]![0];
    expect(request.character_id).toBeNull();
    expect(request.expected_revision).toBeNull();
    expect(request.draft.profile).toMatchObject({ name: "Aria", definition: "A sea captain", description: null, nickname: null });
    expect(request.draft.presentation).toEqual(start.presentation);
    expect(request.draft.scenes).toHaveLength(1);
    expect(request.draft.scenes[0]).toMatchObject({ ordinal: 0, content: { format_version: 1, parts: [{ kind: "text", text: "The docks at dawn" }] }, direction: null });
    expect(request.draft.defaults.default_scene_id).toBe(request.draft.scenes[0]!.id);
    await waitFor(() => expect(bind).toHaveBeenCalledTimes(1));
    expect(bind.mock.calls[0]).toEqual([expect.objectContaining({ character_id: "c-new", lorebook_id: "l1", expected_revision: 1 })]);
    await waitFor(() => expect(history.location.pathname).toBe("/"));
  });

  it("retries with the same operation id and uses a new one once the draft changes", async () => {
    let fail = true;
    const save = vi.fn((request: CharacterSaveRequest_Deserialize) =>
      fail ? { status: "error" as const, error: { code: "unavailable" as const, message: "down", details: null } } : { status: "ok" as const, data: saved(request) },
    );
    await renderCreate({ characterSave: save });
    await fillRequired();
    expect(await screen.findByPlaceholderText("Personality, speaking style, background, what they know")).toBeTruthy();
    fireEvent.click(continueButton());
    expect(await screen.findByPlaceholderText("What you call them")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Create character" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0]![0].draft.scenes).toEqual([]);
    expect(save.mock.calls[0]![0].draft.defaults.default_scene_id).toBeNull();
    await waitFor(() => expect((screen.getByRole("button", { name: "Create character" }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByRole("button", { name: "Create character" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(2));
    expect(save.mock.calls[1]![0].client_operation_id).toBe(save.mock.calls[0]![0].client_operation_id);
    await waitFor(() => expect((screen.getByRole("button", { name: "Create character" }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.change(screen.getByPlaceholderText("What you call them"), { target: { value: "Cap" } });
    fail = false;
    fireEvent.click(screen.getByRole("button", { name: "Create character" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(3));
    expect(save.mock.calls[2]![0].client_operation_id).not.toBe(save.mock.calls[0]![0].client_operation_id);
    expect(save.mock.calls[2]![0].draft.profile.nickname).toBe("Cap");
  });

  it("takes the system back to the previous step, then asks before discarding", async () => {
    const history = await renderCreate({}, "en", true);
    fireEvent.change(await screen.findByPlaceholderText("Character name"), { target: { value: "Aria" } });
    fireEvent.click(continueButton());
    expect(await screen.findByRole("textbox", { name: "Opening message" })).toBeTruthy();
    await act(async () => {
      history.back();
    });
    expect(await screen.findByPlaceholderText("Character name")).toBeTruthy();
    expect(history.location.pathname).toBe("/characters/new");
    await act(async () => {
      history.back();
    });
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("Discard this character?")).toBeTruthy();
    fireEvent.click(within(dialog).getByRole("button", { name: "Discard" }));
    await waitFor(() => expect(history.location.pathname).toBe("/"));
  });

  it("keeps the native avatar selection and appearance when returning from scenes", async () => {
    const source = { uri: "/tmp/avatar.png" };
    const ingest = vi.fn(() => ({ status: "ok" as const, data: { asset_id: "avatar", url: "https://example.test/avatar.png" } }));
    await renderCreate({ filesPickOpen: () => ({ status: "ok", data: { sources: [source] } }), assetsIngest: ingest });
    fireEvent.click(await screen.findByRole("radio", { name: "Off" }));
    fireEvent.click(screen.getByRole("button", { name: "Choose image" }));
    await screen.findByRole("button", { name: "Remove" });
    expect(ingest).toHaveBeenCalledWith({ source, role: "avatar" });
    fireEvent.change(screen.getByPlaceholderText("Character name"), { target: { value: "Aria" } });
    fireEvent.click(screen.getByRole("radio", { name: "Banner" }));
    fireEvent.click(continueButton());
    await screen.findByRole("textbox", { name: "Opening message" });
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(await screen.findByRole("button", { name: "Remove" })).toBeTruthy();
    expect(screen.getByRole("radio", { name: "Banner" }).getAttribute("aria-checked")).toBe("true");
    expect(screen.getByRole("radio", { name: "Off" }).getAttribute("aria-checked")).toBe("true");
  });

  it("previews and saves custom endpoints and an optional middle color across mode changes", async () => {
    const save = vi.fn((request: CharacterSaveRequest_Deserialize) => ({ status: "ok" as const, data: saved(request) }));
    await renderCreate({ characterSave: save });
    expect((await screen.findByRole("radio", { name: "Auto" })).getAttribute("aria-checked")).toBe("true");
    fireEvent.click(screen.getByRole("radio", { name: "Custom" }));
    const setColor = async (label: string, color: string) => {
      const picker = screen.getByLabelText(label);
      fireEvent.click(picker);
      const input = screen.getByRole("textbox", { name: "Hex value" });
      fireEvent.change(input, { target: { value: color } });
      fireEvent.blur(input);
      fireEvent.click(picker);
      await waitFor(() => expect(screen.queryByRole("textbox", { name: "Hex value" })).toBeNull());
    };
    await setColor("Start color", "#112233");
    await setColor("End color", "#445566");
    fireEvent.click(screen.getByRole("button", { name: "Add middle color" }));
    await setColor("Middle color", "#778899");
    const preview = screen.getByText("Your character").closest("button")!.firstElementChild as HTMLElement;
    expect(preview.style.background).toBe("linear-gradient(135deg, rgb(17, 34, 51), rgb(119, 136, 153), rgb(68, 85, 102))");
    fireEvent.click(screen.getByRole("button", { name: "Remove middle color" }));
    expect(screen.queryByLabelText("Middle color")).toBeNull();
    expect(preview.style.background).toBe("linear-gradient(135deg, rgb(17, 34, 51), rgb(68, 85, 102))");
    fireEvent.click(screen.getByRole("button", { name: "Add middle color" }));
    await setColor("Middle color", "#778899");
    fireEvent.click(screen.getByRole("radio", { name: "Off" }));
    expect(preview.style.background).toBe("var(--color-surface-1)");
    expect(screen.queryByLabelText("Start color")).toBeNull();
    fireEvent.click(screen.getByRole("radio", { name: "Auto" }));
    expect(preview.style.background).toBe("var(--color-surface-1)");
    expect(within(preview).getByText("?")).toBeTruthy();
    fireEvent.click(screen.getByRole("radio", { name: "Custom" }));
    expect(screen.getByLabelText("Start color").textContent).toBe("#112233");
    expect(screen.getByLabelText("Middle color").textContent).toBe("#778899");
    expect(screen.getByLabelText("End color").textContent).toBe("#445566");
    await fillRequired();
    fireEvent.click(continueButton());
    fireEvent.click(await screen.findByRole("button", { name: "Create character" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0]![0].draft.presentation).toMatchObject({ disable_gradient: false, custom_gradient_enabled: true, custom_gradient_colors: ["#112233", "#445566", "#778899"] });
  });

  it("starts a custom palette with the avatar's detected colors when available", async () => {
    const gradient = vi.fn(() => ({ status: "ok" as const, data: {
      colors: [{ r: 17, g: 34, b: 51, hex: "#112233" }, { r: 68, g: 85, b: 102, hex: "#445566" }, { r: 119, g: 136, b: 153, hex: "#778899" }],
      gradient_css: "linear-gradient(135deg, #112233, #778899, #445566)", dominant_hue: null, text_color: "#ffffff", text_secondary: "#cccccc",
    } }));
    await renderCreate({
      filesPickOpen: () => ({ status: "ok", data: { sources: [{ uri: "/tmp/avatar.png" }] } }),
      assetsIngest: () => ({ status: "ok", data: { asset_id: "avatar", url: "https://example.test/avatar.png" } }),
      avatarGradient: gradient,
    });
    fireEvent.click(await screen.findByRole("button", { name: "Choose image" }));
    await waitFor(() => expect(gradient).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole("radio", { name: "Custom" }));
    expect(screen.getByLabelText("Start color").textContent).toBe("#112233");
    expect(screen.getByLabelText("End color").textContent).toBe("#445566");
    expect(screen.getByLabelText("Middle color").textContent).toBe("#778899");
  });

  it("leaves the draft open when importing a card is cancelled", async () => {
    const save = vi.fn();
    const pick = vi.fn(() => ({ status: "ok" as const, data: { sources: [] } }));
    const history = await renderCreate({ filesPickOpen: pick, characterSave: save });
    fireEvent.click(await screen.findByRole("button", { name: "Choose file" }));
    await waitFor(() => expect(pick).toHaveBeenCalledExactlyOnceWith({ kinds: ["character_card"], multiple: false }));
    expect(history.location.pathname).toBe("/characters/new");
    expect(save).not.toHaveBeenCalled();
    expect(await screen.findByPlaceholderText("Character name")).toBeTruthy();
  });

  it("edits selected scenes live, toggles the default and allows empty scenes", async () => {
    await renderCreate();
    fireEvent.change(await screen.findByPlaceholderText("Character name"), { target: { value: "Aria" } });
    fireEvent.click(continueButton());
    const first = await screen.findByRole("button", { name: "Scene 1" });
    expect(first.getAttribute("aria-pressed")).toBe("true");
    expect(within(first).getByText("Empty")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Skip" })).toBeNull();
    expect(continueButton().disabled).toBe(false);
    fireEvent.change(screen.getByRole("textbox", { name: "Opening message" }), { target: { value: "The harbor at dawn" } });
    expect(within(first).getByText("The harbor at dawn")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Add a scene" }));
    const second = screen.getByRole("button", { name: "Scene 2" });
    expect(second.getAttribute("aria-pressed")).toBe("true");
    expect(first.getAttribute("aria-pressed")).toBe("false");
    expect(document.activeElement).toBe(screen.getByRole("textbox", { name: "Opening message" }));
    expect(within(second).getByText("Empty")).toBeTruthy();
    expect(continueButton().disabled).toBe(false);
    fireEvent.change(screen.getByRole("textbox", { name: "Opening message" }), { target: { value: "A storm at sea" } });
    fireEvent.click(screen.getByRole("button", { name: "Make default" }));
    expect(within(second).getByText("Default")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Default scene" }));
    expect(within(second).queryByText("Default")).toBeNull();
    fireEvent.click(first);
    expect((screen.getByRole("textbox", { name: "Opening message" }) as HTMLTextAreaElement).value).toBe("The harbor at dawn");
    fireEvent.change(screen.getByRole("textbox", { name: "Opening message" }), { target: { value: "" } });
    expect(continueButton().disabled).toBe(false);
    await deleteScene();
    expect((screen.getByRole("textbox", { name: "Opening message" }) as HTMLTextAreaElement).value).toBe("A storm at sea");
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    await screen.findByPlaceholderText("Character name");
    fireEvent.click(continueButton());
    expect((await screen.findByRole("textbox", { name: "Opening message" }) as HTMLTextAreaElement).value).toBe("A storm at sea");
    await deleteScene();
    expect(screen.queryByRole("textbox", { name: "Opening message" })).toBeNull();
    expect(screen.getByText("No scenes yet")).toBeTruthy();
    expect((screen.getByRole("button", { name: "Skip" }) as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Write the first scene" }));
    expect(screen.getByRole("button", { name: "Scene 1" })).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "Opening message" })).toBeTruthy();
  });

  it("warns about blank scenes other than the open one without blocking Continue and asks before deleting", async () => {
    await renderCreate();
    fireEvent.change(await screen.findByPlaceholderText("Character name"), { target: { value: "Aria" } });
    fireEvent.click(continueButton());
    await screen.findByRole("textbox", { name: "Opening message" });
    expect(screen.queryByRole("alert")).toBeNull();
    expect(continueButton().textContent).toContain("Continue to details");
    expect(continueButton().disabled).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Add a scene" }));
    expect(within(screen.getByRole("alert")).getByText("1 empty scene")).toBeTruthy();
    fireEvent.change(screen.getByRole("textbox", { name: "Opening message" }), { target: { value: "An opening" } });
    expect(within(screen.getByRole("alert")).getByText("1 empty scene")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Scene 1" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Opening message" }), { target: { value: "First opening" } });
    expect(screen.queryByRole("alert")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Delete scene" }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("Are you sure?")).toBeTruthy();
    expect(within(dialog).getByText("Scene 1 and its opening message will be deleted.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Scene 1" })).toBeTruthy();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect((screen.getByRole("textbox", { name: "Opening message" }) as HTMLTextAreaElement).value).toBe("First opening");
    await deleteScene();
    await waitFor(() => expect(screen.queryByRole("button", { name: "Scene 2" })).toBeNull());
    expect((screen.getByRole("textbox", { name: "Opening message" }) as HTMLTextAreaElement).value).toBe("An opening");
  });

  it("saves the design note and reference images from Extras", async () => {
    const save = vi.fn((request: CharacterSaveRequest_Deserialize) => ({ status: "ok" as const, data: saved(request) }));
    await renderCreate({
      characterSave: save,
      filesPickOpen: () => ({ status: "ok", data: { sources: [{ uri: "/tmp/ref.png" }] } }),
      assetsIngest: () => ({ status: "ok", data: { asset_id: "ref", url: "https://example.test/ref.png" } }),
    });
    await fillRequired();
    fireEvent.click(continueButton());
    fireEvent.change(await screen.findByRole("textbox", { name: "Design note" }), { target: { value: " Grey coat " } });
    fireEvent.click(screen.getByRole("button", { name: "Add image" }));
    fireEvent.click(await screen.findByRole("button", { name: "Remove image" }));
    expect(screen.queryByRole("button", { name: "Remove image" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Add image" }));
    await screen.findByRole("button", { name: "Remove image" });
    fireEvent.click(screen.getByRole("button", { name: "Create character" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    const draft = save.mock.calls[0]![0].draft;
    expect(draft.profile.design_description).toBe(" Grey coat ");
    expect(draft.media).toEqual([{ asset_id: "ref", slot: "design_reference", ordinal: 0 }]);
  });

  it("picks the memory policy from the dynamic memory setting", async () => {
    await renderCreate({ settingsGet: () => ({ status: "ok", data: { global: { dynamic_memory: { enabled: true } } } as never }) });
    await fillRequired();
    const dynamic = await screen.findByRole("radio", { name: /^Dynamic/ });
    expect(dynamic.getAttribute("aria-checked")).toBe("true");
    expect(within(dynamic).getByText("Recommended")).toBeTruthy();
    fireEvent.click(screen.getByRole("radio", { name: /^Manual/ }));
    expect(screen.getByRole("radio", { name: /^Manual/ }).getAttribute("aria-checked")).toBe("true");
  });

  it("disables dynamic memory when the setting is off", async () => {
    await renderCreate();
    await fillRequired();
    const dynamic = await screen.findByRole("radio", { name: /^Dynamic/ }) as HTMLButtonElement;
    expect(dynamic.disabled).toBe(true);
    expect(within(dynamic).getByText("Turn on dynamic memory in settings to use it")).toBeTruthy();
    expect(screen.getByRole("radio", { name: /^Manual/ }).getAttribute("aria-checked")).toBe("true");
  });

  it("inserts placeholders at the selection and keeps direction visible across scene selections", async () => {
    await renderCreate();
    fireEvent.change(await screen.findByPlaceholderText("Character name"), { target: { value: "Aria" } });
    fireEvent.click(continueButton());
    const opening = await screen.findByRole("textbox", { name: "Opening message" }) as HTMLTextAreaElement;
    fireEvent.change(opening, { target: { value: "Hello captain!" } });
    opening.setSelectionRange(6, 13);
    fireEvent.click(screen.getByRole("button", { name: "{{char}}" }));
    expect(opening.value).toBe("Hello {{char}}!");
    expect(opening.selectionStart).toBe(14);
    expect(document.activeElement).toBe(opening);
    fireEvent.click(screen.getByRole("button", { name: "{{user}}" }));
    expect(opening.value).toBe("Hello {{char}}{{user}}!");
    const direction = screen.getByRole("textbox", { name: "Direction" }) as HTMLTextAreaElement;
    expect(direction.rows).toBe(3);
    expect(screen.queryByRole("button", { name: "Direction" })).toBeNull();
    fireEvent.change(direction, { target: { value: "Slow burn" } });
    fireEvent.click(screen.getByRole("button", { name: "Add a scene" }));
    expect((screen.getByRole("textbox", { name: "Direction" }) as HTMLTextAreaElement).value).toBe("");
    fireEvent.click(screen.getByRole("button", { name: "Scene 1" }));
    expect((screen.getByRole("textbox", { name: "Direction" }) as HTMLTextAreaElement).value).toBe("Slow burn");
  });

  it("opens the selected scene as a mobile page and both back actions return to the list", async () => {
    const history = await renderCreate({ appStatus: () => ({ status: "ok", data: { ...mockAppStatus, platform: "android", ui_state: { onboarding: { completed: true } } } }) }, "en", true);
    fireEvent.change(await screen.findByPlaceholderText("Character name"), { target: { value: "Aria" } });
    fireEvent.click(continueButton());
    const first = await screen.findByRole("button", { name: "Scene 1" });
    expect(screen.queryByRole("textbox", { name: "Opening message" })).toBeNull();
    fireEvent.click(first);
    fireEvent.change(await screen.findByRole("textbox", { name: "Opening message" }), { target: { value: "A mobile opening" } });
    expect(screen.queryByRole("button", { name: "Add a scene" })).toBeNull();
    expect(screen.queryByRole("button", { name: /^(Continue to |Skip$)/ })).toBeNull();
    expect(screen.queryByText("Step 2 of 4")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Back to scenes" }));
    expect(within(await screen.findByRole("button", { name: "Scene 1" })).getByText("A mobile opening")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Add a scene" }));
    expect(await screen.findByRole("textbox", { name: "Opening message" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Scene 2" })).toBeTruthy();
    await act(async () => { history.back(); });
    expect(await screen.findByRole("button", { name: "Scene 2" })).toBeTruthy();
    expect(screen.queryByRole("textbox", { name: "Opening message" })).toBeNull();
    expect(history.location.pathname).toBe("/characters/new");
    fireEvent.click(screen.getByRole("button", { name: "Scene 2" }));
    await deleteScene();
    expect(await screen.findByRole("button", { name: "Scene 1" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Scene 2" })).toBeNull();
  });

  it("keeps prompt slots separate and only shows the companion slot in companion mode", async () => {
    const catalog: PromptSummary[] = [
      { id: "direct-default", name: "Local RP Default", kind: "direct_chat", archived: false, origin: { type: "user" }, app_default: true, revision: 1, created_at: 1, updated_at: 1 },
      { id: "direct-bundled", name: "Built-in RP", kind: "direct_chat", archived: false, origin: { type: "built_in", key: "prompt_app_default", protected: true, required: true, edited: false }, app_default: false, revision: 1, created_at: 1, updated_at: 1 },
      { id: "group-rp-default", name: "Group RP", kind: "group_chat_roleplay", archived: false, origin: { type: "built_in", key: "prompt_app_group_chat_roleplay", protected: true, required: true, edited: false }, app_default: false, revision: 1, created_at: 1, updated_at: 1 },
      { id: "group-chat-default", name: "Group Chat", kind: "group_chat_conversational", archived: false, origin: { type: "built_in", key: "prompt_app_group_chat", protected: true, required: true, edited: false }, app_default: false, revision: 1, created_at: 1, updated_at: 1 },
      { id: "companion-default", name: "Companion Default", kind: "companion_chat", archived: false, origin: { type: "built_in", key: "prompt_app_companion", protected: true, required: true, edited: false }, app_default: false, revision: 1, created_at: 1, updated_at: 1 },
    ];
    const prompts = vi.fn(({ kind }: { kind?: PromptKind | null }) => ({ status: "ok" as const, data: { items: catalog.filter((entry) => entry.kind === kind), next_cursor: null } }));
    const save = vi.fn((request: CharacterSaveRequest_Deserialize) => ({ status: "ok" as const, data: saved(request) }));
    await renderCreate({ promptsList: prompts, characterSave: save });
    expect((await screen.findByRole("radio", { name: "Roleplay" })).getAttribute("aria-checked")).toBe("true");
    await fillRequired();
    await waitFor(() => expect(screen.getByRole("button", { name: /^One-to-one chats/ }).textContent).toContain("Local RP Default"));
    await waitFor(() => expect(screen.getByRole("button", { name: /^Group roleplay/ }).textContent).toContain("Group RP"));
    await waitFor(() => expect(screen.getByRole("button", { name: /^Group conversation/ }).textContent).toContain("Group Chat"));
    expect(screen.queryByRole("button", { name: /^Companion/ })).toBeNull();
    expect(prompts.mock.calls.some(([request]) => request.kind === "companion_chat")).toBe(false);
    const choose = async (slot: string, prompt: string) => {
      fireEvent.click(screen.getByRole("button", { name: new RegExp(`^${slot}`) }));
      const dialog = await screen.findByRole("dialog");
      expect(within(dialog).queryByText("Companion Default")).toBeNull();
      fireEvent.click(within(dialog).getByRole("button", { name: new RegExp(`^${prompt}`) }));
      await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    };
    await choose("One-to-one chats", "Local RP Default");
    await choose("Group roleplay", "Group RP");
    await choose("Group conversation", "Group Chat");
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    await screen.findByRole("textbox", { name: "Opening message" });
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    fireEvent.click(await screen.findByRole("radio", { name: "Companion" }));
    await waitFor(() => expect(screen.getByRole("radio", { name: "Companion" }).getAttribute("aria-checked")).toBe("true"));
    fireEvent.click(continueButton());
    fireEvent.click(continueButton());
    await screen.findByText("Companion Default", undefined, { timeout: 5000 });
    fireEvent.click(screen.getByRole("button", { name: /^Companion/ }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).queryByText("Local RP Default")).toBeNull();
    fireEvent.click(within(dialog).getByRole("button", { name: /^Companion Default/ }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(continueButton());
    fireEvent.click(await screen.findByRole("button", { name: "Create character" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0]![0].draft.defaults).toMatchObject({
      interaction_mode: "companion", direct_prompt_id: "direct-default", group_roleplay_prompt_id: "group-rp-default", group_conversation_prompt_id: "group-chat-default",
      companion_soul: { prompting: { prompt_template_id: "companion-default", style_notes: "" } },
    });
  });

  it("keeps the character and selected mode intact when companion defaults cannot load", async () => {
    await renderCreate({ characterDraftDefault: (mode) => mode === "companion" ? { status: "error", error: { code: "unavailable", message: "unavailable", details: null } } : { status: "ok", data: start } });
    const name = await screen.findByPlaceholderText("Character name");
    fireEvent.change(name, { target: { value: "Aria" } });
    fireEvent.click(screen.getByRole("radio", { name: "Companion" }));
    await waitFor(() => expect(screen.getByRole("radio", { name: "Roleplay" }).closest("fieldset")?.hasAttribute("disabled")).toBe(false));
    expect(screen.getByRole("radio", { name: "Roleplay" }).getAttribute("aria-checked")).toBe("true");
    expect((name as HTMLInputElement).value).toBe("Aria");
  });

  it("offers the emotion model when companion mode is chosen without it and downloads inside the sheet", async () => {
    const install = vi.fn(() => ({ status: "ok" as const, data: { job_id: "j1" } }));
    let watched = false;
    await renderCreate(
      { companionEmotionStatus: () => ({ status: "ok", data: { type: "not_installed" } }), companionEmotionInstall: install },
      "en",
      false,
      {
        jobWatch: (_args, emit) => {
          watched = true;
          emit({ type: "progress", job: { id: "j1", kind: "artifact_install", subject: { kind: "artifact_install", id: "thymos", display_name: "Lettuce Thymos" }, subject_detail: null, state: "running", progress: { current: 1, total: 10, unit: "bytes", label_code: "download", bytes_per_second: null }, created_at: 0, updated_at: 0, failure: null, result: null } } as never);
          return new Promise(() => {});
        },
      },
    );
    fireEvent.change(await screen.findByPlaceholderText("Character name"), { target: { value: "Aria" } });
    fireEvent.click(screen.getByRole("radio", { name: "Companion" }));
    const sheet = await screen.findByRole("dialog", { name: "Download the emotion model" });
    expect(screen.getByRole("radio", { name: "Companion" }).getAttribute("aria-checked")).toBe("true");
    fireEvent.click(within(sheet).getByRole("button", { name: "Download" }));
    await waitFor(() => expect(install).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(watched).toBe(true));
    expect(await within(sheet).findByText("Keep the app open until it finishes.")).toBeTruthy();
    expect(screen.getByRole("dialog", { name: "Download the emotion model" })).toBeTruthy();
  });

  it("closes the emotion model offer and keeps companion mode", async () => {
    await renderCreate({ companionEmotionStatus: () => ({ status: "ok", data: { type: "not_installed" } }) });
    fireEvent.change(await screen.findByPlaceholderText("Character name"), { target: { value: "Aria" } });
    fireEvent.click(screen.getByRole("radio", { name: "Companion" }));
    const sheet = await screen.findByRole("dialog", { name: "Download the emotion model" });
    fireEvent.click(within(sheet).getByRole("button", { name: "Not now" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Download the emotion model" })).toBeNull());
    expect(screen.getByRole("radio", { name: "Companion" }).getAttribute("aria-checked")).toBe("true");
  });

  it("waits for companion defaults before allowing forward navigation", async () => {
    let resolve!: (result: { status: "ok"; data: CharacterDraft }) => void;
    const pending = new Promise<{ status: "ok"; data: CharacterDraft }>((done) => { resolve = done; });
    await renderCreate({ characterDraftDefault: (mode) => mode === "companion" ? pending : { status: "ok", data: start } });
    fireEvent.change(await screen.findByPlaceholderText("Character name"), { target: { value: "Aria" } });
    fireEvent.click(screen.getByRole("radio", { name: "Companion" }));
    expect(continueButton().disabled).toBe(true);
    await act(async () => { resolve({ status: "ok", data: companionDraft as CharacterDraft }); });
    await waitFor(() => expect(continueButton().disabled).toBe(false));
    expect(screen.getByRole("radio", { name: "Companion" }).getAttribute("aria-checked")).toBe("true");
  });

  it("renders its copy through t() only", async () => {
    await renderCreate({}, "cimode");
    expect(await screen.findByPlaceholderText("identity.namePlaceholder")).toBeTruthy();
    expect(document.body.textContent).not.toMatch(/Who is it|Character name/);
  });
});

describe("character form", () => {
  it("closes the gap when a scene is removed and moves the default to the first scene left", () => {
    let form = newForm(start);
    form = withAddedScene(form, { text: "One", direction: "" }, "s1");
    form = withAddedScene(form, { text: "Two", direction: "Slow burn" }, "s2");
    expect(form.draft.defaults.default_scene_id).toBe("s1");
    expect(form.draft.scenes[1]!.direction).toBe("Slow burn");
    form = withoutScene(form, "s1");
    expect(form.draft.scenes.map((scene) => [scene.id, scene.ordinal])).toEqual([["s2", 0]]);
    expect(form.draft.defaults.default_scene_id).toBe("s2");
  });

  it("stores blank optional texts as null and trims the name", () => {
    const form = newForm({ ...start, profile: { ...start.profile, name: " Aria ", nickname: "  ", definition: "Kind" } });
    const draft = draftToSave(form);
    expect(draft.profile).toMatchObject({ name: "Aria", nickname: null, definition: "Kind" });
  });
});

describe("optional scenes", () => {
  it("omits empty scenes on save, reindexes survivors and repairs scene references", () => {
    let form = withAddedScene(newForm(start), { text: "   ", direction: "Unused guidance" }, "empty");
    form = withAddedScene(form, { text: "An opening", direction: "Keep this" }, "opening");
    form = { ...form, draft: { ...form.draft,
      variants: [{ id: "v1", scene_id: "empty", ordinal: 0, content: { format_version: 1, parts: [{ kind: "text", text: "Variant" }] }, direction: null }],
      starters: [{ id: "starter", name: "Starter", ordinal: 0, messages: [], scene_id: "empty", prompt_id: null, lorebooks: { kind: "inherit" } }],
    } };
    const result = draftToSave(form);
    expect(result.scenes.map((scene) => [scene.id, scene.ordinal])).toEqual([["opening", 0]]);
    expect(result.defaults.default_scene_id).toBe("opening");
    expect(result.variants).toEqual([]);
    expect(result.starters[0]!.scene_id).toBeNull();
    expect(form.draft.scenes).toHaveLength(2);
    expect(draftToSave(withDefaultScene(form, null)).defaults.default_scene_id).toBeNull();
  });

  it("does not treat the initial empty scene as entered work or save it", () => {
    const form = withAddedScene(newForm(start), { text: "", direction: "" }, "initial");
    expect(isTouched(form, start)).toBe(false);
    expect(draftToSave(form)).toEqual(start);
    expect(isTouched(withAddedScene(form, { text: "", direction: "Remember this" }, "direction"), start)).toBe(true);
  });
});
