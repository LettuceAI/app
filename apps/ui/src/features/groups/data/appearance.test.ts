import { describe, expect, it } from "vitest";
import type { CharacterDraft_Serialize } from "@/api/generated/bindings";
import defaultCharacter from "../fixtures/default-character.json";
import { newGroupForm } from "./form";
import { appearanceFields, changeGroupAppearance, groupAppearance } from "./appearance";

const make = () => newGroupForm((defaultCharacter as CharacterDraft_Serialize).presentation.chat_appearance, false).draft;

describe("group appearance inheritance", () => {
  it("keeps an explicit default while unrelated fields follow app changes", () => {
    const preferences = { chatAppearance: { fontSize: "large", bubbleRadius: "pill" } };
    const own = changeGroupAppearance(make(), preferences, "bubbleRadius", "rounded", "#123456");
    expect(groupAppearance(own, preferences)).toMatchObject({ fontSize: "large", bubbleRadius: "rounded" });
    expect(own.presentation_overrides).toEqual(["bubble_radius"]);
    expect(groupAppearance({ ...own, presentation_overrides: [] }, preferences).bubbleRadius).toBe("pill");
  });

  it("matches retained historical defaults when inferring old group overrides", () => {
    const draft = { ...make(), presentation_overrides: null };
    expect([...appearanceFields(draft, {})]).toEqual([]);
    expect(groupAppearance(draft, {}).participantsBarBackground).toBe("fading");
    draft.presentation.font_size = "small";
    const preferences = { appearanceInheritanceBaseline: { fontSize: "medium" }, chatAppearance: { fontSize: "large" } };
    expect([...appearanceFields(draft, preferences)]).toEqual(["font_size"]);
    expect(groupAppearance(draft, preferences).fontSize).toBe("small");
  });

  it("serializes native placement and details and preserves deferred widget data", () => {
    const draft = make();
    draft.presentation.chat_widget_slots = { left: [], right: [] };
    const placement = changeGroupAppearance(draft, {}, "messageInfoPlacement", "insideBubble", "#123456");
    expect(placement.presentation.message_info_placement).toBe("inside_bubble");
    expect(groupAppearance(placement, {}).messageInfoPlacement).toBe("insideBubble");
    const details = changeGroupAppearance(placement, {}, "details", ["model", "total"], "#123456");
    expect(details.presentation.show_message_model).toBe(true);
    expect(details.presentation.show_message_input_tokens).toBe(false);
    expect(groupAppearance(details, {}).details).toEqual(["model", "total"]);
    expect(details.presentation.chat_widget_slots).toEqual(draft.presentation.chat_widget_slots);
  });

  it("stores custom colors without invalid native enum values and clears them for presets", () => {
    const own = changeGroupAppearance(make(), {}, "userBubbleColor", "custom", "#123456");
    expect(own.presentation.user_bubble_color).toBe("accent");
    expect(groupAppearance(own, {}).userBubbleColor).toBe("custom");
    const preset = changeGroupAppearance(own, {}, "userBubbleColor", "info", "#123456");
    expect(preset.presentation.user_bubble_color_hex).toBeNull();
    expect(groupAppearance(preset, {}).userBubbleColor).toBe("info");
  });
});
