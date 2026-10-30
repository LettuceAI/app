import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createBrowserHistory, createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CharacterDraft_Serialize, CharacterSummary, ConversationSummary, GroupDraft_Serialize, GroupSaveRequest_Deserialize, GroupView_Serialize } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";
import { draftToSave, newGroupForm, sceneFromText } from "./data/form";
import { groupsQuery } from "./data/queries";
import defaultCharacter from "./fixtures/default-character.json";
import { createApiClient } from "@/api/client";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));
const characterDraft = defaultCharacter as CharacterDraft_Serialize;
const character = (id: string, name: string): CharacterSummary => ({ id, name, avatar: null, banner: null, description: `${name}'s description`, card_style: "circle", gradient: { type: "none" }, revision: 1, created_at: 1, updated_at: 1 });
const cast = [character("a", "Aria"), character("b", "Bram"), character("c", "Cleo")];
function group(draft: GroupDraft_Serialize): GroupView_Serialize {
  return { id: "g1", draft, archived: false, assets: [], revision: 1, created_at: 1, updated_at: 1 };
}
const savedGroup = group({ ...newGroupForm(characterDraft.presentation.chat_appearance, false).draft, name: "Harbor crew", members: cast.slice(0, 2).map((item, ordinal) => ({ character_id: item.id, ordinal, muted: false, model_profile_override: null })) });
const chat = (id: string, title: string, updatedAt: number): ConversationSummary => ({ id, title, kind: "group", source: { type: "group", group_id: "g1" }, avatars: [], background: null, revision: 1, last_message_preview: "By the harbor", updated_at: updatedAt, archived: false, message_count: 1, chat_mode: "conversation", missing_models: [], memory_blocked: null, origin_conversation_id: null, origin_message_id: null });
async function renderGroups(path = "/groups", calls: MockCallHandlers = {}, browser = false) {
  const transport = createMockTransport({ calls: {
    appStatus: () => ({ status: "ok", data: { ...mockAppStatus, ui_state: { onboarding: { completed: true } } } }),
    characterDraftDefault: () => ({ status: "ok", data: characterDraft }),
    groupGet: () => ({ status: "ok", data: savedGroup }),
    groupLorebooksList: () => ({ status: "ok", data: [] }),
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
    fireEvent.click(screen.getByRole("radio", { name: "Recent" }));
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

  it("requires two members and changes the wizard when roleplay is chosen", async () => {
    const { runtime } = await renderGroups("/groups/new", {}, true);
    expect((await screen.findByRole("button", { name: "Continue to setup" }) as HTMLButtonElement).disabled).toBe(true);
    await selectMembers();
    fireEvent.click(screen.getByRole("radio", { name: "Roleplay" }));
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
    fireEvent.click(screen.getByRole("button", { name: "Continue to extras" }));
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
    fireEvent.click(screen.getByRole("radio", { name: "Roleplay" }));
    fireEvent.click(screen.getByRole("button", { name: "Continue to scene" }));
    fireEvent.click(await screen.findByRole("radio", { name: /^Write one/ }));
    fireEvent.change(screen.getByRole("textbox", { name: "Scene" }), { target: { value: "At the harbor" } });
    fireEvent.click(screen.getByRole("button", { name: "Continue to extras" }));
    fireEvent.click((await screen.findByText("Persona")).closest("button")!);
    fireEvent.click(within(await screen.findByRole("dialog", { name: "Persona" })).getByRole("button", { name: "No persona" }));
    fireEvent.click(screen.getByRole("button", { name: "Create group" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/groups"));
    expect(save.mock.calls[0]![0].draft).toMatchObject({ name: "Crew", chat_mode: "roleplay", speaker_selection: "round_robin", memory_policy: "manual", persona: { kind: "disabled" }, members: [{ character_id: "a", ordinal: 0 }, { character_id: "b", ordinal: 1 }], starting_scene: { scene: { content: { parts: [{ kind: "text", text: "At the harbor" }] } } } });
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

  it("omits a retained roleplay scene when saving in conversation mode", () => {
    const form = newGroupForm(characterDraft.presentation.chat_appearance, false);
    form.sceneSource = "custom";
    form.draft.starting_scene = sceneFromText("A scene", "scene1");
    expect(draftToSave(form, "Crew").starting_scene).toBeNull();
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
    expect(screen.getAllByText("Set for this group").length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0]![0]).toMatchObject({ expected_revision: 1, draft: { presentation: { show_message_author: true }, presentation_overrides: ["show_message_author"] } });
    await waitFor(() => expect(screen.queryByRole("button", { name: "Save" })).toBeNull());
    fireEvent.click(screen.getByRole("button", { name: "Follow app" }));
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(2));
    expect(save.mock.calls[1]![0].draft.presentation_overrides).toEqual([]);
  });

  it("inserts a scene image at the caret and completes character placeholders", async () => {
    const initial = { ...savedGroup, draft: { ...savedGroup.draft, chat_mode: "roleplay" as const, starting_scene: sceneFromText("Start End", "scene") } };
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
    expect(screen.queryByRole("tab", { name: "Scene" })).toBeNull();
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
    let current = { ...savedGroup, draft: { ...savedGroup.draft, chat_mode: "roleplay" as const, starting_scene: starting } };
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
