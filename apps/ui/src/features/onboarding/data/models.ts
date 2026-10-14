import { minimalModelConfig } from "@/entities/model";
export { minimalModelConfig } from "@/entities/model";
import type { QueryClient } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { ModelModality } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import type { FlowState, SavedModel } from "../flow/flow";

export { activeFilterCount, defaultDisplayName, filterModels, modelName, priceOf, noFilters, acceptsImages, vendorNames, isFree, lastSegment, pricePerMillion, type ModelFilters, type ModelSort, type ModelQuery } from "@/entities/model/remote-models";
import { acceptsImages, defaultDisplayName } from "@/entities/model/remote-models";

export function externalModelId(flow: Pick<FlowState, "model" | "manualId">): string {
  return flow.model?.id ?? flow.manualId.trim();
}

export interface SaveModelInput {
  api: ApiClient;
  queryClient: QueryClient;
  flow: Pick<FlowState, "model" | "manualId" | "displayName" | "account" | "savedModel">;
}

export async function saveChosenModel({ api, queryClient, flow }: SaveModelInput): Promise<SavedModel> {
  const account = flow.account;
  if (!account) throw new Error("no connected account");
  const externalId = externalModelId(flow);
  const inputScopes: ModelModality[] = flow.model && acceptsImages(flow.model) ? ["text", "image"] : ["text"];
  const view = await api.call("modelSave", {
    model: {
      id: flow.savedModel?.id ?? null,
      provider_account_id: account.id,
      external_model_id: externalId,
      display_name: flow.displayName.trim() || (flow.model ? defaultDisplayName(flow.model) : externalId),
      kind: "chat",
      config: minimalModelConfig,
      input_scopes: inputScopes,
      output_scopes: ["text"],
      remote_metadata: flow.model,
    },
    expected_revision: flow.savedModel?.revision ?? null,
    client_operation_id: crypto.randomUUID(),
  });
  await queryClient.invalidateQueries({ queryKey: queryKeys.models.all });
  return { id: view.id, revision: view.revision };
}
