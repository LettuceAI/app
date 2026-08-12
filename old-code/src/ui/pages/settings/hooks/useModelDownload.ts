import { useCallback, useEffect, useRef, useState } from "react";
import { storageBridge } from "../../../../core/storage/files";

export type DownloadProgress = {
  downloaded: number;
  total: number;
  status: string;
  currentFileIndex: number;
  totalFiles: number;
  currentFileName: string;
};

const IDLE_PROGRESS: DownloadProgress = {
  downloaded: 0,
  total: 0,
  status: "idle",
  currentFileIndex: 0,
  totalFiles: 0,
  currentFileName: "",
};

export type UseModelDownloadOptions = {
  /** Called when a download finishes successfully. */
  onComplete?: () => void | Promise<void>;
  /** Called when a download fails or is cancelled (for state cleanup). */
  onFailure?: (status: "failed" | "cancelled") => void | Promise<void>;
};

export type UseModelDownloadResult = {
  progress: DownloadProgress;
  isDownloading: boolean;
  /** True once a download has been observed in-flight or completed in this hook's lifetime. */
  attached: boolean;
  error: string | null;
  setError: (err: string | null) => void;
  /** Start a download by invoking the supplied bridge call. */
  start: (begin: () => Promise<void>) => Promise<void>;
  cancel: () => Promise<void>;
};

/**
 * Shared download lifecycle. Tracks the global embedding/companion download progress slot,
 * exposes start/cancel helpers, and fires lifecycle callbacks. The actual command to invoke is
 * passed to `start()` so this hook can drive both the embedding download and any companion
 * download with the same plumbing.
 */
export function useModelDownload(options: UseModelDownloadOptions = {}): UseModelDownloadResult {
  const { onComplete, onFailure } = options;
  const [progress, setProgress] = useState<DownloadProgress>(IDLE_PROGRESS);
  const [isDownloading, setIsDownloading] = useState(false);
  const [attached, setAttached] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const onCompleteRef = useRef(onComplete);
  const onFailureRef = useRef(onFailure);
  useEffect(() => {
    onCompleteRef.current = onComplete;
    onFailureRef.current = onFailure;
  }, [onComplete, onFailure]);

  useEffect(() => {
    let mounted = true;
    let unsubscribe: (() => void) | null = null;

    (async () => {
      try {
        const current = await storageBridge.getEmbeddingDownloadProgress();
        if (mounted && current.status === "downloading") {
          setProgress(current);
          setIsDownloading(true);
          setAttached(true);
        }

        unsubscribe = await storageBridge.listenToEmbeddingDownloadProgress((data) => {
          if (!mounted) return;
          setProgress(data);
          if (data.status === "downloading") {
            setIsDownloading(true);
            setAttached(true);
          } else if (data.status === "completed") {
            setIsDownloading(false);
            setAttached(true);
            void onCompleteRef.current?.();
          } else if (data.status === "failed" || data.status === "cancelled") {
            setIsDownloading(false);
            void onFailureRef.current?.(data.status);
          }
        });
      } catch (err) {
        console.error("Failed to setup download listener:", err);
      }
    })();

    return () => {
      mounted = false;
      if (unsubscribe) unsubscribe();
    };
  }, []);

  const start = useCallback(async (begin: () => Promise<void>) => {
    setError(null);
    setIsDownloading(true);
    setAttached(true);
    try {
      await begin();
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      if (message.includes("Download already in progress")) {
        // race — another caller started it; we'll just attach via the progress listener.
        return;
      }
      setError(message);
      setIsDownloading(false);
      throw err;
    }
  }, []);

  const cancel = useCallback(async () => {
    try {
      await storageBridge.cancelEmbeddingDownload();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      throw err;
    }
  }, []);

  return { progress, isDownloading, attached, error, setError, start, cancel };
}
