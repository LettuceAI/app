import type { QueryClient } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import { isApiFailure, type ApiFailureCode } from "@/api/client";
import type { ProviderDescriptorContract } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import type { FlowState, SavedAccount } from "../flow/flow";
import { accountsQuery } from "./queries";

export type ConnectError =
  | { kind: "rejected"; providerMessage: string | null }
  | { kind: "unreachable" }
  | { kind: "code"; code: ApiFailureCode };

export function classifyConnectError(error: unknown): ConnectError {
  if (!isApiFailure(error)) return { kind: "code", code: "internal" };
  const details = error.details;
  if (error.code === "invalid_input" && details?.type === "provider_verification") {
    return { kind: "rejected", providerMessage: details.provider_message };
  }
  if (error.code === "unavailable" || error.code === "transport") return { kind: "unreachable" };
  return { kind: "code", code: error.code };
}

export interface ConnectInput {
  api: ApiClient;
  queryClient: QueryClient;
  descriptor: ProviderDescriptorContract;
  flow: Pick<FlowState, "apiKey" | "label" | "baseUrl" | "account">;
  fallbackLabel: string;
}

export async function connectProvider({ api, queryClient, descriptor, flow, fallbackLabel }: ConnectInput): Promise<SavedAccount> {
  const baseUrl = flow.baseUrl.trim() || null;
  const apiKey = flow.apiKey.trim() || null;
  if (descriptor.verifies_key) {
    await api.call("providerVerify", {
      type: "draft",
      draft: { provider_kind: descriptor.kind, base_url: baseUrl, api_key: apiKey, config: {} },
    });
  }
  const accounts = await queryClient.fetchQuery({ ...accountsQuery(api), staleTime: 0 });
  const known = flow.account?.kind === descriptor.kind ? flow.account : null;
  const listed = known
    ? accounts.find((account) => account.id === known.id)
    : accounts.find((account) => account.provider_kind === descriptor.kind);
  const existing = listed ? { id: listed.id, revision: listed.revision } : known;
  const view = await api.call("providerAccountSave", {
    account: {
      id: existing?.id ?? null,
      provider_kind: descriptor.kind,
      label: flow.label.trim() || fallbackLabel,
      base_url: baseUrl,
      enabled: true,
      streaming_enabled: true,
      allow_invalid_tls: false,
      config: {},
    },
    api_key: apiKey,
    clear_api_key: false,
    expected_revision: existing?.revision ?? null,
    client_operation_id: crypto.randomUUID(),
  });
  await queryClient.invalidateQueries({ queryKey: queryKeys.providers.accounts() });
  await queryClient.invalidateQueries({ queryKey: queryKeys.providers.models(view.id) });
  return { id: view.id, revision: view.revision, kind: view.provider_kind };
}

export function canConnect(
  descriptor: ProviderDescriptorContract | null,
  flow: Pick<FlowState, "apiKey" | "baseUrl">,
): boolean {
  if (!descriptor) return false;
  if (descriptor.api_key === "required" && flow.apiKey.trim() === "") return false;
  if (descriptor.endpoint_editable && descriptor.default_endpoint === null && flow.baseUrl.trim() === "") return false;
  return true;
}
