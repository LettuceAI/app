import { act, cleanup, configure, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createBrowserHistory, createMemoryHistory, type RouterHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CharacterDraft_Serialize, CompanionEmotionVector, CompanionSoulView, CompanionStateView, ConversationSettingsView, ConversationView, JobView_Serialize, MemoryCycleView, MemoryItemView, MemoryView, MessageCompanionEffect, SoulFactView, ThymosStatus, TimelineMessage } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers, type MockStreamHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";
import companionDraft from "./fixtures/character.json";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));
configure({ asyncUtilTimeout: 3000 });
const histories: RouterHistory[] = [];
afterEach(() => { cleanup(); histories.splice(0).forEach((history) => history.destroy()); });

const NOW = 1_700_000_000_000;
const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const vector = (patch: Partial<CompanionEmotionVector> = {}): CompanionEmotionVector => ({ warmth: null, trust: null, calm: null, vulnerability: null, longing: null, hurt: null, tension: null, irritation: null, affection_intensity: null, reassurance_need: null, ...patch });
const message = (id = "a1", role: TimelineMessage["role"] = "assistant", text = "A reply."): TimelineMessage => ({ id, role, author_participant_id: role === "user" ? "pu" : "pc", parts: [{ type: "text", text }], reasoning: null, created_at: NOW, candidate_index: null, candidate_count: 1, pinned: false, scene_image: null });
const conversation = (): ConversationView => ({ id: "c1", origin_conversation_id: null, origin_message_id: null, kind: "direct", title: "Harbour", branch: { branch_id: "b1", head_message_id: "a1" }, participants: [{ id: "pc", role: "character", name: "Mira", character_id: "ch1", avatar: null }, { id: "pu", role: "user", name: "You", character_id: null, avatar: null }], messages: { items: [message("u1", "user", "Hello."), message()], next_cursor: null }, pending_turn_id: null, can_send: true, revision: 4, settings_revision: 2, archived: false, source: { type: "direct", character_id: "ch1" }, chat_mode: null, missing_models: [], memory_blocked: null });
const settings = (): ConversationSettingsView => ({ conversation_id: "c1", kind: "direct", title: "Harbour", revision: 4, settings_revision: 2, author_note: null, persona: { id: null, source: "app_default" }, prompt: { id: null, source: "app_default" }, roleplay_prompt: null, lorebooks: { ids: [], source: "app_default" }, model: { id: "m1", source: "character" }, background: { asset: null, hidden: false, source: "character" }, scene: { id: null, source: "app_default" }, memory: { mode: "dynamic", source: "character" }, chat_mode: null, disable_character_lorebooks: null, speaker_selection: null, members: null, model_settings: null, companion_time: { mode: "live", anchor_at: null, set_at: null, effective_at: NOW } });
const state = (): CompanionStateView => ({ conversation_id: "c1", character_id: "ch1", persona_id: null, relationship: { closeness: 0.46, trust: -0.38, affection: 0.52, tension: 0.22, stability: 0.71, interaction_count: 1284, last_interaction_at: NOW }, felt: vector({ warmth: 0.72, longing: 0.41 }), expressed: vector({ warmth: 0.58 }), blocked: vector({ hurt: 0.18 }), momentum: vector({ trust: 0.24 }), active_drivers: ["caring"], active_signals: [], confidence: null, emotions_updated_at: NOW, updated_at: NOW });
const fact = (id = "f1", patch: Partial<SoulFactView> = {}): SoulFactView => ({ id, category: "likes", value: "Enjoys morning runs.", kind: "add", policy: "adaptive", slot: "running", confidence: 0.82, evidence_count: 1, weight: 0.7, valid_from: NOW, valid_until: null, locked: false, created_at: NOW, source_memory_ids: ["m1"], supersedes: [], superseded_by: null, superseded_at: null, ...patch });
const soul = (): CompanionSoulView => {
  const config = structuredClone(companionDraft).defaults.companion_soul;
  return { character_id: "ch1", conversation_id: "c1", config: { ...config, soul: { ...config.soul, essence: "Patient and curious." }, authored_facts: [], relationship_defaults: { closeness: 0.1, trust: 0.1, affection: 0.05, tension: 0 } }, growth: { owner: "conversation", revision: 8, active_count: 1, superseded_count: 0, facts: [fact()] } };
};
const item = (id = "m1", patch: Partial<MemoryItemView> = {}): MemoryItemView => ({ id, short_id: id, text: "We went running by the lake.", category: "milestone", origin: "model", pinned: false, temperature: "hot", observed_at: NOW, observed_time_precision: null, token_count: 12, cycle_label: "1-20", ...patch });
const memory = (): MemoryView => ({ items: [item()], allowed_categories: ["profile", "relationship", "milestone", "boundary", "preference", "routine", "episodic", "emotional_snapshot"], summary: null, status: { run_mode: "auto", interval: 20, messages_since_last_cycle: 11, messages_until_next_cycle: 9, total_conversation_messages: 11, pending_approval_count: null, skipped: false, latest_cycle_status: null, latest_job_id: null, failure: null, paused_reason: null }, revision: 4 });
const effect = (patch: Partial<MessageCompanionEffect> = {}): MessageCompanionEffect => ({ status: "ready", summary: null, relationship: { closeness: 0.04, trust: 0.03, affection: null, tension: -0.05, stability: null }, felt: vector({ warmth: 0.12, hurt: -0.08 }), expressed: vector(), blocked: vector(), signals_added: ["relief"], signals_removed: ["distress"], memories_added: ["A reassuring moment."], memories_updated: [], memories_superseded: [], ...patch });

