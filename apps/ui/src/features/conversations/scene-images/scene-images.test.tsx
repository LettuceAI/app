import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { dismissTopLayer, toast } from "@lettuceai/crisp";
import { afterEach, expect, it, vi } from "vitest";
import type { ConversationSettingsView, ConversationView, JobEvent, JobView, ModelView, ProviderAccountView, SceneImageView, SettingsView, TimelineMessage } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers, type MockStreamHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

const pending: SceneImageView = { state: "pending", mode: "ask_first", prompt: "A quiet harbour at sunset", job_id: null, failure: null };
const reply = (scene_image: SceneImageView | null = null): TimelineMessage => ({
  id: "a1", role: "assistant", author_participant_id: "pc", parts: [{ type: "text", text: "The harbour falls silent." }],
  reasoning: null, created_at: 1_700_000_000_000, candidate_index: 0, candidate_count: 1, pinned: false, scene_image,
});
const settings = {
  conversation_id: "c1", kind: "direct", title: "Harbour", revision: 1, settings_revision: 1, author_note: null,
  persona: { id: null, source: "app_default" }, prompt: { id: null, source: "app_default" }, lorebooks: { ids: [], source: "app_default" },
  model: { id: "writer", source: "app_default" }, background: { asset: null, hidden: false, source: "app_default" },
  scene: { id: null, source: "app_default" }, memory: { mode: "disabled", source: "app_default" },
  disable_character_lorebooks: null, disable_character_notes: null, speaker_selection: null, members: null, model_settings: { chat_parameters: {}, llama_cpp: {}, stable_diffusion: {} } as never, time: { enabled: false, mode: "live", anchor_at: null, set_at: null, effective_at: 1_700_000_000_000 }, companion: false,
} satisfies ConversationSettingsView;
const imageSettings = {
  avatar_enabled: false, avatar_model_profile_id: null, scene_enabled: true, scene_automatic_enabled: true, scene_mode: "ask_first" as const,
  scene_model_profile_id: "image", scene_writer_model_profile_id: "writer", creation_helper_model_profile_id: null, scene_default_size: null,
};
const imageModel = { id: "image", kind: "image", provider_account_id: "provider", display_name: "Image model", external_model_id: "image", input_scopes: ["text", "image"], output_scopes: ["image"] } as unknown as ModelView;
const writerModel = { ...imageModel, id: "writer", kind: "chat", display_name: "Writer", output_scopes: ["text"] } as ModelView;
const job = (patch: Partial<JobView> = {}): JobView => ({ id: "j1", kind: "image_generate", subject: { kind: "image_request", id: "r1" }, subject_detail: null, state: "running", progress: { current: 2, total: 10, unit: "items", label_code: null, bytes_per_second: null }, created_at: 1, updated_at: 1, result: null, failure: null, ...patch });

