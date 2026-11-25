import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TimelineMessage } from "@/api/generated/bindings";
import { createTestI18n } from "@/shared/testing/i18n";
import { EditSheet } from "./EditSheet";

const media = (id: string, kind: "image" | "audio") => ({ type: "media" as const, asset: { asset_id: id, url: `asset://${id}` }, role: "attachment" as const, kind });

const message: TimelineMessage = {
  id: "u1", role: "user", author_participant_id: "pu", reasoning: null, created_at: 1_700_000_000_000, effective_time: 1_700_000_000_000, candidate_index: null, candidate_count: 1, pinned: false, scene_image: null,
  parts: [{ type: "text", text: "Look at these." }, media("first", "image"), media("second", "image"), media("voice", "audio")],
};

async function mount(onSave = vi.fn()) {
  const i18n = await createTestI18n();
  const sheet = (current: TimelineMessage | null) => (
    <I18nextProvider i18n={i18n}>
      <EditSheet message={current} characterName="Cassian" busy={false} onClose={() => undefined} onSave={onSave} />
    </I18nextProvider>
  );
  const { rerender } = render(sheet(null));
  rerender(sheet(message));
  return onSave;
}

describe("EditSheet", () => {
  afterEach(cleanup);

  it("removes an attached image and keeps the rest", async () => {
    const onSave = await mount();
    expect(await screen.findAllByRole("button", { name: "Remove Attached image" })).toHaveLength(2);
    fireEvent.click(screen.getAllByRole("button", { name: "Remove Attached image" })[0]!);
    expect(screen.getAllByRole("button", { name: "Remove Attached image" })).toHaveLength(1);
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(onSave).toHaveBeenCalledWith(message, "Look at these.", ["second", "voice"]);
  });

  it("keeps every attachment when nothing is removed", async () => {
    const onSave = await mount();
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    expect(onSave).toHaveBeenCalledWith(message, "Look at these.", ["first", "second", "voice"]);
  });
});
