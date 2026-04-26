import { memo, useCallback, useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { convertFileSrc } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import { Copy, Download, Image as ImageIcon, Loader2, Trash2 } from "lucide-react";
import { useLocation, useNavigate } from "react-router-dom";

import {
  deleteImageLibraryItem,
  downloadImageLibraryItem,
  listImageLibraryItems,
  listReferencedBackgroundImagePaths,
  type ImageLibraryItem,
} from "../../../core/storage/repo";
import { BottomMenu } from "../../components";
import { cn } from "../../design-tokens";
import { confirmBottomMenu } from "../../components/ConfirmBottomMenu";
import { toast } from "../../components/toast";
import { isRenderableImageUrl } from "../../../core/utils/image";
import { useI18n } from "../../../core/i18n/context";
import {
  buildAvatarLibrarySelectionKey,
  buildBackgroundLibrarySelectionKey,
  type AvatarLibrarySelectionPayload,
  type BackgroundLibrarySelectionPayload,
} from "../../components/AvatarPicker/librarySelection";

type FilterOption = "All" | "Backgrounds" | "Avatars" | "Attachments" | "Other";
type SortOption = "Newest" | "Largest" | "Name";

const SORTS: SortOption[] = ["Newest", "Largest", "Name"];
const GRID_GAP = 12;
const GRID_OVERSCAN_ROWS = 3;
const assetUrlCache = new Map<string, string>();

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unit = units[0];
  for (let index = 1; index < units.length && value >= 1024; index += 1) {
    value /= 1024;
    unit = units[index];
  }
  return `${value.toFixed(value >= 10 ? 0 : 1)} ${unit}`;
}

function formatDate(timestamp: number): string {
  if (!timestamp) return "Unknown";
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  }).format(new Date(timestamp));
}

function getStoredImageId(item: ImageLibraryItem): string | null {
  if (item.bucket !== "stored") return null;
  const stem = item.filename.replace(/\.[^.]+$/, "");
  return stem || null;
}

function getImageKind(
  item: ImageLibraryItem,
  backgroundIds: Set<string>,
): Exclude<FilterOption, "All"> {
  if (item.bucket === "avatar") return "Avatars";
  if (item.bucket === "attachment") return "Attachments";
  const storedId = getStoredImageId(item);
  if (storedId && backgroundIds.has(storedId)) return "Backgrounds";
  return "Other";
}

function imageKindLabel(kind: Exclude<FilterOption, "All">): string {
  if (kind === "Backgrounds") return "Background";
  if (kind === "Avatars") return "Avatar";
  if (kind === "Attachments") return "Attachment";
  return "Stored";
}

function matchesFilter(item: ImageLibraryItem, filter: FilterOption, backgroundIds: Set<string>) {
  if (filter === "All") return true;
  return getImageKind(item, backgroundIds) === filter;
}

function sortItems(items: ImageLibraryItem[], sort: SortOption) {
  const next = [...items];
  if (sort === "Largest") {
    return next.sort((a, b) => b.sizeBytes - a.sizeBytes || b.updatedAt - a.updatedAt);
  }
  if (sort === "Name") {
    return next.sort((a, b) => a.filename.localeCompare(b.filename) || b.updatedAt - a.updatedAt);
  }
  return next.sort((a, b) => b.updatedAt - a.updatedAt || a.filename.localeCompare(b.filename));
}

function copyToClipboard(value: string, label: string) {
  navigator.clipboard
    .writeText(value)
    .then(() => toast.success(`${label} copied`, value))
    .catch((error) => {
      console.error(`Failed to copy ${label.toLowerCase()}:`, error);
      toast.error(`Failed to copy ${label.toLowerCase()}`);
    });
}

function getAssetUrl(filePath: string): string {
  const cached = assetUrlCache.get(filePath);
  if (cached) return cached;
  const next = convertFileSrc(filePath);
  assetUrlCache.set(filePath, next);
  return next;
}

function compactPath(value: string): string {
  if (value.length <= 56) return value;
  return `${value.slice(0, 20)}...${value.slice(-28)}`;
}