async function open(options: { initial?: TimelineMessage; calls?: MockCallHandlers; streams?: MockStreamHandlers; path?: string; group?: boolean; enabled?: boolean; models?: ModelView[] } = {}) {
  let message = options.initial ?? reply();
  const transport = createMockTransport({ calls: {
    appStatus: () => ({ status: "ok", data: { ...mockAppStatus, ui_state: { onboarding: { completed: true } } } }),
    conversationOpen: () => ({ status: "ok", data: {
      id: "c1", origin_conversation_id: null, origin_message_id: null, kind: options.group ? "group" : "direct", title: "Harbour", branch: { branch_id: "b1", head_message_id: "a1" },
      participants: [{ id: "pc", role: "character", name: "Cassian", character_id: "ch1", avatar: null }, { id: "pu", role: "user", name: "You", character_id: null, avatar: null }],
      messages: { items: [message], next_cursor: null }, pending_turn_id: null, can_send: true, revision: 1, settings_revision: 1, archived: false,
      source: options.group ? { type: "group", source_group_id: "g1" } : { type: "direct", character_id: "ch1" }, missing_models: [], memory_blocked: null,
    } as ConversationView }),
    conversationSettingsGet: () => ({ status: "ok", data: settings }),
    settingsGet: () => ({ status: "ok", data: { global: { image_generation: { ...imageSettings, scene_enabled: options.enabled ?? true }, ui_preferences: {} }, revision: 1 } as unknown as SettingsView }),
    modelsList: () => ({ status: "ok", data: { models: options.models ?? [imageModel, writerModel], default_model_id: "writer", revision: 1 } }),
    providerAccountsList: () => ({ status: "ok", data: [{ id: "provider", provider_kind: "openai", label: "Provider" } as ProviderAccountView] }),
    charactersList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
    conversationsLatestByCharacter: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
    conversationsList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
    llmMetricsForMessage: () => ({ status: "ok", data: null }), messageUsage: () => ({ status: "ok", data: null }),
    messageSceneCast: () => ({ status: "ok", data: { characters: [{ character_id: "ch1", name: "Cassian", avatar: null, selected: true }], persona: null } }),
    ...options.calls,
  }, ...(options.streams ? { streams: options.streams } : {}) });
  const runtime = await createAppRuntime({ transport, history: createMemoryHistory({ initialEntries: ["/chat/c1", options.path ?? "/chat/c1"] }), i18n: await createTestI18n("en") });
  await act(async () => { render(<App runtime={runtime} />); });
  await screen.findAllByText("The harbour falls silent.");
  return { runtime, transport, setMessage: (next: TimelineMessage) => { message = next; } };
}

async function openFlow() {
  fireEvent.contextMenu(screen.getByText("The harbour falls silent."));
  fireEvent.click(await screen.findByRole("button", { name: /Generate scene image/ }));
  const dialog = await screen.findByRole("dialog", { name: "Generate scene image" });
  await within(dialog).findByRole("button", { name: /Write a prompt/ });
  return dialog;
}

afterEach(() => { cleanup(); toast.dismiss(); });

it("opens Markdown images without asset lookups and supports dismissing the viewer", async () => {
  const prompt = vi.fn<NonNullable<MockCallHandlers["imageAssetPrompt"]>>(() => ({ status: "ok", data: null }));
  await open({ initial: { ...reply(), parts: [{ type: "text", text: "The harbour falls silent.\n\n![Harbor view](https://example.com/harbor.png)" }] }, calls: { imageAssetPrompt: prompt } });
  fireEvent.click(screen.getByRole("button", { name: "Harbor view" }));
  const viewer = await screen.findByRole("dialog", { name: "Harbor view" });
  expect(within(viewer).getByAltText("Harbor view").getAttribute("src")).toBe("https://example.com/harbor.png");
  expect(prompt).not.toHaveBeenCalled();
  fireEvent.keyDown(document, { key: "Escape" });
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "Harbor view" })).toBeNull());
  fireEvent.click(screen.getByRole("button", { name: "Harbor view" }));
  await screen.findByRole("dialog", { name: "Harbor view" });
  await act(async () => { expect(dismissTopLayer()).toBe(true); });
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "Harbor view" })).toBeNull());
});

it("opens a message image in the shared viewer and shows its recorded generation prompt", async () => {
  const prompt = vi.fn<NonNullable<MockCallHandlers["imageAssetPrompt"]>>(() => ({ status: "ok", data: "Cassian at the harbour\nSoft evening light" }));
  await open({ initial: { ...reply(), parts: [...reply().parts, { type: "media", asset: { asset_id: "scene", url: "asset://scene" }, kind: "image", role: "attachment" }] }, calls: { imageAssetPrompt: prompt } });
  expect(prompt).not.toHaveBeenCalled();
  const thumbnail = screen.getByRole("button", { name: "Attached image" });
  thumbnail.focus();
  const focus = vi.spyOn(thumbnail, "focus");
  fireEvent.click(thumbnail);
  const viewer = await screen.findByRole("dialog", { name: "Attached image" });
  await within(viewer).findByText("Cassian at the harbour Soft evening light");
  expect(within(viewer).getByText("Image prompt")).toBeTruthy();
  expect(prompt).toHaveBeenCalledExactlyOnceWith({ asset_id: "scene" });
  fireEvent.keyDown(document, { key: "Escape" });
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "Attached image" })).toBeNull());
  expect(focus).toHaveBeenCalledWith({ preventScroll: true });
});

