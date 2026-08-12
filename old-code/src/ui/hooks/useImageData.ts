import { useEffect, useState, useMemo, useRef } from "react";
import { convertToImageUrl } from "../../core/storage";
import { isRenderableImageUrl } from "../../core/utils/image";

interface UseImageDataOptions {
  lazy?: boolean;
}

type UseImageDataStateResult = {
  imageUrl: string | undefined;
  loading: boolean;
};

const resolvedImageUrlCache = new Map<string, string | undefined>();
const pendingImageUrlLoads = new Map<string, Promise<string | undefined>>();

function getCachedImageUrl(imageIdOrData: string | undefined | null): string | undefined {
  if (!imageIdOrData) {
    return undefined;
  }
  if (isRenderableImageUrl(imageIdOrData)) {
    return imageIdOrData;
  }
  return resolvedImageUrlCache.get(imageIdOrData);
}

function hasCachedImageUrl(imageIdOrData: string | undefined | null): boolean {
  if (!imageIdOrData) {
    return false;
  }
  if (isRenderableImageUrl(imageIdOrData)) {
    return true;
  }
  return resolvedImageUrlCache.has(imageIdOrData);
}

async function loadImageUrl(imageIdOrData: string): Promise<string | undefined> {
  if (isRenderableImageUrl(imageIdOrData)) {
    return imageIdOrData;
  }

  if (resolvedImageUrlCache.has(imageIdOrData)) {
    return resolvedImageUrlCache.get(imageIdOrData);
  }

  const pending = pendingImageUrlLoads.get(imageIdOrData);
  if (pending) {
    return pending;
  }

  const request = convertToImageUrl(imageIdOrData)
    .then((url) => {
      if (url !== undefined) {
        resolvedImageUrlCache.set(imageIdOrData, url);
      }
      pendingImageUrlLoads.delete(imageIdOrData);
      return url;
    })
    .catch((error) => {
      pendingImageUrlLoads.delete(imageIdOrData);
      throw error;
    });

  pendingImageUrlLoads.set(imageIdOrData, request);
  return request;
}

export async function preloadImageUrls(
  imageIds: (string | undefined | null)[],
): Promise<void> {
  const validIds = imageIds.filter((id): id is string => !!id);

  if (validIds.length === 0) {
    return;
  }

  try {
    await Promise.all(validIds.map((id) => loadImageUrl(id)));
  } catch (error) {
    console.error("Failed to preload image URLs:", error);
  }
}

/**
 * Hook to automatically load image URLs from image IDs.
 *
 * Loads images immediately and asynchronously to avoid blocking the UI thread.
 * Results are cached to prevent reloading the same image across re-renders.
 */
export function useImageDataState(
  imageIdOrData: string | undefined | null,
  options?: UseImageDataOptions,
): UseImageDataStateResult {
  const [imageUrl, setImageUrl] = useState<string | undefined>(() =>
    getCachedImageUrl(imageIdOrData),
  );
  const [loading, setLoading] = useState<boolean>(
    () => !!imageIdOrData && !hasCachedImageUrl(imageIdOrData) && !(options?.lazy ?? false),
  );
  const lastProcessedIdRef = useRef<string | null | undefined>(undefined);

  const memoizedOptions = useMemo<UseImageDataOptions>(
    () => ({
      lazy: options?.lazy ?? false,
    }),
    [options?.lazy],
  );

  const [shouldLoad, setShouldLoad] = useState(!memoizedOptions.lazy);

  useEffect(() => {
    if (!imageIdOrData) {
      setImageUrl(undefined);
      setLoading(false);
      lastProcessedIdRef.current = imageIdOrData;
      return;
    }

    if (lastProcessedIdRef.current === imageIdOrData && (imageUrl !== undefined || !loading)) {
      return;
    }

    const cachedImageUrl = getCachedImageUrl(imageIdOrData);
    if (hasCachedImageUrl(imageIdOrData)) {
      setImageUrl(cachedImageUrl);
      setLoading(false);
      lastProcessedIdRef.current = imageIdOrData;
      return;
    }

    if (!shouldLoad) {
      setLoading(false);
      return;
    }

    console.log("[useImageData] Loading image for ID:", imageIdOrData);
    let cancelled = false;
    setLoading(true);

    void loadImageUrl(imageIdOrData)
      .then((url: string | undefined) => {
        if (!cancelled) {
          console.log("[useImageData] Successfully loaded image:", url ? "present" : "failed");
          setImageUrl(url);
          setLoading(false);
          lastProcessedIdRef.current = imageIdOrData;
        }
      })
      .catch((err: any) => {
        console.error("[useImageData] Failed to load image:", err);
        if (!cancelled) {
          setImageUrl(undefined);
          setLoading(false);
          lastProcessedIdRef.current = imageIdOrData;
        }
      });

    return () => {
      cancelled = true;
    };
  }, [imageIdOrData, shouldLoad, imageUrl, loading]);

  useEffect(() => {
    if (!memoizedOptions.lazy) {
      setShouldLoad(true);
    }
  }, [memoizedOptions.lazy]);

  const currentImageUrl =
    lastProcessedIdRef.current === imageIdOrData ? imageUrl : getCachedImageUrl(imageIdOrData);

  return { imageUrl: currentImageUrl, loading };
}

export function useImageData(
  imageIdOrData: string | undefined | null,
  options?: UseImageDataOptions,
): string | undefined {
  return useImageDataState(imageIdOrData, options).imageUrl;
}

/**
 * Trigger image loading for a lazy-loaded image
 * This is used internally by Chat component to preload on mount
 */
export function usePreloadImage(imageIdOrData: string | undefined | null): string | undefined {
  return useImageData(imageIdOrData, { lazy: false });
}
