import { describe, expect, it } from "vitest";
import type { ConversationBranchList, ConversationBranchView, ConversationSummary } from "@/api/generated/bindings";
import { buildTree, type TreeItem } from "./tree-model";

function chat(id: string, origin: string | null, updatedAt: number): ConversationSummary {
  return {
    id, title: id, kind: "direct", revision: 1, updated_at: updatedAt, archived: false, avatars: [], background: null, last_message_preview: null,
    source: { type: "direct", character_id: "ch" }, message_count: 1, chat_mode: null, missing_models: [], memory_blocked: null,
    origin_conversation_id: origin, origin_message_id: origin ? "m" : null,
  };
}

function branch(id: string, parent: string | null, updatedAt: number, active = false): ConversationBranchView {
  return { id, label: id, parent_branch_id: parent, fork_message_id: parent ? "m" : null, message_count: 1, updated_at: updatedAt, active };
}

const single: ConversationBranchList = { revision: 1, branches: [branch("root", null, 1, true)] };

function shape(items: readonly TreeItem[]): unknown[] {
  return items.map((item) => [item.id, ...(item.children.length ? [shape(item.children)] : [])]);
}

describe("buildTree", () => {
  it("starts at the oldest ancestor and keeps the siblings of the current chat", () => {
    const chats = [chat("a", null, 1), chat("b", "a", 3), chat("c", "b", 5), chat("d", "a", 2), chat("x", null, 9)];
    expect(shape(buildTree(chats, "c", single))).toEqual([["chat:a", [["chat:b", [["chat:c"]]], ["chat:d"]]]]);
  });

  it("puts the current chat's own branches before the chats branched from it", () => {
    const chats = [chat("a", null, 1), chat("b", "a", 2)];
    const list: ConversationBranchList = { revision: 3, branches: [branch("root", null, 1), branch("s1", "root", 5, true), branch("s2", "root", 4), branch("deep", "s2", 6)] };
    expect(shape(buildTree(chats, "a", list))).toEqual([["chat:a", [["branch:root", [["branch:s1"], ["branch:s2", [["branch:deep"]]]]], ["chat:b"]]]]);
  });

  it("leaves out a lone original branch because the chat stands for it", () => {
    expect(shape(buildTree([chat("a", null, 1)], "a", single))).toEqual([["chat:a"]]);
  });

  it("treats an origin that is gone as no origin and survives a cycle", () => {
    expect(shape(buildTree([chat("a", "gone", 1)], "a", single))).toEqual([["chat:a"]]);
    expect(shape(buildTree([chat("a", "b", 1), chat("b", "a", 2)], "a", single)).length).toBe(1);
  });
});
