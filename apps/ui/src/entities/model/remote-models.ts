import type { RemoteModelContract } from "@/api/generated/bindings";
import { defaultModelName, vendorOf } from "@/entities/provider";

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
  return defaultModelName(model.display_name, model.id);
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