it("does not label an uploaded image with a prompt and loads each selected image separately", async () => {
  const prompt = vi.fn<NonNullable<MockCallHandlers["imageAssetPrompt"]>>(({ asset_id }) => ({ status: "ok", data: asset_id === "generated" ? "The generated picture" : null }));
  const images = ["uploaded", "generated"].map((asset_id) => ({ type: "media" as const, asset: { asset_id, url: `asset://${asset_id}` }, kind: "image" as const, role: "attachment" as const }));
  await open({ initial: { ...reply(), parts: [...reply().parts, ...images] }, calls: { imageAssetPrompt: prompt } });
  fireEvent.click(screen.getAllByRole("button", { name: "Attached image" })[0]!);
  const viewer = await screen.findByRole("dialog", { name: "Attached image" });
  await waitFor(() => expect(prompt).toHaveBeenCalledWith({ asset_id: "uploaded" }));
  expect(within(viewer).queryByText("Image prompt")).toBeNull();
  fireEvent.click(within(viewer).getByRole("button", { name: "Next image" }));
  await within(viewer).findByText("The generated picture");
  expect(prompt).toHaveBeenCalledWith({ asset_id: "generated" });
});

it("restores the chat reading position after Generate closes the drawer", async () => {
  const generate = vi.fn<NonNullable<MockCallHandlers["messageSceneImageGenerate"]>>(() => ({ status: "ok", data: { job_id: "j1" } }));
  await open({ calls: { messageSceneImageGenerate: generate } });
  const scroller = document.getElementById("m-a1")!.closest<HTMLElement>(".overflow-y-auto")!;
  let top = 600;
  Object.defineProperties(scroller, {
    clientHeight: { configurable: true, get: () => 400 },
    scrollHeight: { configurable: true, get: () => 2400 },
    scrollTop: { configurable: true, get: () => top, set: (value: number) => { top = Math.max(0, Math.min(value, 2000)); } },
  });
  fireEvent.wheel(scroller, { deltaY: -100 });
  fireEvent.scroll(scroller);
  const dialog = await openFlow();
  fireEvent.click(within(dialog).getByRole("button", { name: /Write a prompt/ }));
  fireEvent.change(within(dialog).getByRole("textbox", { name: "Scene prompt" }), { target: { value: "Stay by this message" } });
  scroller.scrollTop = 0;
  fireEvent.click(within(dialog).getByRole("button", { name: "Generate image" }));
  await waitFor(() => expect(generate).toHaveBeenCalledTimes(1));
  await waitFor(() => expect(scroller.scrollTop).toBe(600));
});

it("writes a manual prompt, refuses blank input and submits the trimmed prompt once", async () => {
  const generate = vi.fn<NonNullable<MockCallHandlers["messageSceneImageGenerate"]>>(() => ({ status: "ok", data: { job_id: "j1" } }));
  const { runtime } = await open({ calls: { messageSceneImageGenerate: generate } });
  const dialog = await openFlow();
  expect(runtime.router.state.location.search).toMatchObject({ sceneImage: "a1" });
  fireEvent.click(within(dialog).getByRole("button", { name: /Write a prompt/ }));
  const submit = within(dialog).getByRole("button", { name: "Generate image" });
  expect(submit).toHaveProperty("disabled", true);
  fireEvent.change(within(dialog).getByRole("textbox", { name: "Scene prompt" }), { target: { value: "  Cassian by the harbour  " } });
  fireEvent.click(submit);
  fireEvent.click(submit);
  await waitFor(() => expect(generate).toHaveBeenCalledTimes(1));
  expect(generate.mock.calls[0]?.[0]).toEqual({ message_id: "a1", prompt: "Cassian by the harbour", client_operation_id: expect.any(String), cast: { character_ids: ["ch1"], include_persona: false } });
  await waitFor(() => expect(runtime.router.state.location.search.sceneImage).toBeUndefined());
});

