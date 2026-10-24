import { useQueryClient } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { JobView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { DownloadJob } from "@/entities/download";
import { modelName } from "./data";

export function SpeechDownload({ api, id, modelId, onUpdate }: { api: ApiClient; id: string; modelId: string; onUpdate: (job: JobView) => void }) {
  const client = useQueryClient();
  return <DownloadJob api={api} id={id} name={modelName(modelId)} onUpdate={onUpdate} onDone={() => client.invalidateQueries({ queryKey: queryKeys.speech.all })} />;
}
