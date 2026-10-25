import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ApiFailure, isInsideShell } from "./client";
import { openExternalUrl } from "./external-links";

vi.mock("./client", async (original) => ({ ...await original<typeof import("./client")>(), isInsideShell: vi.fn(() => true) }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));
import { openUrl } from "@tauri-apps/plugin-opener";

beforeEach(() => {
  vi.mocked(isInsideShell).mockReturnValue(true);
  vi.mocked(openUrl).mockReset().mockResolvedValue(undefined);
});
afterEach(() => vi.restoreAllMocks());

it("uses the native opener with the default browser", async () => {
  const browser = vi.spyOn(window, "open").mockReturnValue(null);
  await openExternalUrl("https://github.com/LettuceAI/app/issues/new?version=1.2.3");
  expect(openUrl).toHaveBeenCalledWith("https://github.com/LettuceAI/app/issues/new?version=1.2.3");
  expect(browser).not.toHaveBeenCalled();
});

it("keeps a native rejection visible instead of trying another browser path", async () => {
  const browser = vi.spyOn(window, "open").mockReturnValue(null);
  vi.mocked(openUrl).mockRejectedValueOnce("URL not allowed");
  await expect(openExternalUrl("https://github.com/LettuceAI/app")).rejects.toMatchObject({ code: "unavailable" });
  expect(browser).not.toHaveBeenCalled();
});

it.each(["not a URL", "javascript:alert(1)", "file:///etc/passwd", "lettuceai://open/settings"])("rejects non-web URL %s before calling the host", async (url) => {
  const result = openExternalUrl(url);
  await expect(result).rejects.toBeInstanceOf(ApiFailure);
  await expect(result).rejects.toMatchObject({ code: "invalid_input" });
  expect(openUrl).not.toHaveBeenCalled();
});

it("opens a separate browser tab in the standalone UI preview", async () => {
  vi.mocked(isInsideShell).mockReturnValue(false);
  const browser = vi.spyOn(window, "open").mockReturnValue(null);
  await openExternalUrl("https://www.lettuceai.app/changelog");
  expect(browser).toHaveBeenCalledWith("https://www.lettuceai.app/changelog", "_blank", "noopener,noreferrer");
  expect(openUrl).not.toHaveBeenCalled();
});
