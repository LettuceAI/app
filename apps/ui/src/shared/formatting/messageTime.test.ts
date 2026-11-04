import { describe, expect, it } from "vitest";
import { formatMessageTime } from "./messageTime";

describe("message clock format", () => {
  it.each([
    [0, "12:04 AM"],
    [7, "7:04 AM"],
    [12, "12:04 PM"],
    [19, "7:04 PM"],
  ] as const)("formats hour %s with the correct day period", (hour, expected) => {
    const at = new Date(2026, 10, 4, hour, 4);
    expect(formatMessageTime(at, "time12", "en-US").replace(/\s/g, " ")).toBe(expected);
  });

  it("keeps the 24-hour option independent of the locale's default clock", () => {
    const at = new Date(2026, 10, 4, 19, 4);
    expect(formatMessageTime(at, "time", "en-US")).toBe("19:04");
    expect(formatMessageTime(at, "time", "en-GB")).toBe("19:04");
    expect(formatMessageTime(at, "time12", "en-GB")).toMatch(/^7:04\s*pm$/i);
    const midnight = new Date(2026, 10, 4, 0, 4);
    expect(formatMessageTime(midnight, "time", "en-US")).toBe("00:04");
    expect(formatMessageTime(midnight, "time12", "en-GB")).toMatch(/^12:04\s*am$/i);
  });

  it("keeps dates confined to the date-and-time option", () => {
    const at = new Date(2026, 10, 4, 19, 4).getTime();
    expect(formatMessageTime(at, "datetime", "en-US")).toContain("Nov 4, 2026");
    expect(formatMessageTime(at, "time12", "en-US")).not.toContain("2026");
  });
});
