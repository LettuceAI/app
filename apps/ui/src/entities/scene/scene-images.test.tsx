import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { LibrarySceneDraft_Serialize } from "@/api/generated/bindings";
import { SceneImages } from "./SceneImages";

const scene: LibrarySceneDraft_Serialize = {
  id: "s1", ordinal: 0, direction: null, selected_variant_id: null,
  content: { format_version: 1, parts: [{ kind: "text", text: "Before " }, { kind: "inline_asset", link_id: "l1" }, { kind: "text", text: " after " }, { kind: "inline_asset", link_id: "l1" }] },
  assets: [{ id: "l1", asset_id: "a1", slot: "inline", ordinal: 0 }],
};

describe("SceneImages", () => {
  afterEach(cleanup);

  it("shows an image used twice once and inserts its token again when tapped", () => {
    const onInsert = vi.fn();
    const onRemove = vi.fn();
    render(<SceneImages scene={scene} assets={[{ asset_id: "a1", url: "asset://a1" }]} disabled={false} label={(number) => `Image ${number}`} removeLabel={(number) => `Remove image ${number}`} onInsert={onInsert} onRemove={onRemove} />);
    expect(screen.getAllByRole("button", { name: /^Remove image/ })).toHaveLength(1);
    fireEvent.click(screen.getByRole("button", { name: "Image 1" }));
    expect(onInsert).toHaveBeenCalledWith("l1");
    fireEvent.click(screen.getByRole("button", { name: "Remove image 1" }));
    expect(onRemove).toHaveBeenCalledWith("l1");
  });
});
