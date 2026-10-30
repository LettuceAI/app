import { describe, expect, it } from "vitest";
import { sceneFromText } from "./form";
import { editedScene, sceneEditorText, sceneImageToken } from "./scene-editor";

describe("group starting scene content", () => {
  it("preserves inline image order, direction and background while editing text", () => {
    const scene = sceneFromText("Opening", "scene").scene;
    scene.direction = "Quietly";
    scene.assets = [{ id: "image", asset_id: "asset", slot: "inline", ordinal: 0 }, { id: "background", asset_id: "back", slot: "background", ordinal: 0 }];
    scene.content.parts.push({ kind: "inline_asset", link_id: "image" }, { kind: "text", text: "End" });
    const value = sceneEditorText(scene);
    const next = editedScene(scene, value.replace("Opening", 'Hello {{@"Aria"}}'));
    expect(next.content.parts).toEqual([{ kind: "text", text: 'Hello {{@"Aria"}}' }, { kind: "inline_asset", link_id: "image" }, { kind: "text", text: "End" }]);
    expect(next.assets).toEqual(scene.assets);
    expect(next.direction).toBe("Quietly");
  });

  it("removes only deleted image links and keeps unknown tokens as text", () => {
    const scene = sceneFromText("", "scene").scene;
    scene.assets = [{ id: "image", asset_id: "asset", slot: "inline", ordinal: 0 }];
    expect(editedScene(scene, "Text").assets).toEqual([]);
    expect(editedScene(scene, sceneImageToken("unknown")).content.parts).toEqual([{ kind: "text", text: sceneImageToken("unknown") }]);
  });

  it("keeps shared image links that a retained variant still uses", () => {
    const scene = sceneFromText("Opening", "scene").scene;
    scene.assets = [{ id: "image", asset_id: "asset", slot: "inline", ordinal: 0 }];
    const variant = { id: "variant", scene_id: scene.id, ordinal: 0, direction: "Alternative", content: { format_version: 1, parts: [{ kind: "inline_asset" as const, link_id: "image" }] } };
    expect(editedScene(scene, "Changed opening", [variant]).assets).toEqual(scene.assets);
  });
});
