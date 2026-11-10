import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  CharacterDraft,
  CharacterView_Serialize,
  SettingsUpdateRequest_Deserialize,
  SettingsView,
} from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";
import companionDraft from "../characters/fixtures/default-companion-draft.json";
import roleplayDraft from "../characters/fixtures/default-draft.json";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

afterEach(() => cleanup());

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

const row = async (title: string) => (await screen.findByText(title)).closest("button") as HTMLButtonElement;

const settingsView = (developer: boolean): SettingsView =>
  ({
    device: { revision: 6, trusted_certificates: [] },
    global: { developer_mode_enabled: developer },
    sampler_defaults: {},
    default_model_profile_id: null,
    default_prompt_document_id: null,
    dynamic_memory_model_profile_id: null,
    group_speaker_model_profile_id: null,
    revision: 4,
  }) as unknown as SettingsView;

async function mount(path: string, developer: boolean, calls: MockCallHandlers = {}) {
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: () => ok({ ...mockAppStatus, platform: "android", ui_state: { onboarding: { completed: true } } }),
        settingsGet: () => ok(settingsView(developer)),
        jobsList: () => ok({ items: [], next_cursor: null }),
        ...calls,
      },
    }),
    history: createMemoryHistory({ initialEntries: [path] }),
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return runtime;
}

