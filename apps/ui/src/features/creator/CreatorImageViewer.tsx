import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Button, ImageViewer, toast } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { AssetRef } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { useOverlayBack } from "@/shared/hooks/useOverlayBack";

/** The session's images full size, with the prompt a generated one was made from. */
export function CreatorImageViewer({ api, images, index, alt, onIndexChange }: {
  api: ApiClient;
  images: readonly AssetRef[];
  index: number | null;
  alt: string;
  onIndexChange: (index: number | null) => void;
}) {
  const { t } = useTranslation("creator");
  const selected = index === null ? null : images[index];
  const prompt = useQuery({
    queryKey: [...queryKeys.media.all, "generation-prompt", selected?.asset_id],
    queryFn: () => api.call("imageAssetPrompt", { asset_id: selected!.asset_id }),
    enabled: Boolean(selected),
  });
  useOverlayBack(index !== null, () => onIndexChange(null));
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(prompt.data ?? "");
      toast.success(t("copied"));
    } catch {
      toast.error(t("copyFailed"));
    }
  };
  return (
    <ImageViewer
      images={images.map((image) => ({ src: image.url, alt }))}
      index={index}
      onIndexChange={onIndexChange}
      details={prompt.data ? (
        <div className="mx-auto max-w-3xl space-y-2">
          <div className="flex justify-end"><Button size="sm" variant="ghost" onClick={() => void copy()}>{t("copy")}</Button></div>
          <p className="select-text whitespace-pre-wrap break-words text-sm">{prompt.data}</p>
        </div>
      ) : undefined}
    />
  );
}
