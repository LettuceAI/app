import { beforeEach, expect, it, vi } from "vitest";
import { isInsideShell } from "./client";
import { createBackButton } from "./back-button";

const native = vi.hoisted(() => ({
  handler: null as ((payload: { canGoBack: boolean }) => void) | null,
  unregister: vi.fn().mockResolvedValue(undefined),
  exit: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("./client", () => ({ isInsideShell: vi.fn(() => true) }));
vi.mock("@tauri-apps/api/app", () => ({
  onBackButtonPress: vi.fn(async (handler: (payload: { canGoBack: boolean }) => void) => {
    native.handler = handler;
    return { unregister: native.unregister };
  }),
  exit: native.exit,
}));

beforeEach(() => {
  vi.clearAllMocks();
  native.handler = null;
  vi.mocked(isInsideShell).mockReturnValue(true);
});

it("only handles the back button inside the Android shell", async () => {
  expect(await createBackButton("linux")).toBeNull();
  expect(await createBackButton("ios")).toBeNull();
  vi.mocked(isInsideShell).mockReturnValue(false);
  expect(await createBackButton("android")).toBeNull();
});

it("forwards presses with the history flag, unregisters and exits", async () => {
  const button = await createBackButton("android");
  const onBack = vi.fn();
  const stop = await button!.listen(onBack);
  native.handler!({ canGoBack: true });
  native.handler!({ canGoBack: false });
  expect(onBack.mock.calls).toEqual([[true], [false]]);
  stop();
  expect(native.unregister).toHaveBeenCalledOnce();
  await button!.exit();
  expect(native.exit).toHaveBeenCalledWith(0);
});
