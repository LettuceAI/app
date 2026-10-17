import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createBrowserHistory, createMemoryHistory, type RouterHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PersonaSaveRequest_Deserialize, PersonaView_Serialize } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

const defaultRecord = { persona_id: null, revision: 7, created_at: 1, updated_at: 1 };

function saved(request: PersonaSaveRequest_Deserialize): PersonaView_Serialize {
  return { id: "p-new", archived: false, draft: request.draft, assets: [], revision: 1, created_at: 1, updated_at: 1 };
}

async function renderCreate(calls: MockCallHandlers = {}, browser = false, entries = ["/", "/personas/new"]) {
  let history: RouterHistory;
  if (browser) {
    window.history.replaceState(null, "", "/");
    history = createBrowserHistory();
  } else {
    history = createMemoryHistory({ initialEntries: entries });
  }
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: () => ({ status: "ok", data: { ...mockAppStatus, ui_state: { onboarding: { completed: true } } } }),
        charactersList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        conversationsList: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        conversationsLatestByCharacter: () => ({ status: "ok", data: { items: [], next_cursor: null } }),
        personasList: () => ({ status: "ok", data: { items: [], next_cursor: null, default_persona: defaultRecord } }),
        libraryCounts: () => ({ status: "ok", data: { characters: 0, personas: 1, lorebooks: 1, images: 0, audio: 0 } }),
        lorebooksList: () => ({
          status: "ok",
          data: { items: [{ id: "l1", name: "Harbor lore", status: "active", detection: "recent_messages", icon: null, revision: 1, created_at: 1, updated_at: 1 }], next_cursor: null },
        }),
        ...calls,
      },
    }),
    history,
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  if (browser) {
    await act(async () => {
      history.push("/personas/new");
    });
  }
  return runtime;
}

const primary = () => screen.getByRole("button", { name: /^(Continue to |Create persona$)/ }) as HTMLButtonElement;

afterEach(() => {
  cleanup();
});

describe("persona creation", () => {
  it("needs a name and a description before moving on", async () => {
    await renderCreate();
    const name = await screen.findByRole("textbox", { name: "Name" });
    expect(primary().textContent).toContain("Continue to extras");
    expect(primary().disabled).toBe(true);
    fireEvent.change(name, { target: { value: "Wren" } });
    expect(primary().disabled).toBe(true);
    fireEvent.change(screen.getByRole("textbox", { name: "Description" }), { target: { value: "A scribe" } });
    expect(primary().disabled).toBe(false);
    fireEvent.change(name, { target: { value: "   " } });
    expect(primary().disabled).toBe(true);
  });

  it("saves the persona, attaches lorebooks, sets the default and returns to the library", async () => {
    const save = vi.fn((request: PersonaSaveRequest_Deserialize) => ({ status: "ok" as const, data: saved(request) }));
    const bind = vi.fn(() => ({ status: "ok" as const, data: { bindings: [], owner_revision: 2 } }));
    const setDefault = vi.fn(() => ({ status: "ok" as const, data: { ...defaultRecord, persona_id: "p-new", revision: 8 } }));
    const runtime = await renderCreate({
      personaSave: save,
      personaLorebooksBind: bind,
      personaDefaultSet: setDefault,
      filesPickOpen: () => ({ status: "ok", data: { sources: [{ uri: "/tmp/ref.png" }] } }),
      assetsIngest: () => ({ status: "ok", data: { asset_id: "ref", url: "https://example.test/ref.png" } }),
    });
    fireEvent.change(await screen.findByRole("textbox", { name: "Name" }), { target: { value: "  Wren  " } });
    fireEvent.change(screen.getByRole("textbox", { name: "Nickname" }), { target: { value: "   " } });
    fireEvent.click(screen.getByRole("switch", { name: "Use as default" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Description" }), { target: { value: "A scribe " } });
    fireEvent.click(primary());
    fireEvent.change(await screen.findByRole("textbox", { name: "Design note" }), { target: { value: "Grey coat" } });
    fireEvent.click(screen.getByRole("button", { name: "Add image" }));
    await screen.findByRole("button", { name: "Remove image" });
    fireEvent.click(screen.getByRole("button", { name: /^Lorebooks/ }));
    const sheet = await screen.findByRole("dialog");
    fireEvent.click(within(sheet).getByRole("checkbox", { name: "Harbor lore" }));
    fireEvent.click(within(sheet).getByRole("button", { name: "Done" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(primary());
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0]![0].draft).toEqual(expect.objectContaining({
      title: "Wren",
      description: "A scribe",
      nickname: null,
      design_description: "Grey coat",
      media: [{ asset_id: "ref", slot: "design_reference", ordinal: 0 }],
    }));
    await waitFor(() => expect(bind).toHaveBeenCalledWith(expect.objectContaining({ persona_id: "p-new", lorebook_id: "l1", expected_revision: 1 })));
    await waitFor(() => expect(setDefault).toHaveBeenCalledWith(expect.objectContaining({ persona_id: "p-new", expected_revision: 7 })));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/library"));
  });

  it("refuses files that are not personas", async () => {
    const commit = vi.fn(() => ({ status: "ok" as const, data: { kind: "imported" as const, file_kind: "uec" as never, id: "x", warnings: [] } }));
    await renderCreate({
      filesPickOpen: () => ({ status: "ok", data: { sources: [{ uri: "/tmp/card.png" }] } }),
      importPreview: () => ({ status: "ok", data: { kind: "uec", name: "Card", digest: "d", content: { kind: "lorebook", name: "Lore" }, uec3: null, images: [], warnings: [] } as never }),
      importCommit: commit,
    });
    fireEvent.click(await screen.findByRole("button", { name: "Choose file" }));
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Name" })).toBeTruthy());
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(commit).not.toHaveBeenCalled();
  });

  it("opens from the Create menu", async () => {
    const runtime = await renderCreate({}, false, ["/"]);
    fireEvent.click(await screen.findByRole("button", { name: "Create" }));
    fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: /^Create a persona/ }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/personas/new"));
    expect(await screen.findByRole("textbox", { name: "Name" })).toBeTruthy();
  });

  it("asks before discarding entered work", async () => {
    const runtime = await renderCreate({}, true);
    fireEvent.change(await screen.findByRole("textbox", { name: "Name" }), { target: { value: "Wren" } });
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    const dialog = await screen.findByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "Keep editing" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(runtime.router.state.location.pathname).toBe("/personas/new");
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Discard" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/"));
  });
});
