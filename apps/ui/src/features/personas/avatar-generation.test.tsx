import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AssetRef, AvatarGenerateRequest, AvatarGenerationOptions, JobView } from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers, type MockStreamHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({ createWindowController: vi.fn().mockResolvedValue(null) }));

const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const asset = (id: string): AssetRef => ({ asset_id: id, url: `asset://${id}`, preview_url: null });
const model = { id: "image-model", name: "Image model", provider_kind: "openai", supports_editing: true };
const usable: AvatarGenerationOptions = { enabled: true, model };
const job = (id: string): JobView => ({
  id, kind: "image_generate", subject: { kind: "image_request", id }, subject_detail: null, state: "succeeded",
  progress: { current: 0, total: null, unit: null, label_code: null, bytes_per_second: null }, created_at: 1, updated_at: 1, failure: null,
  result: { type: "image_generation", images: [{ asset: asset(id), mime_type: "image/png", width: 512, height: 512, text: null }], rejected_outputs: 0 },
});

async function renderCreate(calls: MockCallHandlers = {}, streams: MockStreamHandlers = {}, options: AvatarGenerationOptions = usable) {
  const history = createMemoryHistory({ initialEntries: ["/", "/personas/new"], initialIndex: 1 });
  const runtime = await createAppRuntime({
    transport: createMockTransport({
      calls: {
        appStatus: () => ok({ ...mockAppStatus, ui_state: { onboarding: { completed: true } } }),
        personasList: () => ok({ items: [], next_cursor: null, default_persona: { persona_id: null, revision: 1, created_at: 1, updated_at: 1 } }),
        avatarGenerationOptions: () => ok(options),
        avatarGenerate: (request: AvatarGenerateRequest) => ok({ job_id: request.request_id, model, prompt: "rendered" }),
        jobCancel: () => ok(null),
        ...calls,
      },
      streams: { jobWatch: ([{ job_id }]) => ok(job(job_id)), ...streams },
    }),
    history,
    i18n: await createTestI18n("en"),
  });
  await act(async () => { render(<App runtime={runtime} />); });
  return runtime;
}

const open = async () => {
  fireEvent.click(await screen.findByRole("button", { name: "Generate avatar" }));
  return await screen.findByRole("dialog");
};

afterEach(() => cleanup());

