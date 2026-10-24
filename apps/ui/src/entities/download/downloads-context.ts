import { createContext, useContext } from "react";
import { downloadProgressOf, terminalJobStates } from "@/entities/job";
import type { JobView } from "@/api/generated/bindings";

export interface Downloads {
  downloads: readonly JobView[];
  error: unknown;
  retry: () => void;
}

export const DownloadsContext = createContext<Downloads>({ downloads: [], error: null, retry: () => undefined });

export const useDownloads = () => useContext(DownloadsContext);

export function useDownloadProgress(job: JobView | null, id: string | undefined = job?.id) {
  const { downloads } = useDownloads();
  const shared = downloads.find((value) => value.id === id);
  return downloadProgressOf(job && terminalJobStates.has(job.state) ? job : shared ?? job);
}
