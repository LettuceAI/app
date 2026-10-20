import { useCallback, useRef, useState } from "react";
import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import { toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { MemoryView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { memoryQuery } from "../data/queries";
import { useMemoryError } from "./errors";

export async function refreshMemory(queryClient: QueryClient, api: ApiClient, conversationId: string): Promise<void> {
  void queryClient.invalidateQueries({ queryKey: [...queryKeys.memory.detail(conversationId), "cycles"] });
  try {
    await queryClient.fetchQuery({ ...memoryQuery(api, conversationId), staleTime: 0 });
  } catch {
    return;
  }
}

export type MemoryEdit = (view: MemoryView, operationId: string) => Promise<unknown>;
export type MemoryWrite = (key: string, edit: MemoryEdit) => Promise<boolean>;

export function useMemoryWrite(api: ApiClient, conversationId: string): MemoryWrite {
  const queryClient = useQueryClient();
  const errorText = useMemoryError();
  const tail = useRef<Promise<unknown>>(Promise.resolve());
  const [pending] = useState(() => new Map<string, { view: MemoryView; id: string; edit: MemoryEdit }>());
  return useCallback(
    (key, edit) => {
      const run = tail.current.then(async (): Promise<boolean> => {
        let operation = pending.get(key);
        if (!operation) {
          const current = queryClient.getQueryData<MemoryView>(queryKeys.memory.detail(conversationId));
          if (!current) return false;
          operation = { view: current, id: crypto.randomUUID(), edit };
          pending.set(key, operation);
        }
        try {
          await operation.edit(operation.view, operation.id);
          pending.delete(key);
        } catch (error) {
          const conflict = isApiFailure(error) && error.code === "conflict";
          if (conflict) pending.delete(key);
          toast.error(errorText(error));
          if (conflict) await refreshMemory(queryClient, api, conversationId);
          return false;
        }
        await refreshMemory(queryClient, api, conversationId);
        return true;
      });
      tail.current = run;
      return run;
    },
    [api, conversationId, queryClient, errorText, pending],
  );
}
