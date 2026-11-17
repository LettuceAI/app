import { useState } from "react";
import { useInfiniteQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Button, EmptyState, MediaTile, MediaTileSkeleton, SearchField, Sheet } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { AssetRef } from "@/api/generated/bindings";
import { mediaQuery } from "@/api/media-library-query";
import { FailureState } from "@/shared/ui/FailureState";

export function AvatarLibraryButton({ onClick, disabled, banner = false }: { onClick: () => void; disabled: boolean; banner?: boolean }) {
  const { t } = useTranslation("image-input");
  return <Button size="sm" variant="ghost" disabled={disabled} onClick={onClick}>{t(banner ? "chooseBannerFromLibrary" : "chooseFromLibrary")}</Button>;
}

function LibraryImages({ api, onPick }: { api: ApiClient; onPick: (asset: AssetRef) => void }) {
  const { t } = useTranslation("library");
  const [query, setQuery] = useState("");
  const images = useInfiniteQuery(mediaQuery(api, "image", query.trim()));
  const items = images.data?.pages.flatMap((page) => page.items) ?? [];

  return <div className="space-y-3">
    <SearchField value={query} onChange={setQuery} placeholder={t("search")} aria-label={t("search")} />
    {images.isPending ? (
      <div role="status" aria-label={t("filters.images")} className="grid grid-cols-3 gap-2 sm:grid-cols-4">
        {[0, 1, 2, 3, 4, 5].map((index) => <MediaTileSkeleton key={index} />)}
      </div>
    ) : items.length > 0 ? (
      <div className="grid grid-cols-3 gap-2 sm:grid-cols-4">
        {items.map((item) => <MediaTile key={item.asset.asset_id} src={item.asset.preview_url ?? item.asset.url}
          alt={item.source_label?.trim() || item.asset.asset_id} onClick={() => onPick(item.asset)} />)}
      </div>
    ) : !images.isError && (
      <EmptyState size="sm" title={query.trim() ? t("empty.matches", { query: query.trim() }) : t("empty.images.title")} />
    )}
    {images.isError ? (
      <FailureState size="sm" error={images.error} onRetry={() => void (images.isFetchNextPageError ? images.fetchNextPage() : images.refetch())} />
    ) : images.hasNextPage && (
      <Button variant="secondary" block loading={images.isFetchingNextPage} onClick={() => void images.fetchNextPage()}>{t("loadMore")}</Button>
    )}
  </div>;
}

export function AvatarLibrarySheet({ api, open, banner, onClose, onPick }: {
  api: ApiClient;
  open: boolean;
  banner: boolean;
  onClose: () => void;
  onPick: (asset: AssetRef) => void;
}) {
  const { t } = useTranslation("image-input");
  return <Sheet open={open} onClose={onClose} size="md" title={t(banner ? "chooseBannerFromLibrary" : "chooseFromLibrary")}>
    {open && <LibraryImages api={api} onPick={onPick} />}
  </Sheet>;
}
