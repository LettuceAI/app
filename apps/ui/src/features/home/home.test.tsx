import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CharacterSummary, ConversationSummary } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";
import { customGradientCss, textOnColors } from "./data/gradient";
import { exportFileName } from "./data/actions";
import { sortByActivity } from "./data/home-characters";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

function character(id: string, name: string, patch: Partial<CharacterSummary> = {}): CharacterSummary {
  return { id, name, avatar: null, banner: null, description: null, card_style: "circle", gradient: { type: "none" }, revision: 1, created_at: 1, updated_at: 1, ...patch };
}

function chat(updatedAt: number, archived = false): ConversationSummary {
  return { updated_at: updatedAt, archived } as ConversationSummary;
}

async function renderHome(characters: CharacterSummary[], latest: { source_id: string; conversation: ConversationSummary }[] = []) {
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: () => ({ status: "ok", data: { ...mockAppStatus, ui_state: { onboarding: { completed: true } } } }),
        charactersList: () => ({ status: "ok", data: { items: characters, next_cursor: null } }),
        conversationsLatestByCharacter: () => ({ status: "ok", data: { items: latest, next_cursor: null } }),
      },
    }),
    history: createMemoryHistory({ initialEntries: ["/"] }),
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
}

afterEach(() => {
  cleanup();
});

describe("home characters", () => {
  it("puts the most recently active character first: its newest chat, else its own last change", () => {
    const sorted = sortByActivity(
      [character("a", "Old chat", { updated_at: 50 }), character("b", "Edited", { updated_at: 40 }), character("c", "Recent chat", { updated_at: 1 })],
      new Map([["a", chat(10)], ["c", chat(90)]]),
    );
    expect(sorted.map((home) => home.character.name)).toEqual(["Recent chat", "Edited", "Old chat"]);
  });

  it("hides a character whose newest chat is archived", () => {
    const sorted = sortByActivity([character("a", "Hidden"), character("b", "Shown")], new Map([["a", chat(10, true)]]));
    expect(sorted.map((home) => home.character.name)).toEqual(["Shown"]);
  });

  it("builds custom gradients and picks readable text the way the previous app did", () => {
    expect(customGradientCss(["#111111"])).toBe("#111111");
    expect(customGradientCss(["#111111", "#222222"])).toBe("linear-gradient(135deg, #111111, #222222)");
    expect(customGradientCss(["#111111", "#222222", "#333333"])).toBe("linear-gradient(135deg, #111111, #333333, #222222)");
    expect(textOnColors(["#ffffff"]).fg).toBe("#1a1a1a");
    expect(textOnColors(["#000000"]).fg).toBe("#ffffff");
  });

  it("lists characters with their description, falls back when there is none, and filters by search", async () => {
    await renderHome([character("a", "Aria", { description: "A sea captain" }), character("b", "Bram")]);
    expect(await screen.findByText("Aria")).toBeTruthy();
    expect(screen.getByText("A sea captain")).toBeTruthy();
    expect(screen.getByText("No description yet")).toBeTruthy();
    fireEvent.change(screen.getAllByPlaceholderText("Search characters")[0]!, { target: { value: "captain" } });
    expect(await screen.findByText("1 of 2")).toBeTruthy();
    await waitFor(() => expect(screen.queryByText("Bram")).toBeNull());
    expect(screen.getByText("Aria")).toBeTruthy();
  });

  it("suggests an export file name the way the backend names exports", () => {
    expect(exportFileName("Sam O'Neil", "uec", new Date("2026-10-13T08:00:00Z"))).toBe("character_sam_o_neil_2026-10-13.uec");
    expect(exportFileName("Aria", "v3", new Date("2026-10-13T08:00:00Z"))).toBe("character_aria_2026-10-13.json");
  });

  it("says when there are no characters", async () => {
    await renderHome([]);
    expect(await screen.findByText("No characters yet")).toBeTruthy();
  });
});