describe("developer mode", () => {
  it("hides the Developer section while developer mode is off", async () => {
    await mount("/settings", false);
    await screen.findByText("Intelligence");
    expect(screen.queryByRole("button", { name: /Developer tools/ })).toBeNull();
    expect(screen.getByRole("button", { name: /Reset/ })).toBeTruthy();
  });

  it("shows the Developer section while developer mode is on", async () => {
    await mount("/settings", true);
    expect(await screen.findByRole("button", { name: /Developer tools/ })).toBeTruthy();
  });

  it("does not open the Developer page by address while developer mode is off", async () => {
    await mount("/settings/developer", false);
    expect(await screen.findByText("Nothing here")).toBeTruthy();
    expect(screen.queryByText("Test chats")).toBeNull();
  });

  it("opens the Developer page by address while developer mode is on", async () => {
    await mount("/settings/developer", true);
    expect(await screen.findByText("Test chats")).toBeTruthy();
    for (const title of [
      "Roleplay chat",
      "Companion chat, filled",
      "Group chat",
      "Companion group chat",
      "Roleplay chat with widgets",
      "Companion chat with widgets",
    ])
      expect(await row(title)).toBeTruthy();
  });

  it("turns developer mode on from About with only that field in the patch", async () => {
    let view = settingsView(false);
    const requests: SettingsUpdateRequest_Deserialize[] = [];
    await mount("/settings/about", false, {
      settingsGet: () => ok(view),
      settingsUpdate: (request) => {
        requests.push(request);
        view = settingsView(true);
        return ok(view);
      },
    });
    fireEvent.click(await screen.findByRole("button", { name: /^Turn on developer mode/ }));
    await waitFor(() => expect(requests).toHaveLength(1));
    expect(requests[0]).toEqual({
      expected_revision: 4,
      patch: {
        section: "general",
        pure_mode: null,
        analytics_enabled: null,
        update_checks_enabled: null,
        developer_mode_enabled: true,
        auto_download_character_card_avatars: null,
        manual_mode_context_window: null,
        lorebook_scan_depth: null,
      },
    });
    expect(await screen.findByText("Developer mode is on")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Turn off" })).toBeTruthy();
  });
});

describe("developer templates", () => {
  function backend(order: string[]) {
    const record =
      <K extends string, T>(name: K, reply: (request: never) => T) =>
      (request: never) => {
        order.push(name);
        return ok(reply(request));
      };
    return {
      characterDraftDefault: (mode: string | null) => {
        order.push(`characterDraftDefault:${mode}`);
        return ok((mode === "companion" ? companionDraft : roleplayDraft) as unknown as CharacterDraft);
      },
      characterSave: (request: { draft: CharacterDraft }) => {
        order.push("characterSave");
        return ok({ id: `character-${order.length}`, draft: request.draft, revision: 1 } as unknown as CharacterView_Serialize);
      },
      personaSave: record("personaSave", () => ({ id: "persona-1", revision: 1 })),
      devConversationSeed: record("devConversationSeed", () => ({ conversation_id: "chat-1" })),
      conversationSettingsGet: record("conversationSettingsGet", () => ({ settings_revision: null, memory: { mode: "disabled", source: "app_default" } })),
      conversationSettingsUpdate: record("conversationSettingsUpdate", () => ({})),
      memoryGet: record("memoryGet", () => ({ items: [], allowed_categories: ["profile", "preference"], summary: null, status: {}, revision: 7 })),
      memoryAdd: record("memoryAdd", () => ({ revision: 8, memory_id: "m" })),
    } as unknown as MockCallHandlers;
  }

  it("creates the roleplay setup, seeds the chat, links the persona and opens it", async () => {
    const order: string[] = [];
    const seeds: unknown[] = [];
    const calls = backend(order);
    const seed = calls.devConversationSeed;
    calls.devConversationSeed = (request) => {
      seeds.push(request);
      return seed!(request);
    };
    const runtime = await mount("/settings/developer", true, calls);
    fireEvent.click(await row("Roleplay chat"));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/chat/chat-1"));
    expect(order.slice(0, 6)).toEqual([
      "characterDraftDefault:roleplay",
      "characterSave",
      "personaSave",
      "devConversationSeed",
      "conversationSettingsGet",
      "conversationSettingsUpdate",
    ]);
    const request = seeds[0] as { target: { type: string }; lines: { speaker_character_id: string | null; minutes_ago: number }[]; relationship: unknown };
    expect(request.target.type).toBe("direct");
    expect(request.relationship).toBeNull();
    expect(request.lines).toHaveLength(14);
    expect(request.lines[0]?.speaker_character_id).toBeNull();
    expect(request.lines[1]?.speaker_character_id).toMatch(/^character-/);
    const ages = request.lines.map((line) => line.minutes_ago);
    expect(ages).toEqual([...ages].sort((a, b) => b - a));
  });

  it("creates the companion setup with a relationship and five memories", async () => {
    const order: string[] = [];
    const calls = backend(order);
    const seeds: unknown[] = [];
    const seed = calls.devConversationSeed;
    calls.devConversationSeed = (request) => {
      seeds.push(request);
      return seed!(request);
    };
    const runtime = await mount("/settings/developer", true, calls);
    fireEvent.click(await row("Companion chat, filled"));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/chat/chat-1"));
    expect(order.slice(0, 11)).toEqual([
      "characterDraftDefault:companion",
      "characterSave",
      "devConversationSeed",
      "conversationSettingsGet",
      "conversationSettingsUpdate",
      "memoryGet",
      "memoryAdd",
      "memoryAdd",
      "memoryAdd",
      "memoryAdd",
      "memoryAdd",
    ]);
    expect((seeds[0] as { relationship: unknown }).relationship).toEqual({ closeness: 0.45, trust: 0.6, affection: 0.35, tension: 0.15, stability: 0.7 });
  });

  it("saves the widget slots and their overrides on the character", async () => {
    const order: string[] = [];
    const saved: CharacterDraft[] = [];
    const calls = backend(order);
    const save = calls.characterSave;
    calls.characterSave = (request) => {
      saved.push(request.draft as unknown as CharacterDraft);
      return save!(request);
    };
    const runtime = await mount("/settings/developer", true, calls);
    fireEvent.click(await row("Companion chat with widgets"));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/chat/chat-1"));
    const presentation = saved[0]?.presentation;
    expect(presentation?.chat_appearance_overrides).toEqual(["chat_column_width", "chat_column_align", "chat_widget_area_enabled", "chat_widget_center_mode", "chat_widget_slots"]);
    expect(presentation?.chat_appearance.chat_widget_area_enabled).toBe(true);
    const types = [...(presentation?.chat_appearance.chat_widget_slots.left ?? []), ...(presentation?.chat_appearance.chat_widget_slots.right ?? [])].map((node) => node.type);
    expect(types).toEqual(expect.arrayContaining(["companion_state", "memory", "stat_tracker", "dice", "time", "image", "box"]));
  });

  it("toasts the error and keeps what was created when a step fails", async () => {
    const order: string[] = [];
    const calls = backend(order);
    calls.devConversationSeed = () => {
      order.push("devConversationSeed");
      return { status: "error", error: { code: "unsupported", message: "diagnostic", details: null } };
    };
    const runtime = await mount("/settings/developer", true, calls);
    fireEvent.click(await row("Roleplay chat"));
    await waitFor(() => expect(order).toContain("devConversationSeed"));
    await waitFor(async () => expect((await row("Roleplay chat")).hasAttribute("disabled")).toBe(false));
    expect(runtime.router.state.location.pathname).toBe("/settings/developer");
    expect(screen.queryByText("diagnostic")).toBeNull();
  });
});
