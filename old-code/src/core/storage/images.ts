import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { isRenderableImageUrl } from "../utils/image";

/**
 * Stores a base64-encoded image and returns a reference ID
 * This reduces performance issues from large image data URLs in state
 *
 * In Tauri v2, images are stored as files on disk via the Rust backend
 * and displayed through Tauri's asset protocol.
 */
/**
 * Generates a UUID v4 for use as an image ID
 */
function generateImageId(): string {
  if (typeof crypto !== "undefined" && crypto.randomUUID) {
    return crypto.randomUUID();
  }
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, "0"));
  return (
    hex.slice(0, 4).join("") +
    "-" +
    hex.slice(4, 6).join("") +
    "-" +
    hex.slice(6, 8).join("") +
    "-" +
    hex.slice(8, 10).join("") +
    "-" +
    hex.slice(10, 16).join("")
  );
}

/**
 * Converts a base64 data URL or file to an image reference ID
 * Stores the image on disk via Rust backend and returns the unique ID
 */
export async function convertToImageRef(imageData: string): Promise<string> {
  if (!imageData) {
    console.log("[convertToImageRef] No image data provided");
    return "";
  }

  try {
    const imageId = generateImageId();
    console.log("[convertToImageRef] Generated image ID:", imageId);

    const result = await invoke<string>("storage_write_image", {
      imageId: imageId,
      base64Data: imageData,
    });

    console.log("[convertToImageRef] Rust returned:", result);
    console.log("[convertToImageRef] Successfully saved image with ID:", imageId);
    return imageId;
  } catch (error) {
    console.error("[convertToImageRef] failed to save image:", error);
    return "";
  }
}

/**
 * Converts an image ID to a displayable URL using Tauri's asset protocol.
 */
export async function convertToImageUrl(imageIdOrDataUrl: string): Promise<string | undefined> {
  if (isRenderableImageUrl(imageIdOrDataUrl)) {
    return imageIdOrDataUrl;
  }

  try {
    const filePath = await getImageRef(imageIdOrDataUrl);
    if (!filePath) {
      return undefined;
    }

    const assetUrl = convertFileSrc(filePath);
    console.log("[convertToImageUrl] Loaded asset URL for ID:", imageIdOrDataUrl);
    return assetUrl;
  } catch (error) {
    console.error("Failed to read image:", error);
    return undefined;
  }
}

export async function convertFilePathToDataUrl(filePath: string): Promise<string | null> {
  if (!filePath) {
    return null;
  }

  try {
    const assetUrl = convertFileSrc(filePath);
    const response = await fetch(assetUrl);
    if (!response.ok) {
      throw new Error(`Failed to read image asset (${response.status})`);
    }

    const blob = await response.blob();
    return await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(reader.result as string);
      reader.onerror = () => reject(new Error("Failed to convert image asset to data URL"));
      reader.readAsDataURL(blob);
    });
  } catch (error) {
    console.error("Failed to convert file path to data URL:", error);
    return null;
  }
}

/**
 * Preloads multiple image URLs for performance optimization
 * Call this when entering the Chat component to eagerly load images
 *
 * @param imageIds - Array of image IDs to preload
 * @returns Promise that resolves when all images are preloaded
 */
export async function preloadImages(imageIds: (string | undefined | null)[]): Promise<void> {
  const validIds = imageIds.filter((id): id is string => !!id && !isRenderableImageUrl(id));

  if (validIds.length === 0) {
    return;
  }

  try {
    await Promise.all(validIds.map((id) => convertToImageUrl(id)));
  } catch (error) {
    console.error("Failed to preload images:", error);
  }
}

/**
 * Retrieves the file path for a stored image by its reference ID
 */
export async function getImageRef(imageId: string): Promise<string | null> {
  if (!imageId) {
    return null;
  }

  try {
    return await invoke<string>("storage_get_image_path", { imageId });
  } catch (error) {
    console.error("Failed to retrieve image reference:", error);
    return null;
  }
}

/**
 * Deletes a stored image by its reference ID
 */
export async function deleteImageRef(imageId: string): Promise<void> {
  if (!imageId) {
    return;
  }

  try {
    await invoke("storage_delete_image", { imageId });
  } catch (error) {
    console.error("Failed to delete image reference:", error);
  }
}
