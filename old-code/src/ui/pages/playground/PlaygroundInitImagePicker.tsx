import { useEffect, useState } from "react";
import { ImageOff, Loader } from "lucide-react";

import { BottomMenu } from "../../components/BottomMenu";
import { useI18n } from "../../../core/i18n/context";
import { resolveGeneratedImageUrl } from "../../../core/image-generation";
import {
  listPlaygroundHistory,
  type PlaygroundGenerationImage,
} from "../../../core/image-generation/playground";

type HistoryImage = {
  entryId: string;
  image: PlaygroundGenerationImage;
};

function Thumb({
  image,
  onClick,
}: {
  image: PlaygroundGenerationImage;
  onClick: () => void;
}) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    void resolveGeneratedImageUrl({
      assetId: image.assetId,
      filePath: image.filePath,
      mimeType: image.mimeType ?? "image/png",
      url: image.url ?? undefined,
    })
      .then((resolved) => {
        if (!cancelled) setUrl(resolved ?? null);
      })
      .catch(() => {
        if (!cancelled) setUrl(null);
      });
    return () => {
      cancelled = true;
    };
  }, [image.assetId, image.filePath, image.url]);

  return (
    <button
      type="button"
      onClick={onClick}
      className="group relative aspect-square overflow-hidden rounded-xl border border-fg/10 bg-fg/5 transition-all hover:border-accent/50 active:scale-[0.98]"
    >
      {url ? (
        <img
          src={url}
          alt=""
          loading="lazy"
          decoding="async"
          className="h-full w-full object-cover transition group-hover:brightness-110"
        />
      ) : (
        <span className="flex h-full w-full items-center justify-center text-fg/20">
          <ImageOff size={16} />
        </span>
      )}
    </button>
  );
}

export function PlaygroundInitImagePicker({
  isOpen,
  onClose,
  onPick,
}: {
  isOpen: boolean;
  onClose: () => void;
  onPick: (image: PlaygroundGenerationImage) => void;
}) {
  const { t } = useI18n();
  const [images, setImages] = useState<HistoryImage[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!isOpen) return;
    let cancelled = false;
    setLoading(true);
    listPlaygroundHistory(60)
      .then((entries) => {
        if (cancelled) return;
        const collected: HistoryImage[] = [];
        for (const entry of entries) {
          if (entry.status !== "complete") continue;
          for (const image of entry.images) {
            collected.push({ entryId: entry.id, image });
          }
        }
        setImages(collected);
      })
      .catch(() => {
        if (!cancelled) setImages([]);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [isOpen]);

  return (
    <BottomMenu isOpen={isOpen} onClose={onClose} title={t("playground.prompt.pickFromHistory")}>
      {loading ? (
        <div className="flex items-center justify-center py-10">
          <Loader size={18} className="animate-spin text-fg/30" />
        </div>
      ) : images.length === 0 ? (
        <p className="rounded-xl border border-dashed border-fg/10 bg-fg/2 px-4 py-8 text-center text-[12.5px] text-fg/45">
          {t("playground.prompt.noHistoryImages")}
        </p>
      ) : (
        <div className="grid max-h-[55vh] grid-cols-3 gap-2 overflow-y-auto sm:grid-cols-4">
          {images.map(({ entryId, image }, index) => (
            <Thumb
              key={`${entryId}:${image.assetId || image.filePath || index}`}
              image={image}
              onClick={() => {
                onPick(image);
                onClose();
              }}
            />
          ))}
        </div>
      )}
    </BottomMenu>
  );
}
