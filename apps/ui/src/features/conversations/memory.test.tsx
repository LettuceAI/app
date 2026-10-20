import { act, cleanup, configure, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  ConversationSettingsView, ConversationView, JobView_Serialize, MemoryAddRequest, MemoryCategory, MemoryCyclePage, MemoryCycleView, MemoryItemView, MemoryMode, MemoryStatusView, MemorySummaryView, TimelineMessage,
} from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers, type MockStreamHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

configure({ asyncUtilTimeout: 4000 });

afterEach(() => {
  cleanup();
});

function message(id: string, role: TimelineMessage["role"], text: string, patch: Partial<TimelineMessage> = {}): TimelineMessage {
  return {
    id, role, author_participant_id: role === "user" ? "pu" : "pc", parts: [{ type: "text", text }], reasoning: null,
    created_at: 1_700_000_000_000, candidate_index: null, candidate_count: 1, pinned: false, scene_image: null, ...patch,
  };
}

function conversation(messages: TimelineMessage[]): ConversationView {
  return {
    id: "c1", origin_conversation_id: null, origin_message_id: null, kind: "direct", title: "Harbour", branch: { branch_id: "b", head_message_id: null },
    participants: [
      { id: "pu", role: "user", name: "Hunter", character_id: null, avatar: null },
      { id: "pc", role: "character", name: "Cassian", character_id: "ch1", avatar: null },
    ],
    messages: { items: messages, next_cursor: null }, pending_turn_id: null, can_send: true, revision: 4, settings_revision: 2, archived: false,
    source: { type: "direct", character_id: "ch1" }, chat_mode: null, missing_models: [], memory_blocked: null,
  };
}

function settingsFor(mode: MemoryMode): ConversationSettingsView {
  return {
    conversation_id: "c1", kind: "direct", title: "Harbour", revision: 4, settings_revision: 2, author_note: null,
    persona: { id: null, source: "app_default" }, prompt: { id: null, source: "app_default" }, roleplay_prompt: null,
    lorebooks: { ids: [], source: "app_default" }, model: { id: "m1", source: "character" }, background: { asset: null, hidden: false, source: "character" },
    scene: { id: null, source: "app_default" }, memory: { mode, source: "app_default" }, chat_mode: null,
    disable_character_lorebooks: null, speaker_selection: null, members: null, model_settings: { chat_parameters: {}, llama_cpp: {}, stable_diffusion: {} } as never,
  };
}

function item(id: string, text: string, patch: Partial<MemoryItemView> = {}): MemoryItemView {
  return {
    id, short_id: id, text, category: null, origin: "user", pinned: false, temperature: "hot", observed_at: null, observed_time_precision: null,
    token_count: 12, cycle_label: null, ...patch,
  };
}

const idleStatus: MemoryStatusView = {
  run_mode: "auto", interval: 20, messages_since_last_cycle: 8, messages_until_next_cycle: 12, total_conversation_messages: 28, pending_approval_count: null,
  skipped: false, latest_cycle_status: null, latest_job_id: null, failure: null, paused_reason: null,
};

function cycle(id: string, patch: Partial<MemoryCycleView> = {}): MemoryCycleView {
  return {
    run_id: id, job_id: `job-${id}`, started_at: 1_700_000_000_000, label: "1-20", window_start: 1, window_end: 20, status: "complete", failure: null,
    summary: null, actions: [], reverted: false, revertable: false, blocked_by: null, ...patch,
  };
}

interface World {
  revision: number;
  items: MemoryItemView[];
  summary: MemorySummaryView | null;
  status: MemoryStatusView;
  cycles: MemoryCycleView[];
  pinned: TimelineMessage[];
  allowed_categories: MemoryCategory[];
}

interface Setup {
  mode?: MemoryMode;
  world?: Partial<World>;
  calls?: MockCallHandlers;
  streams?: MockStreamHandlers;
  path?: string;
  messages?: TimelineMessage[];
}

interface Call {
  command: string;
  request: Record<string, unknown>;
}

