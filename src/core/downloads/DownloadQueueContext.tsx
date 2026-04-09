import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useLocation, useNavigate } from "react-router-dom";
import { toast } from "../../ui/components/toast";
import { Routes } from "../../ui/navigation";

export interface QueuedDownload {
  id: string;
  modelId: string;
  filename: string;
  status: "queued" | "downloading" | "complete" | "error" | "cancelled";
  downloaded: number;
  total: number;
  speedBytesPerSec: number;
  error: string | null;
  resultPath: string | null;
  createModelWhenFinished: boolean;
  mmprojFile: string | false;
  installId: string | null;
  displayName: string | null;
  contextLength: number | null;
  kvType: string | null;
  downloadRole: "model" | "mmproj" | null;
}

interface DownloadQueueContextValue {
  queue: QueuedDownload[];
  /** Enqueue a new download */
  queueDownload: (modelId: string, filename: string) => Promise<void>;
  /** Cancel a queued or active download */
  cancelItem: (id: string) => Promise<void>;
  /** Dismiss a completed/failed/cancelled item from the queue */
  dismissItem: (id: string) => Promise<void>;
  /** Count of active (downloading + queued) items */
  activeCount: number;
  /** Count of completed items ready for model creation */
  completedCount: number;
  /** Whether there are any items in the queue */
  hasItems: boolean;
}

const DownloadQueueContext = createContext<DownloadQueueContextValue | null>(null);

export function useDownloadQueue(): DownloadQueueContextValue {
  const ctx = useContext(DownloadQueueContext);
  if (!ctx) {
    throw new Error("useDownloadQueue must be used within a DownloadQueueProvider");
  }
  return ctx;
}

/**
 * Safely try to use the download queue context.
 * Returns null if not within a provider (useful for optional consumers).
 */
export function useDownloadQueueOptional(): DownloadQueueContextValue | null {
  return useContext(DownloadQueueContext);
}

function extractShortName(modelId: string): string {
  const parts = modelId.split("/");
  return parts[parts.length - 1] || modelId;
}

function isMmprojFilename(filename: string): boolean {
  return filename.toLowerCase().includes("mmproj");
}

export function DownloadQueueProvider({ children }: { children: ReactNode }) {
  const [queue, setQueue] = useState<QueuedDownload[]>([]);
  const location = useLocation();
  const navigate = useNavigate();

  // Track previous queue to detect newly completed / failed items
  const prevQueueRef = useRef<QueuedDownload[]>([]);
  const locationRef = useRef(location.pathname);

  useEffect(() => {
    locationRef.current = location.pathname;
  }, [location.pathname]);

  // Listen to Rust download queue events + fetch initial state
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    // Fetch initial queue state (in case downloads were already in progress)
    invoke<QueuedDownload[]>("hf_get_download_queue")
      .then((q) => {
        setQueue(q);
        prevQueueRef.current = q;
      })
      .catch(() => {});

    listen<QueuedDownload[]>("hf_download_queue", (event) => {
      setQueue(event.payload);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // Detect state transitions and fire toasts when not on HF browser page
  useEffect(() => {
    const prev = prevQueueRef.current;
    const isOnHfPage = locationRef.current.startsWith(Routes.settingsModelsBrowse);

    for (const item of queue) {
      const prevItem = prev.find((p) => p.id === item.id);
      if (!prevItem) continue;

      // Download just completed
      if (prevItem.status !== "complete" && item.status === "complete") {
        if (!isOnHfPage) {
          const displayName = extractShortName(item.modelId).replace(/-GGUF$/i, "");
          toast.success("Download complete", `${displayName} — ${item.filename}`, {
            actionLabel: isMmprojFilename(item.filename) ? undefined : "Create Model",
            onAction: isMmprojFilename(item.filename)
              ? undefined
              : () => {
                  if (!item.resultPath) return;
                  const cleanName = extractShortName(item.modelId).replace(/-GGUF$/i, "");
                  const params = new URLSearchParams();
                  params.set("hfModelPath", item.resultPath);
                  params.set("hfModelName", item.filename);
                  params.set("hfDisplayName", cleanName);
                  navigate(`${Routes.settingsModelsNew}?${params.toString()}`);
                  // Dismiss the item after navigating
                  invoke("hf_dismiss_queue_item", { queueId: item.id }).catch(() => {});
                },
            id: `dl-complete-${item.id}`,
            duration: 10000,
          });
        }
      }

      // Download just failed
      if (prevItem.status !== "error" && item.status === "error" && !isOnHfPage) {
        toast.error("Download failed", `${item.filename}: ${item.error || "Unknown error"}`, {
          id: `dl-error-${item.id}`,
          duration: 8000,
        });
      }
    }

    prevQueueRef.current = queue;
  }, [queue, navigate]);

  const queueDownload = useCallback(async (modelId: string, filename: string) => {
    try {
      await invoke<string>("hf_queue_download", { modelId, filename, metadata: null });
    } catch (err: any) {
      toast.error(
        "Download failed",
        typeof err === "string" ? err : err?.message || "Unknown error",
      );
    }
  }, []);

  const cancelItem = useCallback(async (id: string) => {
    try {
      await invoke("hf_cancel_queue_item", { queueId: id });
    } catch {}
  }, []);

  const dismissItem = useCallback(async (id: string) => {
    try {
      await invoke("hf_dismiss_queue_item", { queueId: id });
    } catch {}
  }, []);

  const activeCount = queue.filter(
    (d) => d.status === "downloading" || d.status === "queued",
  ).length;
  const completedCount = queue.filter((d) => d.status === "complete").length;
  const hasItems = queue.length > 0;

  return (
    <DownloadQueueContext.Provider
      value={{
        queue,
        queueDownload,
        cancelItem,
        dismissItem,
        activeCount,
        completedCount,
        hasItems,
      }}
    >
      {children}
    </DownloadQueueContext.Provider>
  );
}
