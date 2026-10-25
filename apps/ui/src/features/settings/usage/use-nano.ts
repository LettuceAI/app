import { useEffect, useRef } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { ProviderAccountView } from "@/api/generated/bindings";
import { useSettingsApi } from "../data";
import { nanoQuery } from "./data";

export function useNano(account: ProviderAccountView | undefined, visible: boolean) {
  const api = useSettingsApi();
  const client = useQueryClient();
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);
  const id = account?.id ?? "";
  const ready = Boolean(account?.enabled && account.api_key_set);
  const query = useQuery({ ...nanoQuery(api, id), enabled: visible && ready });
  const action = useMutation({
    mutationFn: (accountId: string) =>
      api.call("providerNanogptUsage", { account_id: accountId, refresh: true }),
    onSuccess: (value, accountId) => {
      if (alive.current) client.setQueryData(nanoQuery(api, accountId).queryKey, value);
    },
  });
  const busy = action.isPending && action.variables === id;
  return {
    query,
    ready,
    busy,
    error: action.variables === id ? action.error : null,
    refresh: () => {
      if (ready && !busy) action.mutate(id);
    },
  };
}