it("streams an AI prompt, reviews the cleaned result and allows editing before generation", async () => {
  let emit: ((event: JobEvent) => void) | undefined;
  const generate = vi.fn<NonNullable<MockCallHandlers["messageSceneImageGenerate"]>>(() => ({ status: "ok", data: { job_id: "image-job" } }));
  await open({ calls: { messageScenePromptGenerate: () => ({ status: "ok", data: { job_id: "writer-job" } }), messageSceneImageGenerate: generate }, streams: {
    jobWatch: (_args, onEvent) => { emit = onEvent; return { status: "ok", data: job({ id: "writer-job", kind: "creation_run" }) }; },
  } });
  const dialog = await openFlow();
  fireEvent.click(within(dialog).getByRole("button", { name: /Ask AI to write a prompt/ }));
  await waitFor(() => expect(emit).toBeTypeOf("function"));
  await act(async () => emit!({ type: "text_delta", text: "A draft", reasoning: null }));
  expect(within(dialog).getByRole("textbox", { name: "Scene prompt" })).toHaveProperty("value", "A draft");
  expect(generate).not.toHaveBeenCalled();
  await act(async () => emit!({ type: "completed", job: job({ id: "writer-job", state: "succeeded", result: { type: "generated_text", text: "The cleaned harbour prompt" } }) }));
  const field = within(dialog).getByRole("textbox", { name: "Scene prompt" });
  await waitFor(() => expect(field).toHaveProperty("value", "The cleaned harbour prompt"));
  fireEvent.change(field, { target: { value: "The harbour at night" } });
  fireEvent.click(within(dialog).getByRole("button", { name: "Generate image" }));
  await waitFor(() => expect(generate).toHaveBeenCalledWith(expect.objectContaining({ prompt: "The harbour at night" })));
});

it("restores an ask-first prompt and approves edited text through the approval endpoint", async () => {
  const approve = vi.fn<NonNullable<MockCallHandlers["messageSceneImageApprove"]>>(() => ({ status: "ok", data: { job_id: "j1" } }));
  const generate = vi.fn<NonNullable<MockCallHandlers["messageSceneImageGenerate"]>>();
  await open({ initial: reply(pending), calls: { messageSceneImageApprove: approve, messageSceneImageGenerate: generate } });
  expect(approve).not.toHaveBeenCalled();
  fireEvent.click(await screen.findByRole("button", { name: "Review scene image" }));
  const field = await screen.findByRole("textbox", { name: "Scene prompt" });
  expect(field).toHaveProperty("value", pending.prompt);
  fireEvent.change(field, { target: { value: "A storm over the harbour" } });
  fireEvent.click(screen.getByRole("button", { name: "Generate image" }));
  await waitFor(() => expect(approve).toHaveBeenCalledExactlyOnceWith({ message_id: "a1", prompt: "A storm over the harbour", cast: { character_ids: ["ch1"], include_persona: false } }));
  expect(generate).not.toHaveBeenCalled();
});

it("dismisses a persisted approval and removes it from the timeline", async () => {
  const dismiss = vi.fn<NonNullable<MockCallHandlers["messageSceneImageDismiss"]>>(() => { current.setMessage(reply()); return { status: "ok", data: null }; });
  const current = await open({ initial: reply(pending), calls: { messageSceneImageDismiss: dismiss } });
  fireEvent.click(await screen.findByRole("button", { name: "Dismiss" }));
  await waitFor(() => expect(dismiss).toHaveBeenCalledExactlyOnceWith({ message_id: "a1" }));
  await waitFor(() => expect(screen.queryByText("A scene image is ready to review")).toBeNull());
});

