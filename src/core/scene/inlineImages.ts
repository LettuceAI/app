import { convertToImageRef } from "../storage/images";
import { convertFilePathToDataUrl } from "../storage/images";
import { processBackgroundImage } from "../utils/image";

export const SCENE_IMAGE_TOKEN_PATTERN = /\{\{image:([^:}]+):([^}]+)\}\}/g;

export type SceneImageExt = "gif" | "png" | "webp" | "jpg";

export interface StoredSceneImage {
  imageId: string;
  ext: SceneImageExt;
}

function extFromMime(mime: string): SceneImageExt {
  const value = mime.toLowerCase();
  if (value.includes("gif")) return "gif";
  if (value.includes("png")) return "png";
  if (value.includes("webp")) return "webp";
  return "jpg";
}

function extFromPath(path: string): SceneImageExt {
  const lower = path.toLowerCase();
  if (lower.endsWith(".gif")) return "gif";
  if (lower.endsWith(".png")) return "png";
  if (lower.endsWith(".webp")) return "webp";
  return "jpg";
}

function dataUrlMime(dataUrl: string): string {
  const match = /^data:([^;,]+)/.exec(dataUrl);
  return match?.[1] ?? "";
}

function buildToken(imageId: string, ext: SceneImageExt): string {
  return `{{image:${imageId}:${ext}}}`;
}

export function insertSceneImageToken(
  content: string,
  cursorIndex: number,
  imageId: string,
  ext: SceneImageExt,
): { content: string; nextCursor: number } {
  const token = buildToken(imageId, ext);
  const index = Math.max(0, Math.min(cursorIndex, content.length));
  const before = content.slice(0, index);
  const after = content.slice(index);
  const needsLeadingSpace = before.length > 0 && !/\s$/.test(before);
  const insertion = `${needsLeadingSpace ? " " : ""}${token}`;
  return {
    content: `${before}${insertion}${after}`,
    nextCursor: before.length + insertion.length,
  };
}

async function dataUrlForStorage(file: File): Promise<{ dataUrl: string; ext: SceneImageExt }> {
  const ext = extFromMime(file.type);
  if (ext === "gif" || ext === "png") {
    const dataUrl = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onerror = () => reject(reader.error ?? new Error("Failed to read file"));
      reader.onload = () => resolve(reader.result as string);
      reader.readAsDataURL(file);
    });
    return { dataUrl, ext };
  }
  const dataUrl = await processBackgroundImage(file);
  return { dataUrl, ext: extFromMime(dataUrlMime(dataUrl)) };
}

export async function storeSceneImageFromFile(file: File): Promise<StoredSceneImage | null> {
  const { dataUrl, ext } = await dataUrlForStorage(file);
  const imageId = await convertToImageRef(dataUrl);
  if (!imageId) return null;
  return { imageId, ext };
}

export async function storeSceneImageFromFilePath(
  filePath: string,
): Promise<StoredSceneImage | null> {
  const dataUrl = await convertFilePathToDataUrl(filePath);
  if (!dataUrl) return null;
  const ext = extFromMime(dataUrlMime(dataUrl)) || extFromPath(filePath);
  const imageId = await convertToImageRef(dataUrl);
  if (!imageId) return null;
  return { imageId, ext };
}