const ImageTile = memo(function ImageTile({
  item,
  kindLabel,
  onSelect,
}: {
  item: ImageLibraryItem;
  kindLabel: string;
  onSelect: (item: ImageLibraryItem) => void;
}) {
  return (
    <button
      type="button"
      onClick={() => onSelect(item)}
      className="group relative aspect-square overflow-hidden rounded-2xl border border-fg/10 bg-fg/3 text-left"
    >
      <img
        src={getAssetUrl(item.filePath)}
        alt={item.filename}
        className="h-full w-full object-cover transition duration-300 group-hover:scale-[1.03]"
        loading="lazy"
        decoding="async"
      />
      <div className="absolute inset-0 bg-linear-to-t from-black/85 via-black/20 to-transparent" />
      <div className="absolute left-2 top-2 rounded-full border border-white/15 bg-black/45 px-2 py-1 text-[10px] font-medium uppercase tracking-widest text-white/85 backdrop-blur-md">
        {kindLabel}
      </div>
      <div className="absolute inset-x-0 bottom-0 p-3">
        <div className="truncate text-sm font-semibold text-white">{item.filename}</div>
        <div className="mt-1 flex items-center justify-between gap-2 text-[11px] text-white/70">
          <span>
            {item.width && item.height ? `${item.width} × ${item.height}` : item.mimeType}
          </span>
          <span>{formatBytes(item.sizeBytes)}</span>
        </div>
      </div>
    </button>
  );
});

const ImageLibraryGrid = memo(function ImageLibraryGrid({
  items,
  backgroundIds,
  scrollContainerRef,
  onSelect,
  columnCountOverride,
}: {
  items: ImageLibraryItem[];
  backgroundIds: Set<string>;
  scrollContainerRef: RefObject<HTMLElement | null>;
  onSelect: (item: ImageLibraryItem) => void;
  columnCountOverride?: number;
}) {
  const [viewportWidth, setViewportWidth] = useState(0);
  const [gridWidth, setGridWidth] = useState(0);
  const gridRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const syncViewport = () => setViewportWidth(window.innerWidth);
    syncViewport();

    window.addEventListener("resize", syncViewport);

    return () => {
      window.removeEventListener("resize", syncViewport);
    };
  }, []);

  useEffect(() => {
    const grid = gridRef.current;
    if (!grid) return;

    const syncGridWidth = () => setGridWidth(grid.clientWidth);
    syncGridWidth();

    const resizeObserver = new ResizeObserver(syncGridWidth);
    resizeObserver.observe(grid);

    return () => resizeObserver.disconnect();
  }, [items.length]);

  const columnCount = useMemo(() => {
    if (columnCountOverride && columnCountOverride > 0) return columnCountOverride;
    if (viewportWidth >= 1536) return 6;
    if (viewportWidth >= 1280) return 5;
    if (viewportWidth >= 1024) return 4;
    return 2;
  }, [columnCountOverride, viewportWidth]);

  const itemSize = useMemo(() => {
    const safeGridWidth = gridWidth || Math.max(0, window.innerWidth - 32);
    if (columnCount <= 0) return 0;
    return Math.max(0, (safeGridWidth - GRID_GAP * (columnCount - 1)) / columnCount);
  }, [columnCount, gridWidth]);

  const rowCount = Math.ceil(items.length / columnCount);

  const rowVirtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => scrollContainerRef.current,
    estimateSize: () => itemSize || 200,
    overscan: GRID_OVERSCAN_ROWS,
  });

  useEffect(() => {
    rowVirtualizer.measure();
  }, [columnCount, gridWidth, itemSize, rowVirtualizer]);

  const virtualRows = rowVirtualizer.getVirtualItems();
  const totalHeight = rowVirtualizer.getTotalSize() + Math.max(0, rowCount - 1) * GRID_GAP;

  return (
    <div
      ref={gridRef}
      style={{ contain: "layout paint style", height: totalHeight, position: "relative" }}
    >
      {virtualRows.map((virtualRow) => {
        const startIndex = virtualRow.index * columnCount;
        const rowItems = items.slice(startIndex, startIndex + columnCount);

        return (
          <div
            key={virtualRow.key}
            className="absolute left-0 top-0 grid w-full gap-3 will-change-transform"
            style={{
              gridTemplateColumns: `repeat(${columnCount}, minmax(0, 1fr))`,
              height: itemSize || 200,
              transform: `translateY(${virtualRow.start + virtualRow.index * GRID_GAP}px)`,
            }}
          >
            {rowItems.map((item) => (
              <ImageTile
                key={item.id}
                item={item}
                kindLabel={imageKindLabel(getImageKind(item, backgroundIds))}
                onSelect={onSelect}
              />
            ))}
          </div>
        );
      })}
    </div>
  );
});

