import { expect, it, vi } from "vitest";
import { createApiClient } from "@/api/client";
import { createMockTransport, mockAppStatus } from "@/api/mock-transport";
import { exportChat } from "./chat-actions";

it("exports a chat to Android Downloads without a save picker", async () => {
  const pick = vi.fn(() => ({ status: "ok" as const, data: { target: null } }));
  const write = vi.fn(() => ({ status: "ok" as const, data: { uri: "content://downloads/chat", suggested_name: "chat.jsonl", warnings: [] } }));
  const api = createApiClient(createMockTransport({ calls: {
    appStatus: () => ({ status: "ok", data: { ...mockAppStatus, platform: "android" } }),
    filesPickSave: pick, chatExport: write,
  } }));

  await expect(exportChat(api, "chat-1", "Story")).resolves.toBe("content://downloads/chat");
  expect(write).toHaveBeenCalledExactlyOnceWith({ conversation_id: "chat-1", target: null });
  expect(pick).not.toHaveBeenCalled();
});
