interface ProcessImageOptions {
  /** Maximum width or height (whichever is larger) allowed for the image. */
  maxDimension?: number;
  /** Quality to use when encoding images that support lossy compression. */
  quality?: number;
  /** Force output format (defaults to WebP for background images) */
  format?: "webp" | "jpeg" | "png";
}

const DEFAULT_OPTIONS: Required<ProcessImageOptions> = {
  maxDimension: 1920,
  quality: 0.85,
  format: "webp",
};

export function isRenderableImageUrl(value?: string | null): boolean {
  if (!value) return false;
  const lower = value.trim().toLowerCase();
  return (
    lower.startsWith("http://") ||
    lower.startsWith("https://") ||
    lower.startsWith("data:image") ||
    lower.startsWith("blob:") ||
    lower.startsWith("asset:") ||
    lower.startsWith("tauri:")
  );
}

async function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error ?? new Error("Failed to read file"));
    reader.onload = () => resolve(reader.result as string);
    reader.readAsDataURL(file);
  });
}

async function loadImage(dataUrl: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("Failed to load image"));
    img.src = dataUrl;
  });
}

/**
 * Processes and optimizes background images by:
 * 1. Converting to WebP format for better compression
 * 2. Resizing if larger than maxDimension
 * 3. Returning optimized base64 data URL
 *
 * Note: The returned data URL is a temporary preview. When saving,
 * it will be converted to an image reference and stored via convertToImageRef().
 */
export async function processBackgroundImage(
  file: File,
  options: ProcessImageOptions = {},
): Promise<string> {
  const { maxDimension, quality, format } = { ...DEFAULT_OPTIONS, ...options };

  const originalDataUrl = await readFileAsDataUrl(file);
  const image = await loadImage(originalDataUrl);

  const largestDimension = Math.max(image.width, image.height);

  const scale = largestDimension > maxDimension ? maxDimension / largestDimension : 1;
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(image.width * scale);
  canvas.height = Math.round(image.height * scale);

  const context = canvas.getContext("2d");
  if (!context) {
    return originalDataUrl;
  }

  context.drawImage(image, 0, 0, canvas.width, canvas.height);

  const mimeType = `image/${format}`;
  try {
    return canvas.toDataURL(mimeType, quality);
  } catch {
    return originalDataUrl;
  }
}
