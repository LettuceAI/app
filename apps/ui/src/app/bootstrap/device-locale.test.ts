import { afterEach, describe, expect, it, vi } from "vitest";
import { createI18n } from "@/shared/i18n";
import { followSystemLocale, selectDeviceLocale } from "./device-locale";

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

  it("lets the system language win on Android and falls back to the stored choice", () => {
    vi.stubGlobal("navigator", { language: "de-DE", languages: ["de-DE"] });
    expect(selectDeviceLocale({ locale: "tr" }, () => "fr", true)).toBe("de");
    expect(selectDeviceLocale({ locale: "tr" }, () => "fr")).toBe("tr");
    vi.stubGlobal("navigator", { language: "xx", languages: ["xx"] });
    expect(selectDeviceLocale({ locale: "tr" }, () => null, true)).toBe("tr");
  });
});

describe("following the system language", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("switches and stores the language when the system one changes", async () => {
    const localization = await createI18n("en");
    const store = vi.fn().mockResolvedValue(undefined);
    const stop = followSystemLocale(localization, store);
    vi.stubGlobal("navigator", { language: "tr-TR", languages: ["tr-TR"] });
    window.dispatchEvent(new Event("languagechange"));
    expect(localization.language).toBe("tr");
    expect(store).toHaveBeenCalledWith("tr");
    stop();
    vi.stubGlobal("navigator", { language: "de-DE", languages: ["de-DE"] });
    window.dispatchEvent(new Event("languagechange"));
    expect(localization.language).toBe("tr");
    expect(store).toHaveBeenCalledOnce();
  });
});