async function open(setup: Setup = {}) {
  const world: World = { revision: 5, items: [], summary: null, status: idleStatus, cycles: [], pinned: [], allowed_categories: ["character_trait", "relationship", "plot_event", "world_detail", "preference", "other"], ...setup.world };
  const calls: Call[] = [];
  const edit = (command: string) => (request: Record<string, unknown>) => {
    calls.push({ command, request });
    world.revision += 1;
    return { status: "ok" as const, data: { revision: world.revision, memory_id: null } };
  };
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: () => ({ status: "ok", data: { ...mockAppStatus, platform: "other", ui_state: { onboarding: { completed: true } } } }),
        conversationOpen: () => ({ status: "ok", data: conversation(setup.messages ?? [message("a1", "assistant", "Sit down.")]) }),
        conversationSettingsGet: () => ({ status: "ok", data: settingsFor(setup.mode ?? "dynamic") }),
        modelsList: () => ({ status: "ok", data: { models: [], default_model_id: null, revision: 1 } }),
        providerAccountsList: () => ({ status: "ok", data: [] }),
        charactersList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        conversationsLatestByCharacter: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        conversationsList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        memoryGet: () => ({ status: "ok", data: { items: world.items, summary: world.summary, status: world.status, revision: world.revision, allowed_categories: world.allowed_categories } }),
        memoryCycles: (request: { cursor: string | null }) => {
          const second = world.cycles.length > 2;
          const page = request.cursor === null ? (second ? world.cycles.slice(0, 2) : world.cycles) : world.cycles.slice(2);
          return { status: "ok", data: { items: page, next_cursor: request.cursor === null && second ? "more" : null } };
        },
        conversationPinnedMessages: () => ({ status: "ok", data: { items: world.pinned, next_cursor: null } }),
        memoryAdd: edit("memoryAdd"),
        memoryUpdate: edit("memoryUpdate"),
        memoryDelete: edit("memoryDelete"),
        memoryPin: edit("memoryPin"),
        memorySetTemperature: edit("memorySetTemperature"),
        memorySummaryUpdate: edit("memorySummaryUpdate"),
        memoryCycleRevert: edit("memoryCycleRevert"),
        memoryTrigger: (request: Record<string, unknown>) => {
          calls.push({ command: "memoryTrigger", request });
          return { status: "ok", data: { job_id: "job-new" } };
        },
        memoryRetry: (request: Record<string, unknown>) => {
          calls.push({ command: "memoryRetry", request });
          return { status: "ok", data: { job_id: "job-new" } };
        },
        memorySkip: (request: Record<string, unknown>) => {
          calls.push({ command: "memorySkip", request });
          world.status = { ...world.status, pending_approval_count: null };
          return { status: "ok", data: null };
        },
        memoryErrorDismiss: (request: Record<string, unknown>) => {
          calls.push({ command: "memoryErrorDismiss", request });
          world.status = { ...world.status, failure: null };
          return { status: "ok", data: null };
        },
        jobCancel: (request: Record<string, unknown>) => {
          calls.push({ command: "jobCancel", request });
          world.status = { ...world.status, latest_cycle_status: "cancelled" };
          return { status: "ok", data: null };
        },
        messagePin: (request: Record<string, unknown>) => {
          calls.push({ command: "messagePin", request });
          world.pinned = world.pinned.filter((entry) => entry.id !== request.message_id);
          return { status: "ok", data: { message: message("p1", "assistant", "x"), revision: 5 } };
        },
        conversationMessagesAround: (request: Record<string, unknown>) => {
          calls.push({ command: "conversationMessagesAround", request });
          return { status: "ok", data: { items: [message(String(request.message_id), "assistant", "Found it.")], before_cursor: null, after_cursor: null } };
        },
        ...setup.calls,
      },
      ...(setup.streams ? { streams: setup.streams } : {}),
    }),
    history: createMemoryHistory({ initialEntries: [setup.path ?? "/chat/c1/memory"] }),
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  const called = (command: string) => calls.filter((entry) => entry.command === command);
  return { runtime, world, calls, called };
}

const tabNames = () => screen.getAllByRole("tab").map((tab) => (tab.textContent ?? "").replace(/\d+$/, ""));

