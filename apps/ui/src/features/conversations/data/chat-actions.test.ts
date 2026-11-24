import { expect, it, vi } from "vitest";
import { createApiClient } from "@/api/client";
import { createMockTransport, mockAppStatus } from "@/api/mock-transport";
import { exportChat } from "./chat-actions";

it("exports a chat to the selected Android document", async () => {
  const target = { uri: "content://picked/chat.jsonl" };
  const pick = vi.fn(() => ({ status: "ok" as const, data: { target } }));
  const write = vi.fn(() => ({ status: "ok" as const, data: { uri: target.uri, suggested_name: "chat.jsonl", warnings: [] } }));
  const api = createApiClient(createMockTransport({ calls: {
    appStatus: () => ({ status: "ok", data: { ...mockAppStatus, platform: "android" } }),
    filesPickSave: pick, chatExport: write,
  } }));

  await expect(exportChat(api, "chat-1", "Story")).resolves.toBe(target.uri);
  expect(pick).toHaveBeenCalledExactlyOnceWith({ suggested_name: "Story.jsonl", kind: "chat_log" });
  expect(write).toHaveBeenCalledExactlyOnceWith({ conversation_id: "chat-1", target });
});
