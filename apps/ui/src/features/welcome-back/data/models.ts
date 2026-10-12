import { useCallback, useEffect, useRef, useState } from "react";
import type { ApiClient } from "@/api/client";
import type { ConversationPage, JobView, RequiredModel } from "@/api/generated/bindings";
import { downloadPhaseOf, terminalJobStates, watchJob, type DownloadPhase } from "@/entities/job";

const modelOrder: readonly RequiredModel[] = ["embedding", "emotion"];

async function missingAcrossChats(api: ApiClient): Promise<Set<RequiredModel>> {
  const missing = new Set<RequiredModel>();
  let cursor: string | null = null;
  do {
    const page: ConversationPage = await api.call("conversationsList", {
      kind: null,
      character_id: null,
      source_group_id: null,
      lifecycle: "all",
      cursor,
      limit: null,
    });
    for (const item of page.items) for (const model of item.missing_models) missing.add(model);
    cursor = page.next_cursor;
  } while (cursor !== null && missing.size < modelOrder.length);
  return missing;
}

/**
 * The models the moved data needs on this device. Eidos is offered whenever dynamic memory is on or a chat needs an
 * embedding model and Eidos is not installed, so an adopted older embedding model is never kept in use; Thymos when a
 * chat needs it.
 */
export async function modelsNeededAfterMove(api: ApiClient): Promise<RequiredModel[]> {
  const [missing, settings, embeddings] = await Promise.all([missingAcrossChats(api), api.call("settingsGet"), api.call("embeddingStatus")]);
  const eidos = embeddings.some((model) => model.family === "lettuce_eidos_v5");
  const needed = new Set<RequiredModel>();
  if (!eidos && (settings.global.dynamic_memory.enabled || missing.has("embedding") || missing.has("emotion"))) needed.add("embedding");
  if (missing.has("emotion")) needed.add("emotion");
  return modelOrder.filter((model) => needed.has(model));
}

export interface ModelInstall {
  model: RequiredModel;
  phase: "queued" | DownloadPhase;
  job: JobView | null;
}

function begin(api: ApiClient, model: RequiredModel) {
  return model === "embedding" ? api.call("embeddingInstall", { family: "lettuce_eidos_v5" }) : api.call("companionEmotionInstall");
}

/** Installs the given models one after another, the way their download jobs report. */
export function useModelInstalls(api: ApiClient, models: readonly RequiredModel[]) {
  const [progress, setProgress] = useState<Partial<Record<RequiredModel, Omit<ModelInstall, "model">>>>({});
  const [running, setRunning] = useState(false);
  const abort = useRef<AbortController | null>(null);
  useEffect(() => () => abort.current?.abort(), []);
  const installs: ModelInstall[] = models.map((model) => ({ model, ...(progress[model] ?? { phase: "queued", job: null }) }));

  const patch = (model: RequiredModel, next: Omit<ModelInstall, "model" | "job"> & { job?: JobView | null }) =>
    setProgress((all) => ({ ...all, [model]: { job: all[model]?.job ?? null, ...next } }));

  const start = useCallback(async () => {
    abort.current?.abort();
    const controller = new AbortController();
    abort.current = controller;
    setRunning(true);
    for (const model of models) {
      if (controller.signal.aborted) return;
      if (progress[model]?.phase === "done") continue;
      patch(model, { phase: "downloading", job: null });
      try {
        const accepted = await begin(api, model);
        const finished = await watchJob(
          api,
          accepted.job_id,
          (job) => {
            if (!terminalJobStates.has(job.state)) patch(model, { phase: downloadPhaseOf(job), job });
          },
          controller.signal,
        );
        if (controller.signal.aborted) return;
        patch(model, { phase: downloadPhaseOf(finished), job: finished });
      } catch {
        if (controller.signal.aborted) return;
        patch(model, { phase: "failed" });
      }
    }
    setRunning(false);
  }, [api, models, progress]);

  return { installs, running, start };
}
