import { afterEach, describe, expect, it, vi } from "vitest";
import { isInsideShell } from "./client";
import { typingHaptic } from "./haptics";
import { impactFeedback } from "@tauri-apps/plugin-haptics";

vi.mock("./client", async (original) => ({
  ...(await original<typeof import("./client")>()),
  isInsideShell: vi.fn(() => true),
}));
vi.mock("@tauri-apps/plugin-haptics", () => ({
  impactFeedback: vi.fn(async () => ({ status: "ok", data: null })),
}));
afterEach(() => vi.clearAllMocks());

describe("typing haptics", () => {
  it.each(["linux", "windows", "macos", "other", undefined] as const)(
    "ignores haptics on %s before calling the native plugin",
    async (platform) => {
      await typingHaptic("heavy", platform);
      expect(impactFeedback).not.toHaveBeenCalled();
    },
  );

  it.each(["android", "ios"] as const)("uses the stored strength on %s", async (platform) => {
    await typingHaptic("rigid", platform);
    expect(impactFeedback).toHaveBeenCalledExactlyOnceWith("rigid");
  });

  it("ignores haptics outside the native shell", async () => {
    vi.mocked(isInsideShell).mockReturnValueOnce(false);
    await typingHaptic("heavy", "android");
    expect(impactFeedback).not.toHaveBeenCalled();
  });
});
