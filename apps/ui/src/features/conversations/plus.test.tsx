import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { QueryClient } from "@tanstack/react-query";
import { createApiClient } from "@/api/client";
import type { ConversationSettingsView, ConversationView, JobView, TimelineMessage } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers, type MockStreamHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { updateSettings } from "./data/actions";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

function message(id: string, role: TimelineMessage["role"], text: string, patch: Partial<TimelineMessage> = {}): TimelineMessage {
  return {
    id, role, author_participant_id: role === "user" ? "pu" : role === "assistant" ? "pc" : null, parts: [{ type: "text", text }], reasoning: null,
    created_at: 1_700_000_000_000, candidate_index: null, candidate_count: 1, pinned: false, scene_image: null, ...patch,
  };
}

function view(messages: TimelineMessage[], patch: Partial<ConversationView> = {}): ConversationView {
  return {
    id: "c1", origin_conversation_id: null, origin_message_id: null, kind: "direct", title: "Harbour", branch: { branch_id: "b", head_message_id: null },
    participants: [
      { id: "pu", role: "user", name: "Hunter", character_id: null, avatar: null },
      { id: "pc", role: "character", name: "Cassian", character_id: "ch1", avatar: null },
    ],
    messages: { items: messages, next_cursor: null }, pending_turn_id: null, can_send: true, revision: 4, settings_revision: 2, archived: false,
    source: { type: "direct", character_id: "ch1" }, chat_mode: null, missing_models: [], memory_blocked: null, ...patch,
  };
}

const settings: ConversationSettingsView = {
  conversation_id: "c1", kind: "direct", title: "Harbour", revision: 4, settings_revision: 2, author_note: null,
  persona: { id: null, source: "app_default" }, prompt: { id: null, source: "app_default" }, roleplay_prompt: null,
  lorebooks: { ids: [], source: "app_default" }, model: { id: "m1", source: "character" }, background: { asset: null, hidden: false, source: "character" },
  scene: { id: null, source: "app_default" }, memory: { mode: "manual", source: "app_default" }, chat_mode: null,
  disable_character_lorebooks: null, speaker_selection: null, members: null, model_settings: { chat_parameters: {}, llama_cpp: {}, stable_diffusion: {} } as never,
};

interface Setup {
  messages?: () => TimelineMessage[];
  calls?: MockCallHandlers;
  streams?: MockStreamHandlers;
  platform?: "other" | "android";
  path?: string;
}

async function open(setup: Setup = {}) {
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: () => ({ status: "ok", data: { ...mockAppStatus, platform: setup.platform ?? "other", ui_state: { onboarding: { completed: true } } } }),
        conversationOpen: () => ({ status: "ok", data: view((setup.messages ?? (() => [message("a1", "assistant", "Sit down.")]))()) }),
        conversationSettingsGet: () => ({ status: "ok", data: settings }),
        modelsList: () => ({ status: "ok", data: { models: [], default_model_id: null, revision: 1 } }),
        providerAccountsList: () => ({ status: "ok", data: [] }),
        memoryGet: () => ({ status: "ok", data: { items: [], summary: null, revision: 1, status: { run_mode: "auto", interval: 10, messages_since_last_cycle: 0, messages_until_next_cycle: 10, total_conversation_messages: 1, pending_approval_count: null, skipped: false, latest_cycle_status: null, latest_job_id: null, failure: null, paused_reason: null } } }),
        llmMetricsForMessage: () => ({ status: "ok", data: null }),
        charactersList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        conversationsLatestByCharacter: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        conversationsList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        ...setup.calls,
      },
      ...(setup.streams ? { streams: setup.streams } : {}),
    }),
    history: createMemoryHistory({ initialEntries: [setup.path ?? "/chat/c1"] }),
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return runtime;
}


afterEach(() => {
  cleanup();
  window.localStorage.removeItem("chat.authorNote.inlineEditor");
});

function job(patch: Partial<JobView>): JobView {
  return {
    id: "j1", kind: "speech_transcribe", subject: { kind: "speech_request", id: "r1" }, subject_detail: null, state: "succeeded",
    progress: { current: null, total: null, label_code: null } as unknown as JobView["progress"], created_at: 1, updated_at: 1, failure: null, result: null, ...patch,
  };
}

