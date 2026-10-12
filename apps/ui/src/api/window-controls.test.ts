import { beforeEach, expect, it, vi } from "vitest";
import { isInsideShell } from "./client";
import { createWindowController } from "./window-controls";

const native = vi.hoisted(() => ({
  minimize: vi.fn().mockResolvedValue(undefined),
  toggleMaximize: vi.fn().mockResolvedValue(undefined),
  close: vi.fn().mockResolvedValue(undefined),
  startResizeDragging: vi.fn().mockResolvedValue(undefined),
  isMaximized: vi.fn().mockResolvedValue(false),
  isFullscreen: vi.fn().mockResolvedValue(false),
  onResized: vi.fn(),
}));
vi.mock("./client", () => ({ isInsideShell: vi.fn(() => true) }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => native }));

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(isInsideShell).mockReturnValue(true);
  native.isMaximized.mockResolvedValue(false);
  native.isFullscreen.mockResolvedValue(false);
});

it("does not create native window controls in a browser or on mobile", async () => {
  vi.mocked(isInsideShell).mockReturnValue(false);
  expect(await createWindowController("linux")).toBeNull();
  vi.mocked(isInsideShell).mockReturnValue(true);
  expect(await createWindowController("android")).toBeNull();
  expect(await createWindowController("ios")).toBeNull();
});

it("forwards desktop controls and resize directions to the current window", async () => {
  const controls = await createWindowController("linux");
  expect(controls).not.toBeNull();
  await controls!.minimize();
  await controls!.toggleMaximize();
  await controls!.close();
  await controls!.resize("SouthEast");
  expect(native.minimize).toHaveBeenCalledOnce();
  expect(native.toggleMaximize).toHaveBeenCalledOnce();
  expect(native.close).toHaveBeenCalledOnce();
  expect(native.startResizeDragging).toHaveBeenCalledWith("SouthEast");
});

it("tracks maximized and fullscreen events and detaches on cleanup", async () => {
  let resized!: () => void;
  const unlisten = vi.fn();
  native.onResized.mockImplementation(async (callback: () => void) => { resized = callback; return unlisten; });
  const controls = await createWindowController("windows");
  const changed = vi.fn();
  const failed = vi.fn();
  const stop = await controls!.observeExpanded(changed, failed);
  expect(changed).toHaveBeenLastCalledWith(false);
  native.isMaximized.mockResolvedValue(true);
  resized();
  await vi.waitFor(() => expect(changed).toHaveBeenLastCalledWith(true));
  native.isMaximized.mockResolvedValue(false);
  native.isFullscreen.mockResolvedValue(true);
  resized();
  await vi.waitFor(() => expect(changed).toHaveBeenCalledTimes(3));
  expect(changed).toHaveBeenLastCalledWith(true);
  stop();
  expect(unlisten).toHaveBeenCalledOnce();
  changed.mockClear();
  resized();
  await Promise.resolve();
  await Promise.resolve();
  expect(changed).not.toHaveBeenCalled();
  expect(failed).not.toHaveBeenCalled();
});
