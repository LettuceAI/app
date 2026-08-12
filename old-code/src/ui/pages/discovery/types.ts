import { z } from "zod";

// Discovery Card from the API
export const DiscoveryCardSchema = z.object({
  id: z.string(),
  name: z.string(),
  inChatName: z.string().optional().nullable(),
  path: z.string().optional().nullable(),
  tagline: z.string().optional().nullable(),
  pageDescription: z.string().optional().nullable(),
  author: z.string().optional().nullable(),
  isNsfw: z.boolean().optional().nullable(),
  contentWarnings: z.array(z.string()).default([]),
  tags: z.array(z.string()).default([]),
  views: z.number().optional().nullable(),
  downloads: z.number().optional().nullable(),
  messages: z.number().optional().nullable(),
  createdAt: z.number().optional().nullable(),
  lastUpdateAt: z.number().optional().nullable(),
  likes: z.number().optional().nullable(),
  dislikes: z.number().optional().nullable(),
  totalTokens: z.number().optional().nullable(),
  hasLorebook: z.boolean().optional().nullable(),
  isOc: z.boolean().optional().nullable(),
});

export type DiscoveryCard = z.infer<typeof DiscoveryCardSchema>;

// Discovery Sections (homepage data)
export const DiscoverySectionsSchema = z.object({
  newest: z.array(DiscoveryCardSchema),
  popular: z.array(DiscoveryCardSchema),
  trending: z.array(DiscoveryCardSchema),
});

export type DiscoverySections = z.infer<typeof DiscoverySectionsSchema>;

// Discovery Search Response
export const DiscoverySearchResponseSchema = z.object({
  hits: z.array(DiscoveryCardSchema),
  totalHits: z.number().optional().nullable(),
  hitsPerPage: z.number().optional().nullable(),
  page: z.number().optional().nullable(),
  totalPages: z.number().optional().nullable(),
  processingTimeMs: z.number().optional().nullable(),
  query: z.string().optional().nullable(),
});

export type DiscoverySearchResponse = z.infer<typeof DiscoverySearchResponseSchema>;

// Card Detail (full card info)
export const DiscoveryCardDetailSchema = z.object({
  id: z.string(),
  origin: z.string().optional().nullable(),
  name: z.string(),
  inChatName: z.string().optional().nullable(),
  author: z.unknown().optional().nullable(), // Can be string or object
  path: z.string(),
  tagline: z.string().optional().nullable(),
  description: z.string().optional().nullable(),
  isNsfw: z.boolean().optional().nullable(),
  createdAt: z.string().optional().nullable(),
  hasExpressionPack: z.boolean().optional().nullable(),
  lastUpdatedAt: z.string().optional().nullable(),
  visibility: z.string().optional().nullable(),
  lorebookId: z.union([z.string(), z.number()]).optional().nullable(),
  definitionScenario: z.string().optional().nullable(),
  definitionPersonality: z.string().optional().nullable(),
  definitionCharacterDescription: z.string().optional().nullable(),
  definitionFirstMessage: z.string().optional().nullable(),
  definitionExampleMessages: z.string().optional().nullable(),
  definitionSystemPrompt: z.string().optional().nullable(),
  definitionPostHistoryPrompt: z.string().optional().nullable(),
  tokenTotal: z.number().optional().nullable(),
  tokenDescription: z.number().optional().nullable(),
  tokenPersonality: z.number().optional().nullable(),
  tokenScenario: z.number().optional().nullable(),
  tokenMesExample: z.number().optional().nullable(),
  tokenFirstMes: z.number().optional().nullable(),
  tokenSystemPrompt: z.number().optional().nullable(),
  tokenPostHistoryInstructions: z.number().optional().nullable(),
  analyticsViews: z.number().optional().nullable(),
  analyticsDownloads: z.number().optional().nullable(),
  analyticsMessages: z.number().optional().nullable(),
  isOc: z.boolean().optional().nullable(),
});

export type DiscoveryCardDetail = z.infer<typeof DiscoveryCardDetailSchema>;

// Card Detail Response
export const DiscoveryCardDetailResponseSchema = z.object({
  card: DiscoveryCardDetailSchema,
  ownerCtId: z.string().optional().nullable(),
});

export type DiscoveryCardDetailResponse = z.infer<typeof DiscoveryCardDetailResponseSchema>;

// Sort options for discovery
export type DiscoverySortKey =
  | "created"
  | "updated"
  | "likes"
  | "downloads"
  | "messages"
  | "views"
  | "name";

export type DiscoverySection = "newest" | "popular" | "trending";

// UI State types
export interface DiscoveryFilters {
  showNsfw: boolean;
  sortBy?: DiscoverySortKey;
  descending?: boolean;
}
