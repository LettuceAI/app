import { afterEach, describe, expect, it, vi } from "vitest";
import { selectDeviceLocale } from "./device-locale";

describe("device locale import", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("detects the first supported browser language after missing storage", () => {
    vi.stubGlobal("navigator", { language: "xx", languages: ["xx", "zh-TW", "tr-TR"] });
    expect(selectDeviceLocale({}, () => null)).toBe("zh-Hant");
    expect(selectDeviceLocale({}, () => { throw new Error("denied"); })).toBe("zh-Hant");
  });
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