it("reconnects to an existing image job, shows progress and cancels without creating another job", async () => {
  let emit: ((event: JobEvent) => void) | undefined;
  const cancel = vi.fn<NonNullable<MockCallHandlers["jobCancel"]>>(() => { current.setMessage(reply()); return { status: "ok", data: null }; });
  const generate = vi.fn<NonNullable<MockCallHandlers["messageSceneImageGenerate"]>>();
  const current = await open({ initial: reply({ ...pending, state: "running", mode: "auto", job_id: "j1" }), calls: { jobCancel: cancel, messageSceneImageGenerate: generate }, streams: { jobWatch: (_args, onEvent) => { emit = onEvent; return { status: "ok", data: job() }; } } });
  await screen.findByText(/Generating the scene image.*2 of 10/);
  await act(async () => emit!({ type: "image_progress", progress: { phase: "sampling", step: 12, total_steps: 30, queue_position: null, preview_asset: null } }));
  await screen.findByText(/Generating the scene image.*12 of 30/);
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  await waitFor(() => expect(cancel).toHaveBeenCalledExactlyOnceWith({ job_id: "j1" }));
  expect(generate).not.toHaveBeenCalled();
});

it("refreshes the stored image after the backend event and preserves reply text", async () => {
  const current = await open({ initial: reply(pending) });
  current.setMessage({ ...reply(), parts: [...reply().parts, { type: "media", asset: { asset_id: "scene", url: "asset://scene" }, role: "attachment", kind: "image" }] });
  await act(async () => current.transport.emitAppEvent({ type: "message_scene_image_changed", conversation_id: "c1", message_id: "a1" }));
  const picture = await screen.findByRole("button", { name: "Attached image" });
  expect(picture.querySelector("img")).toHaveProperty("src", "asset://scene");
  expect(screen.getByText("The harbour falls silent.").compareDocumentPosition(picture) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(screen.getByText("The harbour falls silent.")).toBeTruthy();
  expect(screen.queryByText("A scene image is ready to review")).toBeNull();
});

it("keeps the same operation key after an uncertain submission and preserves the draft", async () => {
  const generate = vi.fn<NonNullable<MockCallHandlers["messageSceneImageGenerate"]>>()
    .mockRejectedValueOnce(new Error("connection dropped"))
    .mockResolvedValueOnce({ status: "ok", data: { job_id: "j1" } });
  await open({ calls: { messageSceneImageGenerate: generate } });
  const dialog = await openFlow();
  fireEvent.click(within(dialog).getByRole("button", { name: /Write a prompt/ }));
  const field = within(dialog).getByRole("textbox", { name: "Scene prompt" });
  fireEvent.change(field, { target: { value: "Keep this scene" } });
  fireEvent.click(within(dialog).getByRole("button", { name: "Generate image" }));
  await within(dialog).findByText("Could not generate the scene image");
  expect(field).toHaveProperty("value", "Keep this scene");
  fireEvent.click(within(dialog).getByRole("button", { name: "Generate image" }));
  await waitFor(() => expect(generate).toHaveBeenCalledTimes(2));
  expect(generate.mock.calls[0]?.[0]).toEqual(generate.mock.calls[1]?.[0]);
});

it("closes on Back and cancels a prompt job even if admission finishes after closing", async () => {
  let accept: ((value: { status: "ok"; data: { job_id: string } }) => void) | undefined;
  const cancel = vi.fn<NonNullable<MockCallHandlers["jobCancel"]>>(() => ({ status: "ok", data: null }));
  const current = await open({ calls: { messageScenePromptGenerate: () => new Promise((resolve) => { accept = resolve; }), jobCancel: cancel } });
  const dialog = await openFlow();
  fireEvent.click(within(dialog).getByRole("button", { name: /Ask AI to write a prompt/ }));
  await waitFor(() => expect(accept).toBeTypeOf("function"));
  await act(async () => current.runtime.router.history.back());
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "Generate scene image" })).toBeNull());
  await act(async () => accept!({ status: "ok", data: { job_id: "late-job" } }));
  await waitFor(() => expect(cancel).toHaveBeenCalledExactlyOnceWith({ job_id: "late-job" }));
});

