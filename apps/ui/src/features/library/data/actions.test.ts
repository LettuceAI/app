import { describe, expect, it, vi } from "vitest";
import { createApiClient } from "@/api/client";
import { createMockTransport, mockAppStatus } from "@/api/mock-transport";
import { exportEntry } from "./actions";

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

describe("entity export destinations", () => {
  it.each([
    { kind: "character" as const, command: "characterExport", format: "v3" },
    { kind: "persona" as const, command: "personaExport" },
    { kind: "lorebook" as const, command: "lorebookExport", format: "uec" },
    { kind: "lorebook" as const, command: "lorebookExport", format: "world_info" },
  ] as const)("publishes $kind $format to Android Downloads without a picker", async ({ kind, command, format }) => {
    const pick = vi.fn(() => ok({ target: { uri: "content://picked/export" } }));
    const write = vi.fn(() => ok({ uri: "content://downloads/export", suggested_name: "export.uec", warnings: [] }));
    const api = createApiClient(createMockTransport({ calls: {
      appStatus: () => ok({ ...mockAppStatus, platform: "android" }),
      filesPickSave: pick,
      characterExport: write,
      personaExport: write,
      lorebookExport: write,
    } }));

    await expect(exportEntry(api, { kind, id: "entity-1", name: "Story", description: "", image: null }, format === "world_info" ? format : "uec"))
      .resolves.toMatchObject({ uri: "content://downloads/export" });
    expect(pick).not.toHaveBeenCalled();
    expect(write).toHaveBeenCalledExactlyOnceWith({ id: "entity-1", target: null, ...(command === "personaExport" ? {} : { format }) });
  });

  it("does not export when the desktop save picker is cancelled", async () => {
    const write = vi.fn();
    const api = createApiClient(createMockTransport({ calls: {
      filesPickSave: () => ok({ target: null }),
      personaExport: write,
    } }));
    await expect(exportEntry(api, { kind: "persona", id: "p1", name: "Story", description: "", image: null })).resolves.toBeNull();
    expect(write).not.toHaveBeenCalled();
  });
});
