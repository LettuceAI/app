import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type { AvatarGradientSource } from "./schemas";
import { isRenderableImageUrl } from "../utils/image";

/**
 * Centralized avatar management system
 * Handles avatar uploads, storage, and retrieval for characters and personas
 * Storage structure: avatars/<type>-<id>/avatar_base.webp and avatar_round.webp
 */

export type EntityType = "character" | "persona";

export const AVATAR_BASE_FILENAME = "avatar_base.webp";
export const AVATAR_ROUND_FILENAME = "avatar_round.webp";
export const AVATAR_BANNER_FILENAME = "avatar_banner.webp";
export const AVATAR_UPDATED_EVENT = "lettuce:avatar-updated";

const avatarUrlCache = new Map<string, string>();

export interface GradientColor {
  r: number;
  g: number;
  b: number;
  hex: string;
}

export interface AvatarGradient {
  colors: GradientColor[];
  gradient_css: string;
  dominant_hue: number;
  text_color: string;
  text_secondary: string;
}

/**
 * Creates a prefixed entity ID for avatar storage
 * @param type - Entity type (character or persona)
 * @param id - The entity ID
 * @returns Prefixed ID like "character-<id>" or "persona-<id>"
 */
function getPrefixedEntityId(type: EntityType, id: string): string {
  return `${type}-${id}`;
}

function emitAvatarUpdated(type: EntityType, entityId: string): void {
  if (typeof window === "undefined") return;
  window.dispatchEvent(
    new CustomEvent(AVATAR_UPDATED_EVENT, {
      detail: {
        type,
        entityId,
        at: Date.now(),
      },
    }),
  );
}

async function toDataUrlIfNeeded(imageData: string): Promise<string> {
  if (!imageData || imageData.startsWith("data:")) {
    return imageData;
  }

  if (!isRenderableImageUrl(imageData)) {
    return imageData;
  }

  const response = await fetch(imageData);
  if (!response.ok) {
    throw new Error(`Failed to load image asset (${response.status})`);
  }

  const blob = await response.blob();
  return await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(new Error("Failed to convert image asset to data URL"));
    reader.readAsDataURL(blob);
  });
}

/**
 * Saves an avatar image for a character or persona
 * Copies the image to avatars/<type>-<id>/ directory and converts to WebP
 *
 * @param type - Entity type (character or persona)
 * @param entityId - The character or persona ID
 * @param imageData - Base64 data URL or file path
 * @returns The avatar filename (without path) or empty string on failure
 */
export async function saveAvatar(
  type: EntityType,
  entityId: string,
  imageData: string,
  roundImageData?: string | null,
  bannerImageData?: string | null,
  gradientSource: AvatarGradientSource = "round",
): Promise<string> {
  if (!imageData || !entityId) {
    console.log("[saveAvatar] No image data or entity ID provided");
    return "";
  }

  try {
    const prefixedId = getPrefixedEntityId(type, entityId);
    console.log("[saveAvatar] Saving avatar for entity:", prefixedId);
    const normalizedImageData = await toDataUrlIfNeeded(imageData);
    const normalizedRoundImageData = roundImageData
      ? await toDataUrlIfNeeded(roundImageData)
      : null;
    const normalizedBannerImageData = bannerImageData
      ? await toDataUrlIfNeeded(bannerImageData)
      : null;

    const result = await invoke<string>("storage_save_avatar", {
      entityId: prefixedId,
      base64Data: normalizedImageData,
      roundBase64Data: normalizedRoundImageData,
      bannerBase64Data: normalizedBannerImageData,
    });

    // Clear the gradient cache for this entity since avatar changed
    clearEntityGradientCache(type, entityId);
    clearEntityAvatarUrlCache(type, entityId);
    console.log("[saveAvatar] Cleared gradient cache for entity:", prefixedId);

    try {
      const regeneratedGradient = await generateGradientFromAvatar(
        type,
        entityId,
        AVATAR_BASE_FILENAME,
        true,
        gradientSource,
      );
      if (regeneratedGradient) {
        const cacheKey = `${type}-${entityId}-${gradientSource}`;
        gradientCache.set(cacheKey, regeneratedGradient);
        console.log("[saveAvatar] Regenerated gradient and updated cache for entity:", prefixedId);
      }
    } catch (gradientError) {
      console.error("[saveAvatar] Failed to regenerate gradient after avatar save:", gradientError);
    }

    emitAvatarUpdated(type, entityId);

    console.log("[saveAvatar] Successfully saved avatar:", result);
    return result;
  } catch (error) {
    console.error("[saveAvatar] Failed to save avatar:", error);
    return "";
  }
}

