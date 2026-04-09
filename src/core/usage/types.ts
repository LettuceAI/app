/**
 * TypeScript types for usage tracking (mirrors Rust structures)
 */

export interface RequestCost {
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
  regularPromptTokens?: number;
  cachedPromptTokens?: number;
  cacheWriteTokens?: number;
  reasoningTokens?: number;
  webSearchRequests?: number;
  promptCost: number;
  promptBaseCost?: number;
  cacheReadCost?: number;
  cacheWriteCost?: number;
  completionCost: number;
  completionBaseCost?: number;
  reasoningCost?: number;
  requestCost?: number;
  webSearchCost?: number;
  totalCost: number;
  authoritativeTotalCost?: number | null;
}

export interface RequestUsage {
  id: string;
  timestamp: number; // Unix timestamp in milliseconds
  sessionId: string;
  characterId: string;
  characterName: string;
  modelId: string;
  modelName: string;
  providerId: string;
  providerLabel: string;
  operationType: string; // Type of operation (chat, regenerate, continue, summary, memory_manager, etc.)
  finishReason?: string;

  promptTokens?: number;
  completionTokens?: number;
  totalTokens?: number;
  cachedPromptTokens?: number;
  cacheWriteTokens?: number;
  reasoningTokens?: number;
  imageTokens?: number;
  webSearchRequests?: number;
  apiCost?: number;

  // Token breakdown for prompt analysis
  memoryTokens?: number; // Tokens from memory embeddings
  summaryTokens?: number; // Tokens from memory summary

  cost?: RequestCost;

  success: boolean;
  errorMessage?: string;
  metadata?: Record<string, string>;
}

export interface UsageFilter {
  startTimestamp?: number;
  endTimestamp?: number;
  providerId?: string;
  modelId?: string;
  characterId?: string;
  sessionId?: string;
  successOnly?: boolean;
}

export interface ProviderStats {
  totalRequests: number;
  successfulRequests: number;
  failedRequests: number;
  totalTokens: number;
  totalCost: number;
  averageCostPerRequest: number;
}

export interface ModelStats {
  totalRequests: number;
  successfulRequests: number;
  failedRequests: number;
  totalTokens: number;
  totalCost: number;
  averageCostPerRequest: number;
}

export interface CharacterStats {
  totalRequests: number;
  successfulRequests: number;
  failedRequests: number;
  totalTokens: number;
  totalCost: number;
  averageCostPerRequest: number;
}

export interface UsageStats {
  totalRequests: number;
  successfulRequests: number;
  failedRequests: number;
  totalTokens: number;
  totalCost: number;
  averageCostPerRequest: number;
  byProvider: Record<string, ProviderStats>;
  byModel: Record<string, ModelStats>;
  byCharacter: Record<string, CharacterStats>;
}

export interface AppActiveUsageSummary {
  totalMs: number;
  startedAtMs?: number;
  lastUpdatedAtMs?: number;
  byDayMs: Record<string, number>;
}
