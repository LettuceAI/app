import type { QueryClient } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { GroupDraft_Serialize, GroupView_Serialize } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

export async function refreshGroups(queryClient: QueryClient) {
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: queryKeys.groups.all }),
    queryClient.invalidateQueries({ queryKey: queryKeys.conversations.lists() }),
  ]);
}

export async function launchGroup(api: ApiClient, queryClient: QueryClient, groupId: string, operationId: string) {
  const result = await api.call("conversationLaunchGroup", { group_id: groupId, client_operation_id: operationId });
  await refreshGroups(queryClient);
  return result.conversation_id;
}

export interface SaveAttempt {
  payload: string;
  operationId: string;
  saved?: GroupView_Serialize;
  conversationId?: string;
}

export async function saveGroup(api: ApiClient, queryClient: QueryClient, draft: GroupDraft_Serialize, lorebookIds: string[], attempt: SaveAttempt, existing?: { id: string; revision: number; lorebookIds: string[] }) {
  const saved = attempt.saved ?? await api.call("groupSave", { group_id: existing?.id ?? null, expected_revision: existing?.revision ?? null, draft, client_operation_id: attempt.operationId });
  attempt.saved = saved;
  let revision = saved.revision;
  const current = existing?.lorebookIds ?? [];
  const removed = current.filter((id) => !lorebookIds.includes(id));
  for (const id of removed) {
    const result = await api.call("groupLorebooksUnbind", { group_id: saved.id, lorebook_id: id, expected_revision: revision, client_operation_id: `${attempt.operationId}:remove:${id}` });
    revision = result.owner_revision;
  }
  for (const [index, id] of lorebookIds.entries()) {
    const added = !current.includes(id);
    const result = await api.call(added ? "groupLorebooksBind" : "groupLorebooksReorder", { group_id: saved.id, lorebook_id: id, expected_revision: revision, target_ordinal: index, client_operation_id: `${attempt.operationId}:book:${index}` });
    revision = result.owner_revision;
  }
  await refreshGroups(queryClient);
  return saved.id;
}
