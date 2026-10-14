import { describe, expect, it } from "vitest";
import { formatSequences, parseSequences } from "./model-editor-sequences";

describe("model sequence fields", () => {
  it("round-trips delimiters, escapes, whitespace and empty strings", () => {
    const values = ["\n", ":", '"', "*", "a,b", "\\", " leading ", "\t", "", "two  spaces", "\r"];
    expect(parseSequences(formatSequences(values))).toEqual(values);
  });
  it("accepts the demo syntax and rejects an unfinished escape", () => {
    expect(parseSequences('\\n, :, ", *')).toEqual(["\n", ":", '"', "*"]);
    expect(() => parseSequences("broken\\")).toThrow();
  });
});