interface World { state: CompanionStateView | null; soul: CompanionSoulView; memory: MemoryView; effect: MessageCompanionEffect | null; settings: ConversationSettingsView; emotion: ThymosStatus; cycles: MemoryCycleView[] }
interface Setup { path?: string; companion?: boolean; world?: Partial<World>; calls?: MockCallHandlers; streams?: MockStreamHandlers; name?: string; language?: "en" | "cimode"; browser?: boolean }

async function open(setup: Setup = {}) {
  const world: World = { state: state(), soul: soul(), memory: memory(), effect: effect(), settings: settings(), emotion: { type: "installed", source_revision: "test" }, cycles: [], ...setup.world };
  const draft = structuredClone(companionDraft) as CharacterDraft_Serialize;
  draft.profile.name = setup.name ?? "Mira";
  draft.profile.definition = "Patient and curious.";
  draft.defaults.interaction_mode = setup.companion === false ? "roleplay" : "companion";
  if (setup.companion === false) world.settings.companion_time = null;
  const requests: { command: string; request: unknown }[] = [];
  const record = (command: string, request: unknown) => requests.push({ command, request: structuredClone(request) });
  const calls: MockCallHandlers = {
    appStatus: () => ok({ ...mockAppStatus, platform: "other", ui_state: { onboarding: { completed: true } } }),
    conversationOpen: () => ok(conversation()),
    conversationSettingsGet: () => ok(structuredClone(world.settings)),
    conversationSettingsUpdate: (request) => {
      record("conversationSettingsUpdate", request);
      const time = request.patch.companion_time;
      if (time) world.settings = { ...world.settings, settings_revision: (world.settings.settings_revision ?? 0) + 1, companion_time: time.type === "reset" || time.mode === "live" ? { mode: "live", anchor_at: null, set_at: null, effective_at: NOW } : { mode: time.mode, anchor_at: time.anchor_at, set_at: time.mode === "ticking" ? NOW : null, effective_at: time.anchor_at ?? NOW } };
      return ok(structuredClone(world.settings));
    },
    characterGet: () => ok({ id: "ch1", archived: false, draft, assets: [], revision: 1, created_at: NOW, updated_at: NOW }),
    characterLorebooksList: () => ok([]),
    companionStateGet: () => { record("companionStateGet", null); return ok(structuredClone(world.state)); },
    companionSoulGet: (request) => { record("companionSoulGet", request); return ok(structuredClone(world.soul)); },
    companionEmotionStatus: () => ok(world.emotion),
    companionEmotionInstall: () => { record("companionEmotionInstall", null); return ok({ job_id: "install1" }); },
    companionSoulGrowthLock: (request) => {
      record("companionSoulGrowthLock", request);
      world.soul = { ...world.soul, growth: { ...world.soul.growth, revision: world.soul.growth.revision + 1, facts: world.soul.growth.facts.map((entry) => entry.id === request.fact_id ? { ...entry, locked: request.locked } : entry) } };
      return ok(true);
    },
    companionSoulGrowthRemove: (request) => {
      record("companionSoulGrowthRemove", request);
      const facts = world.soul.growth.facts.filter((entry) => entry.id !== request.fact_id);
      world.soul = { ...world.soul, growth: { ...world.soul.growth, facts, active_count: facts.filter((entry) => entry.superseded_by === null).length, revision: world.soul.growth.revision + 1 } };
      return ok(true);
    },
    companionSoulGrowthClear: (request) => { record("companionSoulGrowthClear", request); world.soul = { ...world.soul, growth: { revision: world.soul.growth.revision + 1, owner: "conversation", active_count: 0, superseded_count: 0, facts: [] } }; return ok(1); },
    memoryGet: () => { record("memoryGet", null); return ok(structuredClone(world.memory)); },
    memoryCycles: () => ok({ items: world.cycles, next_cursor: null }),
    memoryAdd: (request) => { record("memoryAdd", request); world.memory = { ...world.memory, items: [...world.memory.items, item("new", { text: request.text, category: request.category, origin: "user" })], revision: world.memory.revision + 1 }; return ok({ revision: world.memory.revision, memory_id: "new" }); },
    memoryUpdate: (request) => { record("memoryUpdate", request); world.memory = { ...world.memory, items: world.memory.items.map((entry) => entry.id === request.memory_id ? { ...entry, text: request.text ?? entry.text } : entry), revision: world.memory.revision + 1 }; return ok({ revision: world.memory.revision, memory_id: request.memory_id }); },
    memoryCycleRevert: (request) => { record("memoryCycleRevert", request); world.cycles = world.cycles.map((entry) => ({ ...entry, reverted: true })); world.memory.revision += 1; return ok({ revision: world.memory.revision, memory_id: null }); },
    messageCompanionEffect: () => { record("messageCompanionEffect", null); return ok(structuredClone(world.effect)); },
    llmMetricsForMessage: () => ok(null),
    messageUsage: () => ok(null),
    modelsList: () => ok({ models: [], default_model_id: null, revision: 1 }),
    providerAccountsList: () => ok([]),
    charactersList: () => ok({ items: [], next_cursor: null }),
    conversationsLatestByCharacter: () => ok({ items: [], next_cursor: null }),
    conversationsList: () => ok({ items: [], next_cursor: null }),
    conversationMessageCount: () => ok({ count: 2 }),
    companionNotesList: () => ok([]),
    personasList: () => ok({ items: [], next_cursor: null, default_persona: { persona_id: null, revision: 1, created_at: NOW, updated_at: NOW } }),
    lorebooksList: () => ok({ items: [], next_cursor: null }),
    jobsList: () => ok({ items: [], next_cursor: null }),
    ...setup.calls,
  };
  const transport = createMockTransport({ calls, ...(setup.streams ? { streams: setup.streams } : {}) });
  if (setup.browser) window.history.replaceState(null, "", setup.path ?? "/chat/c1/companion");
  const history = setup.browser ? createBrowserHistory() : createMemoryHistory({ initialEntries: [setup.path ?? "/chat/c1/companion"] });
  histories.push(history);
  const runtime = await createAppRuntime({ transport, history, i18n: await createTestI18n(setup.language ?? "en") });
  await act(async () => { render(<App runtime={runtime} />); });
  return { runtime, history, world, transport, called: (command: string) => requests.filter((entry) => entry.command === command) };
}

