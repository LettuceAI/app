import { describe, expect, it } from "vitest";
import { selectDeviceLocale } from "./device-locale";

describe("device locale import", () => {
  it("keeps the device choice ahead of readable legacy storage", () => {
    expect(selectDeviceLocale({ locale: "tr" }, () => "de")).toBe("tr");
  });

  it("imports a readable legacy locale", () => {
    expect(selectDeviceLocale({}, () => "tr")).toBe("tr");
  });

  it("rejects an invalid legacy locale", () => {
    expect(selectDeviceLocale({}, () => "not-a-locale")).toBe("en");
  });

  it("defaults when legacy storage is absent or unreadable", () => {
    expect(selectDeviceLocale({}, () => null)).toBe("en");
    expect(selectDeviceLocale({}, () => { throw new Error("denied"); })).toBe("en");
  });
});
