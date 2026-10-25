import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { toast } from "@lettuceai/crisp";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApiClient } from "@/api/client";
import type {
  ConversationSettingsView,
  ConversationView,
  GenerationEvent,
  TimelineMessage,
  SettingsView,
} from "@/api/generated/bindings";
import { typingHaptic } from "@/api/haptics";
import {
  createMockTransport,
  mockAppStatus,
  type MockCallHandlers,
  type MockStreamHandlers,
} from "@/api/mock-transport";
import { QueryClient } from "@tanstack/react-query";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";
import { selectCandidate } from "./data/actions";
import { GenerationStore } from "./data/generation-store";
import { groupOf, statsOf } from "./data/messages";

vi.mock("@/api/window-controls", () => ({
  createWindowController: vi.fn().mockResolvedValue(null),
}));
vi.mock("@/api/haptics", () => ({ typingHaptic: vi.fn().mockResolvedValue(undefined) }));

function message(
  id: string,
  role: TimelineMessage["role"],
  text: string,
  patch: Partial<TimelineMessage> = {},
): TimelineMessage {
  return {
    id,
    role,
    author_participant_id: role === "user" ? "pu" : role === "assistant" ? "pc" : null,
    parts: [{ type: "text", text }],
    reasoning: null,
    created_at: 1_700_000_000_000,
    candidate_index: null,
    candidate_count: 1,
    pinned: false,
    scene_image: null,
    ...patch,
  };
}

function view(
  messages: TimelineMessage[],
  patch: Partial<ConversationView> = {},
): ConversationView {
  return {
    id: "c1",
    origin_conversation_id: null,
    origin_message_id: null,
    kind: "direct",
    title: "Harbour",
    branch: { branch_id: "b", head_message_id: null },
    participants: [
      { id: "pu", role: "user", name: "Hunter", character_id: null, avatar: null },
      { id: "pc", role: "character", name: "Cassian", character_id: "ch1", avatar: null },
    ],
    messages: { items: messages, next_cursor: null },
    pending_turn_id: null,
    can_send: true,
    revision: 4,
    settings_revision: 2,
    archived: false,
    source: { type: "direct", character_id: "ch1" },
    chat_mode: null,
    missing_models: [],
    memory_blocked: null,
    ...patch,
  };
}

const settings: ConversationSettingsView = {
  conversation_id: "c1",
  kind: "direct",
  title: "Harbour",
  revision: 4,
  settings_revision: 2,
  author_note: null,
  persona: { id: null, source: "app_default" },
  prompt: { id: null, source: "app_default" },
  roleplay_prompt: null,
  lorebooks: { ids: [], source: "app_default" },
  model: { id: "m1", source: "character" },
  background: { asset: null, hidden: false, source: "character" },
  scene: { id: null, source: "app_default" },
  memory: { mode: "manual", source: "app_default" },
  chat_mode: null,
  disable_character_lorebooks: null,
  speaker_selection: null,
  members: null,
  model_settings: { chat_parameters: {}, llama_cpp: {}, stable_diffusion: {} } as never,
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
        appStatus: () => ({
          status: "ok",
          data: {
            ...mockAppStatus,
            platform: setup.platform ?? "other",
            ui_state: { onboarding: { completed: true } },
          },
        }),
        conversationOpen: () => ({
          status: "ok",
          data: view((setup.messages ?? (() => [message("a1", "assistant", "Sit down.")]))()),
        }),
        conversationSettingsGet: () => ({ status: "ok", data: settings }),
        modelsList: () => ({
          status: "ok",
          data: { models: [], default_model_id: null, revision: 1 },
        }),
        providerAccountsList: () => ({ status: "ok", data: [] }),
        memoryGet: () => ({
          status: "ok",
          data: {
            items: [],
            allowed_categories: [
              "character_trait",
              "relationship",
              "plot_event",
              "world_detail",
              "preference",
              "other",
            ],
            summary: null,
            revision: 1,
            status: {
              run_mode: "auto",
              interval: 10,
              messages_since_last_cycle: 0,
              messages_until_next_cycle: 10,
              total_conversation_messages: 1,
              pending_approval_count: null,
              skipped: false,
              latest_cycle_status: null,
              latest_job_id: null,
              failure: null,
              paused_reason: null,
            },
          },
        }),
        llmMetricsForMessage: () => ({ status: "ok", data: null }),
        charactersList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        conversationsLatestByCharacter: () => ({
          status: "ok",
          data: { items: [], next_cursor: null },
        }),
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
  toast.dismiss();
  cleanup();
});

