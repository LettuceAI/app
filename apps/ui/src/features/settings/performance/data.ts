import { queryOptions } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { LlmMetricsPage, LlmMetricView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

const PAGE_SIZE = 500;

export interface MetricSummary {
  id: string;
  createdAt: number;
  modelName: string | null;
  backend: string | null;
  gpuLayers: number | null;
  nCtx: number | null;
  nBatch: number | null;
  nUbatch: number | null;
  kvType: string | null;
  modelSizeBytes: number | null;
  promptTokens: number | null;
  completionTokens: number | null;
  totalTokens: number | null;
  ttftMs: number | null;
  decodeTokensPerSecond: number | null;
  generationElapsedMs: number | null;
  nativePromptEvalTokensPerSecond: number | null;
  nativeGenerationComputeMs: number | null;
  nativeGenerationTokensPerSecond: number | null;
  appGenerationOverheadMs: number | null;
  finishReason: string | null;
}

export interface MetricSample {
  tMs: number;
  tps: number;
  ctxFill: number;
}

const record = (value: unknown): Record<string, unknown> =>
  value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : {};
const number = (value: unknown) => (typeof value === "number" && Number.isFinite(value) ? value : null);
const text = (value: unknown) => (typeof value === "string" && value.trim() ? value : null);

export function summaryOf(view: LlmMetricView): MetricSummary {
  const summary = record(view.summary);
  return {
    id: view.id,
    createdAt: view.created_at,
    modelName: text(summary.modelName),
    backend: text(summary.backend),
    gpuLayers: number(summary.gpuLayers),
    nCtx: number(summary.nCtx),
    nBatch: number(summary.nBatch),
    nUbatch: number(summary.nUbatch),
    kvType: text(summary.kvType),
    modelSizeBytes: number(summary.modelSizeBytes),
    promptTokens: number(summary.promptTokens),
    completionTokens: number(summary.completionTokens),
    totalTokens: number(summary.totalTokens),
    ttftMs: number(summary.ttftMs),
    decodeTokensPerSecond: number(summary.decodeTokensPerSecond),
    generationElapsedMs: number(summary.generationElapsedMs),
    nativePromptEvalTokensPerSecond: number(summary.nativePromptEvalTokensPerSecond),
    nativeGenerationComputeMs: number(summary.nativeGenerationComputeMs),
    nativeGenerationTokensPerSecond: number(summary.nativeGenerationTokensPerSecond),
    appGenerationOverheadMs: number(summary.appGenerationOverheadMs),
    finishReason: text(summary.finishReason),
  };
}

export function samplesOf(view: LlmMetricView | null | undefined): MetricSample[] {
  return (view?.samples ?? []).flatMap((sample) => {
    const entry = record(sample);
    const tMs = number(entry.tMs);
    const tps = number(entry.tps);
    const ctxFill = number(entry.ctxFill);
    return tMs === null || tps === null || ctxFill === null ? [] : [{ tMs, tps, ctxFill }];
  });
}

/** Every recorded generation, newest first, read page by page. */
export function metricsListQuery(api: ApiClient) {
  return queryOptions({
    queryKey: queryKeys.llmMetrics.list(),
    queryFn: async () => {
      const items: MetricSummary[] = [];
      let cursor: string | null = null;
      do {
        const page: LlmMetricsPage = await api.call("llmMetricsList", { cursor, limit: PAGE_SIZE });
        items.push(...page.items.map(summaryOf));
        cursor = page.next_cursor;
      } while (cursor);
      return items;
    },
  });
}

export function metricDetailQuery(api: ApiClient, id: string | null) {
  return queryOptions({
    queryKey: queryKeys.llmMetrics.detail(id ?? ""),
    queryFn: () => api.call("llmMetricsGet", { id: id! }),
    enabled: id !== null,
  });
}