describe("companion chat", () => {
  it("keeps tabs in the route, supports back and forward, and defaults invalid tabs", async () => {
    const app = await open({ path: "/chat/c1/companion?tab=invalid" });
    expect((await screen.findByRole("tab", { name: "Relationship" })).getAttribute("aria-selected")).toBe("true");
    fireEvent.click(screen.getByRole("tab", { name: /^Soul/ }));
    await waitFor(() => expect(app.runtime.router.state.location.search).toEqual({ tab: "soul" }));
    expect(await screen.findByText("Patient and curious.")).toBeTruthy();
    await act(async () => { app.history.back(); });
    await waitFor(() => expect(screen.getByRole("tab", { name: "Relationship" }).getAttribute("aria-selected")).toBe("true"));
    await act(async () => { app.history.forward(); });
    await waitFor(() => expect(screen.getByRole("tab", { name: /^Soul/ }).getAttribute("aria-selected")).toBe("true"));
    expect(app.called("companionSoulGet")[0]?.request).toEqual({ character_id: "ch1", conversation_id: "c1" });
  });

  it("reloads the selected tab and opens the character editor's Soul tab", async () => {
    const app = await open({ path: "/chat/c1/companion?tab=soul" });
    fireEvent.click(await screen.findByRole("button", { name: "Edit" }));
    await waitFor(() => expect(app.runtime.router.state.location.pathname).toBe("/characters/ch1/edit"));
    expect(app.runtime.router.state.location.search).toEqual({ tab: "soul" });
    expect((await screen.findByRole("tab", { name: "Soul" })).getAttribute("aria-selected")).toBe("true");
  });

  it.each([true, false])("routes the header entry correctly for companion=%s", async (companion) => {
    const app = await open({ path: "/chat/c1", companion });
    fireEvent.click(await screen.findByRole("button", { name: companion ? "Companion" : /^Memories,/ }));
    await waitFor(() => expect(app.runtime.router.state.location.pathname).toBe(companion ? "/chat/c1/companion" : "/chat/c1/memory"));
  });

  it("routes the settings memory row to the companion page", async () => {
    const app = await open({ path: "/chat/c1?panel=settings", calls: { characterGet: () => new Promise(() => {}) } });
    fireEvent.click(await screen.findByRole("button", { name: /^Open memories/ }));
    await waitFor(() => expect(app.runtime.router.state.location.pathname).toBe("/chat/c1/companion"));
  });

  it("keeps the bond visible while memory data is still loading", async () => {
    await open({ calls: { memoryGet: () => new Promise(() => {}) } });
    expect(await screen.findByRole("meter", { name: "Closeness" })).toBeTruthy();
    expect(screen.getByText("Recent timeline")).toBeTruthy();
    expect(screen.queryByText("0 events")).toBeNull();
  });

  it("explains unavailable sources when memory is off", async () => {
    const current = settings();
    current.memory.mode = "disabled";
    await open({ path: "/chat/c1/companion?tab=soul", world: { settings: current } });
    fireEvent.click(await screen.findByRole("button", { name: "from 1 memory" }));
    const sources = await screen.findByRole("dialog", { name: "Where this came from" });
    expect(within(sources).queryByText("The source memories are no longer available.")).toBeNull();
    expect(within(sources).getByText("Nothing is remembered or sent with the replies in this chat.")).toBeTruthy();
  });

  it("shows real bond values, default markers, signed deltas, and emotional scales", async () => {
    await open();
    const closeness = await screen.findByRole("meter", { name: "Closeness" });
    expect(closeness.getAttribute("aria-valuenow")).toBe("0.46");
    expect(closeness.getAttribute("aria-valuemin")).toBe("-1");
    expect(closeness.getAttribute("aria-description")).toBe("Character default: 0.1");
    expect(screen.getByRole("meter", { name: "Stability" }).getAttribute("aria-description")).toBeNull();
    expect(screen.getByText("+36")).toBeTruthy();
    expect(screen.getByText("-48")).toBeTruthy();
    expect(screen.getByText("1,284")).toBeTruthy();
    expect(screen.getAllByRole("meter", { name: "Warmth" })[0]?.getAttribute("aria-valuemax")).toBe("1");
    expect(screen.getByText("Caring")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /We went running/ }));
    expect(await screen.findByRole("dialog", { name: "Memory" })).toBeTruthy();
  });

  it("shows the null state without inventing relationship values", async () => {
    await open({ world: { state: null } });
    expect(await screen.findByText("Nothing to show yet")).toBeTruthy();
    expect(screen.queryByRole("meter", { name: "Closeness" })).toBeNull();
  });

  it("offers the existing model install flow when the emotion model is missing", async () => {
    const job: JobView_Serialize = { id: "install1", kind: "artifact_install", subject: { kind: "artifact_install", id: "thymos", display_name: "Lettuce Thymos" }, subject_detail: null, state: "running", progress: { current: 1, total: 10, unit: "bytes", label_code: null, bytes_per_second: null }, created_at: NOW, updated_at: NOW, failure: null, result: null };
    const app = await open({ world: { emotion: { type: "not_installed" } }, streams: { jobWatch: () => ok(job) } });
    expect(await screen.findByText("Feelings aren't being tracked")).toBeTruthy();
    const install = screen.getByRole("button", { name: "Install" });
    await waitFor(() => expect(install.hasAttribute("disabled")).toBe(false));
    fireEvent.click(install);
    await waitFor(() => expect(app.called("companionEmotionInstall")).toHaveLength(1));
    expect(await screen.findByText("Lettuce Thymos")).toBeTruthy();
  });

  it("locks, removes and clears growth for the conversation owner and refetches", async () => {
    const app = await open({ path: "/chat/c1/companion?tab=soul" });
    fireEvent.click(await screen.findByRole("button", { name: "Lock" }));
    await screen.findByRole("button", { name: "Unlock" });
    const locked = app.called("companionSoulGrowthLock")[0]?.request;
    expect(locked).toEqual({ character_id: "ch1", conversation_id: "c1", fact_id: "f1", locked: true, client_operation_id: expect.any(String) });
    fireEvent.click(screen.getByRole("button", { name: "Remove this growth" }));
    fireEvent.click(await screen.findByRole("button", { name: "Remove" }));
    await waitFor(() => expect(app.called("companionSoulGrowthRemove")).toHaveLength(1));
    expect(app.called("companionSoulGrowthRemove")[0]?.request).toMatchObject({ character_id: "ch1", conversation_id: "c1", fact_id: "f1" });
    await screen.findByText("No growth yet. As memories form, the traits that change with you appear here.");
    app.world.soul = { ...app.world.soul, growth: { ...app.world.soul.growth, active_count: 1, facts: [fact("f2")] } };
    await act(async () => app.transport.emitAppEvent({ type: "memory_changed", conversation_id: "c1" }));
    fireEvent.click(await screen.findByRole("button", { name: "Clear growth" }));
    fireEvent.click(await screen.findByRole("button", { name: "Clear all" }));
    await waitFor(() => expect(app.called("companionSoulGrowthClear")).toHaveLength(1));
    expect(app.called("companionSoulGrowthClear")[0]?.request).toMatchObject({ character_id: "ch1", conversation_id: "c1", client_operation_id: expect.any(String) });
    expect(app.called("companionSoulGet").length).toBeGreaterThanOrEqual(4);
    expect(screen.queryByRole("button", { name: "Look for changes" })).toBeNull();
  });

  it("shows replaced text, keeps historical facts locked and opens source memories", async () => {
    const current = soul();
    current.growth.facts = [fact("old", { value: "Avoids running.", superseded_by: "f1", superseded_at: NOW }), fact("f1", { kind: "adjust", supersedes: ["old"], policy: "historical", locked: true })];
    current.growth.superseded_count = 1;
    await open({ path: "/chat/c1/companion?tab=soul", world: { soul: current } });
    expect((await screen.findByText("Avoids running.")).className).toContain("line-through");
    expect(screen.getByRole("button", { name: "Unlock" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByText("1 earlier version retired.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "from 1 memory" }));
    const sources = await screen.findByRole("dialog", { name: "Where this came from" });
    expect(within(sources).getByText("We went running by the lake.")).toBeTruthy();
    fireEvent.click(within(sources).getByRole("button", { name: /We went running/ }));
    expect(await screen.findByRole("dialog", { name: "Memory" })).toBeTruthy();
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Where this came from" })).toBeNull());
  });

  it("reuses the memories list and editor for an add and an edit", async () => {
    const app = await open({ path: "/chat/c1/companion?tab=memories" });
    fireEvent.click(await screen.findByRole("button", { name: "Add" }));
    const sheet = await screen.findByRole("dialog", { name: "Add memory" });
    fireEvent.change(within(sheet).getByRole("textbox", { name: "Memory" }), { target: { value: "Keep the lighthouse in mind." } });
    expect(within(sheet).getByRole("combobox", { name: "Kind" })).toBeTruthy();
    fireEvent.click(within(sheet).getByRole("button", { name: "Add" }));
    await waitFor(() => expect(app.called("memoryAdd")).toHaveLength(1));
    expect(app.called("memoryAdd")[0]?.request).toMatchObject({ conversation_id: "c1", text: "Keep the lighthouse in mind.", category: "profile", expected_revision: 4, client_operation_id: expect.any(String) });
    fireEvent.click(await screen.findByRole("button", { name: /We went running/ }));
    const editor = await screen.findByRole("dialog", { name: "Memory" });
    expect(within(editor).getByRole("radiogroup", { name: "Temperature" })).toBeTruthy();
    expect(within(editor).getByRole("switch", { name: "Pinned" })).toBeTruthy();
    fireEvent.change(within(editor).getByRole("textbox", { name: "Memory" }), { target: { value: "We ran past the lighthouse." } });
    fireEvent.click(within(editor).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(app.called("memoryUpdate")).toHaveLength(1));
  });

  it("reuses cycle revert with the companion confirmation copy", async () => {
    const cycle: MemoryCycleView = { run_id: "r1", job_id: "j1", started_at: NOW, label: "1-20", window_start: 1, window_end: 20, status: "complete", failure: null, summary: "A quiet evening.", actions: [], reverted: false, revertable: true, blocked_by: null };
    const app = await open({ path: "/chat/c1/companion?tab=activity", world: { cycles: [cycle] } });
    fireEvent.click(await screen.findByRole("button", { name: /Messages 1 to 20/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Undo this update" }));
    expect(await screen.findByText("Memories it added are removed and the ones it changed come back. Your own edits are kept.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Undo update" }));
    await waitFor(() => expect(app.called("memoryCycleRevert")).toHaveLength(1));
  });

  it.each([false, true])("hides the effect row for companion=%s when no effect is available", async (companion) => {
    const app = await open({ path: "/chat/c1", companion, world: { effect: null } });
    fireEvent.contextMenu(await screen.findByText("A reply."));
    await screen.findByRole("button", { name: /Copy text/ });
    if (companion) await waitFor(() => expect(app.called("messageCompanionEffect")).toHaveLength(1));
    else expect(app.called("messageCompanionEffect")).toHaveLength(0);
    expect(screen.queryByRole("button", { name: "What this reply changed" })).toBeNull();
  });

  it("shows a processing effect, then refreshes the effect and state from the settled event", async () => {
    const app = await open({ path: "/chat/c1", world: { effect: effect({ status: "processing" }) } });
    fireEvent.contextMenu(await screen.findByText("A reply."));
    fireEvent.click(await screen.findByRole("button", { name: /What this reply changed/ }));
    const sheet = await screen.findByRole("dialog", { name: "What this reply changed" });
    expect(within(sheet).getByRole("status", { name: "Looking at what changed" })).toBeTruthy();
    app.world.effect = effect();
    await act(async () => app.transport.emitAppEvent({ type: "message_effect_settled", conversation_id: "c1", message_id: "a1" }));
    expect(await within(sheet).findByText("+4")).toBeTruthy();
    expect(within(sheet).getByText("-5").className).toContain("text-accent");
    expect(within(sheet).getByText("What they feel")).toBeTruthy();
    expect(within(sheet).getByText("Relief")).toBeTruthy();
    expect(within(sheet).getByText("A reassuring moment.")).toBeTruthy();
    expect(app.called("messageCompanionEffect").length).toBeGreaterThanOrEqual(2);
    await act(async () => app.runtime.router.navigate({ to: "/chat/$conversationId/companion", params: { conversationId: "c1" } }));
    await screen.findByRole("meter", { name: "Closeness" });
    const before = app.called("companionStateGet").length;
    app.world.state = { ...state(), relationship: { ...state().relationship, closeness: 0.6 } };
    await act(async () => app.transport.emitAppEvent({ type: "message_effect_settled", conversation_id: "c1", message_id: "a1" }));
    await waitFor(() => expect(screen.getByRole("meter", { name: "Closeness" }).getAttribute("aria-valuenow")).toBe("0.6"));
    expect(app.called("companionStateGet").length).toBeGreaterThan(before);
  });

  it.each(["failed", "invalidated"] as const)("shows a one-line reason for a %s effect", async (status) => {
    await open({ path: "/chat/c1", world: { effect: effect({ status }) } });
    fireEvent.contextMenu(await screen.findByText("A reply."));
    fireEvent.click(await screen.findByRole("button", { name: /What this reply changed/ }));
    const sheet = await screen.findByRole("dialog", { name: "What this reply changed" });
    expect(within(sheet).getByText(status === "failed" ? "The reply changes could not be analysed." : "These changes no longer apply to this reply.")).toBeTruthy();
  });

  it.each(["live", "frozen", "ticking"] as const)("shows the %s time row and sends a set with the selected anchor", async (mode) => {
    const view = settings();
    view.companion_time = { mode, anchor_at: mode === "live" ? null : NOW, set_at: mode === "ticking" ? NOW : null, effective_at: NOW };
    const app = await open({ path: "/chat/c1?panel=settings", world: { settings: view } });
    const row = await screen.findByRole("button", { name: /^Companion time/ });
    expect(row.textContent).toContain(mode === "live" ? "Real time" : mode === "frozen" ? "Stopped at" : "Running from");
    fireEvent.click(row);
    const sheet = await screen.findByRole("dialog", { name: "Companion time" });
    fireEvent.click(within(sheet).getByRole("radio", { name: "Running" }));
    fireEvent.click(within(sheet).getByRole("button", { name: "Apply" }));
    await waitFor(() => expect(app.called("conversationSettingsUpdate")).toHaveLength(1));
    expect(app.called("conversationSettingsUpdate")[0]?.request).toMatchObject({ conversation_id: "c1", expected_settings_revision: 2, patch: { companion_time: { type: "set", mode: "ticking", anchor_at: NOW } } });
  });

  it("resets only an overridden clock and omits the row for normal chats", async () => {
    const view = settings();
    view.companion_time = { mode: "frozen", anchor_at: NOW, set_at: null, effective_at: NOW };
    const app = await open({ path: "/chat/c1?panel=settings", world: { settings: view } });
    fireEvent.click(await screen.findByRole("button", { name: /^Companion time/ }));
    fireEvent.click(within(await screen.findByRole("dialog", { name: "Companion time" })).getByRole("button", { name: "Reset" }));
    await waitFor(() => expect(app.called("conversationSettingsUpdate")[0]?.request).toMatchObject({ patch: { companion_time: { type: "reset" } } }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Companion time" })).toBeNull());
    fireEvent.click(screen.getByRole("button", { name: /^Companion time/ }));
    expect(within(await screen.findByRole("dialog", { name: "Companion time" })).queryByRole("button", { name: "Reset" })).toBeNull();
    cleanup();
    await open({ path: "/chat/c1?panel=settings", companion: false });
    await screen.findByRole("button", { name: /^Open memories/ });
    expect(screen.queryByRole("button", { name: /^Companion time/ })).toBeNull();
  });

  it("closes sources on browser back and opens them fresh next time", async () => {
    const app = await open({ path: "/chat/c1/companion?tab=relationship", browser: true });
    fireEvent.click(await screen.findByRole("tab", { name: /^Soul/ }));
    fireEvent.click(await screen.findByRole("button", { name: "from 1 memory" }));
    await act(async () => app.history.back());
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Where this came from" })).toBeNull());
    expect(app.runtime.router.state.location.search).toEqual({ tab: "soul" });
    fireEvent.click(screen.getByRole("button", { name: "from 1 memory" }));
    expect(await screen.findByRole("dialog", { name: "Where this came from" })).toBeTruthy();
  });

  it("dismisses the top confirmation before its memory sheet on browser back", async () => {
    const app = await open({ browser: true });
    fireEvent.click(await screen.findByRole("tab", { name: /^Memories/ }));
    fireEvent.click(await screen.findByRole("button", { name: /We went running/ }));
    const memory = await screen.findByRole("dialog", { name: "Memory" });
    fireEvent.click(within(memory).getByRole("button", { name: "Delete" }));
    expect(await screen.findByRole("dialog", { name: "Delete this memory?" })).toBeTruthy();
    await act(async () => app.history.back());
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Delete this memory?" })).toBeNull());
    expect(screen.getByRole("dialog", { name: "Memory" })).toBeTruthy();
    expect(app.runtime.router.state.location.search).toEqual({ tab: "memories" });
    await act(async () => app.history.back());
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Memory" })).toBeNull());
    expect(app.runtime.router.state.location.search).toEqual({ tab: "memories" });
  });

  it("uses phone Bond wording and hides desktop counts and the state column", async () => {
    const original = window.matchMedia;
    vi.spyOn(window, "matchMedia").mockImplementation((query) => ({ ...original(query), matches: query.includes("max-width") || query.includes("prefers-reduced-motion") }));
    await open({ path: "/chat/c1/companion?tab=memories", name: "A very long companion name that extends beyond sixty characters in the header" });
    expect(await screen.findByRole("tab", { name: "Bond" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Memories" })).toBeTruthy();
    expect(screen.queryByText("Current state")).toBeNull();
    expect(screen.queryByText("Context summary")).toBeNull();
  });

  it("rolls back an optimistic lock, keeps the fact, and localizes a typed failure", async () => {
    type Failure = { status: "error"; error: { code: "conflict"; message: string; details: null } };
    let resolve!: (failure: Failure) => void;
    const pending = new Promise<Failure>((done) => { resolve = done; });
    const app = await open({ path: "/chat/c1/companion?tab=soul", calls: { companionSoulGrowthLock: () => pending } });
    fireEvent.click(await screen.findByRole("button", { name: "Lock" }));
    expect(await screen.findByRole("button", { name: "Unlock" })).toBeTruthy();
    await act(async () => resolve({ status: "error", error: { code: "conflict", message: "PRIVATE_DIAGNOSTIC", details: null } }));
    expect(await screen.findByRole("button", { name: "Lock" })).toBeTruthy();
    expect(screen.getByText("Enjoys morning runs.")).toBeTruthy();
    expect(await screen.findByText(app.runtime.i18n.t("errors.code.conflict"))).toBeTruthy();
    expect(screen.queryByText("PRIVATE_DIAGNOSTIC")).toBeNull();
  });

  it("refetches a clock conflict and preserves the anchor draft when retry fails", async () => {
    let reads = 0;
    const update = vi.fn((request: { expected_settings_revision: number | null }) => reads > 1 && request.expected_settings_revision === 3 ? { status: "error" as const, error: { code: "unavailable" as const, message: "PRIVATE_DIAGNOSTIC", details: null } } : { status: "error" as const, error: { code: "conflict" as const, message: "PRIVATE_DIAGNOSTIC", details: null } });
    const app = await open({ path: "/chat/c1?panel=settings", calls: { conversationSettingsGet: () => { reads += 1; return ok({ ...settings(), settings_revision: reads > 1 ? 3 : 2 }); }, conversationSettingsUpdate: update } });
    fireEvent.click(await screen.findByRole("button", { name: /^Companion time/ }));
    const sheet = await screen.findByRole("dialog", { name: "Companion time" });
    fireEvent.click(within(sheet).getByRole("radio", { name: "Stopped" }));
    fireEvent.click(within(sheet).getByRole("button", { name: "Apply" }));
    await waitFor(() => expect(update).toHaveBeenCalledTimes(2));
    expect(update.mock.calls.map(([request]) => request.expected_settings_revision)).toEqual([2, 3]);
    expect(within(sheet).getByRole("radio", { name: "Stopped" }).getAttribute("aria-checked")).toBe("true");
    expect(await screen.findByText(app.runtime.i18n.t("errors.code.unavailable"))).toBeTruthy();
    expect(screen.queryByText("PRIVATE_DIAGNOSTIC")).toBeNull();
  });

  it("refreshes memory and private Soul growth only for the changed conversation", async () => {
    const app = await open({ path: "/chat/c1/companion?tab=memories" });
    await screen.findByText("We went running by the lake.");
    const before = app.called("memoryGet").length;
    const souls = app.called("companionSoulGet").length;
    await act(async () => app.transport.emitAppEvent({ type: "memory_changed", conversation_id: "other" }));
    expect(app.called("memoryGet")).toHaveLength(before);
    expect(app.called("companionSoulGet")).toHaveLength(souls);
    app.world.memory = { ...app.world.memory, items: [item("m2", { text: "An evening by the harbour." })] };
    await act(async () => app.transport.emitAppEvent({ type: "memory_changed", conversation_id: "c1" }));
    expect(await screen.findByText("An evening by the harbour.")).toBeTruthy();
    expect(app.called("memoryGet").length).toBeGreaterThan(before);
    expect(app.called("companionSoulGet").length).toBeGreaterThan(souls);
  });

  it("uses effective running time in the settings row and does not fabricate the first message date", async () => {
    const view = settings();
    view.companion_time = { mode: "ticking", anchor_at: NOW, set_at: NOW, effective_at: NOW + 86_400_000 };
    await open({ path: "/chat/c1?panel=settings", world: { settings: view } });
    expect((await screen.findByRole("button", { name: /^Companion time/ })).textContent).toContain(new Intl.DateTimeFormat("en", { dateStyle: "medium", timeStyle: "short" }).format(NOW + 86_400_000));
    cleanup();
    await open({ calls: { conversationOpen: () => ok({ ...conversation(), messages: { ...conversation().messages, next_cursor: "older" } }) } });
    await screen.findByRole("meter", { name: "Closeness" });
    expect(screen.queryByText(/^Together since/)).toBeNull();
  });

  it("omits missing source contents and keeps a useful source sheet", async () => {
    const current = soul();
    current.growth.facts = [fact("f1", { source_memory_ids: ["gone"] })];
    await open({ path: "/chat/c1/companion?tab=soul", world: { soul: current } });
    fireEvent.click(await screen.findByRole("button", { name: "from 1 memory" }));
    expect(await screen.findByText("The source memories are no longer available.")).toBeTruthy();
    expect(within(screen.getByRole("dialog", { name: "Where this came from" })).queryByText("We went running by the lake.")).toBeNull();
  });
});
