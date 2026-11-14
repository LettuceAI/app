import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { EmptyState, MediaTile, MediaTileSkeleton, Sheet } from "@lettuceai/crisp";
import { useApi } from "@/api/use-api";
import { FailureState } from "@/shared/ui/FailureState";
import { pickerQuery, type PlaygroundAsset } from "./playground-data";

export function InitImagePicker({ open, onClose, onPick }: {
  open: boolean;
  onClose: () => void;
  onPick: (asset: PlaygroundAsset) => void;
}) {
  const { t } = useTranslation("playground");
  const history = useQuery({ ...pickerQuery(useApi()), enabled: open });
  const images = (history.data?.entries ?? [])
    .filter((entry) => entry.status === "complete")
    .flatMap((entry) => entry.images.flatMap((image) => (image.asset ? [{ key: `${entry.id}:${image.asset.asset_id}`, asset: image.asset }] : [])));
  return (
    <Sheet open={open} onClose={onClose} size="md" title={t("prompt.pickFromHistory")}>
      {history.isError ? (
        <FailureState size="sm" error={history.error} onRetry={() => void history.refetch()} />
      ) : history.isPending ? (
        <div role="status" aria-label={t("prompt.pickFromHistory")} className="grid grid-cols-3 gap-2 sm:grid-cols-4">
          {[0, 1, 2, 3, 4, 5].map((index) => <MediaTileSkeleton key={index} />)}
        </div>
      ) : images.length === 0 ? (
        <EmptyState size="sm" title={t("prompt.noHistoryImages")} />
      ) : (
        <div className="grid grid-cols-3 gap-2 sm:grid-cols-4">
          {images.map(({ key, asset }) => (
            <MediaTile
              key={key}
              src={asset.preview_url ?? asset.url}
              alt={t("prompt.pickFromHistory")}
              onClick={() => {
                onPick(asset);
                onClose();
              }}
            />
          ))}
        </div>
      )}
    </Sheet>
  );
}
