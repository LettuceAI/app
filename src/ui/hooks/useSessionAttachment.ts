import { useEffect, useMemo, useState, useRef } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";

function withCacheBust(url: string, token: string | number): string {
  const separator = url.includes("?") ? "&" : "?";
  return `${url}${separator}v=${encodeURIComponent(String(token))}`;
}

/**
 * Hook to load attachment data from storage when only storagePath is available.
 * Returns a displayable URL for the image, loading it from disk if needed.
 *
 * @param data - An already-displayable image URL (if already loaded)
 * @param storagePath - The relative storage path to load from disk
 * @returns The image URL for display, or undefined if loading/not available
 */
export function useSessionAttachment(
  data: string | undefined | null,
  storagePath: string | undefined | null,
): string | undefined {
  const [loadedData, setLoadedData] = useState<string | undefined>(undefined);
  const lastPathRef = useRef<string | null | undefined>(undefined);
  const loadingRef = useRef<boolean>(false);
  const requestVersionRef = useRef<number>(0);

  useEffect(() => {
    const requestVersion = ++requestVersionRef.current;

    if (data) {
      setLoadedData(data);
      return;
    }

    if (!storagePath) {
      setLoadedData(undefined);
      return;
    }

    if (lastPathRef.current === storagePath && loadedData !== undefined) {
      return;
    }

    // Avoid concurrent loads
    if (loadingRef.current) {
      return;
    }

    loadingRef.current = true;
    lastPathRef.current = storagePath;

    invoke<string>("storage_get_session_attachment_path", { storagePath })
      .then((filePath) => {
        if (requestVersionRef.current !== requestVersion) {
          return;
        }
        setLoadedData(withCacheBust(convertFileSrc(filePath), requestVersion));
        loadingRef.current = false;
      })
      .catch((err) => {
        if (requestVersionRef.current !== requestVersion) {
          return;
        }
        console.error("[useSessionAttachment] Failed to load attachment:", storagePath, err);
        setLoadedData(undefined);
        loadingRef.current = false;
      });
  }, [data, storagePath, loadedData]);

  return loadedData;
}

/**
 * Interface for attachment with optional data/storagePath
 */
interface LazyAttachment {
  id: string;
  data?: string | null;
  storagePath?: string | null;
  mimeType?: string;
  filename?: string | null;
  width?: number | null;
  height?: number | null;
}

interface AttachmentCacheEntry {
  data: string;
  storagePath?: string | null;
}

const MAX_ATTACHMENT_CACHE_ENTRIES = 300;

function pruneCacheMap<T>(map: Map<string, T>, validIds: Set<string>) {
  for (const key of map.keys()) {
    if (!validIds.has(key)) {
      map.delete(key);
    }
  }
}

function trimCacheToLimit<T>(map: Map<string, T>, limit: number) {
  if (map.size <= limit) return;
  const removeCount = map.size - limit;
  let removed = 0;
  for (const key of map.keys()) {
    map.delete(key);
    removed += 1;
    if (removed >= removeCount) break;
  }
}

/**
 * Hook to load multiple attachments with lazy loading support.
 * Returns attachments with their data loaded from storage if needed.
 */
export function useSessionAttachments(
  attachments: LazyAttachment[] | undefined | null,
): LazyAttachment[] {
  const [loadedAttachments, setLoadedAttachments] = useState<LazyAttachment[]>([]);
  const dataMapRef = useRef<Map<string, AttachmentCacheEntry>>(new Map());
  const requestVersionRef = useRef<number>(0);
  const attachmentsKey = useMemo(
    () =>
      (attachments ?? [])
        .map(
          (att) =>
            `${att.id}:${att.storagePath ?? ""}:${att.data ? att.data.length : 0}:${att.width ?? ""}:${att.height ?? ""}`,
        )
        .join("|"),
    [attachments],
  );

  useEffect(() => {
    const requestVersion = ++requestVersionRef.current;
    let cancelled = false;

    if (!attachments || attachments.length === 0) {
      setLoadedAttachments([]);
      dataMapRef.current.clear();
      return;
    }

    const validIds = new Set(attachments.map((att) => att.id));
    pruneCacheMap(dataMapRef.current, validIds);

    // Check which attachments need loading
    const toLoad: LazyAttachment[] = [];
    const result: LazyAttachment[] = [];

    for (const att of attachments) {
      if (att.data) {
        result.push(att);
        dataMapRef.current.set(att.id, {
          data: att.data,
          storagePath: att.storagePath ?? null,
        });
        continue;
      }

      const cachedEntry = dataMapRef.current.get(att.id);
      if (cachedEntry && (cachedEntry.storagePath ?? null) === (att.storagePath ?? null)) {
        result.push({ ...att, data: cachedEntry.data });
        continue;
      }

      if (att.storagePath) {
        toLoad.push(att);
        result.push(att);
      } else {
        dataMapRef.current.delete(att.id);
        result.push(att);
      }
    }

    setLoadedAttachments(result);

    if (toLoad.length > 0) {
      for (const att of toLoad) {
        if (!att.storagePath) continue;

        invoke<string>("storage_get_session_attachment_path", { storagePath: att.storagePath })
          .then((filePath) => {
            if (cancelled || requestVersionRef.current !== requestVersion) {
              return;
            }
            const assetUrl = withCacheBust(convertFileSrc(filePath), requestVersion);
            dataMapRef.current.set(att.id, {
              data: assetUrl,
              storagePath: att.storagePath ?? null,
            });

            setLoadedAttachments((prev) =>
              prev.map((a) => (a.id === att.id ? { ...a, data: assetUrl } : a)),
            );
            trimCacheToLimit(dataMapRef.current, MAX_ATTACHMENT_CACHE_ENTRIES);
          })
          .catch((err) => {
            if (cancelled || requestVersionRef.current !== requestVersion) return;
            console.error("[useSessionAttachments] Failed to load:", att.storagePath, err);
          });
      }
    }

    return () => {
      cancelled = true;
    };
  }, [attachmentsKey]);

  return loadedAttachments;
}
