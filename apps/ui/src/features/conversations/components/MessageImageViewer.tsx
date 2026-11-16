import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Button, cn, ImageViewer, LoadingState, toast } from "@lettuceai/crisp";
import { useApi } from "@/api/use-api";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { useOverlayBack } from "@/shared/hooks/useOverlayBack";
import type { MediaItem } from "../data/messages";

export function MessageImageViewer({ images, index, onIndexChange }: { images: readonly MediaItem[]; index: number | null; onIndexChange: (index: number | null) => void }) {
  const api = useApi();
  const { t } = useTranslation("conversations");
  const selected = index === null ? null : images[index];
  const [expanded, setExpanded] = useState(false);
  const prompt = useQuery({
    queryKey: [...queryKeys.media.all, "generation-prompt", selected?.id],
    queryFn: () => api.call("imageAssetPrompt", { asset_id: selected!.id }),
    enabled: Boolean(selected),
  });
  useOverlayBack(index !== null, () => onIndexChange(null));
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(prompt.data ?? "");
      toast.success(t("toast.copied"));
    } catch { toast.error(t("toast.copyFailed")); }
  };
  return <ImageViewer images={images.map((image) => ({ src: image.url!, alt: image.name ?? t("message.image") }))} index={index} onIndexChange={onIndexChange}
    details={!selected ? undefined : prompt.isPending ? <LoadingState /> : prompt.isError ? <FailureState size="sm" error={prompt.error} onRetry={() => void prompt.refetch()} /> : prompt.data ? <div className="mx-auto max-w-3xl space-y-2 px-4">
      <div className="flex items-center justify-between gap-3"><p className="text-xs text-fg-3">{t("message.imagePrompt")}</p><Button size="sm" variant="ghost" onClick={() => void copy()}>{t("message.copyImagePrompt")}</Button></div>
      <p className={cn("select-text whitespace-pre-wrap break-words text-sm", expanded ? "max-h-48 overflow-y-auto" : "line-clamp-3")}>{prompt.data}</p>
      {prompt.data.length > 200 && <Button size="sm" variant="ghost" onClick={() => setExpanded((value) => !value)}>{t(expanded ? "message.showLessPrompt" : "message.showMorePrompt")}</Button>}
    </div> : undefined} />;
}
