import { createContext, useContext } from "react";
import type { JobView } from "@/api/generated/bindings";

export interface Downloads {
  downloads: readonly JobView[];
  error: unknown;
  retry: () => void;
}

export const DownloadsContext = createContext<Downloads>({ downloads: [], error: null, retry: () => undefined });

export const useDownloads = () => useContext(DownloadsContext);
