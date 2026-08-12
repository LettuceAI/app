import { storageBridge } from "./files";
import {
  LlmMetricSummarySchema,
  LlmMetricDetailSchema,
  type LlmMetricSummary,
  type LlmMetricDetail,
} from "./schemas";

export async function listLlmMetrics(limit?: number): Promise<LlmMetricSummary[]> {
  const raw = await storageBridge.llmMetricsList(limit);
  if (!Array.isArray(raw)) return [];
  const out: LlmMetricSummary[] = [];
  for (const item of raw) {
    const parsed = LlmMetricSummarySchema.safeParse(item);
    if (parsed.success) out.push(parsed.data);
  }
  return out;
}

export async function getLlmMetric(id: string): Promise<LlmMetricDetail | null> {
  const raw = await storageBridge.llmMetricsGet(id);
  if (raw == null) return null;
  const parsed = LlmMetricDetailSchema.safeParse(raw);
  return parsed.success ? parsed.data : null;
}

export async function getLlmMetricByMessage(messageId: string): Promise<LlmMetricDetail | null> {
  const raw = await storageBridge.llmMetricsGetByMessage(messageId);
  if (raw == null) return null;
  const parsed = LlmMetricDetailSchema.safeParse(raw);
  return parsed.success ? parsed.data : null;
}

export async function attachLlmMetricMessage(id: string, messageId: string): Promise<void> {
  try {
    await storageBridge.llmMetricsAttachMessage(id, messageId);
  } catch {
    // best-effort: remote generations have no metric row to attach to
  }
}

export async function hasLlmMetrics(): Promise<boolean> {
  const list = await listLlmMetrics(1);
  return list.length > 0;
}

export async function clearLlmMetrics(): Promise<void> {
  await storageBridge.llmMetricsClear();
}
