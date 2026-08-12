import { memo, useCallback, useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { createPortal } from "react-dom";
import { useVirtualizer } from "@tanstack/react-virtual";
import { convertFileSrc } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import { ArrowUpDown, Copy, Download, Image as ImageIcon, Loader2, Search, Trash2, X } from "lucide-react";
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
import { useI18n, type TranslationKey, type TranslateParams } from "../../../core/i18n/context";

type TFn = (key: TranslationKey, params?: TranslateParams) => string;
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
const FORMAT_DISPLAY_ORDER = ["webp", "png", "jpg", "jpeg", "gif"] as const;

type ImageLibraryGroup = {
  id: string;
  item: ImageLibraryItem;
  variants: ImageLibraryItem[];
  searchableText: string;
};

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

function formatDate(timestamp: number, t: TFn): string {
  if (!timestamp) return t("library.imageLibrary.unknownDate");
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

function sortLabel(option: SortOption, t: TFn): string {
  if (option === "Newest") return t("library.imageLibrary.sort.newest");
  if (option === "Largest") return t("library.imageLibrary.sort.largest");
  return t("library.imageLibrary.sort.name");
}

function imageKindLabel(kind: Exclude<FilterOption, "All">, t: TFn): string {
  if (kind === "Backgrounds") return t("library.imageLibrary.kinds.background");
  if (kind === "Avatars") return t("library.imageLibrary.kinds.avatar");
  if (kind === "Attachments") return t("library.imageLibrary.kinds.attachment");
  return t("library.imageLibrary.kinds.stored");
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

function copyToClipboard(value: string, label: string, t: TFn) {
  navigator.clipboard
    .writeText(value)
    .then(() => toast.success(t("library.imageLibrary.copy.copied", { label }), value))
    .catch((error) => {
      console.error(`Failed to copy ${label.toLowerCase()}:`, error);
      toast.error(t("library.imageLibrary.copy.failed", { label: label.toLowerCase() }));
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

function getFileExtension(filename: string): string {
  const match = /\.([^.]+)$/.exec(filename);
  return match?.[1]?.toLowerCase() ?? "";
}

function getVariantDisplayLabel(item: ImageLibraryItem): string {
  const extension = getFileExtension(item.filename);
  return extension ? extension.toUpperCase() : item.mimeType;
}

function getGroupingKey(item: ImageLibraryItem): string {
  return item.groupKey;
}

function getFormatSortIndex(item: ImageLibraryItem): number {
  const extension = getFileExtension(item.filename);
  const index = FORMAT_DISPLAY_ORDER.indexOf(
    extension as (typeof FORMAT_DISPLAY_ORDER)[number],
  );
  return index === -1 ? FORMAT_DISPLAY_ORDER.length : index;
}

function sortGroupedVariants(items: ImageLibraryItem[]): ImageLibraryItem[] {
  return [...items].sort(
    (a, b) =>
      getFormatSortIndex(a) - getFormatSortIndex(b) ||
      b.updatedAt - a.updatedAt ||
      a.filename.localeCompare(b.filename),
  );
}

function chooseRepresentativeItem(items: ImageLibraryItem[]): ImageLibraryItem {
  return sortGroupedVariants(items)[0] ?? items[0];
}

function groupImageLibraryItems(items: ImageLibraryItem[]): ImageLibraryGroup[] {
  const groups = new Map<string, ImageLibraryItem[]>();
  for (const item of items) {
    const key = getGroupingKey(item);
    const existing = groups.get(key);
    if (existing) {
      existing.push(item);
    } else {
      groups.set(key, [item]);
    }
  }

  return Array.from(groups.entries()).map(([id, groupItems]) => {
    const variants = sortGroupedVariants(groupItems);
    const item = chooseRepresentativeItem(variants);
    const searchableText = [
      ...variants.map((variant) => variant.filename.toLowerCase()),
      ...variants.map((variant) => variant.storagePath.toLowerCase()),
      ...variants.map((variant) => variant.entityId?.toLowerCase() ?? ""),
      ...variants.map((variant) => variant.sessionId?.toLowerCase() ?? ""),
      ...variants.map((variant) => variant.characterId?.toLowerCase() ?? ""),
    ].join("\n");

    return { id, item, variants, searchableText };
  });
}

const ImageTile = memo(function ImageTile({
  group,
  kindLabel,
  onSelect,
}: {
  group: ImageLibraryGroup;
  kindLabel: string;
  onSelect: (group: ImageLibraryGroup) => void;
}) {
  const { item, variants } = group;
  return (
    <button
      type="button"
      onClick={() => onSelect(group)}
      className="group relative aspect-square overflow-hidden rounded-2xl border border-fg/10 bg-fg/3 text-left transition hover:border-fg/20 hover:shadow-lg hover:shadow-black/20"
    >
      <img
        src={getAssetUrl(item.filePath)}
        alt={item.filename}
        className="h-full w-full object-cover transition duration-300 group-hover:scale-[1.04]"
        loading="lazy"
        decoding="async"
      />
      <div className="pointer-events-none absolute inset-0 bg-linear-to-t from-black/75 via-black/0 to-transparent opacity-0 transition-opacity duration-200 group-hover:opacity-100" />
      <div className="absolute left-2 top-2 flex items-center gap-1.5 rounded-md border border-white/10 bg-black/65 px-1.5 py-0.5 text-[10px] font-medium text-white shadow-sm shadow-black/40 backdrop-blur-md">
        <span className="h-1.5 w-1.5 rounded-full bg-white/80" />
        {kindLabel}
      </div>
      {variants.length > 1 ? (
        <div className="absolute right-2 top-2 rounded-md border border-white/10 bg-black/65 px-1.5 py-0.5 text-[10px] font-medium text-white shadow-sm shadow-black/40 backdrop-blur-md">
          {variants.length}×
        </div>
      ) : null}
      <div className="pointer-events-none absolute inset-x-0 bottom-0 translate-y-1 p-2.5 opacity-0 transition-all duration-200 group-hover:translate-y-0 group-hover:opacity-100">
        <div className="flex items-center justify-between gap-2 text-[11px] text-white/85">
          <span className="truncate">
            {item.width && item.height ? `${item.width} × ${item.height}` : item.mimeType}
          </span>
          <span className="shrink-0">{formatBytes(item.sizeBytes)}</span>
        </div>
      </div>
    </button>
  );
});

const ImageLibraryGrid = memo(function ImageLibraryGrid({
  groups,
  backgroundIds,
  scrollContainerRef,
  onSelect,
  columnCountOverride,
}: {
  groups: ImageLibraryGroup[];
  backgroundIds: Set<string>;
  scrollContainerRef: RefObject<HTMLElement | null>;
  onSelect: (group: ImageLibraryGroup) => void;
  columnCountOverride?: number;
}) {
  const { t } = useI18n();
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
  }, [groups.length]);

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

  const rowCount = Math.ceil(groups.length / columnCount);

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
        const rowItems = groups.slice(startIndex, startIndex + columnCount);

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
            {rowItems.map((group) => (
              <ImageTile
                key={group.id}
                group={group}
                kindLabel={imageKindLabel(getImageKind(group.item, backgroundIds), t)}
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
  toolbarHost,
}: {
  scrollContainerRef: RefObject<HTMLElement | null>;
  embedded?: boolean;
  mode?: "default" | "picker";
  onUseItem?: (item: ImageLibraryItem) => Promise<void> | void;
  fixedFilter?: FilterOption;
  hideFilterTabs?: boolean;
  columnCountOverride?: number;
  toolbarHost?: HTMLElement | null;
}) {
  const { t } = useI18n();
  const [items, setItems] = useState<ImageLibraryItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<FilterOption>(fixedFilter ?? "All");
  const [sort, setSort] = useState<SortOption>("Newest");
  const [showSortMenu, setShowSortMenu] = useState(false);
  const [selectedGroup, setSelectedGroup] = useState<ImageLibraryGroup | null>(null);
  const [backgroundIds, setBackgroundIds] = useState<Set<string>>(new Set());
  const [downloadingItemId, setDownloadingItemId] = useState<string | null>(null);
  const [usingItemId, setUsingItemId] = useState<string | null>(null);
  const [deletingItemId, setDeletingItemId] = useState<string | null>(null);
  const [showDownloadMenu, setShowDownloadMenu] = useState(false);
  const [searchFocused, setSearchFocused] = useState(false);
  const [isWide, setIsWide] = useState(false);

  useEffect(() => {
    const mq = window.matchMedia("(min-width: 1024px)");
    const sync = () => setIsWide(mq.matches);
    sync();
    mq.addEventListener("change", sync);
    return () => mq.removeEventListener("change", sync);
  }, []);

  const collapseChips = isWide && !hideFilterTabs && (searchFocused || query.trim().length > 0);

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

  const groups = useMemo(() => groupImageLibraryItems(items), [items]);

  const counts = useMemo(
    () => ({
      all: groups.length,
      backgrounds: groups.filter((group) => getImageKind(group.item, backgroundIds) === "Backgrounds")
        .length,
      avatars: groups.filter((group) => getImageKind(group.item, backgroundIds) === "Avatars").length,
      attachments: groups.filter((group) => getImageKind(group.item, backgroundIds) === "Attachments")
        .length,
      other: groups.filter((group) => getImageKind(group.item, backgroundIds) === "Other").length,
    }),
    [backgroundIds, groups],
  );

  const filteredGroups = useMemo(() => {
    const lowered = query.trim().toLowerCase();
    const next = groups.filter((group) => {
      if (!matchesFilter(group.item, filter, backgroundIds)) return false;
      if (!lowered) return true;
      return group.searchableText.includes(lowered);
    });
    return sortItems(
      next.map((group) => group.item),
      sort,
    )
      .map((item) => next.find((group) => group.item.id === item.id))
      .filter((group): group is ImageLibraryGroup => Boolean(group));
  }, [backgroundIds, filter, groups, query, sort]);

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
        setSelectedGroup((current) => {
          if (!current) return null;
          const remainingVariants = current.variants.filter((variant) => variant.id !== item.id);
          if (remainingVariants.length === 0) return null;
          return {
            ...current,
            item: chooseRepresentativeItem(remainingVariants),
            variants: sortGroupedVariants(remainingVariants),
            searchableText: [
              ...remainingVariants.map((variant) => variant.filename.toLowerCase()),
              ...remainingVariants.map((variant) => variant.storagePath.toLowerCase()),
              ...remainingVariants.map((variant) => variant.entityId?.toLowerCase() ?? ""),
              ...remainingVariants.map((variant) => variant.sessionId?.toLowerCase() ?? ""),
              ...remainingVariants.map((variant) => variant.characterId?.toLowerCase() ?? ""),
            ].join("\n"),
          };
        });

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

  const toolbarMarkup = (
    <div
      className={cn(
        "flex flex-col gap-2 lg:flex-row lg:items-center lg:gap-3",
        toolbarHost ? "min-w-0 flex-1" : "mb-4",
      )}
    >
        {!hideFilterTabs && (
          <motion.div
            className="flex shrink-0 items-center overflow-hidden"
            animate={{
              width: collapseChips ? 0 : "auto",
              opacity: collapseChips ? 0 : 1,
              marginRight: collapseChips ? -12 : 0,
            }}
            initial={false}
            transition={{ type: "spring", stiffness: 380, damping: 36, mass: 0.7 }}
          >
          <div className="flex items-center gap-1 overflow-x-auto rounded-xl border border-fg/8 bg-surface-el/40 p-1 [-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
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
            ].map((option) => {
              const isActive = filter === option.key;
              return (
                <button
                  key={option.key}
                  type="button"
                  onClick={() => setFilter(option.key)}
                  className={cn(
                    "relative flex shrink-0 items-center gap-1.5 rounded-lg px-3 py-1.5 text-[13px] transition-colors",
                    isActive
                      ? "text-fg"
                      : "text-fg/60 hover:text-fg",
                  )}
                >
                  {isActive && (
                    <motion.span
                      layoutId="image-library-filter-pill"
                      className="absolute inset-0 rounded-lg bg-fg/12 shadow-sm shadow-black/20"
                      transition={{ type: "spring", stiffness: 500, damping: 38, mass: 0.6 }}
                    />
                  )}
                  <span className="relative z-10 font-medium">{option.label}</span>
                  <span
                    className={cn(
                      "relative z-10 rounded-md px-1 text-[11px] tabular-nums transition-colors",
                      isActive ? "bg-fg/10 text-fg/70" : "text-fg/40",
                    )}
                  >
                    {option.count}
                  </span>
                </button>
              );
            })}
          </div>
          </motion.div>
        )}

        <div className="flex min-w-0 flex-1 items-center gap-2">
          <div className="relative min-w-0 flex-1">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-fg/35" />
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              onFocus={() => setSearchFocused(true)}
              onBlur={() => setSearchFocused(false)}
              onKeyDown={(event) => {
                if (event.key === "Escape") {
                  setQuery("");
                  (event.target as HTMLInputElement).blur();
                }
              }}
              placeholder={t("library.imageLibrary.searchPlaceholder")}
              className={cn(
                "w-full rounded-xl border border-fg/10 bg-surface-el/20 py-2.5 pl-9 text-sm text-fg outline-none transition focus:border-fg/25",
                query ? "pr-9" : "pr-3",
              )}
            />
            {query && (
              <button
                type="button"
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => setQuery("")}
                className="absolute right-2 top-1/2 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded-md text-fg/45 transition hover:bg-fg/10 hover:text-fg"
                title={t("library.imageLibrary.clearSearch")}
              >
                <X className="h-3.5 w-3.5" />
              </button>
            )}
          </div>
          <button
            type="button"
            onClick={() => setShowSortMenu(true)}
            className="flex shrink-0 items-center gap-1.5 rounded-xl border border-fg/10 bg-surface-el/20 px-3 py-2.5 text-sm font-medium text-fg/70 transition hover:bg-fg/5 hover:text-fg"
            title={t("library.imageLibrary.actions.sort")}
          >
            <ArrowUpDown className="h-4 w-4" />
            <span className="hidden text-xs text-fg/55 sm:inline">{sortLabel(sort, t)}</span>
          </button>
        </div>
      </div>
  );

  return (
    <>
      {toolbarHost ? createPortal(toolbarMarkup, toolbarHost) : toolbarMarkup}

      {loading ? (
        <div className="grid grid-cols-2 gap-3 lg:grid-cols-4 xl:grid-cols-5">
          {Array.from({ length: 12 }).map((_, index) => (
            <div
              key={index}
              className="aspect-square animate-pulse rounded-2xl border border-fg/10 bg-fg/4"
            />
          ))}
        </div>
      ) : filteredGroups.length === 0 ? (
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
          groups={filteredGroups}
          backgroundIds={backgroundIds}
          scrollContainerRef={scrollContainerRef}
          onSelect={setSelectedGroup}
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
                  <span>{sortLabel(option, t)}</span>
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
        isOpen={Boolean(selectedGroup)}
        onClose={() => setSelectedGroup(null)}
        title={
          selectedGroup
            ? t("library.imageLibrary.detailsTitle", {
                kind: imageKindLabel(getImageKind(selectedGroup.item, backgroundIds), t),
              })
            : ""
        }
      >
        {selectedGroup && (() => {
          const item = selectedGroup.item;
          const kind = getImageKind(item, backgroundIds);
          const kindLabel = imageKindLabel(kind, t);
          const dimensions =
            item.width && item.height ? `${item.width} × ${item.height}` : null;
          const ext = getFileExtension(item.filename).toUpperCase();
          const downloading = selectedGroup.variants.some(
            (variant) => downloadingItemId === variant.id,
          );
          const isDeleting = deletingItemId === item.id;
          const hasContext = Boolean(item.entityId || item.sessionId || item.characterId);

          return (
            <div className={mode === "picker" ? "space-y-3" : "space-y-3"}>
              <div className="overflow-hidden rounded-2xl bg-fg/3 ring-1 ring-fg/8">
                <img
                  src={getAssetUrl(item.filePath)}
                  alt={item.filename}
                  className="max-h-[48vh] w-full object-contain"
                />
              </div>

              {mode === "picker" ? (
                <button
                  type="button"
                  onClick={() => void handleUseItem(item)}
                  disabled={usingItemId === item.id}
                  className={cn(
                    "flex w-full items-center justify-center gap-2 rounded-xl border px-4 py-3 text-sm font-medium transition",
                    usingItemId === item.id
                      ? "cursor-wait border-fg/10 bg-fg/6 text-fg/55"
                      : "border-fg/10 bg-fg/4 text-fg/80 hover:bg-fg/8 hover:text-fg",
                  )}
                >
                  {usingItemId === item.id ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : null}
                  {usingItemId === item.id
                    ? t("library.imageLibrary.actions.using")
                    : t("library.imageLibrary.actions.useThis")}
                </button>
              ) : (
                <>
                  <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5 text-[12px] text-fg/65">
                    <span className="inline-flex items-center gap-1.5 rounded-full bg-fg/8 px-2 py-0.5 text-[11px] font-medium text-fg">
                      <span className="h-1.5 w-1.5 rounded-full bg-fg/60" />
                      {kindLabel}
                    </span>
                    {dimensions && <span className="tabular-nums">{dimensions}</span>}
                    <span className="tabular-nums">{formatBytes(item.sizeBytes)}</span>
                    {ext && <span className="font-mono uppercase tracking-wide">{ext}</span>}
                    <span className="text-fg/45">·</span>
                    <span>{formatDate(item.updatedAt, t)}</span>
                  </div>

                  <button
                    type="button"
                    onClick={() => copyToClipboard(item.filename, t("library.imageLibrary.copyLabels.filename"), t)}
                    className="flex w-full items-center gap-2 rounded-xl border border-fg/10 bg-surface-el/40 px-3 py-2 text-left text-fg/75 transition hover:bg-fg/5 hover:text-fg"
                    title={t("library.imageLibrary.copyFilename")}
                  >
                    <Copy className="h-3.5 w-3.5 shrink-0 text-fg/40" />
                    <span className="min-w-0 truncate font-mono text-[12px]">
                      {item.filename}
                    </span>
                  </button>

                  {selectedGroup.variants.length > 1 && (
                    <div className="flex flex-wrap items-center gap-1.5">
                      <span className="text-[11px] uppercase tracking-[0.12em] text-fg/40">
                        {t("library.imageLibrary.formatsLabel")}
                      </span>
                      {selectedGroup.variants.map((variant) => (
                        <span
                          key={variant.id}
                          className="rounded-md bg-fg/6 px-2 py-0.5 text-[11px] font-medium text-fg/75"
                        >
                          {getVariantDisplayLabel(variant)}
                        </span>
                      ))}
                    </div>
                  )}

                  <details className="group rounded-xl bg-surface-el/40">
                    <summary className="flex cursor-pointer list-none items-center justify-between gap-3 px-3 py-2.5 text-[12px]">
                      <span className="text-fg/55">
                        {t("library.imageLibrary.storagePath")}
                      </span>
                      <span className="min-w-0 flex-1 truncate text-right font-mono text-[11px] text-fg/55">
                        {compactPath(item.storagePath)}
                      </span>
                      <span className="shrink-0 text-[11px] text-fg/40 transition group-open:hidden">
                        {t("library.imageLibrary.show")}
                      </span>
                      <span className="hidden shrink-0 text-[11px] text-fg/40 group-open:inline">
                        {t("library.imageLibrary.hide")}
                      </span>
                    </summary>
                    <div className="border-t border-fg/8 px-3 pb-3 pt-2">
                      <div className="break-all font-mono text-[11px] text-fg/70">
                        {item.storagePath}
                      </div>
                    </div>
                  </details>

                  {hasContext && (
                    <details className="group rounded-xl bg-surface-el/40">
                      <summary className="flex cursor-pointer list-none items-center justify-between gap-3 px-3 py-2.5 text-[12px]">
                        <span className="text-fg/55">{t("library.imageLibrary.contextLabel")}</span>
                        <span className="min-w-0 flex-1 truncate text-right text-[11px] text-fg/55">
                          {[item.entityType, item.characterId, item.sessionId]
                            .filter(Boolean)
                            .join(" · ") || t("library.imageLibrary.contextLinkedFallback")}
                        </span>
                        <span className="shrink-0 text-[11px] text-fg/40 group-open:hidden">
                          {t("library.imageLibrary.show")}
                        </span>
                        <span className="hidden shrink-0 text-[11px] text-fg/40 group-open:inline">
                          {t("library.imageLibrary.hide")}
                        </span>
                      </summary>
                      <div className="border-t border-fg/8 px-3 pb-3 pt-2">
                        <div className="space-y-1 text-[12px] text-fg/70">
                          {item.entityType && item.entityId && (
                            <p>
                              <span className="text-fg/45">{item.entityType}:</span>{" "}
                              <span className="font-mono">{item.entityId}</span>
                            </p>
                          )}
                          {item.characterId && (
                            <p>
                              <span className="text-fg/45">{t("library.imageLibrary.contextRoles.character")}</span>{" "}
                              <span className="font-mono">{item.characterId}</span>
                            </p>
                          )}
                          {item.sessionId && (
                            <p>
                              <span className="text-fg/45">{t("library.imageLibrary.contextRoles.session")}</span>{" "}
                              <span className="font-mono">{item.sessionId}</span>
                            </p>
                          )}
                          {item.role && (
                            <p>
                              <span className="text-fg/45">{t("library.imageLibrary.contextRoles.role")}</span> {item.role}
                            </p>
                          )}
                        </div>
                      </div>
                    </details>
                  )}

                  <div className="flex items-center gap-2 pt-1">
                    <button
                      type="button"
                      onClick={() => copyToClipboard(item.storagePath, t("library.imageLibrary.copyLabels.storagePath"), t)}
                      className="flex flex-1 items-center justify-center gap-2 rounded-xl border border-fg/10 bg-fg/4 px-3 py-2.5 text-sm font-medium text-fg/75 transition hover:bg-fg/8 hover:text-fg"
                    >
                      <Copy className="h-4 w-4" />
                      {t("library.imageLibrary.actions.copyPath")}
                    </button>
                    <button
                      type="button"
                      onClick={() => {
                        if (selectedGroup.variants.length > 1) {
                          setShowDownloadMenu(true);
                          return;
                        }
                        void handleDownload(item);
                      }}
                      disabled={downloading}
                      className={cn(
                        "flex flex-1 items-center justify-center gap-2 rounded-xl border px-3 py-2.5 text-sm font-medium transition",
                        downloading
                          ? "cursor-wait border-fg/10 bg-fg/6 text-fg/55"
                          : "border-accent/30 bg-accent/10 text-accent hover:bg-accent/20",
                      )}
                    >
                      {downloading ? (
                        <Loader2 className="h-4 w-4 animate-spin" />
                      ) : (
                        <Download className="h-4 w-4" />
                      )}
                      {downloading
                        ? t("library.imageLibrary.actions.saving")
                        : selectedGroup.variants.length > 1
                          ? t("library.imageLibrary.downloadFormat", {
                              download: t("library.imageLibrary.actions.download"),
                            })
                          : t("library.imageLibrary.actions.download")}
                    </button>
                    <button
                      type="button"
                      onClick={() => void handleDeleteItem(item)}
                      disabled={isDeleting}
                      title={t("library.imageLibrary.actions.delete")}
                      className={cn(
                        "flex h-10.5 w-10.5 shrink-0 items-center justify-center rounded-xl border transition",
                        isDeleting
                          ? "cursor-wait border-red-500/15 bg-red-500/8 text-red-200/65"
                          : "border-red-500/20 bg-red-500/10 text-red-200 hover:bg-red-500/20",
                      )}
                    >
                      {isDeleting ? (
                        <Loader2 className="h-4 w-4 animate-spin" />
                      ) : (
                        <Trash2 className="h-4 w-4" />
                      )}
                    </button>
                  </div>
                </>
              )}
            </div>
          );
        })()}
      </BottomMenu>

      <BottomMenu
        isOpen={showDownloadMenu}
        onClose={() => setShowDownloadMenu(false)}
        title={t("library.imageLibrary.actions.download")}
      >
        <div className="space-y-2">
          {selectedGroup?.variants.map((variant) => {
            const isDownloading = downloadingItemId === variant.id;
            return (
              <button
                key={variant.id}
                type="button"
                onClick={() => {
                  setShowDownloadMenu(false);
                  void handleDownload(variant);
                }}
                disabled={isDownloading}
                className={cn(
                  "flex w-full items-center justify-between rounded-xl border px-4 py-3 text-left text-sm transition",
                  isDownloading
                    ? "cursor-wait border-fg/10 bg-fg/6 text-fg/55"
                    : "border-fg/10 bg-surface-el/50 text-fg/75 hover:bg-fg/5 hover:text-fg",
                )}
              >
                <span className="font-medium">{getVariantDisplayLabel(variant)}</span>
                <span className="text-xs text-fg/55">{formatBytes(variant.sizeBytes)}</span>
              </button>
            );
          })}
        </div>
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
