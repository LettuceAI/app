import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { toast } from "@lettuceai/crisp";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CharacterSummary, CreatorEditRequest, CreatorRegenerateRequest, CreatorSendRequest, CreatorRevisionRequest, CreatorStartRequest, CreatorView, SettingsView_Serialize } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));
beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(600);
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(1000);
});
afterEach(() => { toast.dismiss(); cleanup(); vi.restoreAllMocks(); });
const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const failure = () => ({ status: "error" as const, error: { code: "unavailable" as const, message: "private diagnostic", details: null } });
const initial = (): CreatorView => {
  const draft = { name: "Mira", definition: "A lighthouse keeper.", scenes: [], avatar: null };
  const proposal = { id: "first", parent_id: null, ordinal: 0, source: "start" as const, draft, created_at: 1 };
  return {
    summary: { id: "work", kind: "character", name: "Mira", target_id: null, applied_id: null, updated_at: 1 },
    revision: 1, proposal, history: [proposal], turns: [], active_job_id: null, reviewed: false, target_changed: false, model_profile_id: null, images: [],
  };
};

async function setup(overrides: MockCallHandlers = {}, phone = false, path = "/creator/work") {
  let work = initial();
  const edits: CreatorEditRequest[] = [];
  const reviews: CreatorRevisionRequest[] = [];
  const applies: CreatorRevisionRequest[] = [];
  const transport = createMockTransport({ calls: {
    appStatus: () => ok({ ...mockAppStatus, platform: phone ? "android" : "other", ui_state: { onboarding: { completed: true } } }),
    settingsGet: () => ok({ revision: 1, default_model_profile_id: null, global: { dynamic_memory: { enabled: false }, creation_helper: { model_profile_id: null, streaming: true, enabled_tools: null, tool_fallback: "native" } } } as unknown as SettingsView_Serialize),
    modelsList: () => ok({ models: [], default_model_id: null, revision: 1 }),
    libraryCounts: () => ok({ characters: 0, personas: 0, lorebooks: 0, images: 0, audio: 0 }),
    charactersList: () => ok({ items: [], next_cursor: null }),
    personasList: () => ok({ items: [], next_cursor: null, default_persona: { persona_id: null, revision: 1, created_at: 1, updated_at: 1 } }),
    conversationsList: () => ok({ items: [], next_cursor: null }),
    conversationsLatestByCharacter: () => ok({ items: [], next_cursor: null }),
    creatorsList: () => ok({ items: [work.summary], next_cursor: null }),
    creatorGet: () => ok(structuredClone(work)),
    creatorEdit: (request) => {
      edits.push(request);
      const proposal = { id: `edit-${work.revision + 1}`, parent_id: work.proposal.id, ordinal: work.proposal.ordinal + 1, source: "edit" as const, draft: request.draft, created_at: 2 };
      work = { ...work, revision: work.revision + 1, proposal, history: [...work.history, proposal], reviewed: false };
      return ok(work);
    },
    creatorReview: (request) => {
      reviews.push(request);
      work = { ...work, revision: work.revision + 1, reviewed: true, proposal: { ...work.proposal, id: "reviewed", parent_id: work.proposal.id, source: "review" } };
      return ok(work);
    },
    creatorApply: (request) => {
      applies.push(request);
      work = { ...work, summary: { ...work.summary, applied_id: "saved-character" } };
      return ok(work);
    },
    ...overrides,
  } });
  const history = createMemoryHistory({ initialEntries: [path] });
  const runtime = await createAppRuntime({ transport, history, i18n: await createTestI18n("en") });
  await act(async () => { render(<App runtime={runtime} />); });
  await screen.findByDisplayValue("Mira");
  return { runtime, history, edits, reviews, applies };
}

