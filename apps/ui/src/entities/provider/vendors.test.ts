import { describe, expect, it } from "vitest";
import { defaultModelName, vendorOf } from "./vendors";

describe("model vendors", () => {
  it("names a new model without the company prefix when the company is known from the id", () => {
    expect(defaultModelName("DeepSeek: DeepSeek V3.2", "deepseek/deepseek-v3.2")).toBe("DeepSeek V3.2");
    expect(defaultModelName("Anthropic: Claude Sonnet 4.6", "anthropic/claude-sonnet-4.6")).toBe("Claude Sonnet 4.6");
    expect(defaultModelName("Arcee AI: Trinity Large Preview (free)", "arcee-ai/trinity-large-preview:free")).toBe("Arcee AI: Trinity Large Preview (free)");
    expect(defaultModelName(null, "deepseek/deepseek-v3.2")).toBe("deepseek-v3.2");
    expect(defaultModelName("Gemini 2.5 Flash", "gemini-2.5-flash")).toBe("Gemini 2.5 Flash");
  });

  it("finds the company behind an id, or none", () => {
    expect(vendorOf("x-ai/grok-4.1-fast")?.vendor).toBe("xAI");
    expect(vendorOf("pony-alpha")).toBeNull();
  });
});