export function ImageLibraryPanel({
  scrollContainerRef,
  embedded: _embedded = false,
  mode = "default",
  onUseItem,
  fixedFilter,
  hideFilterTabs = false,
  columnCountOverride,
}: {
  scrollContainerRef: RefObject<HTMLElement | null>;
  embedded?: boolean;
  mode?: "default" | "picker";
  onUseItem?: (item: ImageLibraryItem) => Promise<void> | void;
  fixedFilter?: FilterOption;
  hideFilterTabs?: boolean;
  columnCountOverride?: number;
}) {
  const { t } = useI18n();
  const [items, setItems] = useState<ImageLibraryItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<FilterOption>(fixedFilter ?? "All");
  const [sort, setSort] = useState<SortOption>("Newest");
  const [showSortMenu, setShowSortMenu] = useState(false);
  const [selectedItem, setSelectedItem] = useState<ImageLibraryItem | null>(null);
  const [backgroundIds, setBackgroundIds] = useState<Set<string>>(new Set());
  const [downloadingItemId, setDownloadingItemId] = useState<string | null>(null);
  const [usingItemId, setUsingItemId] = useState<string | null>(null);
  const [deletingItemId, setDeletingItemId] = useState<string | null>(null);

  useEffect(() => {
    if (fixedFilter) {
      setFilter(fixedFilter);
    }
  }, [fixedFilter]);

  useEffect(() => {
    const load = async () => {
      try {
        setLoading(true);
        const [libraryItems, referencedBackgrounds] = await Promise.all([
          listImageLibraryItems(),
          listReferencedBackgroundImagePaths(),
        ]);

        const nextBackgroundIds = new Set<string>();
        for (const value of referencedBackgrounds) {
          if (!value || isRenderableImageUrl(value)) continue;
          nextBackgroundIds.add(value);
        }

        setItems(libraryItems);
        setBackgroundIds(nextBackgroundIds);
      } catch (error) {
        console.error("Failed to load image library:", error);
        toast.error(t("library.imageLibrary.messages.loadFailed"));
      } finally {
        setLoading(false);
      }
    };

    void load();
  }, [t]);

  const counts = useMemo(
    () => ({
      all: items.length,
      backgrounds: items.filter((item) => getImageKind(item, backgroundIds) === "Backgrounds")
        .length,
      avatars: items.filter((item) => getImageKind(item, backgroundIds) === "Avatars").length,
      attachments: items.filter((item) => getImageKind(item, backgroundIds) === "Attachments")
        .length,
      other: items.filter((item) => getImageKind(item, backgroundIds) === "Other").length,
    }),
    [backgroundIds, items],
  );

  const filteredItems = useMemo(() => {
    const lowered = query.trim().toLowerCase();
    const next = items.filter((item) => {
      if (!matchesFilter(item, filter, backgroundIds)) return false;
      if (!lowered) return true;
      return (
        item.filename.toLowerCase().includes(lowered) ||
        item.storagePath.toLowerCase().includes(lowered) ||
        item.entityId?.toLowerCase().includes(lowered) ||
        item.sessionId?.toLowerCase().includes(lowered) ||
        item.characterId?.toLowerCase().includes(lowered)
      );
    });
    return sortItems(next, sort);
  }, [backgroundIds, filter, items, query, sort]);

  const handleDownload = async (item: ImageLibraryItem) => {
    try {
      setDownloadingItemId(item.id);
      const savedPath = await downloadImageLibraryItem(item);
      toast.success(t("library.imageLibrary.messages.saved"), savedPath);
    } catch (error) {
      console.error("Failed to download image:", error);
      toast.error(
        t("library.imageLibrary.messages.downloadFailed"),
        error instanceof Error ? error.message : String(error),
      );
    } finally {
      setDownloadingItemId((current) => (current === item.id ? null : current));
    }
  };

  const handleUseItem = useCallback(
    async (item: ImageLibraryItem) => {
      if (!onUseItem) return;
      try {
        setUsingItemId(item.id);
        await onUseItem(item);
      } catch (error) {
        console.error("Failed to use image library item:", error);
        toast.error(
          t("library.imageLibrary.messages.useFailed"),
          error instanceof Error ? error.message : String(error),
        );
      } finally {
        setUsingItemId((current) => (current === item.id ? null : current));
      }
    },
    [onUseItem, t],
  );

  const handleDeleteItem = useCallback(
    async (item: ImageLibraryItem) => {
      const confirmed = await confirmBottomMenu({
        title: t("library.imageLibrary.deleteConfirm.title"),
        message: t("library.imageLibrary.deleteConfirm.message", { filename: item.filename }),
        confirmLabel: t("library.imageLibrary.actions.delete"),
        cancelLabel: t("common.buttons.cancel"),
        destructive: true,
      });
      if (!confirmed) return;

      try {
        setDeletingItemId(item.id);
        await deleteImageLibraryItem(item);
        setItems((current) => current.filter((entry) => entry.id !== item.id));
        setSelectedItem((current) => (current?.id === item.id ? null : current));

        const storedId = getStoredImageId(item);
        if (storedId) {
          setBackgroundIds((current) => {
            if (!current.has(storedId)) return current;
            const next = new Set(current);
            next.delete(storedId);
            return next;
          });
        }

        toast.success(t("library.imageLibrary.messages.deleted"), item.filename);
      } catch (error) {
        console.error("Failed to delete image library item:", error);
        toast.error(
          t("library.imageLibrary.messages.deleteFailed"),
          error instanceof Error ? error.message : String(error),
        );
      } finally {
        setDeletingItemId((current) => (current === item.id ? null : current));
      }
    },
    [t],
  );

  return (
    <>
      <div className="mb-4 space-y-3">
        {!hideFilterTabs && (
          <div className="flex items-center gap-2 overflow-x-auto pb-1">
            {[
              {
                key: "All" as const,
                label: t("library.imageLibrary.filters.all"),
                count: counts.all,
              },
              {
                key: "Backgrounds" as const,
                label: t("library.imageLibrary.filters.backgrounds"),
                count: counts.backgrounds,
              },
              {
                key: "Avatars" as const,
                label: t("library.imageLibrary.filters.avatars"),
                count: counts.avatars,
              },
              {
                key: "Attachments" as const,
                label: t("library.imageLibrary.filters.attachments"),
                count: counts.attachments,
              },
              {
                key: "Other" as const,
                label: t("library.imageLibrary.filters.other"),
                count: counts.other,
              },
            ].map((option) => (
              <button
                key={option.key}
                type="button"
                onClick={() => setFilter(option.key)}
                className={cn(
                  "flex shrink-0 items-center gap-2 rounded-xl border px-3 py-2 text-sm transition",
                  filter === option.key
                    ? "border-fg/15 bg-fg/10 text-fg"
                    : "border-fg/10 bg-surface-el/20 text-fg/65 hover:bg-fg/5 hover:text-fg",
                )}
              >
                <span className="font-medium">{option.label}</span>
                <span className="rounded-md bg-fg/8 px-1.5 py-0.5 text-[11px] text-fg/55">
                  {option.count}
                </span>
              </button>
            ))}
          </div>
        )}

        <div className="flex items-center gap-3">
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t("library.imageLibrary.searchPlaceholder")}
            className="min-w-0 flex-1 rounded-xl border border-fg/10 bg-surface-el/20 px-3 py-3 text-sm text-fg outline-none transition focus:border-fg/25"
          />
          <button
            type="button"
            onClick={() => setShowSortMenu(true)}
            className="shrink-0 rounded-xl border border-fg/10 bg-surface-el/20 px-3 py-3 text-sm font-medium text-fg/70 transition hover:bg-fg/5 hover:text-fg"
          >
            {t("library.imageLibrary.actions.sort")}
          </button>
        </div>
      </div>

      {loading ? (
        <div className="grid grid-cols-2 gap-3 lg:grid-cols-4 xl:grid-cols-5">
          {Array.from({ length: 12 }).map((_, index) => (
            <div
              key={index}
              className="aspect-square animate-pulse rounded-2xl border border-fg/10 bg-fg/4"
            />
          ))}
        </div>
      ) : filteredItems.length === 0 ? (
        <motion.div
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          className="flex min-h-[45vh] flex-col items-center justify-center rounded-2xl border border-dashed border-fg/12 bg-fg/2 px-6 text-center"
        >
          <div className="mb-4 flex h-16 w-16 items-center justify-center rounded-2xl border border-fg/10 bg-fg/4">
            <ImageIcon className="h-7 w-7 text-fg/35" />
          </div>
          <h2 className="text-lg font-semibold text-fg">{t("library.imageLibrary.empty.title")}</h2>
          <p className="mt-2 max-w-md text-sm text-fg/55">
            {t("library.imageLibrary.empty.description")}
          </p>
        </motion.div>
      ) : (
        <ImageLibraryGrid
          items={filteredItems}
          backgroundIds={backgroundIds}
          scrollContainerRef={scrollContainerRef}
          onSelect={setSelectedItem}
          columnCountOverride={columnCountOverride}
        />
      )}

      <BottomMenu
        isOpen={showSortMenu}
        onClose={() => setShowSortMenu(false)}
        title={t("library.imageLibrary.actions.sort")}
      >
        <div className="space-y-5">
          <div>
            <div className="space-y-2">
              {SORTS.map((option) => (
                <button
                  key={option}
                  type="button"
                  onClick={() => {
                    setSort(option);
                    setShowSortMenu(false);
                  }}
                  className={cn(
                    "flex w-full items-center justify-between rounded-xl px-4 py-3 text-left text-sm transition",
                    sort === option
                      ? "bg-fg/10 text-fg"
                      : "border border-fg/10 bg-surface-el/50 text-fg/65 hover:bg-fg/5 hover:text-fg",
                  )}
                >
                  <span>{option}</span>
                  {sort === option && (
                    <span className="text-xs font-medium text-accent">
                      {t("library.imageLibrary.active")}
                    </span>
                  )}
                </button>
              ))}
            </div>
          </div>
        </div>
      </BottomMenu>

      <BottomMenu
        isOpen={Boolean(selectedItem)}
        onClose={() => setSelectedItem(null)}
        title={selectedItem?.filename ?? ""}
      >
        {selectedItem && (
          <div className={mode === "picker" ? "space-y-3" : "space-y-4"}>
            <div className="overflow-hidden rounded-2xl border border-fg/10 bg-fg/3">
              <img
                src={getAssetUrl(selectedItem.filePath)}
                alt={selectedItem.filename}
                className="max-h-[50vh] w-full object-contain"
              />
            </div>

            {mode === "picker" ? (
              <button
                type="button"
                onClick={() => void handleUseItem(selectedItem)}
                disabled={usingItemId === selectedItem.id}
                className={cn(
                  "flex w-full items-center justify-center gap-2 rounded-xl border px-4 py-3 text-sm font-medium transition",
                  usingItemId === selectedItem.id
                    ? "cursor-wait border-fg/10 bg-fg/6 text-fg/55"
                    : "border-fg/10 bg-fg/4 text-fg/80 hover:bg-fg/8 hover:text-fg",
                )}
              >
                {usingItemId === selectedItem.id ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : null}
                {usingItemId === selectedItem.id
                  ? t("library.imageLibrary.actions.using")
                  : t("library.imageLibrary.actions.useThis")}
              </button>
            ) : (
              <>
                <div className="grid grid-cols-2 gap-2">
                  {[
                    {
                      label: "Kind",
                      value: imageKindLabel(getImageKind(selectedItem, backgroundIds)),
                    },
                    { label: "Type", value: selectedItem.mimeType },
                    {
                      label: "Dimensions",
                      value:
                        selectedItem.width && selectedItem.height
                          ? `${selectedItem.width} × ${selectedItem.height}`
                          : "Unknown",
                    },
                    { label: "Size", value: formatBytes(selectedItem.sizeBytes) },
                    { label: "Updated", value: formatDate(selectedItem.updatedAt) },
                    {
                      label: "Scope",
                      value:
                        getImageKind(selectedItem, backgroundIds) === "Backgrounds"
                          ? "Chat background"
                          : (selectedItem.variant ?? "Standard"),
                    },
                  ].map((field) => (
                    <div
                      key={field.label}
                      className="rounded-xl border border-fg/10 bg-surface-el/60 px-3 py-2.5"
                    >
                      <div className="text-[11px] uppercase tracking-[0.12em] text-fg/40">
                        {field.label}
                      </div>
                      <div className="mt-1 text-[13px] font-medium text-fg">{field.value}</div>
                    </div>
                  ))}
                </div>

                <details className="rounded-xl border border-fg/10 bg-surface-el/60">
                  <summary className="cursor-pointer list-none px-3 py-3">
                    <div className="flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div className="text-[11px] uppercase tracking-[0.12em] text-fg/40">
                          Storage Path
                        </div>
                        <div className="mt-1 truncate font-mono text-[11px] text-fg/60">
                          {compactPath(selectedItem.storagePath)}
                        </div>
                      </div>
                      <div className="shrink-0 text-xs font-medium text-fg/50">Show</div>
                    </div>
                  </summary>
                  <div className="border-t border-fg/10 px-3 pb-3 pt-2">
                    <div className="break-all font-mono text-xs text-fg/75">
                      {selectedItem.storagePath}
                    </div>
                  </div>
                </details>

                {(selectedItem.entityId || selectedItem.sessionId || selectedItem.characterId) && (
                  <details className="rounded-xl border border-fg/10 bg-surface-el/60">
                    <summary className="cursor-pointer list-none px-3 py-3">
                      <div className="flex items-center justify-between gap-3">
                        <div className="min-w-0">
                          <div className="text-[11px] uppercase tracking-[0.12em] text-fg/40">
                            Context
                          </div>
                          <div className="mt-1 truncate text-xs text-fg/60">
                            {[
                              selectedItem.entityType,
                              selectedItem.characterId,
                              selectedItem.sessionId,
                            ]
                              .filter(Boolean)
                              .join(" • ") || "Linked record"}
                          </div>
                        </div>
                        <div className="shrink-0 text-xs font-medium text-fg/50">Show</div>
                      </div>
                    </summary>
                    <div className="border-t border-fg/10 px-3 pb-3 pt-2">
                      <div className="space-y-1 text-sm text-fg/75">
                        {selectedItem.entityType && selectedItem.entityId && (
                          <p>
                            {selectedItem.entityType}:{" "}
                            <span className="font-mono">{selectedItem.entityId}</span>
                          </p>
                        )}
                        {selectedItem.characterId && (
                          <p>
                            character: <span className="font-mono">{selectedItem.characterId}</span>
                          </p>
                        )}
                        {selectedItem.sessionId && (
                          <p>
                            session: <span className="font-mono">{selectedItem.sessionId}</span>
                          </p>
                        )}
                        {selectedItem.role && <p>role: {selectedItem.role}</p>}
                      </div>
                    </div>
                  </details>
                )}

                <div className="grid grid-cols-2 gap-2">
                  <button
                    type="button"
                    onClick={() => copyToClipboard(selectedItem.storagePath, "Storage path")}
                    className="flex items-center justify-center gap-2 rounded-xl border border-fg/10 bg-fg/4 px-4 py-3 text-sm font-medium text-fg/75 transition hover:bg-fg/8 hover:text-fg"
                  >
                    <Copy className="h-4 w-4" />
                    {t("library.imageLibrary.actions.copyPath")}
                  </button>
                  <button
                    type="button"
                    onClick={() => void handleDownload(selectedItem)}
                    disabled={downloadingItemId === selectedItem.id}
                    className={cn(
                      "flex items-center justify-center gap-2 rounded-xl border px-4 py-3 text-sm font-medium transition",
                      downloadingItemId === selectedItem.id
                        ? "cursor-wait border-fg/10 bg-fg/6 text-fg/55"
                        : "border-fg/10 bg-fg/4 text-fg/75 hover:bg-fg/8 hover:text-fg",
                    )}
                  >
                    {downloadingItemId === selectedItem.id ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <Download className="h-4 w-4" />
                    )}
                    {downloadingItemId === selectedItem.id
                      ? t("library.imageLibrary.actions.saving")
                      : t("library.imageLibrary.actions.download")}
                  </button>
                </div>

                <button
                  type="button"
                  onClick={() => void handleDeleteItem(selectedItem)}
                  disabled={deletingItemId === selectedItem.id}
                  className={cn(
                    "flex w-full items-center justify-center gap-2 rounded-xl border px-4 py-3 text-sm font-medium transition",
                    deletingItemId === selectedItem.id
                      ? "cursor-wait border-red-500/15 bg-red-500/8 text-red-200/65"
                      : "border-red-500/20 bg-red-500/10 text-red-200 hover:bg-red-500/16",
                  )}
                >
                  {deletingItemId === selectedItem.id ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <Trash2 className="h-4 w-4" />
                  )}
                  {deletingItemId === selectedItem.id
                    ? t("library.imageLibrary.actions.deleting")
                    : t("library.imageLibrary.actions.delete")}
                </button>
              </>
            )}
          </div>
        )}
      </BottomMenu>
    </>
  );
}

