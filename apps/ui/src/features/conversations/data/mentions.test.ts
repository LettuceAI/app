import { describe, expect, it } from "vitest";
import { activeMention, findMention, insertMention, toggleMention } from "./mentions";

describe("mentions", () => {
  it("adds a plain mention and a quoted one for a name with spaces", () => {
    expect(toggleMention("", "Rell")).toBe("@Rell ");
    expect(toggleMention("hello", "Ruben Ochoa")).toBe('hello @"Ruben Ochoa" ');
  });

  it("removes a mention it added and leaves the rest of the draft", () => {
    expect(toggleMention("hello @Rell and more", "Rell")).toBe("hello and more");
    expect(toggleMention('@"Ruben Ochoa" hi', "Ruben Ochoa")).toBe("hi");
  });

  it("matches case-insensitively and stops at trailing punctuation", () => {
    expect(findMention("ask @rell, please", "Rell")).toEqual({ index: 4, length: 5 });
    expect(findMention("ask @Rellis", "Rell")).toBeNull();
  });

  it("escapes names that hold regex characters", () => {
    expect(findMention("@A.B hi", "A.B")).not.toBeNull();
    expect(findMention("@AxB hi", "A.B")).toBeNull();
  });

  it("finds the mention being typed at the cursor", () => {
    expect(activeMention("hi @Ru", 6)).toEqual({ start: 3, query: "Ru" });
    expect(activeMention("@", 1)).toEqual({ start: 0, query: "" });
    expect(activeMention('hi @"Ruben Oc', 13)).toEqual({ start: 3, query: "Ruben Oc" });
  });

  it("ignores an at sign that is not a mention start", () => {
    expect(activeMention("mail@host", 9)).toBeNull();
    expect(activeMention("hi @Ru be", 9)).toBeNull();
    expect(activeMention('hi @"Ruben Ochoa" yes', 22)).toBeNull();
  });

  it("replaces the typed part with the full mention and keeps the rest", () => {
    expect(insertMention("hi @Ru there", { start: 3, query: "Ru" }, 6, "Rell")).toEqual({ text: "hi @Rell  there", cursor: 9 });
    expect(insertMention("@", { start: 0, query: "" }, 1, "Ruben Ochoa")).toEqual({ text: '@"Ruben Ochoa" ', cursor: 15 });
  });
});