describe("avatar generation panel", () => {
  it("generates a preview and applies it to the draft only when Use is chosen", async () => {
    const save = vi.fn();
    const runtime = await renderCreate({ personaSave: save });
    fireEvent.change(await screen.findByRole("textbox", { name: "Name" }), { target: { value: "Wren" } });
    const dialog = await open();
    expect(runtime.router.state.location.search).toMatchObject({ avatarPanel: true });
    const use = within(dialog).getByRole("button", { name: "Use this avatar" }) as HTMLButtonElement;
    expect(use.disabled).toBe(true);
    await waitFor(() => expect((within(dialog).getByRole("button", { name: "Generate" }) as HTMLButtonElement).disabled).toBe(true));
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Describe the avatar" }), { target: { value: "a fox" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Generate" }));
    await waitFor(() => expect((within(dialog).getByRole("button", { name: "Use this avatar" }) as HTMLButtonElement).disabled).toBe(false));
    expect(within(dialog).getByRole("textbox", { name: "Describe the avatar" })).toHaveProperty("value", "a fox");
    expect(within(dialog).getAllByRole("img").some((image) => image.getAttribute("src")?.startsWith("asset://"))).toBe(true);
    fireEvent.click(within(dialog).getByRole("button", { name: "Use this avatar" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(runtime.router.state.location.pathname).toBe("/personas/new");
    expect(runtime.router.state.location.search).not.toHaveProperty("avatarPanel");
    expect(document.querySelector('img[src^="asset://"]')).not.toBeNull();
    expect(screen.getByRole("button", { name: "Remove" })).toBeTruthy();
    expect(save).not.toHaveBeenCalled();
  });

  it("discards the preview when the panel is closed without Use, and back closes it before the form", async () => {
    const runtime = await renderCreate();
    fireEvent.change(await screen.findByRole("textbox", { name: "Name" }), { target: { value: "Wren" } });
    const dialog = await open();
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Describe the avatar" }), { target: { value: "a fox" } });
    await waitFor(() => expect((within(dialog).getByRole("button", { name: "Generate" }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(within(dialog).getByRole("button", { name: "Generate" }));
    await waitFor(() => expect((within(dialog).getByRole("button", { name: "Use this avatar" }) as HTMLButtonElement).disabled).toBe(false));
    await act(async () => { runtime.router.history.back(); });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(runtime.router.state.location.pathname).toBe("/personas/new");
    expect(document.querySelector('img[src^="asset://"]')).toBeNull();
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveProperty("value", "Wren");
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Generate avatar" }));
  });

  it("keeps upload available and points to image settings when no model can make avatars", async () => {
    const runtime = await renderCreate({}, {}, { enabled: true, model: null });
    fireEvent.change(await screen.findByRole("textbox", { name: "Name" }), { target: { value: "Wren" } });
    const dialog = await open();
    await within(dialog).findByText("No avatar model chosen");
    expect((within(dialog).getByRole("button", { name: "Upload image" }) as HTMLButtonElement).disabled).toBe(false);
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Describe the avatar" }), { target: { value: "a fox" } });
    expect((within(dialog).getByRole("button", { name: "Generate" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(within(dialog).getByRole("button", { name: "Open image settings" }));
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/settings/imageGeneration"));
    expect(screen.queryByRole("dialog", { name: "Discard changes?" })).toBeNull();
    await act(async () => { runtime.router.history.back(); });
    await waitFor(() => expect(runtime.router.state.location.pathname).toBe("/personas/new"));
    expect(await screen.findByRole("textbox", { name: "Name" })).toHaveProperty("value", "Wren");
    await screen.findByRole("dialog");
  });

  it("cancels a running generation and keeps the previous avatar", async () => {
    const cancel = vi.fn(() => ok(null));
    const release: { current: () => void } = { current: () => undefined };
    const gate = new Promise<void>((resolve) => { release.current = resolve; });
    await renderCreate({ jobCancel: cancel }, { jobWatch: async ([{ job_id }]) => { await gate; return ok({ ...job(job_id), state: "cancelled" as const, result: null }); } });
    fireEvent.change(await screen.findByRole("textbox", { name: "Name" }), { target: { value: "Wren" } });
    const dialog = await open();
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Describe the avatar" }), { target: { value: "a fox" } });
    await waitFor(() => expect((within(dialog).getByRole("button", { name: "Generate" }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(within(dialog).getByRole("button", { name: "Generate" }));
    fireEvent.click(await within(dialog).findByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(cancel).toHaveBeenCalledTimes(1));
    release.current();
    await within(dialog).findByText("Generation cancelled");
    expect((within(dialog).getByRole("button", { name: "Use this avatar" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("edits the selected older version, not the newest one", async () => {
    const requests: AvatarGenerateRequest[] = [];
    await renderCreate({ avatarGenerate: (request: AvatarGenerateRequest) => { requests.push(request); return ok({ job_id: request.request_id, model, prompt: `rendered ${requests.length}` }); } });
    fireEvent.change(await screen.findByRole("textbox", { name: "Name" }), { target: { value: "Wren" } });
    const dialog = await open();
    const prompt = () => within(dialog).getByRole("textbox", { name: "Describe the avatar" });
    const generate = within(dialog).getByRole("button", { name: "Generate" }) as HTMLButtonElement;
    await waitFor(() => expect(generate.disabled).toBe(true));
    fireEvent.change(prompt(), { target: { value: "a fox" } });
    await waitFor(() => expect((within(dialog).getByRole("button", { name: "Generate" }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(within(dialog).getByRole("button", { name: "Generate" }));
    await within(dialog).findByRole("button", { name: "Generated image 1" });
    await waitFor(() => expect((within(dialog).getByRole("button", { name: "Generate" }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(within(dialog).getByRole("button", { name: "Generate" }));
    await within(dialog).findByRole("button", { name: "Generated image 2" });
    await waitFor(() => expect((within(dialog).getByRole("button", { name: "Generate" }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(within(dialog).getByRole("button", { name: "Generated image 1" }));
    fireEvent.click(within(dialog).getByRole("radio", { name: "Edit selected" }));
    fireEvent.change(within(dialog).getByRole("textbox", { name: "What to change" }), { target: { value: "blue scarf" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Apply edit" }));
    await waitFor(() => expect(requests).toHaveLength(3));
    expect(requests[2]).toMatchObject({ source_asset_id: requests[0]!.request_id, kind: { type: "edit", edit_request: "blue scarf", current_avatar_prompt: "rendered 1" } });
    fireEvent.click(within(dialog).getByRole("radio", { name: "New image" }));
    expect(prompt()).toHaveProperty("value", "a fox");
  });

  it("reattaches after the watch disconnects without admitting another request", async () => {
    let watches = 0;
    const generate = vi.fn((request: AvatarGenerateRequest) => ok({ job_id: request.request_id, model, prompt: "rendered" }));
    await renderCreate({ avatarGenerate: generate }, { jobWatch: ([{ job_id }]) => { if (++watches === 1) throw new Error("stream disconnected"); return ok(job(job_id)); } });
    fireEvent.change(await screen.findByRole("textbox", { name: "Name" }), { target: { value: "Wren" } });
    const dialog = await open();
    await waitFor(() => expect((within(dialog).getByRole("button", { name: "Generate" }) as HTMLButtonElement).disabled).toBe(true));
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Describe the avatar" }), { target: { value: "a fox" } });
    await waitFor(() => expect((within(dialog).getByRole("button", { name: "Generate" }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(within(dialog).getByRole("button", { name: "Generate" }));
    await within(dialog).findByText("Waiting for the result");
    expect(within(dialog).getByRole("textbox", { name: "Describe the avatar" })).toHaveProperty("value", "a fox");
    fireEvent.click(within(dialog).getByRole("button", { name: "Try again" }));
    await waitFor(() => expect((within(dialog).getByRole("button", { name: "Use this avatar" }) as HTMLButtonElement).disabled).toBe(false));
    expect(generate).toHaveBeenCalledTimes(1);
    expect(watches).toBe(2);
  });

  it("allows closing after a watch disconnect and cancels the known job", async () => {
    const cancel = vi.fn(() => ok(null));
    await renderCreate({ jobCancel: cancel }, { jobWatch: () => { throw new Error("disconnected"); } });
    const dialog = await open();
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Describe the avatar" }), { target: { value: "a fox" } });
    await waitFor(() => expect((within(dialog).getByRole("button", { name: "Generate" }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(within(dialog).getByRole("button", { name: "Generate" }));
    await within(dialog).findByText("Waiting for the result");
    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(cancel).toHaveBeenCalledTimes(1));
  });

  it("lets the user retry a failed cancellation while the job is still running", async () => {
    const release: { current: () => void } = { current: () => undefined };
    const gate = new Promise<void>((resolve) => { release.current = resolve; });
    let cancellations = 0;
    const cancel = vi.fn(() => {
      if (++cancellations === 1) throw new Error("cancel disconnected");
      release.current();
      return ok(null);
    });
    await renderCreate({ jobCancel: cancel }, { jobWatch: async ([{ job_id }]) => { await gate; return ok({ ...job(job_id), state: "cancelled" as const, result: null }); } });
    const dialog = await open();
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Describe the avatar" }), { target: { value: "a fox" } });
    await waitFor(() => expect((within(dialog).getByRole("button", { name: "Generate" }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(within(dialog).getByRole("button", { name: "Generate" }));
    fireEvent.click(await within(dialog).findByRole("button", { name: "Cancel" }));
    await within(dialog).findByText("Could not cancel");
    const retry = within(dialog).getByRole("button", { name: "Try again" });
    expect((retry as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(retry);
    await waitFor(() => expect(cancel).toHaveBeenCalledTimes(2));
    await within(dialog).findByText("Generation cancelled");
  });
});
