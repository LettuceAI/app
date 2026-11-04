import { contrastRatio } from "@lettuceai/crisp";
import { describe, expect, it } from "vitest";
import { appThemes } from "./themes";
import { themeFromPreferences } from "./context";
import { readChatLook } from "./chat";
import { appAppearancePreferences, chatAppearance } from "./appearance";

it("makes group chats follow normal chat until their own appearance exists, then uses it wholesale", () => {
  const preferences = {
    chatAppearance: { userBubbleColor: "custom", userBubbleColorHex: "#123456", fontSize: "large" },
  };
  expect(readChatLook(appAppearancePreferences(preferences, true))).toMatchObject({ userBubbleColor: "custom", userBubbleColorHex: "#123456", fontSize: "large" });
  const own = { ...preferences, groupChatAppearance: { userBubbleColor: "info", fontSize: "small" } };
  expect(readChatLook(appAppearancePreferences(own, true))).toMatchObject({ userBubbleColor: "info", userBubbleColorHex: null, fontSize: "small", bubbleRadius: "rounded" });
  expect(readChatLook(appAppearancePreferences(own))).toMatchObject({ userBubbleColor: "custom", userBubbleColorHex: "#123456", fontSize: "large" });
  expect(readChatLook(appAppearancePreferences({ ...preferences, groupChatAppearance: {} }, true))).toMatchObject({ userBubbleColor: "accent", fontSize: "medium" });
});

it("defaults to the design lab look while keeping saved choices", () => {
  const defaults = readChatLook({});
  expect(defaults).toMatchObject({
    lineSpacing: "relaxed",
    showMessageAuthor: true,
    showMessageTimestamp: true,
    chatColumnWidth: "full",
    bubbleBlur: "medium",
    bubbleStyle: "filled",
    messageHeaderPlacement: "below",
    fontSize: "medium",
    composerFontSize: "medium",
    bubbleRadius: "rounded",
    bubbleMaxWidth: "normal",
    bubblePadding: "normal",
  });
  expect(chatAppearance(defaults).headerPlacement).toBe("below");
  expect(
    readChatLook({
      lineSpacing: "relaxed",
      bubbleStyle: "bordered",
      messageHeaderPlacement: "above",
    }),
  ).toMatchObject({
    lineSpacing: "relaxed",
    bubbleStyle: "bordered",
    messageHeaderPlacement: "above",
  });
});

describe("customization themes", () => {
  it.each(appThemes)("keeps $name readable for body text and accent controls", (theme) => {
    expect(contrastRatio(theme.colors.fg, theme.colors.surface)).toBeGreaterThanOrEqual(4.5);
    expect(contrastRatio(theme.colors.accent, theme.colors.surface)).toBeGreaterThanOrEqual(3);
  });
  it("restores the palette, radius and density from persisted preferences", () => {
    expect(
      themeFromPreferences({
        themePreset: "forest",
        themeRadius: "round",
        themeDensity: "compact",
      }),
    ).toMatchObject({
      id: "forest",
      radius: "round",
      density: "compact",
      colors: { surface: "#0a1510" },
    });
  });
});
