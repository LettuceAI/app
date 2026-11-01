import { afterEach, beforeEach, describe, expect, mock, test } from "bun:test";

let currentVersion = "2.2.6";
mock.module("@tauri-apps/api/core", () => ({
  invoke: async () => currentVersion,
}));

const { checkForAppUpdate, getAppUpdateDismissKey } = await import("../src/core/app-updates/checkForAppUpdate");
const originalFetch = globalThis.fetch;
const originalWindow = globalThis.window;
const linux = { type: "desktop", os: "linux", arch: "x86_64" } as const;
const platforms = [
  [linux, "LettuceAI_3.0.0_amd64.AppImage"],
  [{ type: "desktop", os: "windows", arch: "x86_64" }, "LettuceAI_3.0.0_x64-setup.exe"],
  [{ type: "desktop", os: "macos", arch: "aarch64" }, "LettuceAI_3.0.0_aarch64.dmg"],
  [{ type: "mobile", os: "android", arch: "aarch64" }, "LettuceAI_3.0.0.apk"],
] as const;

function release(tag: string, body = "", asset = "linux-cpu.zip", extra = {}) {
  return { tag_name: tag, body, html_url: `https://github.com/LettuceAI/app/releases/tag/${tag}`,
    draft: false, prerelease: false, assets: [{ name: asset }], ...extra };
}
function serve(...releases: ReturnType<typeof release>[]) {
  globalThis.fetch = mock(async () => Response.json(releases)) as unknown as typeof fetch;
}

beforeEach(() => {
  currentVersion = "2.2.6";
  globalThis.window = { setTimeout, clearTimeout } as unknown as Window & typeof globalThis;
});
afterEach(() => {
  globalThis.fetch = originalFetch;
  globalThis.window = originalWindow;
});

describe("Next release announcement", () => {
  for (const [platform, asset] of platforms) {
    test(`recognizes the ready unified 3.0.0 release on ${platform.os}`, async () => {
      serve(release("v3.0.0", "# LettuceAI Next\nready!", asset));
      const update = await checkForAppUpdate(platform);
      expect(update).toMatchObject({ latestVersion: "3.0.0", releaseTag: "v3.0.0", isNextRelease: true });
      expect(new URL(update!.downloadUrl).searchParams.get("release")).toBe("v3.0.0");
    });
  }
  for (const tag of ["v3.0.0", "3.0.0", "desktop-release-3.0.0"]) {
    test(`waits for ready! on ${tag}`, async () => {
      serve(release(tag, "Coming soon."));
      expect(await checkForAppUpdate(linux)).toBeNull();
      serve(release(tag, "ready!"));
      expect((await checkForAppUpdate(linux))?.isNextRelease).toBe(true);
    });
  }
  test("rejects drafts, prereleases, other versions and missing installers", async () => {
    serve(release("v3.0.0", "ready!", "source.zip", { draft: true }));
    expect(await checkForAppUpdate(linux)).toBeNull();
    serve(release("v3.0.0", "ready!", "linux-cpu.zip", { prerelease: true }));
    expect(await checkForAppUpdate(linux)).toBeNull();
    serve(release("v3.0.0-beta.1", "ready!"), release("v13.0.0", "ready!"), release("v3.0.0", "ready!", "notes.txt"));
    expect(await checkForAppUpdate(linux)).toBeNull();
  });
  test("does not announce Next when it is already installed", async () => {
    currentVersion = "3.0.0";
    serve(release("v3.0.0", "ready!"));
    expect(await checkForAppUpdate(linux)).toBeNull();
  });
  test("uses one dismissal key across legacy and unified tags", async () => {
    serve(release("v3.0.0", "ready!"));
    const update = (await checkForAppUpdate(linux))!;
    expect(getAppUpdateDismissKey(update, "linux")).toBe(getAppUpdateDismissKey({ ...update, releaseTag: "desktop-release-3.0.0" }, "linux"));
  });
});

describe("release exclusion markers", () => {
  for (const marker of ["Nrelease!", "Ndev!"]) {
    test(`ignores ${marker} even with ready! and falls back to an eligible release`, async () => {
      serve(release("v3.0.0", `ready!\n${marker}`), release("desktop-release-2.2.8", marker), release("desktop-release-2.2.7"));
      const update = await checkForAppUpdate(linux);
      expect(update).toMatchObject({ latestVersion: "2.2.7", isNextRelease: false });
    });
    test(`also ignores ${marker} for dev builds`, async () => {
      currentVersion = "0.1.200-dev.1";
      serve(release("desktop-dev-202-1-abc123", marker, "linux-cpu.zip", { prerelease: true }), release("desktop-dev-201-1-def456", "", "linux-cpu.zip", { prerelease: true }));
      expect((await checkForAppUpdate(linux))?.releaseTag).toBe("desktop-dev-201-1-def456");
    });
  }
  test("keeps platform matching and numeric ordering for ordinary updates", async () => {
    serve(release("desktop-release-2.2.8"), release("desktop-release-2.2.10"), release("desktop-release-2.2.9"), release("android-release-2.2.20", "", "app.apk"));
    const update = (await checkForAppUpdate(linux))!;
    expect(update.latestVersion).toBe("2.2.10");
    expect(getAppUpdateDismissKey(update, "linux")).toBe("app-update-dismissed:linux:desktop-release-2.2.10");
  });
});