/**
 * Loads an avatar image through Tauri's asset protocol.
 *
 * @param type - Entity type (character or persona)
 * @param entityId - The character or persona ID
 * @param avatarFilename - The avatar filename (from character.avatarPath)
 * @returns Displayable URL of the avatar or undefined on failure
 */
export async function loadAvatar(
  type: EntityType,
  entityId: string,
  avatarFilename: string | undefined,
): Promise<string | undefined> {
  if (!entityId || !avatarFilename) {
    return undefined;
  }

  if (isRenderableImageUrl(avatarFilename)) {
    return avatarFilename;
  }

  try {
    const cacheKey = `${type}:${entityId}:${avatarFilename}`;
    const cached = avatarUrlCache.get(cacheKey);
    if (cached) {
      return cached;
    }

    const prefixedId = getPrefixedEntityId(type, entityId);
    const filePath = await invoke<string>("storage_get_avatar_path", {
      entityId: prefixedId,
      filename: avatarFilename,
    });

    console.log("[loadAvatar] Loaded avatar for entity:", prefixedId);
    const assetUrl = convertFileSrc(filePath);
    const versionedUrl = `${assetUrl}${assetUrl.includes("?") ? "&" : "?"}avatar=${encodeURIComponent(avatarFilename)}`;
    avatarUrlCache.set(cacheKey, versionedUrl);
    return versionedUrl;
  } catch (error) {
    console.error("[loadAvatar] Failed to load avatar:", error);
    return undefined;
  }
}

/**
 * Deletes an avatar image
 * Removes from avatars/<type>-<id>/ directory
 *
 * @param type - Entity type (character or persona)
 * @param entityId - The character or persona ID
 * @param avatarFilename - The avatar filename to delete
 */
export async function deleteAvatar(
  type: EntityType,
  entityId: string,
  avatarFilename: string,
): Promise<void> {
  if (!entityId || !avatarFilename) {
    return;
  }

  try {
    const prefixedId = getPrefixedEntityId(type, entityId);
    const filenames = new Set([
      avatarFilename,
      AVATAR_BASE_FILENAME,
      AVATAR_ROUND_FILENAME,
      AVATAR_BANNER_FILENAME,
      "avatar.webp",
    ]);
    await Promise.all(
      Array.from(filenames).map((filename) =>
        invoke("storage_delete_avatar", {
          entityId: prefixedId,
          filename,
        }).catch(() => undefined),
      ),
    );
    clearEntityAvatarUrlCache(type, entityId);
    clearEntityGradientCache(type, entityId);
    emitAvatarUpdated(type, entityId);
    console.log("[deleteAvatar] Deleted avatar for entity:", prefixedId);
  } catch (error) {
    console.error("[deleteAvatar] Failed to delete avatar:", error);
  }
}

/**
 * Preloads multiple avatars for performance optimization
 *
 * @param avatars - Array of { type, entityId, filename } tuples
 */
export async function preloadAvatars(
  avatars: Array<{ type: EntityType; entityId: string; filename: string | undefined }>,
): Promise<void> {
  const validAvatars = avatars.filter((a) => !!a.entityId && !!a.filename);

  if (validAvatars.length === 0) {
    return;
  }

  try {
    await Promise.all(validAvatars.map((a) => loadAvatar(a.type, a.entityId, a.filename!)));
  } catch (error) {
    console.error("[preloadAvatars] Failed to preload avatars:", error);
  }
}

