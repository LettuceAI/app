import { describe, expect, it } from "vitest";
import { sceneDirectiveState, visibleDelta } from "./scene-directive";

function stream(parts: string[]) {
  const state = sceneDirectiveState();
  return { text: parts.map((part) => visibleDelta(state, part)).join(""), state };
}

describe("visibleDelta", () => {
  it("hides the directive and its prompt", () => {
    const { text, state } = stream(["He nods. <img>a sunny street</img> Then he waits."]);
    expect(text).toBe("He nods.  Then he waits.");
    expect(state.seen).toBe(true);
  });

  it("holds back a tag split across deltas", () => {
    const { text, state } = stream(["He nods. <i", "mg>a sunny", " street</i", "mg> Then."]);
    expect(text).toBe("He nods.  Then.");
    expect(state.inside).toBe(false);
  });

  it("hides an unclosed directive while it streams", () => {
    const { text, state } = stream(["Fine. <IMG>a lo", "ng prompt"]);
    expect(text).toBe("Fine. ");
    expect(state.inside).toBe(true);
  });

  it("closes on a continue marker", () => {
    expect(stream(["<img>prompt[continue]rest"]).text).toBe("rest");
  });

  it("leaves text without a directive alone", () => {
    expect(stream(["a <b> tag ", "and < signs"]).text).toBe("a <b> tag and < signs");
  });
});
