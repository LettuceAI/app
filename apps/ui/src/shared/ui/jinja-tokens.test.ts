import { describe, expect, it } from "vitest";
import { jinjaTokens } from "./jinja-tokens";

describe("Jinja highlighting", () => {
  it("preserves template text and distinguishes comments, control flow, strings and model tokens", () => {
    const source = `{# comment\nsecond line #}\n{% for message in messages %}\n{{ message['content'] }}<|im_end|>\n{% endfor %}`;
    const tokens = jinjaTokens(source);
    expect(tokens.map((token) => token.text).join("")).toBe(source);
    expect(tokens.find((token) => token.text === "for")?.className).toBe("text-secondary");
    expect(tokens.find((token) => token.text === "'content'")?.className).toBe("text-accent");
    expect(tokens.find((token) => token.text === "<|im_end|>")?.className).toBe("text-warning");
    expect(tokens.find((token) => token.text.startsWith("{#"))?.className).toContain("italic");
  });
  it("preserves incomplete edits and literal HTML without interpreting them", () => {
    for (const source of ["{{ user[", "{% if foo", "{# unfinished", '<script>alert("x")</script>\n{{ "a\\\\b" }}', "\n\tplain  text\n"]) {
      expect(jinjaTokens(source).map((token) => token.text).join("")).toBe(source);
    }
  });
});
