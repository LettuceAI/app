import type { ApiClient } from "@/api/client";
import type { AssetIngestRole, AssetRef, FileSource } from "@/api/generated/bindings";

export type ImageInputRole = Extract<AssetIngestRole, "avatar" | "background" | "reference_image" | "attachment">;
export interface PickedImage { source: FileSource; asset: AssetRef }

export function ingestImage(api: ApiClient, source: FileSource, role: ImageInputRole): Promise<AssetRef> {
  return api.call("assetsIngest", { source, role });
}

export async function pickImages(api: ApiClient, options: {
  role: ImageInputRole;
  multiple?: boolean;
  onImage?: (image: PickedImage) => void;
}): Promise<PickedImage[]> {
  const picked = await api.call("filesPickOpen", { kinds: ["image"], multiple: options.multiple ?? false });
  const images: PickedImage[] = [];
  const sources = options.multiple ? picked.sources : picked.sources.slice(0, 1);
  for (const source of sources) {
    const image = { source, asset: await ingestImage(api, source, options.role) };
    images.push(image);
    options.onImage?.(image);
  }
  return images;
}

export async function pickImage(api: ApiClient, role: ImageInputRole): Promise<AssetRef | null> {
  return (await pickImages(api, { role }))[0]?.asset ?? null;
}