it.each([{ enabled: false, models: [imageModel, writerModel], title: "Scene generation is off" }, { enabled: true, models: [writerModel], title: "No compatible scene image model" }])("blocks generation when $title", async ({ enabled, models, title }) => {
  await open({ enabled, models, path: "/chat/c1?sceneImage=a1" });
  await screen.findByText(title);
  expect(screen.queryByRole("button", { name: "Generate image" })).toBeNull();
  expect(screen.getByRole("button", { name: "Open image settings" })).toBeTruthy();
});

it("lets a group chat pick who appears in the scene", async () => {
  const generate = vi.fn<NonNullable<MockCallHandlers["messageSceneImageGenerate"]>>(() => ({ status: "ok", data: { job_id: "group-job" } }));
  const write = vi.fn<NonNullable<MockCallHandlers["messageScenePromptGenerate"]>>(() => ({ status: "ok", data: { job_id: "writer-job" } }));
  await open({ group: true, calls: {
    messageSceneImageGenerate: generate,
    messageScenePromptGenerate: write,
    messageSceneCast: () => ({ status: "ok", data: {
      characters: [{ character_id: "ch1", name: "Cassian", avatar: null, selected: true }, { character_id: "ch2", name: "Mira", avatar: null, selected: false }],
      persona: { name: "Sol", avatar: null, selected: true },
    } }),
  } });
  const dialog = await openFlow();
  await within(dialog).findByText("In the picture");
  expect(within(dialog).getByRole("checkbox", { name: "Cassian" }).getAttribute("aria-checked")).toBe("true");
  fireEvent.click(within(dialog).getByRole("checkbox", { name: "Cassian" }));
  fireEvent.click(within(dialog).getByRole("checkbox", { name: "Sol" }));
  await within(dialog).findByText("Choose at least one person for the scene.");
  expect(within(dialog).getByRole("button", { name: /Ask AI to write a prompt/ })).toHaveProperty("disabled", true);
  fireEvent.click(within(dialog).getByRole("checkbox", { name: "Mira" }));
  fireEvent.click(within(dialog).getByRole("button", { name: /Write a prompt/ }));
  fireEvent.change(await within(dialog).findByRole("textbox", { name: "Scene prompt" }), { target: { value: "Mira on the pier" } });
  fireEvent.click(within(dialog).getByRole("button", { name: "Generate image" }));
  await waitFor(() => expect(generate).toHaveBeenCalledWith(expect.objectContaining({ prompt: "Mira on the pier", cast: { character_ids: ["ch2"], include_persona: false } })));
  expect(write).not.toHaveBeenCalled();
});

it("retries a failed image with its retained prompt", async () => {
  const generate = vi.fn<NonNullable<MockCallHandlers["messageSceneImageGenerate"]>>(() => ({ status: "ok", data: { job_id: "retry-job" } }));
  await open({ initial: reply({ ...pending, state: "failed", failure: "interrupted" }), calls: { messageSceneImageGenerate: generate } });
  await screen.findByText("Generation was interrupted. You can try again.");
  fireEvent.click(screen.getByRole("button", { name: "Try again" }));
  const field = await screen.findByRole("textbox", { name: "Scene prompt" });
  expect(field).toHaveProperty("value", pending.prompt);
  fireEvent.click(screen.getByRole("button", { name: "Generate image" }));
  await waitFor(() => expect(generate).toHaveBeenCalledWith(expect.objectContaining({ prompt: pending.prompt })));
});

it("closes the prompt editor if another client switches the reply variant", async () => {
  const current = await open();
  await openFlow();
  current.setMessage({ ...reply(), candidate_index: 1, candidate_count: 2 });
  await act(async () => current.transport.emitAppEvent({ type: "conversation_changed", conversation_id: "c1" }));
  await waitFor(() => expect(current.runtime.router.state.location.search.sceneImage).toBeUndefined());
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "Generate scene image" })).toBeNull());
});
