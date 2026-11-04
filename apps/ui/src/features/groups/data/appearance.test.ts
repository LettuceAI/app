import { describe, expect, it } from "vitest";
import type { CharacterDraft_Serialize } from "@/api/generated/bindings";
import defaultCharacter from "../fixtures/default-character.json";
import { newGroupForm } from "./form";
import { appearanceFields, appearanceLook, changeAppearance, characterAppearance, withCharacterAppearance } from "./appearance";

const make = () => newGroupForm((defaultCharacter as CharacterDraft_Serialize).presentation.chat_appearance, false).draft;

describe("group appearance inheritance", () => {
  it("inherits group defaults before applying the individual group's overrides", () => {
    const preferences = {
      chatAppearance: { fontSize: "large", bubbleRadius: "pill" },
      groupChatAppearance: { fontSize: "small", bubbleRadius: "pill", participantsBarAvatarShape: "boxed" },
    };
    const own = changeAppearance(make(), preferences, "bubbleRadius", "sharp", "#123456");
    expect(appearanceLook(own, preferences)).toMatchObject({ fontSize: "small", bubbleRadius: "sharp", participantsBarAvatarShape: "boxed" });
    expect(appearanceLook({ ...own, presentation_overrides: [] }, preferences)).toMatchObject({ fontSize: "small", bubbleRadius: "pill" });
  });

  it("keeps an explicit default while unrelated fields follow app changes", () => {
    const preferences = { chatAppearance: { fontSize: "large", bubbleRadius: "pill" } };
    const own = changeAppearance(make(), preferences, "bubbleRadius", "rounded", "#123456");
    expect(appearanceLook(own, preferences)).toMatchObject({ fontSize: "large", bubbleRadius: "rounded" });
    expect(own.presentation_overrides).toEqual(["bubble_radius"]);
    expect(appearanceLook({ ...own, presentation_overrides: [] }, preferences).bubbleRadius).toBe("pill");
  });

  it("matches retained historical defaults when inferring old group overrides", () => {
    const draft = { ...make(), presentation_overrides: null };
    expect([...appearanceFields(draft, {})]).toEqual([]);
    expect(appearanceLook(draft, {}).participantsBarBackground).toBe("fading");
    draft.presentation.font_size = "small";
    const preferences = { appearanceInheritanceBaseline: { fontSize: "medium" }, chatAppearance: { fontSize: "large" } };
    expect([...appearanceFields(draft, preferences)]).toEqual(["font_size"]);
    expect(appearanceLook(draft, preferences).fontSize).toBe("small");
  });

  it("serializes native placement and details and preserves deferred widget data", () => {
    const draft = make();
    draft.presentation.chat_widget_slots = { left: [], right: [] };
    const placement = changeAppearance(draft, {}, "messageInfoPlacement", "insideBubble", "#123456");
    expect(placement.presentation.message_info_placement).toBe("inside_bubble");
    expect(appearanceLook(placement, {}).messageInfoPlacement).toBe("insideBubble");
    const details = changeAppearance(placement, {}, "details", ["model", "total"], "#123456");
    expect(details.presentation.show_message_model).toBe(true);
    expect(details.presentation.show_message_input_tokens).toBe(false);
    expect(appearanceLook(details, {}).details).toEqual(["model", "total"]);
    expect(details.presentation.chat_widget_slots).toEqual(draft.presentation.chat_widget_slots);
  });

  it("stores custom colors without invalid native enum values and clears them for presets", () => {
    const own = changeAppearance(make(), {}, "userBubbleColor", "custom", "#123456");
    expect(own.presentation.user_bubble_color).toBe("accent");
    expect(appearanceLook(own, {}).userBubbleColor).toBe("custom");
    const preset = changeAppearance(own, {}, "userBubbleColor", "info", "#123456");
    expect(preset.presentation.user_bubble_color_hex).toBeNull();
    expect(appearanceLook(preset, {}).userBubbleColor).toBe("info");
  });

  it("edits a character's chat appearance with the same per-field overrides and writes it back", () => {
    const presentation = (defaultCharacter as CharacterDraft_Serialize).presentation;
    const preferences = { chatAppearance: { fontSize: "large" } };
    const own = changeAppearance(characterAppearance(presentation), preferences, "bubbleRadius", "sharp", "#123456");
    expect(own.presentation_overrides).toEqual(["bubble_radius"]);
    expect(appearanceLook(own, preferences, false)).toMatchObject({ fontSize: "large", bubbleRadius: "sharp" });
    const written = withCharacterAppearance(presentation, own);
    expect(written.chat_appearance.bubble_radius).toBe("sharp");
    expect(written.chat_appearance_overrides).toEqual(["bubble_radius"]);
    expect(written.card_style).toBe(presentation.card_style);
    expect(characterAppearance({ ...written, chat_appearance_overrides: null }).presentation_overrides).toBeNull();
  });
});
