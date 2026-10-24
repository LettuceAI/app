import { createContext, useContext } from "react";
import { downloadProgressOf, terminalJobStates } from "@/entities/job";
import type { JobView } from "@/api/generated/bindings";

export interface Downloads {
  downloads: readonly JobView[];
  jobs: ReadonlyMap<string, JobView>;
  ready: boolean;
  cancel: ((id: string) => Promise<void>) | null;
  cancelling: ReadonlySet<string>;
  error: unknown;
  retry: () => void;
}

export const DownloadsContext = createContext<Downloads>({ downloads: [], jobs: new Map(), ready: false, cancel: null, cancelling: new Set(), error: null, retry: () => undefined });

export const useDownloads = () => useContext(DownloadsContext);

export function useDownloadView(job: JobView | null, id: string | undefined = job?.id) {
  const { jobs } = useDownloads();
  const shared = id ? jobs.get(id) : undefined;
  return job && terminalJobStates.has(job.state) ? job : shared ?? job;
}

export function useDownloadProgress(job: JobView | null, id: string | undefined = job?.id) {
  return downloadProgressOf(useDownloadView(job, id));
}
