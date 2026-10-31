import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { I18nextProvider } from "react-i18next";
import { createMemoryHistory, createRootRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApiClient } from "@/api/client";
import { createMockTransport } from "@/api/mock-transport";
import { createTestI18n } from "@/shared/testing/i18n";
import { ChatComposer, type ChatComposerProps, type ComposerHandle } from "./ChatComposer";

const preferenceKey = "lettuce.chat.systemSendConfirmationDisabled";

async function mount(patch: Partial<ChatComposerProps> = {}) {
  const onSend = vi.fn().mockResolvedValue(true);
  const onSendSystem = vi.fn().mockResolvedValue(true);
  const onContinue = vi.fn();
  const handle = { current: null as ComposerHandle | null };
  const i18n = await createTestI18n();
  const router = createRouter({ history: createMemoryHistory({ initialEntries: ["/"] }), routeTree: createRootRoute({ component: () => (
    <ChatComposer api={createApiClient(createMockTransport())} conversationId="c" handle={handle} name="Cassian" sending={false} disabled={false} canContinue onSend={onSend} onSendSystem={onSendSystem} onContinue={onContinue} onStop={() => undefined} onAttach={() => undefined} {...patch} />
  ) }) });
  render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={new QueryClient()}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </I18nextProvider>,
  );
  return { onSend, onSendSystem, onContinue, handle, box: await screen.findByRole("textbox") as HTMLTextAreaElement };
}

function enter(box: HTMLTextAreaElement, value = "The sun has set.") {
  fireEvent.change(box, { target: { value } });
  return screen.getByRole("button", { name: "Send message" });
}

afterEach(() => {
  vi.useRealTimers();
  cleanup();
  localStorage.removeItem(preferenceKey);
});

describe("send as system", () => {
  it("asks on the first hold and suppresses the release click", async () => {
    const { box, onSend, onSendSystem } = await mount();
    const button = enter(box);
    vi.useFakeTimers();
    fireEvent(button, new MouseEvent("pointerdown", { bubbles: true, button: 0 }));
    await act(async () => vi.advanceTimersByTime(450));
    fireEvent.pointerUp(button);
    fireEvent.click(button);
    vi.useRealTimers();
    const menu = await screen.findByRole("dialog", { name: "Send as system message?" });
    expect(menu.textContent).toContain("It does not trigger generation.");
    expect(onSend).not.toHaveBeenCalled();
    expect(onSendSystem).not.toHaveBeenCalled();
    fireEvent.click(within(menu).getByRole("button", { name: "Send" }));
    await waitFor(() => expect(onSendSystem).toHaveBeenCalledWith("The sun has set.", []));
    expect(onSend).not.toHaveBeenCalled();
    expect(localStorage.getItem(preferenceKey)).toBe("1");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(box.value).toBe("");
  });

  it("cancels without sending or remembering an acknowledgement", async () => {
    const { box, onSendSystem } = await mount();
    fireEvent.contextMenu(enter(box));
    fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(box.value).toBe("The sun has set.");
    expect(onSendSystem).not.toHaveBeenCalled();
    expect(localStorage.getItem(preferenceKey)).toBeNull();
    fireEvent.contextMenu(screen.getByRole("button", { name: "Send message" }));
    expect(await screen.findByRole("dialog")).toBeTruthy();
  });

  it("uses the legacy acknowledgement and restores text and attachments on failure", async () => {
    localStorage.setItem(preferenceKey, "1");
    const sendSystem = vi.fn().mockResolvedValue(false);
    const { box, handle, onSend } = await mount({ onSendSystem: sendSystem });
    act(() => handle.current?.attach({ id: "image", src: "asset://image", name: "Map" }));
    fireEvent.contextMenu(enter(box));
    await waitFor(() => expect(sendSystem).toHaveBeenCalledWith("The sun has set.", [{ id: "image", src: "asset://image", name: "Map" }]));
    await waitFor(() => expect(box.value).toBe("The sun has set."));
    expect(screen.getByRole("img", { name: "Map" })).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onSend).not.toHaveBeenCalled();
  });

  it("allows an attachment-only system message", async () => {
    localStorage.setItem(preferenceKey, "1");
    const { handle, onSendSystem } = await mount();
    act(() => handle.current?.attach({ id: "audio", name: "Voice" }));
    fireEvent.contextMenu(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(onSendSystem).toHaveBeenCalledWith("", [{ id: "audio", name: "Voice" }]));
  });

  it("keeps continue mode free of system send", async () => {
    const { onContinue, onSendSystem } = await mount();
    const button = screen.getByRole("button", { name: "Let the character continue" });
    fireEvent.contextMenu(button);
    expect(screen.queryByRole("dialog")).toBeNull();
    fireEvent.click(button);
    expect(onContinue).toHaveBeenCalledOnce();
    expect(onSendSystem).not.toHaveBeenCalled();
  });

  it("keeps a normal click as a regular send", async () => {
    const { box, onSend, onSendSystem } = await mount();
    fireEvent.click(enter(box, "Hello"));
    await waitFor(() => expect(onSend).toHaveBeenCalledWith("Hello", []));
    expect(onSendSystem).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("blocks duplicate system sends while the append is pending", async () => {
    localStorage.setItem(preferenceKey, "1");
    let finish!: (accepted: boolean) => void;
    const pending = vi.fn(() => new Promise<boolean>((resolve) => { finish = resolve; }));
    const { box, handle, onSend } = await mount({ onSendSystem: pending });
    const button = enter(box);
    fireEvent.contextMenu(button);
    act(() => handle.current?.replace("Another note"));
    fireEvent.contextMenu(button);
    fireEvent.click(button);
    expect(pending).toHaveBeenCalledOnce();
    expect(onSend).not.toHaveBeenCalled();
    await act(async () => finish(true));
  });

  it("does not open when disabled", async () => {
    const { box, onSendSystem } = await mount({ disabled: true });
    fireEvent.change(box, { target: { value: "Note" } });
    fireEvent.contextMenu(screen.getByRole("button", { name: "Send message" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onSendSystem).not.toHaveBeenCalled();
  });
});
