import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import { accountsQuery, modelsQuery } from "./queries";

export interface ChatModel {
  id: string;
  name: string;
  local: boolean;
  provider: string;
  externalId: string;
  providerKind: string | null;
}

const localProviderKind = "llamacpp";

/** The chat-capable models with the provider each runs on; `local` models run on this device. */
export function useChatModels(api: ApiClient): { models: ChatModel[]; find: (id: string | null) => ChatModel | null } {
  const models = useQuery(modelsQuery(api));
  const accounts = useQuery(accountsQuery(api));
  return useMemo(() => {
    const byAccount = new Map((accounts.data ?? []).map((account) => [account.id, account]));
    const list = (models.data?.models ?? [])
      .filter((model) => model.kind === "chat")
      .map((model): ChatModel => {
        const account = byAccount.get(model.provider_account_id);
        return { id: model.id, name: model.display_name, local: account?.provider_kind === localProviderKind, provider: account?.label ?? "", externalId: model.external_model_id, providerKind: account?.provider_kind ?? null };
      });
    const byId = new Map(list.map((model) => [model.id, model]));
    return { models: list, find: (id) => (id ? (byId.get(id) ?? null) : null) };
  }, [models.data, accounts.data]);
}