const personaPage = { status: "ok" as const, data: { items: [{ id: "p1", title: "Captain", description: "Runs the ship", nickname: null, avatar: null }], next_cursor: null, default_persona: { persona_id: null, revision: 1, created_at: 1, updated_at: 1 } } };

async function openPlus() {
  fireEvent.click(await screen.findByRole("button", { name: "Add content" }));
}

describe("chat plus menu and microphone", () => {
  it("shows the enabled inline note above the composer and saves edits on Enter", async () => {
    window.localStorage.setItem("chat.authorNote.inlineEditor", "1");
    const update = vi.fn((_args: unknown) => ({ status: "ok" as const, data: { ...settings, author_note: "Stay curious", settings_revision: 3 } }));
    await open({ calls: { conversationSettingsGet: () => ({ status: "ok", data: { ...settings, author_note: "Be kind" } }), conversationSettingsUpdate: update } });
    fireEvent.click(await screen.findByRole("button", { name: "Edit inline author's note" }));
    const field = screen.getByRole("textbox", { name: "Inline author's note" });
    fireEvent.change(field, { target: { value: "  Stay curious  " } });
    expect(update).not.toHaveBeenCalled();
    fireEvent.keyDown(field, { key: "Enter" });
    await waitFor(() => expect(update).toHaveBeenCalledTimes(1));
    expect(update.mock.calls[0]?.[0]).toMatchObject({ expected_settings_revision: 2, patch: { author_note: "Stay curious" } });
    expect(await screen.findByRole("button", { name: "Edit inline author's note" })).toHaveProperty("textContent", "Stay curious");
  });

  it("synchronizes the shared note menu toggle with the composer and remembers it", async () => {
    await open({ calls: { conversationSettingsGet: () => ({ status: "ok", data: { ...settings, author_note: "Be kind" } }) } });
    expect(screen.queryByRole("button", { name: "Edit inline author's note" })).toBeNull();
    await openPlus();
    fireEvent.click(await screen.findByRole("button", { name: /Author's note/ }));
    fireEvent.click(await screen.findByRole("switch", { name: "Show inline author note editor" }));
    expect(window.localStorage.getItem("chat.authorNote.inlineEditor")).toBe("1");
    fireEvent.keyDown(document, { key: "Escape" });
    expect(await screen.findByRole("button", { name: "Edit inline author's note" })).toBeTruthy();
    await openPlus();
    fireEvent.click(await screen.findByRole("button", { name: /Author's note/ }));
    fireEvent.click(await screen.findByRole("switch", { name: "Show inline author note editor" }));
    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("button", { name: "Edit inline author's note" })).toBeNull());
    expect(window.localStorage.getItem("chat.authorNote.inlineEditor")).toBe("0");
  });

  it("cancels inline edits with Escape and saves a trimmed note on blur", async () => {
    window.localStorage.setItem("chat.authorNote.inlineEditor", "1");
    const update = vi.fn((_args: unknown) => ({ status: "ok" as const, data: settings }));
    await open({ calls: { conversationSettingsUpdate: update } });
    fireEvent.click(await screen.findByRole("button", { name: "Edit inline author's note" }));
    let field = screen.getByRole("textbox", { name: "Inline author's note" });
    fireEvent.change(field, { target: { value: "Cancelled" } });
    fireEvent.keyDown(field, { key: "Escape" });
    expect(update).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Edit inline author's note" }));
    field = screen.getByRole("textbox", { name: "Inline author's note" });
    expect(field).toHaveProperty("value", "");
    fireEvent.change(field, { target: { value: "  Keep this  " } });
    fireEvent.blur(field);
    await waitFor(() => expect(update).toHaveBeenCalledTimes(1));
    expect(update.mock.calls[0]?.[0]).toMatchObject({ patch: { author_note: "Keep this" } });
  });

  it("saves the author's note against the settings revision", async () => {
    const update = vi.fn((_args: unknown) => ({ status: "ok" as const, data: { ...settings, author_note: "Be kind", settings_revision: 3 } }));
    await open({ calls: { conversationSettingsUpdate: update } });
    await openPlus();
    fireEvent.click(await screen.findByRole("button", { name: /Author's note/ }));
    fireEvent.change(await screen.findByRole("textbox", { name: "Author's note" }), { target: { value: "  Be kind  " } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(update).toHaveBeenCalled());
    expect(update.mock.calls[0]?.[0]).toMatchObject({ conversation_id: "c1", expected_settings_revision: 2, patch: { author_note: "Be kind" } });
  });

  it("changes the persona", async () => {
    const update = vi.fn((_args: unknown) => ({ status: "ok" as const, data: settings }));
    await open({ calls: { personasList: () => personaPage, conversationSettingsUpdate: update } });
    await openPlus();
    fireEvent.click(await screen.findByRole("button", { name: /^Persona/ }));
    expect(await screen.findByText("Runs the ship")).toBeTruthy();
    fireEvent.click(await screen.findByRole("button", { name: /Captain/ }));
    await waitFor(() => expect(update).toHaveBeenCalled());
    expect(update.mock.calls[0]?.[0]).toMatchObject({ patch: { persona: { type: "set", id: "p1" } } });
  });

  it("replaces the draft with the transcript after recording", async () => {
    const stop = vi.fn((_args: unknown) => ({ status: "ok" as const, data: { job_id: "j1" } }));
    await open({
      calls: { dictationStart: () => ({ status: "ok", data: { capture_id: "cap1" } }), dictationStop: stop },
      streams: {
        jobWatch: () => ({
          status: "ok",
          data: job({ result: { type: "transcription", transcription: { request_id: "r", audio: null, model_id: "w", raw_text: "there", text: "there", detected_language: null, segments: [], applied_corrections: [] } } }),
        }),
      },
    });
    const box = await screen.findByPlaceholderText("Message Cassian…");
    fireEvent.change(box, { target: { value: "Hi" } });
    fireEvent.click(screen.getByRole("button", { name: "Record voice" }));
    fireEvent.click(await screen.findByRole("button", { name: "Stop and transcribe" }));
    await waitFor(() => expect((screen.getByPlaceholderText("Message Cassian…") as HTMLTextAreaElement).value).toBe("there"));
    expect(stop.mock.calls[0]?.[0]).toMatchObject({ capture_id: "cap1", model_id: null });
  });

  it("toasts a mapped message when recording cannot start", async () => {
    await open({
      calls: {
        dictationStart: () => ({
          status: "error",
          error: { code: "model_required", message: "x", details: { type: "speech", failure: { type: "model_required", model: "whisper" } } },
        }),
      },
    });
    fireEvent.click(await screen.findByRole("button", { name: "Record voice" }));
    expect(await screen.findByText("Install a speech recognition model to dictate.")).toBeTruthy();
  });

  it("passes the swap toggle to continue", async () => {
    const proceed = vi.fn((_args: unknown[]) => ({ status: "ok" as const, data: { turn_id: "t9" } }));
    await open({ streams: { conversationContinue: proceed } });
    await openPlus();
    fireEvent.click(await screen.findByRole("button", { name: /^Swap places/ }));
    await act(async () => {
      fireEvent.click(await screen.findByRole("button", { name: "Let the character continue" }));
    });
    await waitFor(() => expect(proceed).toHaveBeenCalled());
    expect(proceed.mock.calls[0]?.[0]).toMatchObject([{ conversation_id: "c1", swap_places: true }]);
  });

  it("shows streamed help text, uses the final result and cancels the job when closed", async () => {
    let emitEvent: (event: unknown) => void = () => {};
    const cancel = vi.fn((_args: unknown) => ({ status: "ok" as const, data: null }));
    await open({
      calls: { conversationHelpMeReply: () => ({ status: "ok", data: { job_id: "jh" } }), jobCancel: cancel },
      streams: {
        jobWatch: (_args, emit) => {
          emitEvent = emit as (event: unknown) => void;
          emit({ type: "text_delta", text: "Hel", reasoning: null });
          return { status: "ok", data: job({ id: "jh", state: "running" }) };
        },
      },
    });
    await openPlus();
    fireEvent.click(await screen.findByRole("button", { name: /Help me reply/ }));
    expect(await screen.findByText("Hel")).toBeTruthy();
    await act(async () => {
      emitEvent({ type: "completed", job: job({ id: "jh", kind: "conversation_generation", result: { type: "generated_text", text: "Hello there" } }) });
    });
    expect(await screen.findByText("Hello there")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Use this" }));
    await waitFor(() => expect((screen.getByPlaceholderText("Message Cassian…") as HTMLTextAreaElement).value).toBe("Hello there"));
    cancel.mockClear();
    await openPlus();
    fireEvent.click(await screen.findByRole("button", { name: /Help me reply/ }));
    fireEvent.click(await screen.findByRole("button", { name: /Write something new/ }));
    await screen.findByText("Hel");
    await act(async () => {
      fireEvent.keyDown(document, { key: "Escape" });
    });
    await waitFor(() => expect(cancel).toHaveBeenCalledWith({ job_id: "jh" }));
  });

  it("retries a settings update once after a conflict with the fresh revision", async () => {
    const update = vi
      .fn()
      .mockReturnValueOnce({ status: "error", error: { code: "conflict", message: "x", details: null } })
      .mockReturnValueOnce({ status: "ok", data: { ...settings, settings_revision: 8 } });
    const api = createApiClient(
      createMockTransport({
        calls: {
          conversationSettingsUpdate: update,
          conversationSettingsGet: () => ({ status: "ok", data: { ...settings, settings_revision: 7 } }),
        },
      }),
    );
    await updateSettings({ api, queryClient: new QueryClient(), conversationId: "c1", isStuck: () => false }, { author_note: "x" });
    expect(update).toHaveBeenCalledTimes(2);
    expect(update.mock.calls[1]?.[0]).toMatchObject({ expected_settings_revision: 7 });
  });

  it("cancels a capture that starts while the page is closing", async () => {
    let release: () => void = () => {};
    const cancel = vi.fn((_args: unknown) => ({ status: "ok" as const, data: null }));
    await open({
      calls: {
        dictationStart: () =>
          new Promise((resolve) => {
            release = () => resolve({ status: "ok", data: { capture_id: "cap9" } });
          }),
        dictationCancel: cancel,
      },
    });
    fireEvent.click(await screen.findByRole("button", { name: "Record voice" }));
    cleanup();
    await act(async () => {
      release();
    });
    await waitFor(() => expect(cancel).toHaveBeenCalledWith({ capture_id: "cap9" }));
  });

  it("rolls the dice and inserts the result into the draft", async () => {
    await open();
    await openPlus();
    fireEvent.click(await screen.findByRole("button", { name: /^Dice/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Roll" }));
    fireEvent.click(await screen.findByRole("button", { name: "Insert into message" }));
    await waitFor(() => expect((screen.getByPlaceholderText("Message Cassian…") as HTMLTextAreaElement).value).toMatch(/^🎲 1d20: \d+$/));
  });

  it("picks a background image, saves it and closes the sheet", async () => {
    const update = vi.fn((_args: unknown) => ({ status: "ok" as const, data: settings }));
    await open({
      calls: {
        filesPickOpen: () => ({ status: "ok", data: { sources: [{ uri: "file:///bg.png" }] } }),
        assetsIngest: () => ({ status: "ok", data: { asset_id: "as1", url: "asset://as1" } }),
        conversationSettingsUpdate: update,
      },
    });
    await openPlus();
    fireEvent.click(await screen.findByRole("button", { name: /^Chat background/ }));
    fireEvent.click(await screen.findByRole("button", { name: /Choose an image/ }));
    await waitFor(() => expect(update).toHaveBeenCalled());
    expect(update.mock.calls[0]?.[0]).toMatchObject({ patch: { background: { type: "image", asset_id: "as1" } } });
    await waitFor(() => expect(screen.queryByRole("button", { name: /Choose an image/ })).toBeNull());
  });

  it("shows the swap banner and ends the swap from it", async () => {
    await open();
    await openPlus();
    fireEvent.click(await screen.findByRole("button", { name: /^Swap places/ }));
    expect(await screen.findByText("Swap places is on")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "End swap" }));
    await waitFor(() => expect(screen.queryByText("Swap places is on")).toBeNull());
  });

  const visionModels = {
    status: "ok" as const,
    data: {
      models: [{ id: "m1", provider_account_id: "a", external_model_id: "x", display_name: "Vision", kind: "chat" as const, config: {}, input_scopes: ["text" as const, "image" as const], output_scopes: ["text" as const], revision: 1, created_at: 1, updated_at: 1 }],
      default_model_id: "m1",
      revision: 1,
    },
  };
  const imagePick = {
    filesPickOpen: () => ({ status: "ok" as const, data: { sources: [{ uri: "file:///pics/cat.png" }] } }),
    assetsIngest: () => ({ status: "ok" as const, data: { asset_id: "as1", url: "asset://as1" } }),
  };
  const sent = { status: "ok" as const, data: { user_message_id: "u1", turn_id: "t1" } };

  async function attachImage() {
    await openPlus();
    fireEvent.click(await screen.findByRole("button", { name: /Upload image/ }));
    await screen.findByRole("button", { name: "Remove cat.png" });
  }

  it("hides the upload rows for a text-only model and shows image upload for a vision model", async () => {
    await open();
    await openPlus();
    await screen.findByRole("button", { name: /Dice/ });
    expect(screen.queryByRole("button", { name: /Upload image/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /Upload audio/ })).toBeNull();
    cleanup();
    await open({ calls: { modelsList: () => visionModels } });
    await openPlus();
    expect(await screen.findByRole("button", { name: /Upload image/ })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Upload audio/ })).toBeNull();
  });

  it("sends an attachment without text, then clears it", async () => {
    const send = vi.fn((_args: unknown[]) => sent);
    await open({ calls: { modelsList: () => visionModels, ...imagePick }, streams: { conversationSend: send } });
    await attachImage();
    fireEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(send).toHaveBeenCalled());
    expect(send.mock.calls[0]?.[0]).toMatchObject([{ conversation_id: "c1", text: "", media: ["as1"], swap_places: false }]);
    await waitFor(() => expect(screen.queryByRole("button", { name: "Remove cat.png" })).toBeNull());
  });

  it("keeps the attachment and explains when the model cannot take it", async () => {
    await open({
      calls: { modelsList: () => visionModels, ...imagePick },
      streams: { conversationSend: () => ({ status: "error", error: { code: "unsupported", message: "x", details: { type: "invalid_field", field: "media.image" } } }) },
    });
    await attachImage();
    fireEvent.click(screen.getByRole("button", { name: "Send message" }));
    expect(await screen.findByText(/can't read images/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Remove cat.png" })).toBeTruthy();
  });

  it("passes the swap toggle to send", async () => {
    const send = vi.fn((_args: unknown[]) => sent);
    await open({ streams: { conversationSend: send } });
    await openPlus();
    fireEvent.click(await screen.findByRole("button", { name: /^Swap places/ }));
    const box = await screen.findByPlaceholderText("Message Cassian…");
    fireEvent.change(box, { target: { value: "Hi" } });
    fireEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(send).toHaveBeenCalled());
    expect(send.mock.calls[0]?.[0]).toMatchObject([{ text: "Hi", media: [], swap_places: true }]);
  });

  it("renders audio parts with a player and images and unknown kinds as pictures", async () => {
    const media = (id: string, kind: "audio" | "image" | "document" | null) => ({ type: "media" as const, asset: { asset_id: id, url: `asset://${id}` }, role: "attachment" as const, kind });
    await open({
      messages: () => [message("u1", "user", "Listen", { parts: [{ type: "text", text: "Listen" }, media("a1", "audio"), media("i1", "image"), media("i2", null), media("d1", "document")] })],
    });
    expect(await screen.findByText("Audio clip")).toBeTruthy();
    expect(screen.getByText("Document")).toBeTruthy();
    expect(screen.getAllByAltText("Attached image")).toHaveLength(2);
  });

  it("shows the attachment in the pending bubble until the stored message arrives", async () => {
    await open({
      calls: { modelsList: () => visionModels, ...imagePick },
      streams: { conversationSend: () => new Promise(() => undefined) },
    });
    await attachImage();
    fireEvent.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(screen.queryByRole("button", { name: "Remove cat.png" })).toBeNull());
    expect(await screen.findByAltText("cat.png")).toBeTruthy();
  });
});
