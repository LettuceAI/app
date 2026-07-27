import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useLocation, useNavigate } from "react-router-dom";
import { toast } from "../../ui/components/toast";
import { Routes } from "../../ui/navigation";
import { SETTINGS_UPDATED_EVENT } from "../storage/repo";

export interface QueuedDownload {
  id: string;
  modelId: string;
  filename: string;
  status: "queued" | "downloading" | "complete" | "error" | "cancelled";
  phase?: "verifying" | null;
  downloaded: number;
  total: number;
  speedBytesPerSec: number;
  error: string | null;
  resultPath: string | null;
  createModelWhenFinished: boolean;
  mmprojFile: string | false;
  mtpFile: string | false;
  mtpBundled?: boolean;
  installId: string | null;
  displayName: string | null;
  contextLength: number | null;
  kvType: string | null;
  llamaOffloadKqv: boolean | null;
  llamaGpuLayers: number | null;
  llamaModelOffloadMode: "auto" | "cpu" | "gpu" | "mixed" | null;
  downloadRole:
    | "model"
    | "diffusion_model"
    | "mmproj"
    | "mtp"
    | "vae"
    | "text_encoder"
    | "vision_encoder"
    | "runtime"
    | "runtime_dependency"
    | null;
  queueKind?: string | null;
  assetRoot?: string | null;
  installKind?: string | null;
  variant?: string | null;
  voiceId?: string | null;
  expectedSize?: number | null;
  sha256?: string | null;
  runtimeRelease?: string | null;
  runtimeAsset?: string | null;
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

export function isMmprojDownloadFilename(filename: string): boolean {
  return filename.toLowerCase().includes("mmproj");
}

export function isMtpDownloadFilename(filename: string): boolean {
  const base = filename.toLowerCase().split("/").pop() ?? "";
  return base.startsWith("mtp-") || base.includes("-mtp.") || base.includes("_mtp.");
}

export function isSidecarDownload(item: QueuedDownload): boolean {
  return (
    item.downloadRole === "mmproj" ||
    item.downloadRole === "mtp" ||
    isMmprojDownloadFilename(item.filename) ||
    isMtpDownloadFilename(item.filename)
  );
}

export function isCreateableModelDownload(item: QueuedDownload): boolean {
  return (
    item.queueKind !== "kokoro" &&
    item.queueKind !== "whisper" &&
    item.queueKind !== "sd" &&
    item.queueKind !== "sdcpp" &&
    item.queueKind !== "civitai_lora" &&
    !isSidecarDownload(item)
  );
}

export interface DownloadGroup {
  installId: string;
  model: QueuedDownload | null;
  items: QueuedDownload[];
}

export function groupQueueDownloads(queue: QueuedDownload[]): {
  groups: DownloadGroup[];
  singles: QueuedDownload[];
} {
  const byInstall = new Map<string, QueuedDownload[]>();
  const singles: QueuedDownload[] = [];
  for (const item of queue) {
    if (item.installId) {
      const list = byInstall.get(item.installId) ?? [];
      list.push(item);
      byInstall.set(item.installId, list);
    } else {
      singles.push(item);
    }
  }
  const groups: DownloadGroup[] = [];
  for (const [installId, items] of byInstall) {
    if (items.length < 2) {
      singles.push(...items);
      continue;
    }
    const model =
      items.find((item) => item.downloadRole === "model") ??
      items.find((item) => item.downloadRole === "diffusion_model") ??
      items.find((item) => !isSidecarDownload(item)) ??
      null;
    groups.push({ installId, model, items });
  }
  return { groups, singles };
}

async function registerCompletedSdDownload(item: QueuedDownload): Promise<void> {
  if (item.queueKind === "sdcpp") {
    window.dispatchEvent(new CustomEvent(SETTINGS_UPDATED_EVENT));
  }
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

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    void listen(SETTINGS_UPDATED_EVENT, () => {
      window.dispatchEvent(new CustomEvent(SETTINGS_UPDATED_EVENT));
    }).then((dispose) => {
      unlisten = dispose;
    });
    return () => unlisten?.();
  }, []);

  // Detect state transitions and fire toasts when not on HF browser page
  useEffect(() => {
    const prev = prevQueueRef.current;
    const isOnHfPage = locationRef.current.startsWith(Routes.settingsModelsBrowse);
    const isOnLoraPage = locationRef.current.startsWith(Routes.settingsModelsLoras);

    for (const item of queue) {
      const prevItem = prev.find((p) => p.id === item.id);
      if (!prevItem) continue;

      // Download just completed
      if (prevItem.status !== "complete" && item.status === "complete") {
        void registerCompletedSdDownload(item);
        const suppressLoraToast = item.queueKind === "civitai_lora" && isOnLoraPage;
        if (
          !isOnHfPage &&
          !suppressLoraToast &&
          item.queueKind !== "sd" &&
          item.queueKind !== "sdcpp"
        ) {
          const displayName =
            item.queueKind === "kokoro" || item.queueKind === "civitai_lora"
              ? item.displayName || item.filename
              : extractShortName(item.modelId).replace(/-GGUF$/i, "");
          toast.success("Download complete", `${displayName} — ${item.filename}`, {
            actionLabel: isCreateableModelDownload(item) ? "Create Model" : undefined,
            onAction: isCreateableModelDownload(item)
              ? () => {
                  if (!item.resultPath) return;
                  const cleanName = extractShortName(item.modelId).replace(/-GGUF$/i, "");
                  const params = new URLSearchParams();
                  params.set("hfModelPath", item.resultPath);
                  params.set("hfModelName", item.filename);
                  params.set("hfDisplayName", cleanName);
                  navigate(`${Routes.settingsModelsNew}?${params.toString()}`);
                  // Dismiss the item after navigating
                  invoke("hf_dismiss_queue_item", { queueId: item.id }).catch(() => {});
                }
              : undefined,
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