describe("Smart creator", () => {
  it("saves hand edits before review and keeps them when saving fails", async () => {
    let attempts = 0;
    const app = await setup({ creatorEdit: () => { attempts += 1; return failure(); } });
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Mira Finch" } });
    fireEvent.click(screen.getByRole("button", { name: "Review and save" }));
    await waitFor(() => expect(attempts).toBe(1));
    expect(screen.queryByRole("dialog", { name: "Review and save" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Save draft changes" }));
    await waitFor(() => expect(attempts).toBe(2));
    await waitFor(() => expect((screen.getByRole("button", { name: "Save draft changes" }) as HTMLButtonElement).disabled).toBe(false));
    expect((screen.getByLabelText("Name") as HTMLInputElement).value).toBe("Mira Finch");
    expect(app.applies).toHaveLength(0);
    expect(screen.queryByText("private diagnostic")).toBeNull();
  });

  it("reviews explicitly and applies exactly the reviewed proposal", async () => {
    const app = await setup();
    fireEvent.click(screen.getByRole("button", { name: "Review and save" }));
    const sheet = await screen.findByRole("dialog", { name: "Review and save" });
    expect(app.reviews).toHaveLength(0);
    expect(app.applies).toHaveLength(0);
    fireEvent.click(within(sheet).getByRole("button", { name: "Create" }));
    await waitFor(() => expect(app.applies).toHaveLength(1));
    expect(app.reviews[0]).toMatchObject({ expected_revision: 1, proposal_id: "first" });
    expect(app.applies[0]).toMatchObject({ expected_revision: 2, proposal_id: "reviewed" });
    await screen.findByText("This version is saved. Open the saved item to continue editing.");
  });

  it("shows each tool call of a reply with its status", async () => {
    const work = initial();
    const turn = {
      id: "turn", message: "Make her older", reply: "Done.", reasoning: "", status: "succeeded" as const, job_id: "job", base_proposal_id: "first",
      result_proposal_id: null, regenerated: false, regenerable: true, references: [], images: [],
      tools: [{ name: "write_definition", status: "succeeded" as const, image: null }, { name: "set_name", status: "failed" as const, image: null }],
      steps: [{ kind: "text" as const, text: "Done." }, { kind: "tool" as const, index: 0 }, { kind: "tool" as const, index: 1 }],
    };
    await setup({ creatorGet: () => ok({ ...work, turns: [turn] }) });
    const steps = await screen.findByRole("list", { name: "Draft changes" });
    expect(within(steps).getByText("Update the definition")).toBeTruthy();
    expect(within(steps).getByLabelText("Done")).toBeTruthy();
    expect(within(steps).getByText("Set the name")).toBeTruthy();
    expect(within(steps).getByLabelText("Failed")).toBeTruthy();
  });

  it("keeps phone draft edits while switching views and renders scene fields", async () => {
    await setup({}, true);
    fireEvent.click(screen.getByRole("tab", { name: "Draft" }));
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Mira Finch" } });
    fireEvent.click(screen.getByRole("button", { name: "Write the first scene" }));
    expect(screen.getByLabelText("Opening message")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Opening message"), { target: { value: "The lamp flickers." } });
    fireEvent.click(screen.getByRole("tab", { name: "Chat" }));
    fireEvent.click(screen.getByRole("tab", { name: "Draft" }));
    expect((screen.getByLabelText("Name") as HTMLInputElement).value).toBe("Mira Finch");
    expect((screen.getByLabelText("Opening message") as HTMLTextAreaElement).value).toBe("The lamp flickers.");
  });

  it("retries a failed reply by regenerating it instead of sending the message again", async () => {
    const work = initial();
    const turn = {
      id: "turn", message: "Make her older", reply: "", reasoning: "", status: "failed" as const, job_id: "job", base_proposal_id: "first",
      result_proposal_id: null, regenerated: false, regenerable: true, references: [], images: [], tools: [], steps: [],
    };
    const regenerations: CreatorRegenerateRequest[] = [];
    const sends: unknown[] = [];
    await setup({
      creatorGet: () => ok({ ...work, turns: [turn] }),
      creatorRegenerate: (request) => { regenerations.push(request); return ok({ job_id: "retry" }); },
      creatorSend: (request) => { sends.push(request); return ok({ job_id: "send" }); },
    });
    fireEvent.click(await screen.findByRole("button", { name: "Try again" }));
    await waitFor(() => expect(regenerations).toHaveLength(1));
    expect(regenerations[0]).toMatchObject({ turn_id: "turn", expected_revision: 1 });
    expect(sends).toHaveLength(0);
  });

  it("carries the draft over to the latest version when the saved item changed", async () => {
    const work = { ...initial(), target_changed: true, summary: { ...initial().summary, target_id: "character" } };
    const starts: CreatorStartRequest[] = [];
    const app = await setup({
      creatorGet: () => ok(work),
      creatorStart: (request) => { starts.push(request); return ok({ ...work, target_changed: false, summary: { ...work.summary, id: "carried" } }); },
    });
    expect((screen.getByRole("button", { name: "Review and save" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Continue on the latest version" }));
    await waitFor(() => expect(starts).toHaveLength(1));
    expect(starts[0]).toMatchObject({ kind: "character", target_id: "character", draft: work.proposal.draft });
    await waitFor(() => expect(app.history.location.pathname).toBe("/creator/carried"));
  });

  it("labels versions by where they came from and leaves review copies out", async () => {
    const work = initial();
    const edit = { id: "edit", parent_id: "first", ordinal: 1, source: "edit" as const, draft: { ...work.proposal.draft, definition: "An old lighthouse keeper." }, created_at: 2 };
    const review = { ...edit, id: "review", parent_id: "edit", ordinal: 2, source: "review" as const };
    await setup({ creatorGet: () => ok({ ...work, proposal: review, history: [work.proposal, edit, review], reviewed: true }) }, false, "/creator/work?panel=history");
    const sheet = await screen.findByRole("dialog", { name: "Draft history" });
    expect(within(sheet).getByText("Version 2")).toBeTruthy();
    expect(within(sheet).queryByText("Version 3")).toBeNull();
    expect(within(sheet).getByText(/Your edit/)).toBeTruthy();
    expect(within(sheet).getByText("Current")).toBeTruthy();
  });

  it("sends the attached characters with the message and shows them on the turn", async () => {
    const cassian: CharacterSummary = {
      id: "c1", name: "Cassian", description: "King of Welan", avatar: null, banner: null,
      card_style: "circle", gradient: { type: "none" }, revision: 7, created_at: 1, updated_at: 1,
    };
    const sends: CreatorSendRequest[] = [];
    await setup({
      charactersList: () => ok({ items: [cassian], next_cursor: null }),
      settingsGet: () => ok({ revision: 1, default_model_profile_id: "model", global: { dynamic_memory: { enabled: false }, creation_helper: { model_profile_id: null, streaming: true, enabled_tools: null, tool_fallback: "native" } } } as unknown as SettingsView_Serialize),
      creatorSend: (request) => { sends.push(request); return ok({ job_id: "job" }); },
    });
    fireEvent.click(screen.getAllByRole("button", { name: /attach/i })[0]!);
    fireEvent.click(await screen.findByRole("button", { name: /^Character/ }));
    const picker = await screen.findByRole("dialog", { name: "Attach characters" });
    fireEvent.click(await within(picker).findByRole("button", { name: /Cassian/ }));
    fireEvent.click(within(picker).getByRole("button", { name: "Done" }));
    fireEvent.change(screen.getByPlaceholderText("Describe what you want to create or change"), { target: { value: "Make his brother" } });
    fireEvent.keyDown(screen.getByPlaceholderText("Describe what you want to create or change"), { key: "Enter" });
    await waitFor(() => expect(sends).toHaveLength(1));
    expect(sends[0]).toMatchObject({ message: "Make his brother", references: [{ kind: "character", id: "c1" }], images: [] });
  });

  it("shows a generated image and attaches it to the next message for an edit", async () => {
    const work = initial();
    const asset = { asset_id: "image", url: "asset://image", preview_url: null };
    const turn = {
      id: "turn", message: "Make a portrait", reply: "Here she is.", reasoning: "", status: "succeeded" as const, job_id: "job", base_proposal_id: "first",
      result_proposal_id: null, regenerated: false, regenerable: true, references: [], images: [],
      tools: [{ name: "generate_image", status: "succeeded" as const, image: { asset, job_id: "image-job", failed: false } }],
      steps: [{ kind: "text" as const, text: "Here she is." }, { kind: "tool" as const, index: 0 }],
    };
    const sends: CreatorSendRequest[] = [];
    await setup({
      creatorGet: () => ok({ ...work, turns: [turn], images: [{ asset, origin: "generated" as const, turn_id: "turn" }] }),
      settingsGet: () => ok({ revision: 1, default_model_profile_id: "model", global: { dynamic_memory: { enabled: false }, creation_helper: { model_profile_id: null, streaming: true, enabled_tools: null, tool_fallback: "native" } } } as unknown as SettingsView_Serialize),
      creatorSend: (request) => { sends.push(request); return ok({ job_id: "job-2" }); },
    });
    const steps = await screen.findByRole("list", { name: "Draft changes" });
    expect(within(steps).getByText("Generate an image")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Use as avatar" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    const input = screen.getByPlaceholderText("Describe what you want to create or change");
    fireEvent.change(input, { target: { value: "Give her a red scarf" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(sends).toHaveLength(1));
    expect(sends[0]).toMatchObject({ message: "Give her a red scarf", images: ["image"] });
  });

  it("saves the draft from the scene drawer", async () => {
    const app = await setup();
    fireEvent.click(screen.getByRole("button", { name: "Write the first scene" }));
    const drawer = await screen.findByRole("dialog", { name: "Scene 1" });
    fireEvent.change(within(drawer).getByLabelText("Opening message"), { target: { value: "The lamp flickers." } });
    fireEvent.click(within(drawer).getByRole("button", { name: "Save draft changes" }));
    await waitFor(() => expect(app.edits).toHaveLength(1));
    expect(app.edits[0]!.draft.scenes[0]).toMatchObject({ content: "The lamp flickers." });
  });

  it("closes the avatar crop when it is cancelled from a generated image", async () => {
    const work = initial();
    const asset = { asset_id: "image", url: "asset://image", preview_url: null };
    const turn = {
      id: "turn", message: "Make a portrait", reply: "Here she is.", reasoning: "", status: "succeeded" as const, job_id: "job", base_proposal_id: "first",
      result_proposal_id: null, regenerated: false, regenerable: true, references: [], images: [],
      tools: [{ name: "generate_image", status: "succeeded" as const, image: { asset, job_id: "image-job", failed: false } }],
      steps: [{ kind: "tool" as const, index: 0 }],
    };
    const app = await setup({ creatorGet: () => ok({ ...work, turns: [turn], images: [{ asset, origin: "generated" as const, turn_id: "turn" }] }) });
    fireEvent.click(await screen.findByRole("button", { name: "Use as avatar" }));
    await waitFor(() => expect(app.history.location.search).toContain("avatarCrop"));
    fireEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(app.history.location.search).not.toContain("avatarCrop"));
  });
});
