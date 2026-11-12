import { describe, expect, it } from "vitest";
import { splitTags } from "./splitTags";

describe("splitTags", () => {
  it("splits a pasted comma list into separate tags without blanks or repeats", () => {
    expect(splitTags(["Lazaro", "Doc, medic,  Lazaro ,, second-in-command"])).toEqual(["Lazaro", "Doc", "medic", "second-in-command"]);
  });
});
