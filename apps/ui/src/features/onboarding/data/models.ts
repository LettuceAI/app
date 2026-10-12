import type { QueryClient } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { ModelModality, RemoteModelContract } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { vendorOf } from "@/entities/provider";
import type { FlowState, SavedModel } from "../flow/flow";

export type ModelSort = "name" | "price" | "context";

export interface ModelFilters {
  vendors: readonly string[];
  imagesOnly: boolean;
  sort: ModelSort;
}

export const noFilters: ModelFilters = { vendors: [], imagesOnly: false, sort: "name" };

export function activeFilterCount(filters: ModelFilters): number {
  return filters.vendors.length + (filters.imagesOnly ? 1 : 0) + (filters.sort !== "name" ? 1 : 0);
}

export function modelName(model: RemoteModelContract): string {
  return model.display_name ?? model.id;
}

export function lastSegment(id: string): string {
  return id.split("/").at(-1) || id;
}

export function defaultDisplayName(model: RemoteModelContract): string {
  return model.display_name ?? lastSegment(model.id);
}

export function isFree(model: RemoteModelContract): boolean {
  return model.input_price === 0 && model.output_price === 0;
}

export function acceptsImages(model: RemoteModelContract): boolean {
  return model.input_modalities?.includes("image") ?? false;
}

export function pricePerMillion(price: number | null): number | null {
  return price === null || price < 0 ? null : price * 1_000_000;
}

export function priceOf(model: RemoteModelContract): { input: number; output: number } | null {
  const input = pricePerMillion(model.input_price);
  const output = pricePerMillion(model.output_price);
  return input === null || output === null ? null : { input, output };
}

export function vendorNames(models: readonly RemoteModelContract[]): string[] {
  const names = models.flatMap((model) => vendorOf(model.id)?.vendor ?? []);
  return [...new Set(names)];
}

function sortKey(model: RemoteModelContract, sort: ModelSort): number {
  if (sort === "price") return pricePerMillion(model.input_price) ?? Number.POSITIVE_INFINITY;
  if (sort === "context") return -(model.context_length ?? 0);
  return 0;
}

export interface ModelQuery {
  text: string;
  onlyFree: boolean;
  filters: ModelFilters;
}

export function filterModels(models: readonly RemoteModelContract[], query: ModelQuery): RemoteModelContract[] {
  const needle = query.text.trim().toLowerCase();
  const { filters } = query;
  return models
    .filter((model) => `${modelName(model)} ${model.id}`.toLowerCase().includes(needle))
    .filter((model) => !query.onlyFree || isFree(model))
    .filter((model) => !filters.imagesOnly || acceptsImages(model))
    .filter((model) => filters.vendors.length === 0 || filters.vendors.includes(vendorOf(model.id)?.vendor ?? ""))
    .toSorted((a, b) => {
      const difference = sortKey(a, filters.sort) - sortKey(b, filters.sort);
      if (difference !== 0 && !Number.isNaN(difference)) return difference;
      return modelName(a).localeCompare(modelName(b));
    });
}

const unknown = "unknown";

export const minimalModelConfig = {
  capabilities: {
    format_version: 1,
    evidence: { source: "unspecified", source_version: 0, observed_at: 0 },
    input_modalities: { text: unknown, image: unknown, audio: unknown },
    output_modalities: { text: unknown, image: unknown, audio: unknown },
    streaming: unknown,
    tools: unknown,
    structured_output: unknown,
    reasoning: unknown,
    prompt_cache: unknown,
    context_length: null,
    max_visible_output_tokens: null,
    max_total_completion_tokens: null,
    parameter_support: {
      temperature: unknown,
      top_p: unknown,
      top_k: unknown,
      frequency_penalty: unknown,
      presence_penalty: unknown,
      repetition_penalty: unknown,
    },
  },
};

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
