import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createBrowserHistory, createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CharacterDraft_Serialize, CharacterSummary, ConversationSummary, GroupDraft_Serialize, GroupNoteView, GroupSaveRequest_Deserialize, GroupView_Serialize } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";
import { draftToSave, newGroupForm, sceneFromText } from "./data/form";
import { groupsQuery } from "./data/queries";
import defaultCharacter from "./fixtures/default-character.json";
import companionFixture from "./fixtures/companion-character.json";
import { createApiClient } from "@/api/client";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));
const characterDraft = defaultCharacter as CharacterDraft_Serialize;
const character = (id: string, name: string): CharacterSummary => ({ id, name, avatar: null, banner: null, description: `${name}'s description`, card_style: "circle", gradient: { type: "none" }, revision: 1, created_at: 1, updated_at: 1 });
const cast = [character("a", "Aria"), character("b", "Bram"), character("c", "Cleo")];
function group(draft: GroupDraft_Serialize): GroupView_Serialize {
  return { id: "g1", draft, archived: false, assets: [], revision: 1, created_at: 1, updated_at: 1 };
}
const savedGroup = group({ ...newGroupForm(characterDraft.presentation.chat_appearance, false).draft, name: "Harbor crew", members: cast.slice(0, 2).map((item, ordinal) => ({ character_id: item.id, ordinal, muted: false, model_profile_override: null })) });
const chat = (id: string, title: string, updatedAt: number): ConversationSummary => ({ id, title, kind: "group", source: { type: "group", group_id: "g1" }, avatars: [], background: null, revision: 1, last_message_preview: "By the harbor", updated_at: updatedAt, archived: false, message_count: 1, missing_models: [], memory_blocked: null, origin_conversation_id: null, origin_message_id: null });
async function renderGroups(path = "/groups", calls: MockCallHandlers = {}, browser = false) {
  const transport = createMockTransport({ calls: {
    appStatus: () => ({ status: "ok", data: { ...mockAppStatus, ui_state: { onboarding: { completed: true } } } }),
    characterDraftDefault: () => ({ status: "ok", data: characterDraft }),
    groupGet: () => ({ status: "ok", data: savedGroup }),
    groupLorebooksList: () => ({ status: "ok", data: [] }),
    groupNotesList: () => ({ status: "ok", data: [] }),
    characterGet: ({ character_id }) => ({ status: "ok", data: { id: character_id, draft: { ...characterDraft, profile: { ...characterDraft.profile, name: cast.find((item) => item.id === character_id)?.name ?? character_id } }, archived: false, assets: [], revision: 1, created_at: 1, updated_at: 1 } }),
    settingsGet: () => ({ status: "ok", data: { global: { dynamic_memory: { enabled: false } } } as never }),
    charactersList: () => ({ status: "ok", data: { items: cast, next_cursor: null } }),
    groupsList: () => ({ status: "ok", data: { items: [{ group: savedGroup, last_activity_at: 1 }], next_cursor: null } }),
    conversationsList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
    modelsList: () => ({ status: "ok", data: { models: [], default_model_id: null, revision: 1 } }),
    providerAccountsList: () => ({ status: "ok", data: [] }),
    promptsList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
    personasList: () => ({ status: "ok", data: { items: [], next_cursor: null, default_persona: { persona_id: null, revision: 1, created_at: 1, updated_at: 1 } } }),
    lorebooksList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
    ...calls,
  } });
  if (browser) window.history.replaceState(null, "", "/groups");
  const history = browser ? createBrowserHistory() : createMemoryHistory({ initialEntries: path === "/groups" ? [path] : ["/groups", path] });
  const runtime = await createAppRuntime({ transport, history, i18n: await createTestI18n("en") });
  await act(async () => { render(<App runtime={runtime} />); });
  if (browser) await act(async () => history.push(path));
  return { runtime, transport };
}
afterEach(cleanup);

async function selectMembers() {
  fireEvent.click((await screen.findByText("Aria")).closest("button")!);
  fireEvent.click(screen.getByText("Bram").closest("button")!);
  fireEvent.click(screen.getByRole("button", { name: "Continue to setup" }));
  await screen.findByRole("textbox", { name: "Group name" });
}

