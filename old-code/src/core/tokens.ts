import { invoke } from "@tauri-apps/api/core";

export async function countTokensBatch(texts: string[]): Promise<number[]> {
  if (texts.length === 0) return [];
  return invoke<number[]>("tokens_count_batch", { texts });
}
