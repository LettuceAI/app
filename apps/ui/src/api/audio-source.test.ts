import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { AudioSource } from "./audio-source";

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn());
  vi.stubGlobal("URL", class extends URL {
    static override createObjectURL = vi.fn(() => "blob:recording");
    static override revokeObjectURL = vi.fn();
  });
});
afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks(); });

it("loads a native asset once, shares pending reads and releases its Blob URL", async () => {
  vi.mocked(fetch).mockResolvedValue(new Response("wav", { headers: { "Content-Type": "audio/wav" } }));
  const source = new AudioSource("lettuce-asset://localhost/recording");
  const first = source.load();
  expect(source.load()).toBe(first);
  await expect(first).resolves.toBe("blob:recording");
  await expect(source.load()).resolves.toBe("blob:recording");
  expect(fetch).toHaveBeenCalledTimes(1);
  expect(URL.createObjectURL).toHaveBeenCalledTimes(1);
  source.release();
  expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:recording");
  source.release();
  expect(URL.revokeObjectURL).toHaveBeenCalledTimes(1);
});

it("rejects a late asset read after release without creating an orphaned URL", async () => {
  let finish!: (value: Response) => void;
  vi.mocked(fetch).mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  const source = new AudioSource("lettuce-asset://localhost/recording");
  const pending = source.load();
  const cancelled = expect(pending).rejects.toMatchObject({ name: "AbortError" });
  source.release();
  finish(new Response("wav"));
  await cancelled;
  expect(URL.createObjectURL).not.toHaveBeenCalled();
  expect(vi.mocked(fetch).mock.calls[0]?.[1]?.signal?.aborted).toBe(true);
});

it("returns a typed missing-asset error and permits a later retry", async () => {
  vi.mocked(fetch).mockResolvedValueOnce(new Response(null, { status: 404 })).mockResolvedValueOnce(new Response("wav"));
  const source = new AudioSource("lettuce-asset://localhost/recording");
  await expect(source.load()).rejects.toMatchObject({ code: "not_found" });
  await expect(source.load()).resolves.toBe("blob:recording");
  expect(fetch).toHaveBeenCalledTimes(2);
  source.release();
});

it("maps asset transport failure to a typed error", async () => {
  vi.mocked(fetch).mockRejectedValue(new TypeError("failed"));
  await expect(new AudioSource("lettuce-asset://localhost/recording").load()).rejects.toMatchObject({ code: "unavailable" });
});

it("preserves sources that the media element can play directly", async () => {
  for (const url of ["blob:owned-by-caller", "https://example.test/audio.wav", "http://lettuce-asset.localhost/recording"]) {
    const source = new AudioSource(url);
    await expect(source.load()).resolves.toBe(url);
    source.release();
  }
  expect(fetch).not.toHaveBeenCalled();
  expect(URL.revokeObjectURL).not.toHaveBeenCalled();
});