describe("group home and creator", () => {
  it("searches groups by member and lists every recent chat separately", async () => {
    await renderGroups("/groups", { conversationsList: (request) => ({ status: "ok", data: { items: request.kind === "group" ? [chat("new", "New voyage", 20), chat("old", "Old voyage", 10)] : [], next_cursor: null } }) });
    expect(await screen.findByText("Harbor crew")).toBeTruthy();
    fireEvent.change(screen.getByPlaceholderText("Search groups"), { target: { value: "Aria" } });
    expect(screen.getByText("1 of 1")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Layout: List" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Recent" }));
    expect(await screen.findByText("New voyage")).toBeTruthy();
    expect(screen.getByText("Old voyage")).toBeTruthy();
    expect(screen.getByText("2 of 2")).toBeTruthy();
  });

  it("fetches every page of groups", async () => {
    const list = vi.fn<NonNullable<MockCallHandlers["groupsList"]>>((request) => ({ status: "ok", data: { items: [{ group: { ...savedGroup, id: request.cursor ?? "first" }, last_activity_at: 1 }], next_cursor: request.cursor === null ? "second" : null } }));
    const api = createApiClient(createMockTransport({ calls: { groupsList: list } }));
    const result = await groupsQuery(api).queryFn!({} as never);
    expect(result.map((item) => item.group.id)).toEqual(["first", "second"]);
    expect(list).toHaveBeenCalledTimes(2);
  });

  it("requires two members and always offers the scene step", async () => {
    const { runtime } = await renderGroups("/groups/new", {}, true);
    expect((await screen.findByRole("button", { name: "Continue to setup" }) as HTMLButtonElement).disabled).toBe(true);
    await selectMembers();
    fireEvent.click(screen.getByRole("button", { name: "Continue to scene" }));
    fireEvent.click(await screen.findByRole("radio", { name: /Write one/ }));
    expect((screen.getByRole("button", { name: "Continue to extras" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(screen.getByRole("textbox", { name: "Scene" }), { target: { value: "At the harbor" } });
    expect((screen.getByRole("button", { name: "Continue to extras" }) as HTMLButtonElement).disabled).toBe(false);
    await act(async () => runtime.router.history.back());
    expect(await screen.findByRole("textbox", { name: "Group name" })).toBeTruthy();
    expect(runtime.router.state.location.pathname).toBe("/groups/new");
  });

  it("keeps the saved group and launch id when opening its first chat fails", async () => {
    const save = vi.fn<NonNullable<MockCallHandlers["groupSave"]>>((request) => ({ status: "ok", data: group(request.draft) }));
    const launch = vi.fn<NonNullable<MockCallHandlers["conversationLaunchGroup"]>>(() => ({ status: "error", error: { code: "unavailable", message: "diagnostic", details: null } }));
    const { runtime } = await renderGroups("/groups/new", { groupSave: save, conversationLaunchGroup: launch });
    await selectMembers();
    fireEvent.click(screen.getByRole("button", { name: "Continue to scene" }));
    fireEvent.click(await screen.findByRole("button", { name: "Continue to extras" }));
    fireEvent.click(await screen.findByRole("button", { name: "Create and chat" }));
    await screen.findByText("The group is saved. Try again to finish attaching lorebooks or opening its chat.");
    fireEvent.click(screen.getByRole("button", { name: "Create and chat" }));
    await waitFor(() => expect(launch).toHaveBeenCalledTimes(2));
    expect(save).toHaveBeenCalledTimes(1);
    expect(launch.mock.calls[0]![0].client_operation_id).toBe(launch.mock.calls[1]![0].client_operation_id);
    expect(runtime.router.state.location.pathname).toBe("/groups/new");
    expect(screen.queryByText("diagnostic")).toBeNull();
  });

  it("saves member order, persona, memory and custom scene in the group draft", async () => {
    const save = vi.fn<NonNullable<MockCallHandlers["groupSave"]>>((request: GroupSaveRequest_Deserialize) => ({ status: "ok", data: group(request.draft) }));
    const { runtime } = await renderGroups("/groups/new", { groupSave: save });
    await selectMembers();
    fireEvent.change(screen.getByRole("textbox", { name: "Group name" }), { target: { value: "  Crew  " } });
    fireEvent.click(screen.getByRole("radio", { name: /^Round Robin/ }));
    fireEvent.click(screen.getByRole("button", { name: "Continue to scene" }));
    fireEvent.click(await screen.findByRole("radio", { name: /^Write one/ }));
    fireEvent.change(screen.getByRole("textbox", { name: "Scene" }), { target: { value: "At the harbor" } });
    fireEvent.click(screen.getByRole("button", { name: "Continue to extras" }));
    fireEvent.click((await screen.findByText("Persona")).closest("button")!);
    fireEvent.click(within(await screen.findByRole("dialog", { name: "Persona" })).getByRole("button", { name: "No persona" }));
    fireEvent.click(screen.getByRole("button", { name: "Create group" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/groups"));
    expect(save.mock.calls[0]![0].draft).toMatchObject({ name: "Crew", speaker_selection: "round_robin", memory_policy: "manual", persona: { kind: "disabled" }, members: [{ character_id: "a", ordinal: 0 }, { character_id: "b", ordinal: 1 }], starting_scene: { scene: { content: { parts: [{ kind: "text", text: "At the harbor" }] } } } });
  });

  it("maps imported group speakers to existing characters before committing", async () => {
    const commit = vi.fn<NonNullable<MockCallHandlers["importCommit"]>>(() => ({ status: "ok", data: { kind: "imported", file_kind: "chat_jsonl", id: "g-imported", warnings: [] } }));
    const { runtime } = await renderGroups("/groups/new", {
      filesPickOpen: () => ({ status: "ok", data: { sources: [{ uri: "/tmp/crew.jsonl" }] } }),
      importPreview: () => ({ status: "ok", data: { kind: "chat_jsonl", name: "Crew", digest: "digest", content: { kind: "chat", title: "Crew", speakers: ["Aria", "Unknown speaker"], message_count: 3, group: true }, uec3: null, images: [], warnings: [] } }),
      importCommit: commit,
    });
    fireEvent.click(await screen.findByRole("button", { name: "Choose file" }));
    const dialog = await screen.findByRole("dialog", { name: "Import a group chat" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Choose file" }));
    const speaker = await within(dialog).findByRole("combobox", { name: "Unknown speaker" });
    expect((within(dialog).getByRole("button", { name: "Import" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(speaker, { target: { value: "b" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Import" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/groups"));
    expect(commit.mock.calls[0]![0]).toMatchObject({ digest: "digest", source: { uri: "/tmp/crew.jsonl" }, options: { speakers: { Aria: "a", "Unknown speaker": "b" } } });
  });

  it("omits a retained scene when no scene source is chosen", () => {
    const form = newGroupForm(characterDraft.presentation.chat_appearance, false);
    form.sceneSource = "none";
    form.draft.starting_scene = sceneFromText("A scene", "scene1");
    expect(draftToSave(form, "Crew").starting_scene).toBeNull();
    form.sceneSource = "custom";
    expect(draftToSave(form, "Crew").starting_scene).toEqual(form.draft.starting_scene);
  });
});


describe("group editor", () => {
  it("edits group appearance with explicit field inheritance and persists only on Save", async () => {
    let current = savedGroup;
    const save = vi.fn<NonNullable<MockCallHandlers["groupSave"]>>((request) => {
      current = { ...current, draft: request.draft, revision: current.revision + 1 };
      return { status: "ok", data: current };
    });
    await renderGroups("/groups/g1/edit?section=appearance", { groupSave: save, groupGet: () => ({ status: "ok", data: current }) });
    const names = await screen.findByRole("switch", { name: "Show names" });
    fireEvent.click(names);
    expect(save).not.toHaveBeenCalled();
    expect(screen.getAllByRole("img", { name: "Set for this group" })).toHaveLength(1);
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0]![0]).toMatchObject({ expected_revision: 1, draft: { presentation: { show_message_author: false }, presentation_overrides: ["show_message_author"] } });
    await waitFor(() => expect(screen.queryByRole("button", { name: "Save" })).toBeNull());
    fireEvent.click(screen.getByRole("button", { name: "Reset Show names to the app appearance" }));
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(2));
    expect(save.mock.calls[1]![0].draft.presentation_overrides).toEqual([]);
  });

  it("shows the preview in a side panel next to the settings on desktop and restyles it as settings change", async () => {
    await renderGroups("/groups/g1/edit?section=appearance");
    const names = await screen.findByRole("switch", { name: "Show names" });
    expect(screen.queryByRole("button", { name: "Preview" })).toBeNull();
    const composer = screen.getByRole("textbox", { name: "Message" });
    const panel = composer.closest("aside")!;
    expect(within(panel).getByRole("radio", { name: "Background image" })).toBeTruthy();
    expect(panel.contains(names)).toBe(false);
    expect(panel.className).toContain("w-130");
    const before = within(panel).queryAllByText("Seraphine").length;
    fireEvent.click(names);
    await waitFor(() => expect(within(composer.closest("aside")!).queryAllByText("Seraphine").length).not.toBe(before));
  });

  it("opens the full-screen preview from the phone header and closes it with Back", async () => {
    const { runtime } = await renderGroups("/groups/g1/edit?section=appearance", { appStatus: () => ({ status: "ok", data: { ...mockAppStatus, platform: "android", ui_state: { onboarding: { completed: true } } } }) });
    await screen.findByRole("switch", { name: "Show names" });
    expect(screen.queryByRole("textbox", { name: "Message" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Preview" }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByRole("textbox", { name: "Message" })).toBeTruthy();
    expect(within(dialog).getByRole("radio", { name: "Background image" })).toBeTruthy();
    expect(runtime.router.state.location.search.preview).toBe(true);
    await act(async () => runtime.router.history.back());
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(runtime.router.state.location.search.preview).toBeUndefined();
    fireEvent.click(screen.getByRole("button", { name: "Preview" }));
    fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Back" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(runtime.router.state.location.search.section).toBe("appearance");
  });

  it("inserts a scene image at the caret and completes character placeholders", async () => {
    const initial = { ...savedGroup, draft: { ...savedGroup.draft, starting_scene: sceneFromText("Start End", "scene") } };
    const save = vi.fn<NonNullable<MockCallHandlers["groupSave"]>>((request) => ({ status: "ok", data: group(request.draft) }));
    const ingest = vi.fn<NonNullable<MockCallHandlers["assetsIngest"]>>(() => ({ status: "ok", data: { asset_id: "image1", url: "asset://image1", media_type: "image/png", width: 100, height: 100 } as never }));
    await renderGroups("/groups/g1/edit?section=scene", { groupGet: () => ({ status: "ok", data: initial }), groupSave: save, filesPickOpen: () => ({ status: "ok", data: { sources: [{ uri: "/tmp/scene.png" }] } }), assetsIngest: ingest });
    const editor = await screen.findByRole("textbox", { name: "Scene" }) as HTMLTextAreaElement;
    editor.setSelectionRange(6, 6);
    fireEvent.click(screen.getByRole("button", { name: "Add image" }));
    await screen.findByRole("button", { name: "Scene image 1" });
    expect(editor.value).toMatch(/^Start \{\{image::[^}]+\}\}End$/);
    fireEvent.change(editor, { target: { value: editor.value + '{{@"Ar', selectionStart: editor.value.length + 6 } });
    await screen.findByRole("button", { name: /Aria.*\{\{@/ });
    fireEvent.keyDown(editor, { key: "Enter" });
    expect(editor.value.endsWith('{{@"Aria"}}')).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0]![0].draft.starting_scene?.scene.content.parts).toMatchObject([{ kind: "text", text: "Start " }, { kind: "inline_asset" }, { kind: "text", text: 'End{{@"Aria"}}' }]);
    expect(ingest.mock.calls[0]![0].role).toBe("reference_image");
  });

  it("uses the design-lab tabs and keeps member changes local until Save", async () => {
    const save = vi.fn<NonNullable<MockCallHandlers["groupSave"]>>((request) => ({ status: "ok", data: group(request.draft) }));
    await renderGroups("/groups/g1/edit", { groupSave: save });
    const name = await screen.findByRole("textbox", { name: "Group name" });
    expect((name as HTMLInputElement).value).toBe("Harbor crew");
    expect(screen.queryByRole("button", { name: "Continue to setup" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
    expect(screen.getByRole("tab", { name: "Scene" })).toBeTruthy();
    fireEvent.change(name, { target: { value: "New crew" } });
    fireEvent.click(screen.getByRole("tab", { name: /Members/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Aria: options" }));
    const menu = await screen.findByRole("dialog", { name: "Aria" });
    fireEvent.click(within(menu).getByRole("switch", { name: "Answers on their own" }));
    expect(save).not.toHaveBeenCalled();
    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(screen.getByRole("button", { name: "Bram: options" }));
    expect((await screen.findByRole("switch", { name: "Answers on their own" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(screen.getByRole("button", { name: "Discard" }));
    fireEvent.click(screen.getByRole("tab", { name: "Group" }));
    expect((await screen.findByRole("textbox", { name: "Group name" }) as HTMLInputElement).value).toBe("Harbor crew");
    await waitFor(() => expect(screen.queryByRole("button", { name: "Save" })).toBeNull());
  });

  it("saves a draft with its revision and stays in the editor", async () => {
    let current = savedGroup;
    const save = vi.fn<NonNullable<MockCallHandlers["groupSave"]>>((request) => {
      current = { ...current, draft: request.draft, revision: current.revision + 1 };
      return { status: "ok", data: current };
    });
    const { runtime } = await renderGroups("/groups/g1/edit", { groupSave: save, groupGet: () => ({ status: "ok", data: current }) });
    fireEvent.change(await screen.findByRole("textbox", { name: "Group name" }), { target: { value: "  Renamed crew  " } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(screen.queryByRole("button", { name: "Save" })).toBeNull());
    expect(runtime.router.state.location.pathname).toBe("/groups/g1/edit");
    expect((screen.getByRole("textbox", { name: "Group name" }) as HTMLInputElement).value).toBe("Renamed crew");
    expect(save.mock.calls[0]![0]).toMatchObject({ group_id: "g1", expected_revision: 1, draft: { name: "Renamed crew" } });
  });

  it("reloads a conflict with local edits on top and saves against the new revision", async () => {
    let current = savedGroup;
    let first = true;
    const save = vi.fn<NonNullable<MockCallHandlers["groupSave"]>>((request) => {
      if (first) {
        first = false;
        current = { ...current, revision: 7, draft: { ...current.draft, name: "Remote name", speaker_selection: "heuristic" } };
        return { status: "error", error: { code: "conflict", message: "diagnostic", details: null } };
      }
      current = { ...current, draft: request.draft, revision: 8 };
      return { status: "ok", data: current };
    });
    await renderGroups("/groups/g1/edit", { groupSave: save, groupGet: () => ({ status: "ok", data: current }) });
    fireEvent.change(await screen.findByRole("textbox", { name: "Group name" }), { target: { value: "Local name" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await screen.findByText("This group changed somewhere else");
    expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Reload" }));
    await waitFor(() => expect(screen.queryByText("This group changed somewhere else")).toBeNull());
    expect((screen.getByRole("textbox", { name: "Group name" }) as HTMLInputElement).value).toBe("Local name");
    fireEvent.click(screen.getByRole("tab", { name: "Chat" }));
    expect((await screen.findByRole("radio", { name: /^Heuristic/ })).getAttribute("aria-checked")).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(2));
    expect(save.mock.calls[1]![0]).toMatchObject({ expected_revision: 7, draft: { name: "Local name", speaker_selection: "heuristic" } });
    expect(save.mock.calls[0]![0].client_operation_id).not.toBe(save.mock.calls[1]![0].client_operation_id);
  });

  it("adds members at the end and lets an unchanged reduced group save other settings", async () => {
    const reduced = { ...savedGroup, draft: { ...savedGroup.draft, members: savedGroup.draft.members.slice(0, 1) } };
    await renderGroups("/groups/g1/edit", { groupGet: () => ({ status: "ok", data: reduced }) });
    fireEvent.change(await screen.findByRole("textbox", { name: "Group name" }), { target: { value: "Still here" } });
    expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(screen.getByRole("tab", { name: /Members/ }));
    expect(await screen.findByText("One member left")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Add characters" }));
    const dialog = await screen.findByRole("dialog", { name: "Add characters" });
    fireEvent.click(within(dialog).getByRole("checkbox", { name: "Bram" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Add 1 character" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(screen.getByRole("button", { name: "Bram: options" })).toBeTruthy();
    expect(screen.queryByText("One member left")).toBeNull();
  });

  it("keeps scene direction and variants when editing an existing opening", async () => {
    const starting = sceneFromText("Old opening", "scene1");
    starting.scene.direction = "At the docks";
    starting.scene.selected_variant_id = "variant1";
    starting.variants = [{ id: "variant1", scene_id: "scene1", ordinal: 0, content: { format_version: 1, parts: [{ kind: "text", text: "Another opening" }] }, direction: null }];
    let current = { ...savedGroup, draft: { ...savedGroup.draft, starting_scene: starting } };
    const save = vi.fn<NonNullable<MockCallHandlers["groupSave"]>>((request) => {
      current = { ...current, revision: 2, draft: request.draft } as typeof current;
      return { status: "ok", data: current };
    });
    await renderGroups("/groups/g1/edit", { groupGet: () => ({ status: "ok", data: current }), groupSave: save });
    fireEvent.click(await screen.findByRole("tab", { name: "Scene" }));
    fireEvent.change(await screen.findByRole("textbox", { name: "Scene" }), { target: { value: "New opening" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0]![0].draft.starting_scene).toMatchObject({ scene: { direction: "At the docks", selected_variant_id: null, content: { parts: [{ kind: "text", text: "New opening" }] } }, variants: starting.variants });
  });

  it("closes member sheets on browser back before asking to discard edits", async () => {
    const { runtime } = await renderGroups("/groups/g1/edit", {}, true);
    fireEvent.change(await screen.findByRole("textbox", { name: "Group name" }), { target: { value: "Draft name" } });
    fireEvent.click(screen.getByRole("tab", { name: /Members/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Aria: options" }));
    await screen.findByRole("dialog", { name: "Aria" });
    await act(async () => runtime.router.history.back());
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(runtime.router.state.location.pathname).toBe("/groups/g1/edit");
    await act(async () => runtime.router.history.back());
    expect(await screen.findByRole("dialog", { name: "Discard your changes?" })).toBeTruthy();
  });
});

const note = (patch: Partial<GroupNoteView> = {}): GroupNoteView => ({ id: "n1", group_id: "g1", visible_to: [], label: "Supplies", content: "Food is running low.", available_at: 1_700_000_000_000, expires_at: null, recurrence: "none", recurrence_window_ms: null, enabled: true, created_at: 1, updated_at: 1, ...patch });

describe("group time awareness and notes", () => {
  it("saves the time default and the character notes switch with the group", async () => {
    let current = savedGroup;
    const save = vi.fn<NonNullable<MockCallHandlers["groupSave"]>>((request) => {
      current = { ...current, draft: request.draft, revision: current.revision + 1 };
      return { status: "ok", data: current };
    });
    await renderGroups("/groups/g1/edit?section=chat", { groupSave: save, groupGet: () => ({ status: "ok", data: current }) });
    const awareness = await screen.findByRole("switch", { name: "Time awareness" });
    expect(awareness.getAttribute("aria-checked")).toBe("false");
    expect(screen.getByRole("switch", { name: "Use character notes" }).getAttribute("aria-checked")).toBe("true");
    fireEvent.click(awareness);
    fireEvent.click(screen.getByRole("switch", { name: "Use character notes" }));
    expect(save).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0]![0].draft).toMatchObject({ time_awareness: true, disable_character_notes: true });
  });

  it("saves companion mode with the group and names members without a Soul when it is switched on", async () => {
    const withSoul = { ...characterDraft.defaults, companion_soul: { ...companionFixture.defaults.companion_soul, soul: { ...companionFixture.defaults.companion_soul.soul, essence: "Calm and watchful." }, authored_facts: [] } };
    let current = savedGroup;
    const save = vi.fn<NonNullable<MockCallHandlers["groupSave"]>>((request) => {
      current = { ...current, draft: request.draft, revision: current.revision + 1 };
      return { status: "ok", data: current };
    });
    await renderGroups("/groups/g1/edit?section=chat", {
      groupSave: save,
      groupGet: () => ({ status: "ok", data: current }),
      characterGet: ({ character_id }) => ({ status: "ok", data: { id: character_id, draft: { ...characterDraft, profile: { ...characterDraft.profile, name: cast.find((item) => item.id === character_id)?.name ?? character_id }, defaults: character_id === "b" ? withSoul : { ...characterDraft.defaults, companion_soul: null } }, archived: false, assets: [], revision: 1, created_at: 1, updated_at: 1 } as never }),
    });
    const companionMode = await screen.findByRole("radio", { name: /^Companion/ });
    expect(companionMode.getAttribute("aria-checked")).toBe("false");
    fireEvent.click(companionMode);
    const dialog = await screen.findByRole("dialog", { name: "No Soul written" });
    expect(within(dialog).getByText(/No Soul is written for: Aria\./)).toBeTruthy();
    expect(within(dialog).queryByRole("button", { name: "Edit the Soul of Bram" })).toBeNull();
    fireEvent.click(within(dialog).getByRole("button", { name: "Continue" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "No Soul written" })).toBeNull());
    expect(screen.getByRole("radio", { name: /^Companion/ }).getAttribute("aria-checked")).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0]![0].draft).toMatchObject({ companion_mode: true });
  });

  it("shows the direct-chat memory rows only while companion mode is on and saves their draft fields", async () => {
    const on = group({ ...savedGroup.draft, companion_mode: true, companion_personal_memories: true, companion_memory_write_back: false });
    const save = vi.fn<NonNullable<MockCallHandlers["groupSave"]>>((request) => ({ status: "ok", data: group(request.draft) }));
    await renderGroups("/groups/g1/edit?section=chat", { groupSave: save, groupGet: () => ({ status: "ok", data: on }) });
    const personal = await screen.findByRole("switch", { name: "Bring direct-chat memories" });
    const writeBack = screen.getByRole("switch", { name: "Write back to direct chats" });
    expect(personal.getAttribute("aria-checked")).toBe("true");
    expect(writeBack.getAttribute("aria-checked")).toBe("false");
    expect(screen.getByText("Each member also recalls its own chats with you. The others never see these.")).toBeTruthy();
    expect(screen.getByText("What happens here stays in this group.")).toBeTruthy();
    fireEvent.click(personal);
    fireEvent.click(writeBack);
    expect(screen.getByText("Members only recall what happened in this group.")).toBeTruthy();
    expect(screen.getByText("What a member takes from this group is also remembered in its own chats with you, if that character shares memory across chats.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0]![0].draft).toMatchObject({ companion_personal_memories: false, companion_memory_write_back: true });
    fireEvent.click(screen.getByRole("radio", { name: /^Roleplay/ }));
    await waitFor(() => expect(screen.queryByRole("switch", { name: "Bring direct-chat memories" })).toBeNull());
    expect(screen.queryByRole("switch", { name: "Write back to direct chats" })).toBeNull();
  });

  it("offers only Continue in the creator's missing Soul warning", async () => {
    await renderGroups("/groups/new");
    fireEvent.click((await screen.findByText("Aria")).closest("button")!);
    fireEvent.click(screen.getByText("Bram").closest("button")!);
    fireEvent.click(screen.getByRole("button", { name: "Continue to setup" }));
    await screen.findByRole("textbox", { name: "Group name" });
    fireEvent.click(await screen.findByRole("radio", { name: /^Companion/ }));
    const dialog = await screen.findByRole("dialog", { name: "No Soul written" });
    expect(within(dialog).getByText(/No Soul is written for: Aria and Bram\./)).toBeTruthy();
    expect(within(dialog).queryByRole("button", { name: /Edit the Soul of/ })).toBeNull();
    expect(within(dialog).getByRole("button", { name: "Continue" })).toBeTruthy();
  });

  it("warns when a character without a Soul joins a companion group and still adds it", async () => {
    const on = group({ ...savedGroup.draft, companion_mode: true });
    const save = vi.fn<NonNullable<MockCallHandlers["groupSave"]>>((request) => ({ status: "ok", data: group(request.draft) }));
    await renderGroups("/groups/g1/edit?section=members", { groupSave: save, groupGet: () => ({ status: "ok", data: on }) });
    fireEvent.click(await screen.findByRole("button", { name: "Add characters" }));
    const sheet = await screen.findByRole("dialog", { name: "Add characters" });
    fireEvent.click(within(sheet).getByRole("checkbox", { name: "Cleo" }));
    fireEvent.click(within(sheet).getByRole("button", { name: "Add 1 character" }));
    const dialog = await screen.findByRole("dialog", { name: "No Soul written" });
    expect(within(dialog).getByText(/No Soul is written for: Cleo\./)).toBeTruthy();
    fireEvent.click(within(dialog).getByRole("button", { name: "Continue" }));
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0]![0].draft.members.map((member) => member.character_id)).toEqual(["a", "b", "c"]);
  });

  it("lists the group's notes with who sees them and toggles one at once", async () => {
    const upsert = vi.fn<NonNullable<MockCallHandlers["groupNotesUpsert"]>>((request) => ({ status: "ok", data: note({ enabled: request.enabled }) }));
    await renderGroups("/groups/g1/edit?section=chat", {
      groupNotesList: () => ({ status: "ok", data: [note({ visible_to: ["a"] }), note({ id: "n2", label: "", content: "Open to all." })] }),
      groupNotesUpsert: upsert,
    });
    expect(await screen.findByText("Supplies")).toBeTruthy();
    expect(screen.getByText(/Aria/)).toBeTruthy();
    expect(screen.getByText("Untitled note")).toBeTruthy();
    fireEvent.click(screen.getByRole("switch", { name: "Supplies enabled" }));
    await waitFor(() => expect(upsert).toHaveBeenCalledTimes(1));
    expect(upsert.mock.calls[0]![0]).toMatchObject({ group_id: "g1", note_id: "n1", visible_to: ["a"], enabled: false });
  });

  it("adds a note aimed at one member and deletes another", async () => {
    const upsert = vi.fn<NonNullable<MockCallHandlers["groupNotesUpsert"]>>(() => ({ status: "ok", data: note() }));
    const remove = vi.fn<NonNullable<MockCallHandlers["groupNotesDelete"]>>(() => ({ status: "ok", data: null }));
    await renderGroups("/groups/g1/edit?section=chat", { groupNotesList: () => ({ status: "ok", data: [note({ id: "old", label: "Old note" })] }), groupNotesUpsert: upsert, groupNotesDelete: remove });
    fireEvent.click(await screen.findByRole("button", { name: /Add a note/ }));
    const dialog = await screen.findByRole("dialog", { name: "New scheduled note" });
    expect(within(dialog).queryByRole("button", { name: "Cleo" })).toBeNull();
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Content" }), { target: { value: "  Aria hears a rumor.  " } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Aria" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(upsert).toHaveBeenCalledTimes(1));
    expect(upsert.mock.calls[0]![0]).toMatchObject({ group_id: "g1", note_id: null, visible_to: ["a"], content: "Aria hears a rumor.", enabled: true });
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "New scheduled note" })).toBeNull());
    fireEvent.click(screen.getAllByRole("button", { name: "Edit" })[0]!);
    const editor = await screen.findByRole("dialog", { name: "Edit scheduled note" });
    fireEvent.click(within(editor).getByRole("button", { name: "Delete note" }));
    const confirmation = await screen.findAllByRole("dialog");
    fireEvent.click(within(confirmation.at(-1)!).getByRole("button", { name: "Delete note" }));
    await waitFor(() => expect(remove).toHaveBeenCalledTimes(1));
    expect(remove.mock.calls[0]![0].note_id).toBe("old");
  });

  it("keeps draft notes in the creator and writes them right after the group is saved", async () => {
    const save = vi.fn<NonNullable<MockCallHandlers["groupSave"]>>((request) => ({ status: "ok", data: group(request.draft) }));
    const upsert = vi.fn<NonNullable<MockCallHandlers["groupNotesUpsert"]>>(() => ({ status: "ok", data: note() }));
    const { runtime } = await renderGroups("/groups/new", { groupSave: save, groupNotesUpsert: upsert });
    await selectMembers();
    fireEvent.click(screen.getByRole("button", { name: "Continue to scene" }));
    fireEvent.click(await screen.findByRole("button", { name: "Continue to extras" }));
    fireEvent.click(await screen.findByRole("switch", { name: "Time awareness" }));
    fireEvent.click(await screen.findByRole("button", { name: /Add a note/ }));
    const dialog = await screen.findByRole("dialog", { name: "New scheduled note" });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Content" }), { target: { value: "Bram knows the way." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Bram" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "New scheduled note" })).toBeNull());
    expect(upsert).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Create group" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/groups"));
    expect(save.mock.calls[0]![0].draft).toMatchObject({ time_awareness: true, disable_character_notes: false });
    expect(upsert).toHaveBeenCalledTimes(1);
    expect(upsert.mock.calls[0]![0]).toMatchObject({ group_id: "g1", note_id: null, visible_to: ["b"], content: "Bram knows the way." });
    expect(save.mock.invocationCallOrder[0]!).toBeLessThan(upsert.mock.invocationCallOrder[0]!);
  });
});