export function AvatarLibraryPickerPage() {
  const mainRef = useRef<HTMLElement | null>(null);
  const navigate = useNavigate();
  const location = useLocation();

  const handleUseItem = useCallback(
    async (item: ImageLibraryItem) => {
      const returnPath =
        typeof location.state === "object" &&
        location.state &&
        "returnPath" in location.state &&
        typeof (location.state as { returnPath?: unknown }).returnPath === "string"
          ? (location.state as { returnPath: string }).returnPath
          : null;

      if (!returnPath) {
        navigate("/library", { replace: true });
        return;
      }

      const selectionKind =
        typeof location.state === "object" &&
        location.state &&
        "selectionKind" in location.state &&
        typeof (location.state as { selectionKind?: unknown }).selectionKind === "string"
          ? (location.state as { selectionKind: string }).selectionKind
          : "avatar";
      const selectionStorageKey =
        typeof location.state === "object" &&
        location.state &&
        "selectionStorageKey" in location.state &&
        typeof (location.state as { selectionStorageKey?: unknown }).selectionStorageKey ===
          "string"
          ? (location.state as { selectionStorageKey: string }).selectionStorageKey
          : returnPath;

      if (selectionKind === "background") {
        const payload: BackgroundLibrarySelectionPayload = {
          filePath: item.filePath,
        };
        sessionStorage.setItem(
          buildBackgroundLibrarySelectionKey(selectionStorageKey),
          JSON.stringify(payload),
        );
      } else {
        const payload: AvatarLibrarySelectionPayload = {
          filePath: item.filePath,
        };
        sessionStorage.setItem(
          buildAvatarLibrarySelectionKey(selectionStorageKey),
          JSON.stringify(payload),
        );
      }
      navigate(returnPath, { replace: true });
    },
    [location.state, navigate],
  );

  return (
    <div className="flex h-full flex-col text-fg/85">
      <main ref={mainRef} className="flex-1 overflow-y-auto px-4 pb-24 pt-4">
        <ImageLibraryPanel scrollContainerRef={mainRef} mode="picker" onUseItem={handleUseItem} />
      </main>
    </div>
  );
}

export function ImageLibraryPage() {
  const mainRef = useRef<HTMLElement | null>(null);

  return (
    <div className="flex h-full flex-col text-fg/85">
      <main ref={mainRef} className="flex-1 overflow-y-auto px-4 pb-24 pt-4">
        <ImageLibraryPanel scrollContainerRef={mainRef} />
      </main>
    </div>
  );
}
