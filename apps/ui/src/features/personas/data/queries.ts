import { queryOptions } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import { loadPersona, readDefaultSlot } from "./actions";

/** The persona as the edit page loads it: the stored view, its lorebooks and the default slot. */
export function personaEditQuery(api: ApiClient, personaId: string) {
  return queryOptions({
    queryKey: ["persona-edit", personaId] as const,
    queryFn: async () => {
      const [loaded, defaultSlot] = await Promise.all([loadPersona(api, personaId), readDefaultSlot(api)]);
      return { ...loaded, defaultSlot };
    },
    staleTime: Infinity,
    gcTime: 0,
    refetchOnWindowFocus: false,
  });
}