/**
 * Generates beautiful gradient colors from an avatar image
 * Uses color analysis and k-means clustering to extract dominant colors
 *
 * @param type - Entity type (character or persona)
 * @param entityId - The character or persona ID
 * @param avatarFilename - The avatar filename (from character.avatarPath)
 * @returns Promise with gradient colors and CSS, or undefined on failure
 */
export async function generateGradientFromAvatar(
  type: EntityType,
  entityId: string,
  avatarFilename: string | undefined,
  force = false,
  source: AvatarGradientSource = "round",
): Promise<AvatarGradient | undefined> {
  if (!entityId || !avatarFilename) {
    return undefined;
  }

  try {
    const prefixedId = getPrefixedEntityId(type, entityId);
    const gradient = await invoke<AvatarGradient>("generate_avatar_gradient", {
      entityId: prefixedId,
      filename: avatarFilename,
      force,
      source,
    });

    console.log(`[generateGradientFromAvatar] Generated gradient for ${prefixedId}:`, gradient);
    return gradient;
  } catch (error) {
    console.error("[generateGradientFromAvatar] Failed to generate gradient:", error);
    return undefined;
  }
}

/**
 * Cache for generated gradients to avoid recomputation
 */
const gradientCache = new Map<string, AvatarGradient>();

export function peekCachedGradient(
  type: EntityType,
  entityId: string,
  source: AvatarGradientSource = "round",
): AvatarGradient | undefined {
  return gradientCache.get(`${type}-${entityId}-${source}`);
}

/**
 * Gets cached or generates a gradient for an avatar
 *
 * @param type - Entity type (character or persona)
 * @param entityId - The character or persona ID
 * @param avatarFilename - The avatar filename (typically "avatar_base.webp")
 * @returns Promise with gradient colors
 */
export async function getCachedGradient(
  type: EntityType,
  entityId: string,
  avatarFilename: string | undefined,
  force = false,
  source: AvatarGradientSource = "round",
): Promise<AvatarGradient | undefined> {
  if (!entityId) {
    return undefined;
  }

  const cacheKey = `${type}-${entityId}-${source}`;

  if (!force && gradientCache.has(cacheKey)) {
    console.log(`[getCachedGradient] Using cached gradient for ${cacheKey}`);
    return gradientCache.get(cacheKey);
  }

  const gradient = await generateGradientFromAvatar(
    type,
    entityId,
    AVATAR_BASE_FILENAME,
    force,
    source,
  );

  if (gradient) {
    console.log(`[getCachedGradient] Cached new gradient for ${cacheKey}`);
    gradientCache.set(cacheKey, gradient);
  }

  void avatarFilename;

  return gradient;
}

/**
 * Forces a fresh gradient calculation and updates the in-memory cache.
 */
export async function recalculateGradient(
  type: EntityType,
  entityId: string,
  source: AvatarGradientSource = "round",
): Promise<AvatarGradient | undefined> {
  clearEntityGradientCache(type, entityId);
  const gradient = await getCachedGradient(type, entityId, AVATAR_BASE_FILENAME, true, source);
  emitAvatarUpdated(type, entityId);
  return gradient;
}

/**
 * Clears the gradient cache
 * Call this when avatars are updated to force regeneration
 */
export function clearGradientCache(): void {
  gradientCache.clear();
}

/**
 * Clears cached gradient for a specific entity
 * Useful when updating a single avatar
 */
export function clearEntityGradientCache(type: EntityType, entityId: string): void {
  const prefix = `${type}-${entityId}-`;
  for (const key of Array.from(gradientCache.keys())) {
    if (key.startsWith(prefix)) {
      gradientCache.delete(key);
    }
  }
}

function clearEntityAvatarUrlCache(type: EntityType, entityId: string): void {
  const prefix = `${type}:${entityId}:`;
  for (const key of Array.from(avatarUrlCache.keys())) {
    if (key.startsWith(prefix)) {
      avatarUrlCache.delete(key);
    }
  }
}
