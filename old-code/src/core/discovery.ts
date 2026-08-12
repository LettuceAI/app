import { invoke } from "@tauri-apps/api/core";

// Types matching Rust structs from discovery.rs
export interface DiscoveryCard {
  id: string;
  name: string;
  inChatName?: string;
  path?: string;
  tagline?: string;
  pageDescription?: string;
  author?: string;
  isNsfw?: boolean;
  contentWarnings: string[];
  tags: string[];
  views?: number;
  downloads?: number;
  messages?: number;
  createdAt?: number;
  lastUpdateAt?: number;
  likes?: number;
  dislikes?: number;
  totalTokens?: number;
  hasLorebook?: boolean;
  isOc?: boolean;
}

export interface DiscoverySections {
  newest: DiscoveryCard[];
  popular: DiscoveryCard[];
  trending: DiscoveryCard[];
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

export interface DiscoveryCardDetail {
  id: string;
  origin?: string;
  name: string;
  inChatName?: string;
  author?: unknown;
  path: string;
  tagline?: string;
  description?: string;
  isNsfw?: boolean;
  createdAt?: string;
  hasExpressionPack?: boolean;
  lastUpdatedAt?: string;
  visibility?: string;
  lorebookId?: string | number;
  definitionScenario?: string;
  definitionPersonality?: string;
  definitionCharacterDescription?: string;
  definitionFirstMessage?: string;
  definitionExampleMessages?: string;
  definitionSystemPrompt?: string;
  definitionPostHistoryPrompt?: string;
  tokenTotal?: number;
  tokenDescription?: number;
  tokenPersonality?: number;
  tokenScenario?: number;
  tokenMesExample?: number;
  tokenFirstMes?: number;
  tokenSystemPrompt?: number;
  tokenPostHistoryInstructions?: number;
  analyticsViews?: number;
  analyticsDownloads?: number;
  analyticsMessages?: number;
  isOc?: boolean;
}

export interface DiscoveryCardDetailResponse {
  card: DiscoveryCardDetail;
  ownerCtId?: string;
}

export type SortOption =
  | "created"
  | "updated"
  | "likes"
  | "downloads"
  | "messages"
  | "views"
  | "name";
export type CardType = "newest" | "popular" | "trending";

/**
 * Fetch all discovery sections (newest, popular, trending)
 */
export async function fetchDiscoverySections(
  sortBy?: SortOption,
  descending?: boolean,
): Promise<DiscoverySections> {
  return invoke<DiscoverySections>("discovery_fetch_sections", {
    sortBy: sortBy ?? null,
    descending: descending ?? null,
  });
}

/**
 * Fetch cards by type (newest, popular, trending)
 */
export async function fetchDiscoveryCards(
  cardType: CardType,
  sortBy?: SortOption,
  descending?: boolean,
): Promise<DiscoveryCard[]> {
  return invoke<DiscoveryCard[]>("discovery_fetch_cards", {
    cardType,
    sortBy: sortBy ?? null,
    descending: descending ?? null,
  });
}

/**
 * Search discovery cards
 */
export async function searchDiscoveryCards(
  query?: string,
  page?: number,
  limit?: number,
): Promise<DiscoverySearchResponse> {
  return invoke<DiscoverySearchResponse>("discovery_search_cards", {
    query: query ?? null,
    page: page ?? null,
    limit: limit ?? null,
  });
}

/**
 * Fetch detailed card information
 */
export async function fetchCardDetail(path: string): Promise<DiscoveryCardDetailResponse> {
  return invoke<DiscoveryCardDetailResponse>("discovery_fetch_card_detail", {
    path,
  });
}

/**
 * Get optimized card image URL
 */
export async function getCardImageUrl(
  path: string,
  format?: string,
  width?: number,
  quality?: number,
): Promise<string> {
  return invoke<string>("get_card_image", {
    path,
    format: format ?? null,
    width: width ?? null,
    quality: quality ?? null,
  });
}

/**
 * Fetch alternate greetings for a character
 */
export async function fetchAlternateGreetings(cardId: string): Promise<string[]> {
  return invoke<string[]>("discovery_fetch_alternate_greetings", {
    cardId,
  });
}

/**
 * Fetch tags for a character
 */
export async function fetchTags(cardId: string): Promise<string[]> {
  return invoke<string[]>("discovery_fetch_tags", {
    cardId,
  });
}

export interface AuthorInfo {
  displayName: string;
  avatarUrl?: string;
  followersCount?: number;
}

/**
 * Fetch author information
 */
export async function fetchAuthorInfo(authorName: string): Promise<AuthorInfo> {
  return invoke<AuthorInfo>("discovery_fetch_author_info", {
    authorName,
  });
}

/**
 * Import a discovery character (downloads avatar, fetches alternate greetings, creates Character)
 * Returns the created character ID
 */
export async function importCharacter(path: string): Promise<string> {
  return invoke<string>("discovery_import_character", {
    path,
  });
}

/**
 * Format number with K/M suffix for display
 */
export function formatCount(num?: number): string {
  if (num === undefined || num === null) return "0";
  if (num >= 1_000_000) {
    return `${(num / 1_000_000).toFixed(1)}M`;
  }
  if (num >= 1_000) {
    return `${(num / 1_000).toFixed(1)}K`;
  }
  return num.toString();
}

/**
 * Format relative time from timestamp
 */
export function formatRelativeTime(timestamp?: number): string {
  if (!timestamp) return "";

  const now = Date.now();
  const diff = now - timestamp * 1000; // Convert from seconds to ms if needed

  const seconds = Math.floor(diff / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);
  const months = Math.floor(days / 30);
  const years = Math.floor(days / 365);

  if (years > 0) return `${years}y ago`;
  if (months > 0) return `${months}mo ago`;
  if (days > 0) return `${days}d ago`;
  if (hours > 0) return `${hours}h ago`;
  if (minutes > 0) return `${minutes}m ago`;
  return "Just now";
}

/**
 * Get author display name from DiscoveryCardDetail
 */
function getAuthorFromPath(path?: string): string | null {
  if (!path) return null;

  const [author] = path.split("/").filter(Boolean);
  if (!author) return null;

  try {
    return decodeURIComponent(author);
  } catch {
    return author;
  }
}

export function getAuthorName(author: unknown, path?: string): string {
  const fallback = getAuthorFromPath(path);
  if (!author) return fallback ?? "Anonymous";
  if (typeof author === "string") return author;
  if (typeof author === "number") return fallback ?? "Anonymous";
  if (typeof author === "object" && author !== null) {
    const obj = author as Record<string, unknown>;
    if (typeof obj.name === "string") return obj.name;
    if (typeof obj.username === "string") return obj.username;
  }
  return fallback ?? "Anonymous";
}
