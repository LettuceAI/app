import type { ConversationBranchList, ConversationBranchView, ConversationSummary } from "@/api/generated/bindings";

export type TreeItem =
  | { kind: "chat"; id: string; chat: ConversationSummary; current: boolean; children: TreeItem[] }
  | { kind: "branch"; id: string; branch: ConversationBranchView; children: TreeItem[] };

function newest<T extends { updated_at: number; id: string }>(items: T[]): T[] {
  return [...items].sort((a, b) => b.updated_at - a.updated_at || a.id.localeCompare(b.id));
}

function branchItems(list: ConversationBranchList): TreeItem[] {
  const children = new Map<string | null, ConversationBranchView[]>();
  const known = new Set(list.branches.map((branch) => branch.id));
  for (const branch of list.branches) {
    const parent = branch.parent_branch_id !== null && known.has(branch.parent_branch_id) ? branch.parent_branch_id : null;
    children.set(parent, [...(children.get(parent) ?? []), branch]);
  }
  const build = (parent: string | null): TreeItem[] =>
    newest(children.get(parent) ?? []).map((branch) => ({ kind: "branch", id: `branch:${branch.id}`, branch, children: build(branch.id) }));
  return build(null);
}

export function buildTree(chats: readonly ConversationSummary[], currentId: string, branches: ConversationBranchList): TreeItem[] {
  const byId = new Map(chats.map((chat) => [chat.id, chat]));
  const current = byId.get(currentId);
  if (!current) return branches.branches.length > 1 ? branchItems(branches) : [];

  const children = new Map<string, ConversationSummary[]>();
  for (const chat of chats) {
    const origin = chat.origin_conversation_id;
    if (origin && byId.has(origin)) children.set(origin, [...(children.get(origin) ?? []), chat]);
  }

  let top = current;
  const seen = new Set([top.id]);
  while (top.origin_conversation_id && byId.has(top.origin_conversation_id) && !seen.has(top.origin_conversation_id)) {
    top = byId.get(top.origin_conversation_id)!;
    seen.add(top.id);
  }

  const forked = branches.branches.length > 1;
  const chatItem = (chat: ConversationSummary, path: Set<string>): TreeItem => {
    const isCurrent = chat.id === currentId;
    const nested = newest(children.get(chat.id) ?? [])
      .filter((child) => !path.has(child.id))
      .map((child) => chatItem(child, new Set([...path, child.id])));
    return {
      kind: "chat",
      id: `chat:${chat.id}`,
      chat,
      current: isCurrent,
      children: [...(isCurrent && forked ? branchItems(branches) : []), ...nested],
    };
  };
  return [chatItem(top, new Set([top.id]))];
}