describe("chat memory page", () => {
  it("shows three tabs in dynamic mode and two in manual mode", async () => {
    await open({ mode: "dynamic" });
    await screen.findByPlaceholderText("Search memories");
    expect(tabNames()).toEqual(["Memories", "Activity", "Pinned"]);
    expect(screen.getByText("Next update in 12 messages")).toBeTruthy();
    cleanup();
    await open({ mode: "manual" });
    await screen.findByPlaceholderText("Search memories");
    expect(tabNames()).toEqual(["Memories", "Pinned"]);
    expect(screen.queryByText("Next update in 12 messages")).toBeNull();
    expect(screen.getByText("Context summary")).toBeTruthy();
  });

  it("explains that memory is off and opens the chat settings", async () => {
    const { runtime } = await open({ mode: "disabled" });
    expect(await screen.findByText("Memory is off for this chat")).toBeTruthy();
    expect(screen.queryByRole("tab")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Open chat settings" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/chat/c1"));
    expect(runtime.router.state.location.search).toEqual({ panel: "settings" });
  });

  it("adds a memory with its category and date using the latest revision", async () => {
    const { called } = await open({ mode: "manual" });
    fireEvent.click(await screen.findByRole("button", { name: "Add memory" }));
    const dialog = await screen.findByRole("dialog", { name: "Add memory" });
    fireEvent.change(within(dialog).getByPlaceholderText("What should the character remember?"), { target: { value: "He hums when he cooks." } });
    fireEvent.change(within(dialog).getByRole("combobox"), { target: { value: "preference" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Add" }));
    await waitFor(() => expect(called("memoryAdd")).toHaveLength(1));
    expect(called("memoryAdd")[0]?.request).toMatchObject({
      conversation_id: "c1", text: "He hums when he cooks.", category: "preference", observed_at: null, expected_revision: 5,
    });
    expect(typeof called("memoryAdd")[0]?.request.client_operation_id).toBe("string");
  });

  it("edits only the fields that changed and keeps the rest", async () => {
    const { called } = await open({ world: { items: [item("m1", "Likes black coffee.", { category: "preference" })] } });
    fireEvent.click(await screen.findByRole("button", { name: /Likes black coffee/ }));
    const dialog = await screen.findByRole("dialog", { name: "Memory" });
    fireEvent.change(within(dialog).getByDisplayValue("Likes black coffee."), { target: { value: "Likes black coffee, no sugar." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(called("memoryUpdate")).toHaveLength(1));
    expect(called("memoryUpdate")[0]?.request).toMatchObject({
      memory_id: "m1", text: "Likes black coffee, no sugar.", category: { type: "keep" }, observed_at: { type: "keep" }, expected_revision: 5,
    });
    expect(called("memorySetTemperature")).toHaveLength(0);
    expect(called("memoryPin")).toHaveLength(0);
  });

  it("applies text, temperature and pin one after another with rising revisions", async () => {
    let finishUpdate: () => void = () => {};
    const gate = new Promise<void>((resolve) => {
      finishUpdate = resolve;
    });
    const { called, world } = await open({
      world: { items: [item("m1", "Likes black coffee.", { temperature: "cold" })] },
      calls: {
        memoryUpdate: async (request: Record<string, unknown>) => {
          await gate;
          world.revision += 1;
          return { status: "ok", data: { revision: world.revision, memory_id: String(request.memory_id) } };
        },
      },
    });
    fireEvent.click(await screen.findByRole("button", { name: /Likes black coffee/ }));
    const dialog = await screen.findByRole("dialog", { name: "Memory" });
    fireEvent.change(within(dialog).getByDisplayValue("Likes black coffee."), { target: { value: "Likes tea." } });
    fireEvent.click(within(dialog).getByRole("radio", { name: "Hot" }));
    fireEvent.click(within(dialog).getByRole("switch", { name: "Pinned" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(world.revision).toBe(5));
    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(called("memorySetTemperature")).toHaveLength(0);
    expect(called("memoryPin")).toHaveLength(0);
    await act(async () => {
      finishUpdate();
    });
    await waitFor(() => expect(called("memoryPin")).toHaveLength(1));
    expect(called("memorySetTemperature")[0]?.request).toMatchObject({ memory_id: "m1", temperature: "hot", expected_revision: 6 });
    expect(called("memoryPin")[0]?.request).toMatchObject({ memory_id: "m1", pinned: true, expected_revision: 7 });
  });

  it("deletes a memory only after the confirmation", async () => {
    const { called } = await open({ world: { items: [item("m1", "Likes black coffee.")] } });
    fireEvent.click(await screen.findByRole("button", { name: /Likes black coffee/ }));
    const dialog = await screen.findByRole("dialog", { name: "Memory" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Delete" }));
    fireEvent.click(within(await screen.findByRole("dialog", { name: "Delete this memory?" })).getByRole("button", { name: "Keep" }));
    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(called("memoryDelete")).toHaveLength(0);
    fireEvent.click(within(await screen.findByRole("dialog", { name: "Memory" })).getByRole("button", { name: "Delete" }));
    const confirmation = await screen.findByRole("dialog", { name: "Delete this memory?" });
    fireEvent.click(within(confirmation).getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(called("memoryDelete")).toHaveLength(1));
    expect(called("memoryDelete")[0]?.request).toMatchObject({ memory_id: "m1", expected_revision: 5 });
  });

  it("filters by category and by search text", async () => {
    await open({
      world: {
        items: [
          item("m1", "Likes black coffee.", { category: "preference" }),
          item("m2", "Never mention his daughter.", { category: "boundary", origin: "model", cycle_label: "21-40" }),
          item("m3", "Drives an old truck.", { category: "preference", pinned: true }),
        ],
      },
    });
    await screen.findByText("3 memories · 1 by the model, 2 by you");
    expect(screen.getByText("Messages 21 to 40")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Boundary" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Relationship" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Preference" }));
    expect(screen.queryByText("Never mention his daughter.")).toBeNull();
    expect(screen.getByText("2 found")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "All" }));
    fireEvent.change(screen.getByPlaceholderText("Search memories"), { target: { value: "truck" } });
    expect(screen.getByText("1 found")).toBeTruthy();
    expect(screen.queryByText("Likes black coffee.")).toBeNull();
    fireEvent.change(screen.getByPlaceholderText("Search memories"), { target: { value: "zebra" } });
    expect(screen.getByText('Nothing matches "zebra"')).toBeTruthy();
  });

  it("saves and clears the context summary", async () => {
    const { called } = await open({ world: { summary: { text: "On the road to the coast.", origin: "model", token_count: 7 } } });
    fireEvent.click(await screen.findByRole("button", { name: /Context summary/ }));
    const dialog = await screen.findByRole("dialog", { name: "Context summary" });
    fireEvent.change(within(dialog).getByDisplayValue("On the road to the coast."), { target: { value: "Arrived at the coast." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(called("memorySummaryUpdate")).toHaveLength(1));
    expect(called("memorySummaryUpdate")[0]?.request).toMatchObject({ summary: { type: "set", text: "Arrived at the coast." }, expected_revision: 5 });
    fireEvent.click(await screen.findByRole("button", { name: /Context summary/ }));
    fireEvent.click(within(await screen.findByRole("dialog", { name: "Context summary" })).getByRole("button", { name: "Clear" }));
    await waitFor(() => expect(called("memorySummaryUpdate")).toHaveLength(2));
    expect(called("memorySummaryUpdate")[1]?.request).toMatchObject({ summary: { type: "clear" }, expected_revision: 6 });
  });
});

describe("memory status card", () => {
  it("asks before updating and can skip", async () => {
    const { called } = await open({ world: { status: { ...idleStatus, pending_approval_count: 20 } } });
    expect(await screen.findByText("Update memory?")).toBeTruthy();
    expect(screen.getByText("20 new messages are ready to be summarized into memory.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Not now" }));
    await waitFor(() => expect(called("memorySkip")).toHaveLength(1));
    await waitFor(() => expect(screen.queryByText("Update memory?")).toBeNull());
    expect(screen.getByText("Next update in 12 messages")).toBeTruthy();
  });

  it("starts an update from the approval and follows its job", async () => {
    const watched: string[] = [];
    const { called } = await open({
      world: { status: { ...idleStatus, pending_approval_count: 20 } },
      streams: {
        jobWatch: (args) => {
          watched.push((args as [{ job_id: string }])[0].job_id);
          return new Promise(() => {});
        },
      },
    });
    fireEvent.click(await screen.findByRole("button", { name: "Update" }));
    await waitFor(() => expect(called("memoryTrigger")).toHaveLength(1));
    expect(await screen.findByText("Updating memory")).toBeTruthy();
    await waitFor(() => expect(watched).toEqual(["job-new"]));
  });

  it("shows a running update and stops it", async () => {
    const job: JobView_Serialize = {
      id: "job-1", kind: "memory_extraction", subject: { kind: "conversation", id: "c1" } as never, subject_detail: null, state: "running",
      progress: { current: 5, total: 10, unit: "items", label_code: null, bytes_per_second: null }, created_at: 1, updated_at: 1, failure: null, result: null,
    };
    const { called } = await open({
      world: { status: { ...idleStatus, latest_cycle_status: "processing", latest_job_id: "job-1" } },
      streams: { jobWatch: () => ({ status: "ok", data: job }) },
    });
    expect(await screen.findByText("Updating memory")).toBeTruthy();
    await waitFor(() => expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("50"));
    fireEvent.click(screen.getByRole("button", { name: "Stop" }));
    await waitFor(() => expect(called("jobCancel")).toHaveLength(1));
    expect(called("jobCancel")[0]?.request).toEqual({ job_id: "job-1" });
  });

  it("explains a failure and retries with another model", async () => {
    const { called } = await open({
      world: { status: { ...idleStatus, failure: "provider_unavailable" } },
      calls: {
        modelsList: () => ({
          status: "ok",
          data: {
            models: [{ id: "m9", kind: "chat", display_name: "Local helper", provider_account_id: "a1", external_model_id: "local" } as never],
            default_model_id: null, revision: 1,
          },
        }),
      },
    });
    expect(await screen.findByText("Memory update failed")).toBeTruthy();
    expect(screen.getByText("The provider didn't answer.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(called("memoryRetry")).toHaveLength(1));
    expect(called("memoryRetry")[0]?.request).toMatchObject({ conversation_id: "c1", model_profile_id: null });
  });

  it("sends the picked model when retrying with another one", async () => {
    const { called } = await open({
      world: { status: { ...idleStatus, failure: "timed_out" } },
      calls: {
        modelsList: () => ({
          status: "ok",
          data: {
            models: [{ id: "m9", kind: "chat", display_name: "Local helper", provider_account_id: "a1", external_model_id: "local" } as never],
            default_model_id: null, revision: 1,
          },
        }),
      },
    });
    fireEvent.click(await screen.findByRole("button", { name: "Use another model" }));
    fireEvent.click(await screen.findByRole("button", { name: /Local helper/ }));
    await waitFor(() => expect(called("memoryRetry")).toHaveLength(1));
    expect(called("memoryRetry")[0]?.request).toMatchObject({ model_profile_id: "m9" });
  });

  it("dismisses a failure", async () => {
    const { called } = await open({ world: { status: { ...idleStatus, failure: "internal" } } });
    await screen.findByText("Memory update failed");
    fireEvent.click(within(screen.getByText("Memory update failed").closest("[role=alert]") as HTMLElement).getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(called("memoryErrorDismiss")).toHaveLength(1));
  });

  it("updates now from the idle card and says manual runs only start here", async () => {
    const { called } = await open({ world: { status: { ...idleStatus, run_mode: "ask_first" } } });
    expect(await screen.findByText("Ask first · every 20 messages")).toBeTruthy();
    expect(screen.getByText("8 of 20")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Update now" }));
    await waitFor(() => expect(called("memoryTrigger")).toHaveLength(1));
    cleanup();
    await open({ world: { status: { ...idleStatus, run_mode: "manual" } } });
    expect(await screen.findByText("Manual · updates only run when you start them here")).toBeTruthy();
  });
});

describe("memory activity and pinned tabs", () => {
  const cycles = [
    cycle("r3", { window_start: 41, window_end: 60, label: "41-60", summary: "The trip took shape.", actions: [{ kind: "created", memory_id: "m1", text: "Afraid of flying." }], revertable: true }),
    cycle("r2", { window_start: 21, window_end: 40, label: "21-40", status: "failed", failure: "timed_out" }),
    cycle("r1", { window_start: 1, window_end: 20, label: "1-20", revertable: false, blocked_by: "r3", actions: [{ kind: "soft_deleted", memory_id: null, text: "The safehouse." }] }),
  ];

  it("pages the cycles, explains a blocked undo and undoes after confirmation", async () => {
    const { called } = await open({ world: { cycles } });
    fireEvent.click(await screen.findByRole("tab", { name: /^Activity/ }));
    fireEvent.click(await screen.findByRole("button", { name: /Messages 41 to 60/ }));
    expect(await screen.findByText("Afraid of flying.")).toBeTruthy();
    expect(screen.getByText("Added")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Messages 21 to 40/ }));
    expect(await screen.findByText("The model took too long to answer.")).toBeTruthy();
    expect(screen.getByText("Failed")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Show older updates" }));
    fireEvent.click(await screen.findByRole("button", { name: /Messages 1 to 20/ }));
    expect(await screen.findByText("Can't be undone: the update for Messages 41 to 60 builds on it.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Messages 41 to 60/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Undo this update" }));
    fireEvent.click(await screen.findByRole("button", { name: "Keep" }));
    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(called("memoryCycleRevert")).toHaveLength(0);
    fireEvent.click(screen.getByRole("button", { name: "Undo this update" }));
    fireEvent.click(await screen.findByRole("button", { name: "Undo update" }));
    await waitFor(() => expect(called("memoryCycleRevert")).toHaveLength(1));
    expect(called("memoryCycleRevert")[0]?.request).toMatchObject({ run_id: "r3", expected_revision: 5 });
  });

  const pinnedMessage = message("p1", "assistant", "Karsk was a choice.", { pinned: true });
  const pinnedUser = message("p2", "user", "I will drive.", { pinned: true });

  it("lists pinned messages with their speaker, unpins one and opens another in the chat", async () => {
    const { called, runtime } = await open({ world: { pinned: [pinnedMessage, pinnedUser] } });
    fireEvent.click(await screen.findByRole("tab", { name: /^Pinned/ }));
    expect(await screen.findByText(/Cassian ·/)).toBeTruthy();
    expect(screen.getByText(/You ·/)).toBeTruthy();
    fireEvent.click(screen.getAllByRole("button", { name: "Unpin" })[0]!);
    await waitFor(() => expect(called("messagePin")).toHaveLength(1));
    expect(called("messagePin")[0]?.request).toMatchObject({ message_id: "p1", pinned: false, expected_revision: 4 });
    await waitFor(() => expect(screen.queryByText("Karsk was a choice.")).toBeNull());
    fireEvent.click(screen.getByRole("button", { name: /I will drive/ }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/chat/c1"));
    await waitFor(() => expect(called("conversationMessagesAround")).toHaveLength(1));
    expect(called("conversationMessagesAround")[0]?.request).toMatchObject({ message_id: "p2" });
    await waitFor(() => expect(runtime.router.state.location.search).toEqual({}));
  });

  it("shows an empty state when nothing is pinned", async () => {
    await open({ world: { pinned: [] } });
    fireEvent.click(await screen.findByRole("tab", { name: /^Pinned/ }));
    expect(await screen.findByText("No pinned messages")).toBeTruthy();
  });
});


describe("memory recovery and constraints", () => {
  it("replays the original add request after a lost reply and a refreshed view", async () => {
    const requests: MemoryAddRequest[] = [];
    let original: MemoryAddRequest | undefined;
    const { world, runtime } = await open({
      mode: "manual",
      calls: {
        memoryAdd: (request) => {
          requests.push(request);
          if (original) {
            if (JSON.stringify(request) !== JSON.stringify(original)) {
              return { status: "error", error: { code: "conflict", message: "request digest changed", details: null } };
            }
          } else {
            original = request;
            world.items = [item("added", request.text)];
            world.revision += 1;
            throw new Error("reply lost after commit");
          }
          return { status: "ok", data: { revision: world.revision, memory_id: "added" } };
        },
      },
    });
    fireEvent.click(await screen.findByRole("button", { name: "Add memory" }));
    const dialog = await screen.findByRole("dialog", { name: "Add memory" });
    fireEvent.change(within(dialog).getByPlaceholderText("What should the character remember?"), { target: { value: "Coffee." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Add" }));
    await waitFor(() => expect(world.items).toHaveLength(1));
    await waitFor(() => expect(within(dialog).getByRole("button", { name: "Add" }).hasAttribute("disabled")).toBe(false));
    await act(async () => { await runtime.queryClient.invalidateQueries({ queryKey: ["memory", "c1"] }); });
    fireEvent.click(within(dialog).getByRole("button", { name: "Add" }));
    await waitFor(() => expect(requests).toHaveLength(2));
    expect(requests[1]).toEqual(requests[0]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add memory" })).toBeNull());
    expect(world.items).toHaveLength(1);
  });

  it("uses a fresh operation and revision after a definitive revision conflict", async () => {
    const requests: MemoryAddRequest[] = [];
    const { world } = await open({
      mode: "manual",
      calls: {
        memoryAdd: (request) => {
          requests.push(request);
          world.revision += 1;
          return requests.length === 1
            ? { status: "error", error: { code: "conflict", message: "revision changed", details: null } }
            : { status: "ok", data: { revision: world.revision, memory_id: "added" } };
        },
      },
    });
    fireEvent.click(await screen.findByRole("button", { name: "Add memory" }));
    const dialog = await screen.findByRole("dialog", { name: "Add memory" });
    fireEvent.change(within(dialog).getByPlaceholderText("What should the character remember?"), { target: { value: "Coffee." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Add" }));
    await waitFor(() => expect(within(dialog).getByRole("button", { name: "Add" }).hasAttribute("disabled")).toBe(false));
    expect(requests).toHaveLength(1);
    fireEvent.click(within(dialog).getByRole("button", { name: "Add" }));
    await waitFor(() => expect(requests).toHaveLength(2));
    expect(requests[1]?.expected_revision).toBe(6);
    expect(requests[1]?.client_operation_id).not.toBe(requests[0]?.client_operation_id);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add memory" })).toBeNull());
  });

  it("unpins a memory before cooling it in the same save", async () => {
    const actions: string[] = [];
    const { world } = await open({
      world: { items: [item("m1", "Coffee.", { pinned: true })] },
      calls: {
        memoryPin: (request) => {
          actions.push("pin");
          world.items = world.items.map((entry) => ({ ...entry, pinned: request.pinned }));
          return { status: "ok", data: { revision: ++world.revision, memory_id: "m1" } };
        },
        memorySetTemperature: (request) => {
          actions.push("temperature");
          if (world.items[0]?.pinned && request.temperature === "cold") {
            return { status: "error", error: { code: "invalid_input", message: "pinned memory cannot be cold", details: null } };
          }
          world.items = world.items.map((entry) => ({ ...entry, temperature: request.temperature }));
          return { status: "ok", data: { revision: ++world.revision, memory_id: "m1" } };
        },
      },
    });
    fireEvent.click(await screen.findByRole("button", { name: /Coffee/ }));
    const dialog = await screen.findByRole("dialog", { name: "Memory" });
    fireEvent.click(within(dialog).getByRole("switch", { name: "Pinned" }));
    fireEvent.click(within(dialog).getByRole("radio", { name: "Cold" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Memory" })).toBeNull());
    expect(actions).toEqual(["pin", "temperature"]);
    expect(world.items[0]).toMatchObject({ pinned: false, temperature: "cold" });
  });

  it("keeps pinned and cold mutually exclusive in the draft", async () => {
    await open({ world: { items: [item("m1", "Coffee.", { pinned: true })] } });
    fireEvent.click(await screen.findByRole("button", { name: /Coffee/ }));
    const dialog = await screen.findByRole("dialog", { name: "Memory" });
    fireEvent.click(within(dialog).getByRole("radio", { name: "Cold" }));
    expect(within(dialog).getByRole("switch", { name: "Pinned" }).getAttribute("aria-checked")).toBe("false");
    fireEvent.click(within(dialog).getByRole("switch", { name: "Pinned" }));
    expect(within(dialog).getByRole("radio", { name: "Hot" }).getAttribute("aria-checked")).toBe("true");
  });

  it("uses the backend's allowed categories and preserves an existing category outside them", async () => {
    const { world, runtime, called } = await open({ world: { items: [item("m1", "Coffee.", { category: "boundary" })] } });
    fireEvent.click(await screen.findByRole("button", { name: /Coffee/ }));
    const dialog = await screen.findByRole("dialog", { name: "Memory" });
    expect((within(dialog).getByRole("combobox") as HTMLSelectElement).value).toBe("boundary");
    expect(within(dialog).getByRole("option", { name: "Boundary" }).hasAttribute("disabled")).toBe(true);
    expect(within(dialog).queryByRole("option", { name: "Milestone" })).toBeNull();
    fireEvent.change(within(dialog).getByDisplayValue("Coffee."), { target: { value: "Black coffee." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(called("memoryUpdate")).toHaveLength(1));
    expect(called("memoryUpdate")[0]?.request.category).toEqual({ type: "keep" });
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Memory" })).toBeNull());
    world.allowed_categories = [...world.allowed_categories, "milestone", "boundary", "profile", "routine", "episodic", "emotional_snapshot"];
    await act(async () => { await runtime.queryClient.invalidateQueries({ queryKey: ["memory", "c1"] }); });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    const adding = await screen.findByRole("dialog", { name: "Add memory" });
    expect(within(adding).getByRole("option", { name: "Milestone" }).hasAttribute("disabled")).toBe(false);
  });

  it("shows loading until the activity query settles", async () => {
    let resolveCycles: (page: MemoryCyclePage) => void = () => {};
    const pending = new Promise<MemoryCyclePage>((resolve) => { resolveCycles = resolve; });
    await open({ calls: { memoryCycles: async () => ({ status: "ok", data: await pending }) } });
    fireEvent.click(await screen.findByRole("tab", { name: /^Activity/ }));
    expect(screen.queryByText("No updates yet")).toBeNull();
    await act(async () => { resolveCycles({ items: [], next_cursor: null }); });
    expect(await screen.findByText("No updates yet")).toBeTruthy();
  });

  it("shows and retries an activity query error instead of reporting an empty history", async () => {
    const read = vi.fn<NonNullable<MockCallHandlers["memoryCycles"]>>()
      .mockResolvedValueOnce({ status: "error", error: { code: "unavailable", message: "offline", details: null } })
      .mockResolvedValue({ status: "ok", data: { items: [], next_cursor: null } });
    await open({ calls: { memoryCycles: read } });
    fireEvent.click(await screen.findByRole("tab", { name: /^Activity/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Try again" }));
    expect(await screen.findByText("No updates yet")).toBeTruthy();
    expect(read).toHaveBeenCalledTimes(2);
  });

  it("keeps loaded activity and retries a failed older page with its cursor", async () => {
    const first = cycle("first", { window_start: 21, window_end: 40 });
    const read = vi.fn<NonNullable<MockCallHandlers["memoryCycles"]>>()
      .mockResolvedValueOnce({ status: "ok", data: { items: [first], next_cursor: "older" } })
      .mockResolvedValueOnce({ status: "error", error: { code: "unavailable", message: "offline", details: null } })
      .mockResolvedValue({ status: "ok", data: { items: [cycle("oldest")], next_cursor: null } });
    await open({ calls: { memoryCycles: read } });
    fireEvent.click(await screen.findByRole("tab", { name: /^Activity/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Show older updates" }));
    const retry = await screen.findByRole("button", { name: "Try again" });
    expect(screen.getByRole("button", { name: /Messages 21 to 40/ })).toBeTruthy();
    fireEvent.click(retry);
    expect(await screen.findByRole("button", { name: /Messages 1 to 20/ })).toBeTruthy();
    expect(read.mock.calls.map(([request]) => request.cursor)).toEqual([null, "older", "older"]);
  });

  it("reconnects a failed job watch and follows the same job to settlement", async () => {
    const job: JobView_Serialize = {
      id: "job-1", kind: "memory_extraction", subject: { kind: "conversation", id: "c1" } as never, subject_detail: null, state: "running",
      progress: { current: 5, total: 10, unit: "items", label_code: null, bytes_per_second: null }, created_at: 1, updated_at: 1, failure: null, result: null,
    };
    let settle: (job: JobView_Serialize) => void = () => {};
    const pending = new Promise<JobView_Serialize>((resolve) => { settle = resolve; });
    const watch = vi.fn<NonNullable<MockStreamHandlers["jobWatch"]>>()
      .mockRejectedValueOnce(new Error("watch disconnected"))
      .mockImplementation(async (_args, emit) => { emit({ type: "progress", job }); return { status: "ok", data: await pending }; });
    const { world, called } = await open({ world: { status: { ...idleStatus, latest_cycle_status: "processing", latest_job_id: "job-1" } }, streams: { jobWatch: watch } });
    const retry = await screen.findByRole("button", { name: "Try again" });
    expect(screen.getByText("Updating memory")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Stop" })).toBeTruthy();
    fireEvent.click(retry);
    await waitFor(() => expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("50"));
    expect(watch.mock.calls.map(([args]) => args[0].job_id)).toEqual(["job-1", "job-1"]);
    expect(called("memoryTrigger")).toHaveLength(0);
    expect(called("memoryRetry")).toHaveLength(0);
    await act(async () => {
      world.status = idleStatus;
      settle({ ...job, state: "succeeded" });
    });
    expect(await screen.findByText("Next update in 12 messages")).toBeTruthy();
  });
});


it("retries a later save step without repeating an already committed text edit", async () => {
  let textWrites = 0;
  const temperatures: unknown[] = [];
  const { world } = await open({
    world: { items: [item("m1", "Coffee.")] },
    calls: {
      memoryUpdate: (request) => {
        textWrites += 1;
        world.items = world.items.map((entry) => ({ ...entry, text: request.text ?? entry.text }));
        return { status: "ok", data: { revision: ++world.revision, memory_id: "m1" } };
      },
      memorySetTemperature: (request) => {
        temperatures.push(request);
        if (temperatures.length === 1) throw new Error("not sent");
        if (request.expected_revision !== world.revision) {
          return { status: "error", error: { code: "conflict", message: "revision changed", details: null } };
        }
        world.items = world.items.map((entry) => ({ ...entry, temperature: request.temperature }));
        return { status: "ok", data: { revision: ++world.revision, memory_id: "m1" } };
      },
    },
  });
  fireEvent.click(await screen.findByRole("button", { name: /Coffee/ }));
  const dialog = await screen.findByRole("dialog", { name: "Memory" });
  fireEvent.change(within(dialog).getByDisplayValue("Coffee."), { target: { value: "Tea." } });
  fireEvent.click(within(dialog).getByRole("radio", { name: "Cold" }));
  fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
  await waitFor(() => expect(temperatures).toHaveLength(1));
  await waitFor(() => expect(within(dialog).getByRole("button", { name: "Save" }).hasAttribute("disabled")).toBe(false));
  fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "Memory" })).toBeNull());
  expect(textWrites).toBe(1);
  expect(temperatures[1]).toEqual(temperatures[0]);
  expect(world.items[0]).toMatchObject({ text: "Tea.", temperature: "cold" });
});


it("starts a new add action after cancelling an uncertain save", async () => {
  const requests: MemoryAddRequest[] = [];
  await open({ mode: "manual", calls: { memoryAdd: (request) => {
    requests.push(request);
    if (requests.length === 1) throw new Error("reply lost");
    return { status: "ok", data: { revision: 6, memory_id: "added" } };
  } } });
  const fillAndAdd = async () => {
    fireEvent.click(await screen.findByRole("button", { name: "Add memory" }));
    const dialog = await screen.findByRole("dialog", { name: "Add memory" });
    fireEvent.change(within(dialog).getByPlaceholderText("What should the character remember?"), { target: { value: "Coffee." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Add" }));
    return dialog;
  };
  const first = await fillAndAdd();
  await waitFor(() => expect(within(first).getByRole("button", { name: "Cancel" }).hasAttribute("disabled")).toBe(false));
  fireEvent.click(within(first).getByRole("button", { name: "Cancel" }));
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add memory" })).toBeNull());
  await fillAndAdd();
  await waitFor(() => expect(requests).toHaveLength(2));
  expect(requests[1]?.client_operation_id).not.toBe(requests[0]?.client_operation_id);
});
