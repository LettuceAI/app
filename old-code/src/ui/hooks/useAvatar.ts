import { useEffect, useState } from "react";
import {
  AVATAR_BANNER_FILENAME,
  AVATAR_ROUND_FILENAME,
  AVATAR_UPDATED_EVENT,
  loadAvatar,
  type EntityType,
} from "../../core/storage/avatars";
import { isRenderableImageUrl } from "../../core/utils/image";

const avatarCache = new Map<string, string | Promise<string>>();

export type AvatarVariant = "base" | "round" | "banner";

/**
 * Invalidate cached avatar for a specific entity
 * Call this when an avatar is updated or deleted
 */
export function invalidateAvatarCache(type: EntityType, entityId: string) {
  const prefix = `${type}:${entityId}:`;
  const keysToDelete: string[] = [];
  avatarCache.forEach((_, key) => {
    if (key.startsWith(prefix)) {
      keysToDelete.push(key);
    }
  });
  keysToDelete.forEach((key) => avatarCache.delete(key));
}

/**
 * Hook to load and display character/persona avatars
 * Automatically fetches avatar from avatars/<type>-<entityId>/ directory
 * Uses global cache to prevent redundant loads
 *
 * @param type - Entity type (character or persona)
 * @param entityId - The character or persona ID
 * @param avatarFilename - The avatar filename stored in entity.avatarPath
 * @returns Data URL of the avatar or undefined if loading/not found
 */
export function useAvatar(
  type: EntityType,
  entityId: string | undefined,
  avatarFilename: string | undefined,
  variant: AvatarVariant = "base",
): string | undefined {
  const [refreshTick, setRefreshTick] = useState(0);
  const [avatarUrl, setAvatarUrl] = useState<string | undefined>(() => {
    if (entityId && avatarFilename) {
      const resolvedFilename =
        variant === "round" && !isRenderableImageUrl(avatarFilename)
          ? AVATAR_ROUND_FILENAME
          : avatarFilename;
      const cacheKey = `${type}:${entityId}:${variant}:${resolvedFilename}`;
      const cached = avatarCache.get(cacheKey);
      if (typeof cached === "string") {
        return cached;
      }
    }
    return undefined;
  });

  useEffect(() => {
    let cancelled = false;

    const fetchAvatar = async () => {
      if (!entityId || !avatarFilename) {
        setAvatarUrl(undefined);
        return;
      }

      if (isRenderableImageUrl(avatarFilename)) {
        const cacheKey = `${type}:${entityId}:${variant}:${avatarFilename}`;
        avatarCache.set(cacheKey, avatarFilename);
        setAvatarUrl(avatarFilename);
        return;
      }

      const primaryFilename =
        variant === "round"
          ? AVATAR_ROUND_FILENAME
          : variant === "banner"
            ? AVATAR_BANNER_FILENAME
            : avatarFilename;
      const cacheKey = `${type}:${entityId}:${variant}:${primaryFilename}`;
      const cached = avatarCache.get(cacheKey);

      if (typeof cached === "string") {
        if (!cancelled) {
          setAvatarUrl(cached);
        }
        return;
      }

      if (cached instanceof Promise) {
        try {
          const url = await cached;
          if (!cancelled) {
            setAvatarUrl(url);
          }
        } catch (error) {
          if (!cancelled) {
            setAvatarUrl(undefined);
          }
        }
        return;
      }

      const loadPromise = (async () => {
        const primary = await loadAvatar(type, entityId, primaryFilename);
        if (primary) {
          avatarCache.set(cacheKey, primary);
          return primary;
        }
        if ((variant === "round" || variant === "banner") && primaryFilename !== avatarFilename) {
          const fallback = await loadAvatar(type, entityId, avatarFilename);
          if (fallback) {
            avatarCache.set(cacheKey, fallback);
          }
          return fallback;
        }
        if (variant === "base" && primaryFilename === "avatar_base.webp") {
          const fallback = await loadAvatar(type, entityId, "avatar.webp");
          if (fallback) {
            avatarCache.set(cacheKey, fallback);
          }
          return fallback;
        }
        return primary;
      })().catch((error) => {
        console.error("[useAvatar] Failed to load avatar:", error);
        avatarCache.delete(cacheKey);
        throw error;
      });

      avatarCache.set(cacheKey, loadPromise as Promise<string>);

      try {
        const url = await loadPromise;
        if (!cancelled) {
          setAvatarUrl(url);
        }
      } catch (error) {
        if (!cancelled) {
          setAvatarUrl(undefined);
        }
      }
    };

    fetchAvatar();

    return () => {
      cancelled = true;
    };
  }, [type, entityId, avatarFilename, variant, refreshTick]);

  useEffect(() => {
    const handleAvatarUpdated = (event: Event) => {
      const detail = (event as CustomEvent<{ type?: EntityType; entityId?: string }>).detail;
      if (!entityId || !detail || detail.type !== type || detail.entityId !== entityId) {
        return;
      }
      invalidateAvatarCache(type, entityId);
      setRefreshTick((value) => value + 1);
    };

    window.addEventListener(AVATAR_UPDATED_EVENT, handleAvatarUpdated as EventListener);
    return () => {
      window.removeEventListener(AVATAR_UPDATED_EVENT, handleAvatarUpdated as EventListener);
    };
  }, [entityId, type]);

  return avatarUrl;
}
