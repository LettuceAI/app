import { describe, expect, it, vi } from "vitest";
import { createApiClient } from "@/api/client";
import { createMockTransport, mockAppStatus } from "@/api/mock-transport";
import { exportEntry } from "./actions";

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

describe("entity export destinations", () => {
  it.each([
    { kind: "character" as const, command: "characterExport", format: "v3" },
    { kind: "persona" as const, command: "personaExport", format: undefined },
    { kind: "lorebook" as const, command: "lorebookExport", format: "uec" },
    { kind: "lorebook" as const, command: "lorebookExport", format: "world_info" },
  ] as const)("exports $kind $format to the selected Android document", async ({ kind, command, format }) => {
    const extension = kind === "character" || format === "world_info" ? "json" : "uec";
    const target = { uri: `content://picked/export.${extension}` };
    const pick = vi.fn(() => ok({ target }));
    const write = vi.fn(() => ok({ uri: target.uri, suggested_name: `Story.${extension}`, warnings: [] }));
    const api = createApiClient(createMockTransport({ calls: {
      appStatus: () => ok({ ...mockAppStatus, platform: "android" }),
      filesPickSave: pick,
      characterExport: write,
      personaExport: write,
      lorebookExport: write,
    } }));

    await expect(exportEntry(api, { kind, id: "entity-1", name: "Story", description: "", image: null }, format === "world_info" ? format : "uec"))
      .resolves.toMatchObject({ uri: target.uri });
    expect(pick).toHaveBeenCalledExactlyOnceWith({
      suggested_name: `Story.${extension}`,
      kind: kind === "character" ? "character_card" : extension === "uec" ? "any" : "json",
    });
    expect(write).toHaveBeenCalledExactlyOnceWith({ id: "entity-1", target, ...(command === "personaExport" ? {} : { format }) });
  });

  it.each(["other", "android"] as const)("does not export when the save picker is cancelled on %s", async (platform) => {
    const pick = vi.fn(() => ok({ target: null }));
    const write = vi.fn();
    const api = createApiClient(createMockTransport({ calls: {
      appStatus: () => ok({ ...mockAppStatus, platform }),
      filesPickSave: pick,
      personaExport: write,
    } }));
    await expect(exportEntry(api, { kind: "persona", id: "p1", name: "Story", description: "", image: null })).resolves.toBeNull();
    expect(pick).toHaveBeenCalledOnce();
    expect(write).not.toHaveBeenCalled();
  });
});
