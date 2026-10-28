import type { ApiClient } from "@/api/client";
import type { MemoryItemView } from "@/api/generated/bindings";
import type { MemoryDraft } from "./MemoryItems";
import type { MemoryWrite } from "./use-memory-write";

export async function saveMemoryDraft(api: ApiClient, conversationId: string, write: MemoryWrite, item: MemoryItemView | null, draft: MemoryDraft, session: number): Promise<boolean> {
  const observed = draft.observedAt?.getTime() ?? null;
  let ok = true;
  if (item === null) {
    const payload = { text: draft.text, category: draft.category, observed_at: observed };
    ok = await write(`add:${session}:${JSON.stringify(payload)}`, (current, operationId) =>
      api.call("memoryAdd", { conversation_id: conversationId, ...payload, expected_revision: current.revision, client_operation_id: operationId }),
    );
  } else {
    const textChanged = draft.text !== item.text;
    const categoryChanged = draft.category !== item.category;
    const dateChanged = observed !== item.observed_at;
    if (textChanged || categoryChanged || dateChanged) {
      const key = `update:${session}:${item.id}:${JSON.stringify([textChanged ? draft.text : null, categoryChanged ? draft.category : "keep", dateChanged ? observed : "keep"])}`;
      ok = await write(key, async (current, operationId) => {
        const stored = current.items.find((entry) => entry.id === item.id);
        const updateText = textChanged && draft.text !== stored?.text;
        const updateCategory = categoryChanged && draft.category !== stored?.category;
        const updateDate = dateChanged && observed !== stored?.observed_at;
        if (stored && !updateText && !updateCategory && !updateDate) return;
        return api.call("memoryUpdate", {
          conversation_id: conversationId,
          memory_id: item.id,
          text: updateText ? draft.text : null,
          category: updateCategory ? { type: "set", value: draft.category } : { type: "keep" },
          observed_at: updateDate ? { type: "set", value: observed } : { type: "keep" },
          expected_revision: current.revision,
          client_operation_id: operationId,
        });
      });
    }
    const setPinned = () => write(`pin:${session}:${item.id}:${draft.pinned}`, async (current, operationId) => {
      if (current.items.some((entry) => entry.id === item.id && entry.pinned === draft.pinned)) return;
      return api.call("memoryPin", {
        conversation_id: conversationId,
        memory_id: item.id,
        pinned: draft.pinned,
        expected_revision: current.revision,
        client_operation_id: operationId,
      });
    });
    if (ok && !draft.pinned && item.pinned) ok = await setPinned();
    if (ok && draft.temperature !== item.temperature) {
      ok = await write(`temperature:${session}:${item.id}:${draft.temperature}`, async (current, operationId) => {
        if (current.items.some((entry) => entry.id === item.id && entry.temperature === draft.temperature)) return;
        return api.call("memorySetTemperature", {
          conversation_id: conversationId,
          memory_id: item.id,
          temperature: draft.temperature,
          expected_revision: current.revision,
          client_operation_id: operationId,
        });
      });
    }
    if (ok && draft.pinned && !item.pinned) ok = await setPinned();
  }
  return ok;
}