async function typeAndSend(text: string) {
  const box = await screen.findByPlaceholderText("Message Cassian…");
  fireEvent.change(box, { target: { value: text } });
  fireEvent.click(screen.getByRole("button", { name: "Send message" }));
}

const accepted = { status: "ok" as const, data: { user_message_id: "u1", turn_id: "t1" } };

describe("conversation screen", () => {
  it("applies the saved composer font and color to the actual chat composer", async () => {
    await open({
      calls: {
        settingsGet: () => ({
          status: "ok",
          data: {
            global: {
              ui_preferences: {
                chatAppearance: { composerFontSize: "xlarge", footerInputColorHex: "#123456" },
              },
            },
            revision: 1,
          } as unknown as SettingsView,
        }),
      },
    });
    const composer = await screen.findByPlaceholderText("Message Cassian…");
    await waitFor(() => expect(composer.classList.contains("text-xl")).toBe(true));
    expect(composer.getAttribute("style")).toContain("1.75rem");
    expect(composer.closest("[style*='background-color']")?.getAttribute("style")).toContain(
      "rgb(18, 52, 86)",
    );
  });

  it("only pulses haptics for completion text on mobile and keeps the 60 ms interval", async () => {
    const pulse = vi.mocked(typingHaptic).mockClear();
    const now = vi.spyOn(performance, "now").mockReturnValue(1000);
    let emit: ((event: GenerationEvent) => void) | undefined;
    await open({
      platform: "android",
      calls: {
        settingsGet: () => ({
          status: "ok",
          data: {
            global: {
              ui_preferences: { accessibility: { haptics: true, hapticIntensity: "heavy" } },
            },
            revision: 1,
          } as unknown as SettingsView,
        }),
      },
      streams: {
        conversationSend: (_args, onEvent) => {
          emit = onEvent;
          onEvent({ type: "started", turn_id: "t1" });
          return accepted;
        },
      },
    });
    await typeAndSend("Hello");
    await waitFor(() => expect(emit).toBeTypeOf("function"));
    await act(async () => {
      emit!({ type: "delta", turn_id: "t1", text: null, reasoning: "Thinking" });
      emit!({ type: "delta", turn_id: "t1", text: "Hello", reasoning: null });
      emit!({ type: "delta", turn_id: "t1", text: " there", reasoning: null });
    });
    expect(pulse).toHaveBeenCalledTimes(1);
    now.mockReturnValue(1070);
    await act(async () => emit!({ type: "delta", turn_id: "t1", text: ".", reasoning: null }));
    expect(pulse).toHaveBeenCalledTimes(2);
    expect(pulse).toHaveBeenLastCalledWith("heavy", "android");
    now.mockReturnValue(1140);
    await act(async () =>
      emit!({
        type: "delta",
        turn_id: "t1",
        text: "<img>A hidden scene prompt</img>",
        reasoning: null,
      }),
    );
    expect(pulse).toHaveBeenCalledTimes(2);
    now.mockRestore();
  });

  it("ignores saved haptic settings during desktop generation", async () => {
    const pulse = vi.mocked(typingHaptic).mockClear();
    let emit: ((event: GenerationEvent) => void) | undefined;
    await open({
      platform: "other",
      calls: {
        settingsGet: () => ({
          status: "ok",
          data: {
            global: {
              ui_preferences: { accessibility: { haptics: true, hapticIntensity: "heavy" } },
            },
            revision: 1,
          } as unknown as SettingsView,
        }),
      },
      streams: {
        conversationSend: (_args, onEvent) => {
          emit = onEvent;
          onEvent({ type: "started", turn_id: "t1" });
          return accepted;
        },
      },
    });
    await typeAndSend("Hello");
    await waitFor(() => expect(emit).toBeTypeOf("function"));
    await act(async () => emit!({ type: "delta", turn_id: "t1", text: "Hello", reasoning: null }));
    expect(pulse).not.toHaveBeenCalled();
  });

  it("updates one top progress toast during model loading and removes it when ready", async () => {
    let emitEvent: (event: GenerationEvent) => void = () => {};
    await open({
      streams: {
        conversationSend: (_args, emit) => {
          emitEvent = emit;
          emit({ type: "started", turn_id: "t1" });
          emit({
            type: "model_loading",
            turn_id: "t1",
            stage: "gpu_offload",
            status: "loading",
            percent: 25,
            model_name: "Gemma.gguf",
            gpus: null,
          });
          return accepted;
        },
      },
    });
    await typeAndSend("Hi");
    const progress = await screen.findByRole("progressbar");
    expect(progress.getAttribute("aria-valuenow")).toBe("25");
    expect(progress.closest("[data-sonner-toaster]")?.getAttribute("data-y-position")).toBe("top");
    expect(screen.queryByText("Loading Gemma.gguf, 25%")).toBeNull();
    expect(screen.queryByRole("status", { name: "Typing" })).toBeNull();
    await act(async () => {
      emitEvent({
        type: "model_loading",
        turn_id: "t1",
        stage: "gpu_offload",
        status: "loading",
        percent: 95,
        model_name: "Gemma.gguf",
        gpus: null,
      });
    });
    expect(screen.getAllByRole("progressbar")).toHaveLength(1);
    await waitFor(() =>
      expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("95"),
    );
    await act(async () => {
      emitEvent({
        type: "model_loading",
        turn_id: "t1",
        stage: "gpu_offload",
        status: "loaded",
        percent: 100,
        model_name: "Gemma.gguf",
        gpus: null,
      });
    });
    await waitFor(() => expect(screen.queryByRole("progressbar")).toBeNull());
    expect(screen.getByRole("status", { name: "Typing" })).toBeTruthy();
  });

  it("pauses the cursor for an image prompt and resumes it for visible text", async () => {
    let emitEvent: (event: GenerationEvent) => void = () => {};
    await open({
      streams: {
        conversationSend: (_args, emit) => {
          emitEvent = emit;
          emit({ type: "started", turn_id: "t1" });
          emit({ type: "delta", turn_id: "t1", text: "He nods.", reasoning: null });
          return accepted;
        },
      },
    });
    await typeAndSend("Hi");
    const cursor = () => document.querySelector('span[aria-hidden="true"].animate-pulse');
    expect(cursor()).not.toBeNull();
    await act(async () => {
      emitEvent({ type: "delta", turn_id: "t1", text: "<img>a sunny street", reasoning: null });
    });
    expect(await screen.findByText("Writing the image prompt")).toBeTruthy();
    expect(
      screen.getByText("Writing the image prompt").closest("[data-sonner-toaster]"),
    ).not.toBeNull();
    expect(screen.getByText("He nods.")).toBeTruthy();
    expect(screen.queryByText(/a sunny street/)).toBeNull();
    expect(cursor()).toBeNull();
    await act(async () => {
      emitEvent({ type: "delta", turn_id: "t1", text: "</img> Then he waits.", reasoning: null });
    });
    await waitFor(() => expect(screen.queryByText("Writing the image prompt")).toBeNull());
    expect(screen.getByText("He nods. Then he waits.")).toBeTruthy();
    expect(cursor()).not.toBeNull();
  });

  it.each(["failed", "cancelled", "unmount"] as const)(
    "dismisses generation progress on %s",
    async (ending) => {
      let emitEvent: (event: GenerationEvent) => void = () => {};
      const runtime = await open({
        streams: {
          conversationSend: (_args, emit) => {
            emitEvent = emit;
            emit({ type: "started", turn_id: "t1" });
            emit({ type: "delta", turn_id: "t1", text: "He nods. <img>sunny", reasoning: null });
            return accepted;
          },
        },
      });
      await typeAndSend("Hi");
      expect(await screen.findByText("Writing the image prompt")).toBeTruthy();
      await act(async () => {
        if (ending === "unmount") await runtime.router.navigate({ to: "/" });
        else if (ending === "failed")
          emitEvent({ type: "failed", turn_id: "t1", code: "provider_unavailable" });
        else emitEvent({ type: "cancelled", turn_id: "t1" });
      });
      await waitFor(() => expect(screen.queryByText("Writing the image prompt")).toBeNull());
    },
  );

  it("puts streamed deltas into one visible bubble before the reply is stored", async () => {
    let stored = [message("a1", "assistant", "Sit down.")];
    let emitEvent: (event: GenerationEvent) => void = () => {};
    await open({
      messages: () => stored,
      streams: {
        conversationSend: (_args, emit) => {
          emitEvent = emit;
          emit({ type: "started", turn_id: "t1" });
          emit({ type: "delta", turn_id: "t1", text: "Hel", reasoning: null });
          emit({ type: "delta", turn_id: "t1", text: "lo", reasoning: null });
          return accepted;
        },
      },
    });
    await typeAndSend("Hi");
    expect(await screen.findByText("Hello")).toBeTruthy();
    expect(screen.getAllByText("Hello")).toHaveLength(1);
    expect(screen.getAllByText("Hi")).toHaveLength(1);
    stored = [...stored, message("u1", "user", "Hi"), message("a2", "assistant", "Hello")];
    await act(async () => {
      emitEvent({ type: "completed", turn_id: "t1", message_id: "a2" });
    });
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "Stop generating" })).toBeNull(),
    );
    expect(screen.getAllByText("Hello")).toHaveLength(1);
    expect(screen.getAllByText("Hi")).toHaveLength(1);
  });

  it("refreshes a changed branch and keeps the draft when a stale send conflicts", async () => {
    let changed = false;
    const proceed = vi.fn<NonNullable<MockStreamHandlers["conversationSend"]>>(() => {
      changed = true;
      return {
        status: "error",
        error: { code: "conflict", message: "The branch changed", details: null },
      };
    });
    await open({
      calls: {
        conversationOpen: () => ({
          status: "ok",
          data: changed
            ? view([message("a2", "assistant", "The newly selected branch")], {
                revision: 5,
                branch: { branch_id: "new-branch", head_message_id: "a2" },
              })
            : view([message("a1", "assistant", "The original branch")]),
        }),
      },
      streams: { conversationSend: proceed },
    });
    await typeAndSend("Keep my draft");
    await waitFor(() => expect(proceed).toHaveBeenCalled());
    expect(proceed.mock.calls[0]?.[0]).toMatchObject([
      { conversation_id: "c1", expected_revision: 4 },
    ]);
    expect(await screen.findByText("The newly selected branch")).toBeTruthy();
    await waitFor(() =>
      expect((screen.getByPlaceholderText("Message Cassian…") as HTMLTextAreaElement).value).toBe(
        "Keep my draft",
      ),
    );
    expect(proceed).toHaveBeenCalledTimes(1);
  });

  it("cancels a turn that was stopped before it started once its id is known", async () => {
    let release: () => void = () => {};
    const cancel = vi.fn((_args: unknown) => ({ status: "ok" as const, data: null }));
    await open({
      calls: { generationCancel: cancel },
      streams: {
        conversationSend: (_args, emit) =>
          new Promise((resolve) => {
            release = () => {
              emit({ type: "started", turn_id: "t1" });
              resolve(accepted);
            };
          }),
      },
    });
    await typeAndSend("Hi");
    fireEvent.click(await screen.findByRole("button", { name: "Stop generating" }));
    expect(cancel).not.toHaveBeenCalled();
    await act(async () => {
      release();
    });
    await waitFor(() => expect(cancel).toHaveBeenCalledWith({ turn_id: "t1" }));
  });

  it("removes the reply when it is stopped before the first token", async () => {
    let emitEvent: (event: GenerationEvent) => void = () => {};
    const cancel = vi.fn(() => ({ status: "ok" as const, data: null }));
    await open({
      calls: { generationCancel: cancel },
      streams: {
        conversationSend: (_args, emit) => {
          emitEvent = emit;
          emit({ type: "started", turn_id: "t1" });
          return accepted;
        },
      },
    });
    await typeAndSend("Hi");
    fireEvent.click(await screen.findByRole("button", { name: "Stop generating" }));
    await waitFor(() => expect(cancel).toHaveBeenCalledWith({ turn_id: "t1" }));
    await act(async () => {
      emitEvent({ type: "cancelled", turn_id: "t1" });
    });
    await waitFor(() => expect(screen.queryByRole("status", { name: "Typing" })).toBeNull());
  });

  it("shows a failed reply with a retry that repeats the turn", async () => {
    const retry = vi.fn((_args: unknown[]) => ({ status: "ok" as const, data: { turn_id: "t2" } }));
    await open({
      streams: {
        conversationSend: (_args, emit) => {
          emit({ type: "started", turn_id: "t1" });
          emit({ type: "failed", turn_id: "t1", code: "timed_out" });
          return accepted;
        },
        conversationRetry: retry,
      },
    });
    await typeAndSend("Hi");
    expect(await screen.findByText("The model took too long to answer.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(retry).toHaveBeenCalled());
    expect(retry.mock.calls[0]?.[0]).toMatchObject([{ conversation_id: "c1", turn_id: "t1" }]);
  });

  it("keeps the failure card and its retry when the retry is rejected", async () => {
    await open({
      streams: {
        conversationSend: (_args, emit) => {
          emit({ type: "started", turn_id: "t1" });
          emit({ type: "failed", turn_id: "t1", code: "timed_out" });
          return accepted;
        },
        conversationRetry: () => ({
          status: "error",
          error: { code: "unavailable", message: "x", details: null },
        }),
      },
    });
    await typeAndSend("Hi");
    fireEvent.click(await screen.findByRole("button", { name: "Try again" }));
    await waitFor(() => expect(screen.getByText("It isn't available right now.")).toBeTruthy());
    expect(screen.getByText("The model took too long to answer.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  });

  it("offers regenerate only on the latest assistant message", async () => {
    await open({
      messages: () => [
        message("a1", "assistant", "Old reply."),
        message("u1", "user", "Go on."),
        message("a2", "assistant", "New reply."),
      ],
    });
    fireEvent.contextMenu(await screen.findByText("Old reply."));
    await screen.findByRole("button", { name: /Copy text/ });
    expect(screen.queryByRole("button", { name: /Regenerate/ })).toBeNull();
    cleanup();
    await open({
      messages: () => [
        message("a1", "assistant", "Old reply."),
        message("u1", "user", "Go on."),
        message("a2", "assistant", "New reply."),
      ],
    });
    fireEvent.contextMenu(await screen.findByText("New reply."));
    expect(await screen.findByRole("button", { name: /Regenerate/ })).toBeTruthy();
  });

  it("regenerates an assistant message from its menu", async () => {
    const regenerate = vi.fn((_args: unknown[]) => ({
      status: "ok" as const,
      data: { turn_id: "t3" },
    }));
    await open({ streams: { conversationRegenerate: regenerate } });
    fireEvent.contextMenu(await screen.findByText("Sit down."));
    fireEvent.click(await screen.findByRole("button", { name: /Regenerate/ }));
    await waitFor(() => expect(regenerate).toHaveBeenCalled());
    expect(regenerate.mock.calls[0]?.[0]).toMatchObject([
      { conversation_id: "c1", message_id: "a1", expected_revision: 4 },
    ]);
  });

  it("selects the neighbouring reply variant through its candidate id", async () => {
    const select = vi.fn(() => ({
      status: "ok" as const,
      data: { message: message("a1", "assistant", "x"), revision: 5 },
    }));
    const api = createApiClient(
      createMockTransport({
        calls: {
          messageCandidates: () => ({
            status: "ok",
            data: {
              items: [
                {
                  id: "k0",
                  index: 0,
                  author_participant_id: null,
                  parts: [],
                  reasoning: null,
                  created_at: 1,
                },
                {
                  id: "k1",
                  index: 1,
                  author_participant_id: null,
                  parts: [],
                  reasoning: null,
                  created_at: 2,
                },
              ],
              next_cursor: null,
            },
          }),
          messageCandidateSelect: select,
        },
      }),
    );
    const done = await selectCandidate(
      { api, queryClient: new QueryClient(), conversationId: "c1", isStuck: () => true },
      message("a1", "assistant", "x", { candidate_index: 0, candidate_count: 2 }),
      1,
    );
    expect(done).toBe(true);
    expect(select).toHaveBeenCalledWith(
      expect.objectContaining({ message_id: "a1", candidate_id: "k1" }),
    );
  });

  it("disables delete for a pinned message", async () => {
    await open({ messages: () => [message("a1", "assistant", "Keep me.", { pinned: true })] });
    fireEvent.contextMenu(await screen.findByText("Keep me."));
    const remove = await screen.findByRole("button", { name: /Delete message/ });
    expect(remove.hasAttribute("disabled")).toBe(true);
    expect(screen.getByText("Unpin it first")).toBeTruthy();
  });

  it("asks before rewinding and then deletes everything after the message", async () => {
    const rewind = vi.fn(() => ({
      status: "ok" as const,
      data: { outcome: { type: "tombstoned" as const, removed: ["a2"] }, revision: 5 },
    }));
    await open({
      messages: () => [message("a1", "assistant", "First."), message("u1", "user", "Second.")],
      calls: { messagesDeleteAfter: rewind },
    });
    fireEvent.contextMenu(await screen.findByText("First."));
    fireEvent.click(await screen.findByRole("button", { name: /Rewind to here/ }));
    expect(rewind).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole("button", { name: "Rewind" }));
    await waitFor(() =>
      expect(rewind).toHaveBeenCalledWith(
        expect.objectContaining({ conversation_id: "c1", message_id: "a1", expected_revision: 4 }),
      ),
    );
  });

  it("hides the tab bar on a phone and keeps it on the home page", async () => {
    await open({ platform: "android" });
    await screen.findByText("Sit down.");
    expect(screen.queryByLabelText("Main navigation")).toBeNull();
    cleanup();
    await open({ platform: "android", path: "/" });
    expect(await screen.findByLabelText("Main navigation")).toBeTruthy();
  });

  it("keeps the message draft when the viewport switches navigation layouts", async () => {
    let compact = false;
    const listeners = new Set<() => void>();
    vi.spyOn(window, "matchMedia").mockImplementation((query) => ({
      matches: compact && query.includes("max-width"),
      media: query,
      onchange: null,
      addEventListener: (_type: string, listener: unknown) => {
        listeners.add(listener as () => void);
      },
      removeEventListener: (_type: string, listener: unknown) => {
        listeners.delete(listener as () => void);
      },
      addListener() {},
      removeListener() {},
      dispatchEvent: () => false,
    }));
    await open();
    const field = await screen.findByPlaceholderText("Message Cassian…");
    fireEvent.change(field, { target: { value: "Keep my unfinished reply" } });
    for (const next of [true, false]) {
      act(() => {
        compact = next;
        for (const listener of [...listeners]) listener();
      });
      expect(await screen.findByPlaceholderText("Message Cassian…")).toHaveProperty(
        "value",
        "Keep my unfinished reply",
      );
    }
  });

  it("opens the memory page from the header", async () => {
    const runtime = await open();
    fireEvent.click(await screen.findByRole("button", { name: /^Memories/ }));
    expect(await screen.findByPlaceholderText("Search memories")).toBeTruthy();
    expect(runtime.router.state.location.pathname).toBe("/chat/c1/memory");
  });
});

describe("conversation data", () => {
  it("groups a run from the same speaker and leaves narration alone", () => {
    const list = [
      message("1", "assistant", "a"),
      message("2", "assistant", "b"),
      message("3", "scene", "c"),
      message("4", "user", "d"),
    ];
    expect([0, 1, 2, 3].map((index) => groupOf(list, index))).toEqual([
      "start",
      "end",
      "only",
      "only",
    ]);
  });

  it("reads the runtime summary and omits what is missing", () => {
    expect(statsOf({ modelName: "/models/gemma.gguf", completionTokens: 12, ttftMs: 500 })).toEqual(
      {
        model: "gemma",
        promptTokens: null,
        completionTokens: 12,
        tokensPerSecond: null,
        firstTokenSeconds: 0.5,
      },
    );
    expect(statsOf({})).toBeNull();
  });

  it("keeps deltas out of the page state", () => {
    const store = new GenerationStore();
    const meta = vi.fn();
    store.subscribeMeta(meta);
    store.begin(null, "Hi");
    store.appendDelta("a", null);
    const calls = meta.mock.calls.length;
    store.appendDelta("b", null);
    store.appendDelta("c", null);
    expect(meta.mock.calls.length).toBe(calls);
    expect(store.getText().text).toBe("abc");
  });
});
