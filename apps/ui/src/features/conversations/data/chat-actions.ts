import type { QueryClient } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { CompanionNoteUpsertRequest } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { watchJob } from "@/entities/job/watch";

function refreshLists(queryClient: QueryClient): Promise<void> {
  return Promise.all([
    queryClient.invalidateQueries({ queryKey: queryKeys.conversations.lists() }),
    queryClient.invalidateQueries({ queryKey: queryKeys.home.characters() }),
  ]).then(() => undefined);
}

export async function renameChat(api: ApiClient, queryClient: QueryClient, conversationId: string, revision: number, title: string): Promise<void> {
  await api.call("conversationRename", { conversation_id: conversationId, expected_revision: revision, title: title.trim() });
  await Promise.all([refreshLists(queryClient), queryClient.invalidateQueries({ queryKey: queryKeys.conversations.detail(conversationId) })]);
}

export async function setArchived(api: ApiClient, queryClient: QueryClient, conversationId: string, archived: boolean): Promise<void> {
  await api.call(archived ? "conversationArchive" : "conversationRestore", { conversation_id: conversationId });
  await Promise.all([refreshLists(queryClient), queryClient.invalidateQueries({ queryKey: queryKeys.conversations.detail(conversationId) })]);
}

export async function removeChat(api: ApiClient, queryClient: QueryClient, conversationId: string): Promise<void> {
  await api.call("conversationDelete", { conversation_id: conversationId });
  await refreshLists(queryClient);
}

export async function duplicateChat(api: ApiClient, queryClient: QueryClient, conversationId: string): Promise<string> {
  const copy = await api.call("conversationDuplicate", { conversation_id: conversationId, with_messages: true, title: null, client_operation_id: crypto.randomUUID() });
  await refreshLists(queryClient);
  return copy.conversation_id;
}

async function refreshBranches(queryClient: QueryClient, conversationId: string): Promise<void> {
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: queryKeys.conversations.detail(conversationId) }),
    queryClient.invalidateQueries({ queryKey: queryKeys.conversations.lists() }),
  ]);
}

export async function selectBranch(api: ApiClient, queryClient: QueryClient, conversationId: string, branchId: string, revision: number): Promise<void> {
  await api.call("conversationBranchSelect", { branch_id: branchId, expected_revision: revision, client_operation_id: crypto.randomUUID() });
  await refreshBranches(queryClient, conversationId);
}

export async function renameBranch(api: ApiClient, queryClient: QueryClient, conversationId: string, branchId: string, revision: number, label: string): Promise<void> {
  await api.call("conversationBranchRename", { branch_id: branchId, label: label.trim(), expected_revision: revision, client_operation_id: crypto.randomUUID() });
  await refreshBranches(queryClient, conversationId);
}

export async function deleteBranch(api: ApiClient, queryClient: QueryClient, conversationId: string, branchId: string, revision: number): Promise<void> {
  await api.call("conversationBranchDelete", { branch_id: branchId, expected_revision: revision, client_operation_id: crypto.randomUUID() });
  await refreshBranches(queryClient, conversationId);
}

export async function exportChat(api: ApiClient, conversationId: string, title: string): Promise<string | null> {
  const picked = await api.call("filesPickSave", { suggested_name: `${title.replace(/[/\\]/g, "_") || "chat"}.jsonl`, kind: "chat_log" });
  if (!picked.target) return null;
  const exported = await api.call("chatExport", { conversation_id: conversationId, target: picked.target });
  return exported.uri;
}

export async function importChatLog(api: ApiClient, queryClient: QueryClient, characterId: string): Promise<string | null> {
  const picked = await api.call("filesPickOpen", { kinds: ["chat_log"], multiple: false });
  const source = picked.sources[0];
  if (!source) return null;
  const preview = await api.call("importPreview", { source });
  const committed = await api.call("importCommit", { source, digest: preview.digest, options: { character_id: characterId }, client_operation_id: crypto.randomUUID() });
  let conversationId: string | null = null;
  if (committed.kind === "imported") conversationId = committed.id;
  else {
    const job = await watchJob(api, committed.job_id, () => undefined, new AbortController().signal);
    if (job.state !== "succeeded" || job.result?.type !== "conversation") throw new Error("import failed");
    conversationId = job.result.conversation_id;
  }
  await refreshLists(queryClient);
  return conversationId;
}

export async function saveNote(api: ApiClient, queryClient: QueryClient, request: Omit<CompanionNoteUpsertRequest, "client_operation_id">): Promise<void> {
  await api.call("companionNotesUpsert", { ...request, client_operation_id: crypto.randomUUID() });
  await queryClient.invalidateQueries({ queryKey: [...queryKeys.characters.all, "notes", request.character_id] });
}

export async function deleteNote(api: ApiClient, queryClient: QueryClient, characterId: string, noteId: string): Promise<void> {
  await api.call("companionNotesDelete", { note_id: noteId, client_operation_id: crypto.randomUUID() });
  await queryClient.invalidateQueries({ queryKey: [...queryKeys.characters.all, "notes", characterId] });
}
