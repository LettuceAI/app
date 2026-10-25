import { contrastRatio } from "@lettuceai/crisp";
import { describe, expect, it } from "vitest";
import { appThemes } from "./themes";
import { themeFromPreferences } from "./context";
import { readChatLook } from "./chat";
import { chatAppearance } from "./appearance";

it("defaults to filled bubbles, normal spacing and a bottom byline while keeping saved choices", () => {
  const defaults = readChatLook({});
  expect(defaults).toMatchObject({
    lineSpacing: "normal",
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
