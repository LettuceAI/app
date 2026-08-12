import { invoke } from "@tauri-apps/api/core";

export interface DiscoveryCard {
  id: string;
  name: string;
  inChatName?: string;
  path?: string;
  tagline?: string;
  pageDescription?: string;
  author?: string;
  isNSFW?: boolean;
  contentWarnings?: string[];
  tags?: string[];
  views?: number;
  downloads?: number;
  messages?: number;
  createdAt?: number;
  lastUpdateAt?: number;
  likes?: number;
  dislikes?: number;
  totalTokens?: number;
  hasLorebook?: boolean;
  isOC?: boolean;
  [key: string]: unknown;
}

export interface DiscoverySearchResponse {
  hits: DiscoveryCard[];
  totalHits?: number;
  hitsPerPage?: number;
  page?: number;
  totalPages?: number;
  processingTimeMs?: number;
  query?: string;
}

export interface DiscoverySearchOptions {
  query?: string;
  page?: number;
  limit?: number;
}

export async function searchDiscoveryCards(
  options: DiscoverySearchOptions = {}
): Promise<DiscoverySearchResponse> {
  return await invoke<DiscoverySearchResponse>("discovery_search_cards", {
    query: options.query ?? null,
    page: options.page ?? null,
    limit: options.limit ?? null,
  });
}
