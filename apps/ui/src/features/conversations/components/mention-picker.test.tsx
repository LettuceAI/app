import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApiClient } from "@/api/client";
import { createMockTransport } from "@/api/mock-transport";
import { createTestI18n } from "@/shared/testing/i18n";
import { ChatComposer } from "./ChatComposer";
import type { Speaker } from "./SpeakerSheet";

vi.mock("@/api/window-controls", () => ({
  createWindowController: vi.fn().mockResolvedValue(null),
}));

const members: Speaker[] = [
  { id: "p1", name: "Ruben Ochoa", avatar: null, characterId: "c1", description: "A tired mechanic", muted: false },
  { id: "p2", name: "Travis Maki", avatar: null, characterId: "c2", description: null, muted: true },
];

async function mount(onSend = vi.fn().mockResolvedValue(true)) {
  const i18n = await createTestI18n();
  const api = createApiClient(createMockTransport({ calls: {}, streams: {} }));
  render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={new QueryClient()}>
        <ChatComposer
          api={api}
          conversationId="c"
          handle={{ current: null }}
          name="Group"
          sending={false}
          disabled={false}
          canContinue={false}
          onSend={onSend}
          onContinue={() => undefined}
          onStop={() => undefined}
          onAttach={() => undefined}
          mentions={members}
        />
      </QueryClientProvider>
    </I18nextProvider>,
  );
  return { onSend, box: screen.getByRole("textbox") as HTMLTextAreaElement };
}

function type(box: HTMLTextAreaElement, value: string) {
  box.focus();
  fireEvent.change(box, { target: { value } });
  box.setSelectionRange(value.length, value.length);
  fireEvent.input(box);
}

afterEach(cleanup);

describe("mention picker", () => {
  it("opens on an at sign, filters while typing and highlights what matched", async () => {
    const { box } = await mount();
    type(box, "@");
    expect(await screen.findByRole("listbox", { name: "Mention a character" })).toBeTruthy();
    expect(screen.getAllByRole("option")).toHaveLength(2);
    type(box, "@tra");
    await waitFor(() => expect(screen.getAllByRole("option")).toHaveLength(1));
    expect(screen.getByRole("option").textContent).toContain("Travis Maki");
    expect(screen.getByRole("option").querySelector(".text-accent")?.textContent).toBe("Tra");
  });

  it("inserts the first match with Enter instead of sending", async () => {
    const { box, onSend } = await mount();
    type(box, "hi @ru");
    await screen.findByRole("listbox");
    fireEvent.keyDown(box, { key: "Enter" });
    await waitFor(() => expect(box.value).toBe('hi @"Ruben Ochoa" '));
    expect(onSend).not.toHaveBeenCalled();
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
  });

  it("inserts with a click and closes with Escape", async () => {
    const { box } = await mount();
    type(box, "@");
    fireEvent.click((await screen.findAllByRole("option"))[1]!);
    await waitFor(() => expect(box.value).toBe('@"Travis Maki" '));
    type(box, '@"Tra');
    await screen.findByRole("listbox");
    fireEvent.keyDown(box, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
  });
});
