import { useEffect, useState, useMemo, useRef } from "react";
import { useNavigate, useParams, useSearchParams, useLocation } from "react-router-dom";
import {
  BookOpen,
  Trash2,
  ChevronRight,
  Star,
  Edit2,
  Download,
  Loader2,
  Search,
  Sparkles,
  GripVertical,
  TestTube2,
  X,
} from "lucide-react";
import { motion, AnimatePresence, type PanInfo, useDragControls } from "framer-motion";
import type { Lorebook, LorebookEntry } from "../../../core/storage/schemas";
import {
  deleteLorebook,
  createBlankLorebookEntry,
  deleteLorebookEntry,
  listCharacterLorebooks,
  listGroupLorebooks,
  listGroupSessionLorebooks,
  listLorebooks,
  listLorebookEntries,
  saveLorebook,
  saveLorebookEntry,
  setCharacterLorebooks,
  setGroupLorebooks,
  setGroupSessionLorebooks,
  reorderLorebookEntries,
} from "../../../core/storage/repo";
import { convertToImageRef, deleteImageRef } from "../../../core/storage";
import { convertFilePathToDataUrl } from "../../../core/storage/images";
import {
  buildAvatarLibrarySelectionKey,
  type AvatarLibrarySelectionPayload,
} from "../../components/AvatarPicker/librarySelection";
import {
  exportLorebook,
  exportLorebookAsUsc,
  downloadJson,
  generateLorebookExportFilenameWithFormat,
} from "../../../core/storage/lorebookTransfer";
import {
  BottomMenu,
  LorebookExportMenu,
  LorebookMetadataMenu,
  MenuButton,
} from "../../components";
import { generateLorebookKeywordDraft } from "../../../core/chat/manager";
import { LorebookAvatar } from "../../components/LorebookAvatar";
import { Switch } from "../../components/Switch";
import { confirmBottomMenu } from "../../components/ConfirmBottomMenu";
import { TopNav } from "../../components/App";
import { toast } from "../../components/toast";
import { useI18n } from "../../../core/i18n/context";
import { Routes, useNavigationManager } from "../../navigation";
import type { LorebookExportFormat } from "../../components/LorebookExportMenu";

const DRAG_HOLD_MS = 450;

function KeywordTagInput({
  keywords,
  onChange,
  caseSensitive,
  onCaseSensitiveChange,
  onGenerate,
  isGenerating,
  canGenerate,
}: {
  keywords: string[];
  onChange: (keywords: string[]) => void;
  caseSensitive: boolean;
  onCaseSensitiveChange: (caseSensitive: boolean) => void;
  onGenerate?: () => void;
  isGenerating?: boolean;
  canGenerate?: boolean;
}) {
  const { t } = useI18n();
  const [inputValue, setInputValue] = useState("");

  const addKeyword = () => {
    const newKeyword = inputValue.trim();
    if (newKeyword && !keywords.includes(newKeyword)) {
      onChange([...keywords, newKeyword]);
      setInputValue("");
    }
  };

  const removeKeyword = (index: number) => {
    onChange(keywords.filter((_, i) => i !== index));
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <label className="text-[11px] font-medium text-fg/70">
          {t("characters.lorebook.keywords")}
        </label>
        <div className="flex items-center gap-3">
          {onGenerate && (
            <button
              type="button"
              onClick={onGenerate}
              disabled={isGenerating || !canGenerate}
              title="Generate keywords from content"
              className="inline-flex items-center gap-1.5 rounded-full border border-accent/30 bg-accent/10 px-2.5 py-1 text-[11px] font-medium text-accent transition hover:bg-accent/20 disabled:cursor-not-allowed disabled:opacity-40"
            >
              {isGenerating ? (
                <Loader2 className="h-3 w-3 animate-spin" />
              ) : (
                <Sparkles className="h-3 w-3" />
              )}
              Generate
            </button>
          )}
          <span className="text-xs text-fg/50">{t("characters.lorebook.caseSensitive")}</span>
          <label
            className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-all duration-200 ${
              caseSensitive ? "bg-accent" : "bg-fg/20"
            }`}
          >
            <input
              type="checkbox"
              checked={caseSensitive}
              onChange={(e) => onCaseSensitiveChange(e.target.checked)}
              className="sr-only"
            />
            <span
              className={`inline-block h-4 w-4 transform rounded-full bg-fg shadow ring-0 transition duration-200 ${
                caseSensitive ? "translate-x-4" : "translate-x-0"
              }`}
            />
          </label>
        </div>
      </div>

      {/* Input with Add button */}
      <div className="flex gap-2">
        <input
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          placeholder={t("characters.lorebook.typeKeyword")}
          className="flex-1 rounded-xl border border-fg/10 bg-surface-el/20 px-3 py-2.5 text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none"
        />
        <button
          type="button"
          onClick={addKeyword}
          disabled={!inputValue.trim()}
          className="rounded-xl border border-accent/40 bg-accent/20 px-4 py-2.5 text-sm font-medium text-accent/70 transition hover:bg-accent/30 disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {t("characters.lorebook.addButton")}
        </button>
      </div>

      {/* Keyword chips */}
      {keywords.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {keywords.map((keyword, index) => (
            <button
              key={`${keyword}-${index}`}
              type="button"
              onClick={() => removeKeyword(index)}
              className="inline-flex items-center gap-1.5 rounded-full bg-fg/10 pl-3 pr-2 py-1.5 text-sm text-fg active:bg-fg/20 transition"
            >
              {keyword}
              <span className="flex h-5 w-5 items-center justify-center rounded-full bg-fg/10">
                <X size={12} className="text-fg/70" />
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function EntryListItem({
  entry,
  originalIndex,
  isDragging,
  onDrag,
  onDragEnd,
  onSelect,
}: {
  entry: LorebookEntry;
  originalIndex: number;
  isDragging: boolean;
  onDrag: (fromIndex: number, info: { offset: { y: number } }) => void;
  onDragEnd: (fromIndex: number, info: PanInfo) => void;
  onSelect: (entry: LorebookEntry) => void;
}) {
  const controls = useDragControls();
  const dragTimeoutRef = useRef<number | null>(null);
  const draggingRef = useRef(false);
  const pendingEventRef = useRef<PointerEvent | null>(null);
  const scrollLockRef = useRef<{
    el: HTMLElement;
    overflow: string;
    touchAction: string;
    scrollTop: number;
  } | null>(null);

  const scheduleDragStart = (event: React.PointerEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();
    event.currentTarget.setPointerCapture?.(event.pointerId);
    pendingEventRef.current = event.nativeEvent;
    if (event.pointerType === "mouse") {
      draggingRef.current = true;
      controls.start(event.nativeEvent);
      return;
    }
    if (dragTimeoutRef.current) {
      window.clearTimeout(dragTimeoutRef.current);
    }
    dragTimeoutRef.current = window.setTimeout(() => {
      dragTimeoutRef.current = null;
      const pendingEvent = pendingEventRef.current;
      if (pendingEvent) {
        draggingRef.current = true;
        controls.start(pendingEvent);
      }
    }, DRAG_HOLD_MS);
  };

  const cancelDragStart = () => {
    if (dragTimeoutRef.current) {
      window.clearTimeout(dragTimeoutRef.current);
      dragTimeoutRef.current = null;
    }
    pendingEventRef.current = null;
  };

  const lockScrollContainer = () => {
    const scrollEl = document.querySelector("main") as HTMLElement | null;
    if (!scrollEl || scrollLockRef.current) return;
    scrollLockRef.current = {
      el: scrollEl,
      overflow: scrollEl.style.overflow,
      touchAction: scrollEl.style.touchAction,
      scrollTop: scrollEl.scrollTop,
    };
    scrollEl.style.overflow = "hidden";
    scrollEl.style.touchAction = "none";
  };

  const unlockScrollContainer = () => {
    if (!scrollLockRef.current) return;
    const { el, overflow, touchAction, scrollTop } = scrollLockRef.current;
    el.style.overflow = overflow;
    el.style.touchAction = touchAction;
    el.scrollTop = scrollTop;
    scrollLockRef.current = null;
  };

  const { t } = useI18n();
  const displayTitle =
    entry.title?.trim() || entry.keywords[0] || t("characters.lorebook.untitledEntry");
  const displaySubtitle = entry.alwaysActive
    ? t("characters.lorebook.alwaysActive")
    : entry.keywords.length > 0
      ? entry.keywords.slice(0, 3).join(", ") + (entry.keywords.length > 3 ? "..." : "")
      : t("characters.lorebook.noKeywords");

  return (
    <motion.div
      layout
      layoutId={entry.id}
      drag="y"
      dragListener={false}
      dragControls={controls}
      dragElastic={0.15}
      dragMomentum={false}
      dragSnapToOrigin
      dragTransition={{ bounceStiffness: 300, bounceDamping: 25 }}
      onDragStart={() => {
        draggingRef.current = true;
        document.body.style.overflow = "hidden";
        document.body.style.touchAction = "none";
        lockScrollContainer();
      }}
      onDrag={(_, info) => onDrag(originalIndex, info)}
      onDragEnd={(_, info) => onDragEnd(originalIndex, info)}
      onPointerMove={(event) => {
        if (dragTimeoutRef.current) {
          pendingEventRef.current = event.nativeEvent;
        }
        if (draggingRef.current) {
          event.preventDefault();
        }
      }}
      onPointerUp={(event) => {
        cancelDragStart();
        draggingRef.current = false;
        event.currentTarget.releasePointerCapture?.(event.pointerId);
        document.body.style.overflow = "";
        document.body.style.touchAction = "";
        unlockScrollContainer();
      }}
      onPointerCancel={(event) => {
        cancelDragStart();
        draggingRef.current = false;
        event.currentTarget.releasePointerCapture?.(event.pointerId);
        document.body.style.overflow = "";
        document.body.style.touchAction = "";
        unlockScrollContainer();
      }}
      whileDrag={{ scale: 1.03, zIndex: 50, boxShadow: "0 15px 40px rgba(0,0,0,0.5)" }}
      transition={{ type: "spring", stiffness: 400, damping: 30 }}
      className={`group relative flex w-full items-center gap-2 overflow-hidden rounded-xl border px-2 py-3 ${isDragging ? "opacity-0" : "cursor-grab active:cursor-grabbing"} ${
        entry.enabled
          ? "border-fg/10 bg-surface-el/90 hover:border-fg/25 hover:bg-surface-el/95"
          : "border-fg/10 bg-surface-el/60 opacity-60 hover:opacity-80"
      }`}
    >
      <button
        type="button"
        onPointerDown={scheduleDragStart}
        onPointerUp={cancelDragStart}
        onPointerLeave={cancelDragStart}
        onPointerCancel={cancelDragStart}
        onContextMenu={(event) => event.preventDefault()}
        className="flex h-10 w-8 shrink-0 items-center justify-center text-fg/30"
        style={{ touchAction: "none" }}
        title={t("characters.lorebook.dragToReorder")}
      >
        <GripVertical size={18} />
      </button>

      <button
        type="button"
        onClick={() => onSelect(entry)}
        className="flex flex-1 items-center gap-3 text-left active:scale-[0.995]"
      >
        <div
          className={`relative flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border ${
            entry.enabled
              ? entry.alwaysActive
                ? "border-info/40 bg-info/20"
                : "border-accent/40 bg-accent/20"
              : "border-fg/10 bg-fg/5"
          }`}
        >
          <BookOpen
            className={`h-5 w-5 ${
              entry.enabled ? (entry.alwaysActive ? "text-info" : "text-accent/80") : "text-fg/40"
            }`}
          />
        </div>

        <div className="relative min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <h3 className="truncate text-sm font-semibold text-fg">{displayTitle}</h3>
            {!entry.enabled && (
              <span className="text-[10px] uppercase tracking-wide text-fg/40">
                {t("characters.lorebook.disabled")}
              </span>
            )}
          </div>
          <p className="line-clamp-1 text-xs text-fg/50">{displaySubtitle}</p>
        </div>

        <span className="relative flex h-9 w-9 shrink-0 items-center justify-center rounded-full border border-fg/10 bg-fg/5 text-fg/70 group-hover:border-fg/25 group-hover:text-fg transition">
          <ChevronRight size={16} />
        </span>
      </button>
    </motion.div>
  );
}

function LorebookListView({
  lorebooks,
  assignedLorebookIds,
  loading,
  enableLabel,
  disableLabel,
  enableDescription,
  disableDescription,
  onSelectLorebook,
  onToggleAssignment,
  onCreateLorebook,
  onDeleteLorebook,
}: {
  lorebooks: Lorebook[];
  assignedLorebookIds: Set<string>;
  loading: boolean;
  enableLabel: string;
  disableLabel: string;
  enableDescription: string;
  disableDescription: string;
  onSelectLorebook: (id: string) => void;
  onToggleAssignment: (id: string, enabled: boolean) => void;
  onCreateLorebook: (name: string) => void;
  onDeleteLorebook: (id: string) => void;
}) {
  const { t } = useI18n();
  const [selectedLorebook, setSelectedLorebook] = useState<Lorebook | null>(null);
  const [showCreateMenu, setShowCreateMenu] = useState(false);
  const [newName, setNewName] = useState("");
  const [searchQuery, setSearchQuery] = useState("");
  const [entryCounts, setEntryCounts] = useState<Map<string, number>>(new Map());

  useEffect(() => {
    if (lorebooks.length === 0) {
      setEntryCounts(new Map());
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const results = await Promise.all(
          lorebooks.map((lb) =>
            listLorebookEntries(lb.id)
              .then((list) => [lb.id, list.length] as const)
              .catch(() => [lb.id, 0] as const),
          ),
        );
        if (cancelled) return;
        setEntryCounts(new Map(results));
      } catch (error) {
        console.error("Failed to count lorebook entries:", error);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [lorebooks]);

  // Listen for add event from TopNav
  useEffect(() => {
    const handleAdd = () => setShowCreateMenu(true);
    window.addEventListener("lorebook:add", handleAdd);
    return () => window.removeEventListener("lorebook:add", handleAdd);
  }, []);

  const handleCreate = () => {
    if (!newName.trim()) return;
    onCreateLorebook(newName.trim());
    setNewName("");
    setShowCreateMenu(false);
  };

  const filteredLorebooks = useMemo(() => {
    if (!searchQuery.trim()) return lorebooks;
    const query = searchQuery.toLowerCase();
    return lorebooks.filter((l) => l.name.toLowerCase().includes(query));
  }, [lorebooks, searchQuery]);

  // Empty state
  const EmptyState = () => (
    <div className="flex h-64 flex-col items-center justify-center">
      <BookOpen className="mb-3 h-12 w-12 text-fg/20" />
      <h3 className="mb-1 text-lg font-medium text-fg">
        {t("characters.lorebook.noLorebooksYet")}
      </h3>
      <p className="mb-4 text-center text-sm text-fg/50">
        {t("characters.lorebook.createLorebookDesc")}
      </p>
      <button
        onClick={() => setShowCreateMenu(true)}
        className="rounded-full border border-accent/40 bg-accent/20 px-6 py-2 text-sm font-medium text-accent/70 transition hover:bg-accent/30"
      >
        {t("characters.lorebook.createLorebook")}
      </button>
    </div>
  );

  // Skeleton
  const Skeleton = () => (
    <div className="space-y-3">
      {[1, 2, 3].map((i) => (
        <div key={i} className="rounded-xl border border-fg/10 bg-surface-el/90 px-4 py-3">
          <div className="flex items-center gap-3">
            <div className="h-10 w-10 animate-pulse rounded-lg bg-fg/10" />
            <div className="flex-1 space-y-2">
              <div className="h-4 w-24 animate-pulse rounded bg-fg/10" />
              <div className="h-3 w-40 animate-pulse rounded bg-fg/10" />
            </div>
          </div>
        </div>
      ))}
    </div>
  );

  return (
    <div className="flex h-full flex-col overflow-hidden text-fg/80">
      {/* Search Bar */}
      {lorebooks.length > 0 && (
        <div className="px-4 py-3">
          <div className="relative">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-fg/40" />
            <input
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder={t("characters.lorebook.searchPlaceholder")}
              className="w-full rounded-xl border border-fg/10 bg-surface-el/20 pl-9 pr-4 py-2 text-sm text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none"
            />
          </div>
        </div>
      )}

      <main className="flex-1 overflow-y-auto px-4 pb-6">
        <motion.div
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.2, ease: "easeOut" }}
          className="space-y-4"
        >
          {/* Lorebook List */}
          {loading ? (
            <Skeleton />
          ) : lorebooks.length === 0 ? (
            <EmptyState />
          ) : filteredLorebooks.length === 0 ? (
            <div className="flex h-32 flex-col items-center justify-center text-fg/50">
              <p>{t("characters.lorebook.noMatchingLorebooks")}</p>
            </div>
          ) : (
            (() => {
              const assigned = filteredLorebooks.filter((lb) =>
                assignedLorebookIds.has(lb.id),
              );
              const available = filteredLorebooks.filter(
                (lb) => !assignedLorebookIds.has(lb.id),
              );
              const renderRow = (lorebook: Lorebook, isAssigned: boolean) => {
                const count = entryCounts.get(lorebook.id);
                return (
                  <motion.button
                    key={lorebook.id}
                    layout="position"
                    onClick={() => setSelectedLorebook(lorebook)}
                    className={`group flex w-full items-center gap-3 rounded-lg border px-3 py-2.5 text-left transition ${
                      isAssigned
                        ? "border-fg/10 border-l-2 border-l-accent bg-accent/4 hover:bg-accent/8"
                        : "border-fg/10 bg-surface-el/40 hover:border-fg/20 hover:bg-surface-el/70"
                    }`}
                  >
                    <div className="flex h-9 w-9 shrink-0 items-center justify-center overflow-hidden rounded-lg border border-fg/10 bg-fg/5">
                      <LorebookAvatar
                        avatarPath={lorebook.avatarPath}
                        name={lorebook.name}
                        iconClassName="h-4 w-4 text-fg/60"
                        fallbackClassName="bg-fg/5"
                      />
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-sm font-semibold text-fg">
                        {lorebook.name}
                      </div>
                      <div className="mt-0.5 truncate font-mono text-[11px] text-fg/40">
                        {count === undefined
                          ? t("characters.lorebook.tapToViewEntries")
                          : t("characters.lorebook.entryCount", { count })}
                      </div>
                    </div>
                    <ChevronRight className="h-4 w-4 shrink-0 text-fg/30 transition group-hover:text-fg/60" />
                  </motion.button>
                );
              };
              const SectionHeader = ({
                label,
                count,
                accent,
              }: {
                label: string;
                count: number;
                accent?: boolean;
              }) => (
                <div
                  className={`flex items-center gap-2 px-1 pb-1.5 pt-1 text-[10px] font-semibold uppercase tracking-[0.12em] ${
                    accent ? "text-accent/80" : "text-fg/45"
                  }`}
                >
                  <span
                    className={`h-1 w-1 rounded-full ${accent ? "bg-accent" : "bg-fg/30"}`}
                  />
                  <span>{label}</span>
                  <span className="font-mono text-fg/35">· {count}</span>
                </div>
              );
              return (
                <div className="space-y-5">
                  {assigned.length > 0 && (
                    <section className="space-y-1.5">
                      <SectionHeader
                        label={t("characters.lorebook.sectionActive")}
                        count={assigned.length}
                        accent
                      />
                      <AnimatePresence initial={false}>
                        {assigned.map((lb) => renderRow(lb, true))}
                      </AnimatePresence>
                    </section>
                  )}
                  {available.length > 0 && (
                    <section className="space-y-1.5">
                      <SectionHeader
                        label={t("characters.lorebook.sectionAvailable")}
                        count={available.length}
                      />
                      <AnimatePresence initial={false}>
                        {available.map((lb) => renderRow(lb, false))}
                      </AnimatePresence>
                    </section>
                  )}
                </div>
              );
            })()
          )}
        </motion.div>
      </main>

      {/* Create Lorebook Menu */}
      <BottomMenu
        isOpen={showCreateMenu}
        onClose={() => {
          setShowCreateMenu(false);
          setNewName("");
        }}
        title={t("characters.lorebook.newLorebookTitle")}
      >
        <div className="space-y-4">
          <div className="space-y-2">
            <label className="text-[11px] font-medium text-fg/70">
              {t("characters.lorebook.nameLabel")}
            </label>
            <input
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleCreate()}
              placeholder={t("characters.lorebook.enterNamePlaceholder")}
              autoFocus
              className="w-full rounded-xl border border-fg/10 bg-surface-el/20 px-3 py-2 text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none"
            />
            <p className="text-xs text-fg/50">{t("characters.lorebook.lorebookExplanation")}</p>
          </div>

          <button
            onClick={handleCreate}
            disabled={!newName.trim()}
            className="w-full rounded-xl border border-accent/40 bg-accent/20 px-4 py-3.5 text-sm font-semibold text-accent/70 transition hover:bg-accent/30 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {t("characters.lorebook.createLorebook")}
          </button>
        </div>
      </BottomMenu>

      {/* Lorebook Actions Menu */}
      <BottomMenu
        isOpen={Boolean(selectedLorebook)}
        onClose={() => setSelectedLorebook(null)}
        title={selectedLorebook?.name || ""}
      >
        {selectedLorebook && (
          <div className="space-y-2">
            <MenuButton
              icon={Edit2}
              title={t("characters.lorebook.viewEntries")}
              description={t("characters.lorebook.editEntriesDesc")}
              color="from-info to-info/80"
              onClick={() => {
                onSelectLorebook(selectedLorebook.id);
                setSelectedLorebook(null);
              }}
            />

            <MenuButton
              icon={Star}
              title={assignedLorebookIds.has(selectedLorebook.id) ? disableLabel : enableLabel}
              description={
                assignedLorebookIds.has(selectedLorebook.id)
                  ? disableDescription
                  : enableDescription
              }
              color="from-accent to-accent/80"
              onClick={() => {
                const isAssigned = assignedLorebookIds.has(selectedLorebook.id);
                onToggleAssignment(selectedLorebook.id, !isAssigned);
                setSelectedLorebook(null);
              }}
            />

            <button
              onClick={async () => {
                const confirmed = await confirmBottomMenu({
                  title: t("characters.lorebook.deleteConfirmTitle"),
                  message: t("characters.lorebook.deleteConfirmMessage"),
                  confirmLabel: "Delete",
                  destructive: true,
                });
                if (!confirmed) return;
                onDeleteLorebook(selectedLorebook.id);
                setSelectedLorebook(null);
              }}
              className="flex w-full items-center gap-3 rounded-xl border border-danger/30 bg-danger/10 px-4 py-3 text-left transition hover:border-danger/50 hover:bg-danger/20"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-lg border border-danger/30 bg-danger/20">
                <Trash2 className="h-4 w-4 text-danger" />
              </div>
              <span className="text-sm font-medium text-danger">
                {t("characters.lorebook.deleteLorebook")}
              </span>
            </button>
          </div>
        )}
      </BottomMenu>
    </div>
  );
}

function EntryListView({
  entries,
  loading,
  onCreateEntry,
  onEditEntry,
  onToggleEntry,
  onDeleteEntry,
  onReorderEntries,
  onOpenPreview,
}: {
  entries: LorebookEntry[];
  loading: boolean;
  onCreateEntry: () => void;
  onEditEntry: (entry: LorebookEntry) => void;
  onToggleEntry: (entry: LorebookEntry, enabled: boolean) => void;
  onDeleteEntry: (id: string) => void;
  onReorderEntries: (entries: LorebookEntry[]) => void;
  onOpenPreview: (() => void) | null;
}) {
  const { t } = useI18n();
  const [selectedEntry, setSelectedEntry] = useState<LorebookEntry | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [dragState, setDragState] = useState<{ fromIndex: number; toIndex: number } | null>(null);

  useEffect(() => {
    const handleAdd = () => onCreateEntry();
    window.addEventListener("lorebook:add", handleAdd);
    return () => window.removeEventListener("lorebook:add", handleAdd);
  }, [onCreateEntry]);

  const filteredEntries = useMemo(() => {
    if (!searchQuery.trim()) return entries;
    const query = searchQuery.toLowerCase();
    return entries.filter((e) => {
      const title = e.title?.toLowerCase() || "";
      const content = e.content?.toLowerCase() || "";
      const keywords = e.keywords.map((k) => k.toLowerCase()).join(" ");
      return title.includes(query) || content.includes(query) || keywords.includes(query);
    });
  }, [entries, searchQuery]);

  const displayEntries = useMemo(() => {
    if (!dragState || dragState.fromIndex === dragState.toIndex) {
      return filteredEntries;
    }
    const result = [...filteredEntries];
    const [removed] = result.splice(dragState.fromIndex, 1);
    result.splice(dragState.toIndex, 0, removed);
    return result;
  }, [filteredEntries, dragState]);

  const handleDrag = (fromIndex: number, info: { offset: { y: number } }) => {
    const offsetSlots = Math.round(info.offset.y / 70);
    const toIndex = Math.max(0, Math.min(entries.length - 1, fromIndex + offsetSlots));
    setDragState({ fromIndex, toIndex });
  };

  const handleDragEnd = (fromIndex: number, info: PanInfo) => {
    const offsetSlots = Math.round(info.offset.y / 70);
    const toIndex = Math.max(0, Math.min(entries.length - 1, fromIndex + offsetSlots));

    if (fromIndex !== toIndex) {
      const reordered = [...entries];
      const [removed] = reordered.splice(fromIndex, 1);
      reordered.splice(toIndex, 0, removed);
      onReorderEntries(reordered);
    }
    setDragState(null);
  };

  // Empty state
  const EmptyState = () => (
    <div className="flex h-64 flex-col items-center justify-center">
      <BookOpen className="mb-3 h-12 w-12 text-fg/20" />
      <h3 className="mb-1 text-lg font-medium text-fg">{t("characters.lorebook.noEntriesYet")}</h3>
      <p className="mb-4 text-center text-sm text-fg/50">
        {t("characters.lorebook.addEntriesToInject")}
      </p>
      <button
        onClick={onCreateEntry}
        className="rounded-full border border-accent/40 bg-accent/20 px-6 py-2 text-sm font-medium text-accent/70 transition hover:bg-accent/30"
      >
        {t("characters.lorebook.createEntry")}
      </button>
    </div>
  );

  // Skeleton
  const Skeleton = () => (
    <div className="space-y-3">
      {[1, 2, 3].map((i) => (
        <div key={i} className="rounded-xl border border-fg/10 bg-surface-el/90 px-4 py-3">
          <div className="flex items-center gap-3">
            <div className="h-10 w-10 animate-pulse rounded-lg bg-fg/10" />
            <div className="flex-1 space-y-2">
              <div className="h-4 w-32 animate-pulse rounded bg-fg/10" />
              <div className="h-3 w-48 animate-pulse rounded bg-fg/10" />
            </div>
          </div>
        </div>
      ))}
    </div>
  );

  return (
    <div className="flex h-full flex-col text-fg/80 overflow-hidden">
      {entries.length > 0 && (
        <div className="flex items-center gap-2 px-4 py-3">
          <div className="relative flex-1">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-fg/40" />
            <input
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder={t("characters.lorebook.searchEntries")}
              className="w-full rounded-xl border border-fg/10 bg-surface-el/20 pl-9 pr-4 py-2 text-sm text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none"
            />
          </div>
          {onOpenPreview && (
            <button
              type="button"
              onClick={onOpenPreview}
              className="flex shrink-0 items-center gap-1.5 rounded-xl border border-fg/10 bg-surface-el/20 px-3 py-2 text-xs font-medium text-fg/70 transition hover:border-fg/25 hover:text-fg"
              title={t("characters.lorebook.preview.openButton")}
            >
              <TestTube2 className="h-3.5 w-3.5" />
              <span className="hidden sm:inline">{t("characters.lorebook.preview.openButton")}</span>
            </button>
          )}
        </div>
      )}

      <main className="flex-1 overflow-y-auto overflow-x-hidden px-4 pb-6">
        <motion.div
          initial={{ opacity: 0, x: 20 }}
          animate={{ opacity: 1, x: 0 }}
          transition={{ duration: 0.2, ease: "easeOut" }}
          className="space-y-4"
        >
          {/* Entry List */}
          {loading ? (
            <Skeleton />
          ) : entries.length === 0 ? (
            <EmptyState />
          ) : filteredEntries.length === 0 ? (
            <div className="flex h-32 flex-col items-center justify-center text-fg/50">
              <p>{t("characters.lorebook.noMatchingEntries")}</p>
            </div>
          ) : (
            <div className="space-y-3">
              {displayEntries.map((entry) => {
                const originalIndex = filteredEntries.findIndex((e) => e.id === entry.id);
                const isDragging = dragState?.fromIndex === originalIndex;

                return (
                  <EntryListItem
                    key={entry.id}
                    entry={entry}
                    originalIndex={originalIndex}
                    isDragging={isDragging}
                    onDrag={handleDrag}
                    onDragEnd={handleDragEnd}
                    onSelect={setSelectedEntry}
                  />
                );
              })}
            </div>
          )}
        </motion.div>
      </main>

      {/* Entry Actions Menu */}
      <BottomMenu
        isOpen={Boolean(selectedEntry)}
        onClose={() => setSelectedEntry(null)}
        title={
          selectedEntry?.title ||
          selectedEntry?.keywords[0] ||
          t("characters.lorebook.entryDefaultName")
        }
      >
        {selectedEntry && (
          <div className="space-y-2">
            <MenuButton
              icon={Edit2}
              title={t("characters.lorebook.editEntry")}
              description={t("characters.lorebook.editEntryDesc")}
              color="from-info to-info/80"
              onClick={() => {
                onEditEntry(selectedEntry);
                setSelectedEntry(null);
              }}
            />

            <MenuButton
              icon={Star}
              title={
                selectedEntry.enabled
                  ? t("characters.lorebook.disableEntry")
                  : t("characters.lorebook.enableEntry")
              }
              description={
                selectedEntry.enabled
                  ? t("characters.lorebook.entryDisabledDesc")
                  : t("characters.lorebook.entryEnabledDesc")
              }
              color="from-accent to-accent/80"
              onClick={() => {
                onToggleEntry(selectedEntry, !selectedEntry.enabled);
                setSelectedEntry(null);
              }}
            />

            <button
              onClick={async () => {
                const confirmed = await confirmBottomMenu({
                  title: t("characters.lorebook.deleteEntry"),
                  message: t("characters.lorebook.deleteConfirmMessage"),
                  confirmLabel: t("common.buttons.delete"),
                  destructive: true,
                });
                if (!confirmed) return;
                onDeleteEntry(selectedEntry.id);
                setSelectedEntry(null);
              }}
              className="flex w-full items-center gap-3 rounded-xl border border-danger/30 bg-danger/10 px-4 py-3 text-left transition hover:border-danger/50 hover:bg-danger/20"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-lg border border-danger/30 bg-danger/20">
                <Trash2 className="h-4 w-4 text-danger" />
              </div>
              <span className="text-sm font-medium text-danger">
                {t("characters.lorebook.deleteEntry")}
              </span>
            </button>
          </div>
        )}
      </BottomMenu>
    </div>
  );
}

export function EntryEditorMenu({
  entry,
  isOpen,
  onClose,
  onSave,
}: {
  entry: LorebookEntry | null;
  isOpen: boolean;
  onClose: () => void;
  onSave: (entry: LorebookEntry) => void;
}) {
  const { t } = useI18n();
  const [draft, setDraft] = useState<LorebookEntry | null>(null);
  const [isKeywordGenerating, setIsKeywordGenerating] = useState(false);
  const [showKeywordReview, setShowKeywordReview] = useState(false);
  const [keywordDirectionPrompt, setKeywordDirectionPrompt] = useState("");
  const [keywordDraftKeywords, setKeywordDraftKeywords] = useState<string[]>([]);

  useEffect(() => {
    if (entry) {
      setDraft({ ...entry });
      setIsKeywordGenerating(false);
      setShowKeywordReview(false);
      setKeywordDirectionPrompt("");
      setKeywordDraftKeywords([]);
    }
  }, [entry]);

  if (!draft) return null;

  const handleSave = () => {
    onSave(draft);
    onClose();
  };

  const handleGenerateKeywords = async () => {
    const content = draft.content.trim();
    if (!content) {
      toast.error("Keyword generation needs entry content first");
      return;
    }

    setIsKeywordGenerating(true);
    try {
      const result = await generateLorebookKeywordDraft({
        title: draft.title?.trim() || null,
        content,
        directionPrompt: keywordDirectionPrompt.trim() || null,
        existingKeywords: draft.keywords,
      });
      if (!result.keywords.length) {
        toast.error("Keyword generation returned no usable keywords");
        return;
      }
      setKeywordDraftKeywords(result.keywords);
      setShowKeywordReview(true);
    } catch (error) {
      console.error("Failed to generate lorebook keywords:", error);
      toast.error(
        "Keyword generation failed",
        error instanceof Error ? error.message : String(error),
      );
    } finally {
      setIsKeywordGenerating(false);
    }
  };

  const handleAcceptKeywords = () => {
    if (!keywordDraftKeywords.length) {
      toast.error("No generated keywords to apply");
      return;
    }
    setDraft((current) =>
      current
        ? {
            ...current,
            keywords: keywordDraftKeywords,
            alwaysActive: false,
          }
        : current,
    );
    setShowKeywordReview(false);
  };

  return (
    <>
      <BottomMenu isOpen={isOpen} onClose={onClose} title={t("characters.lorebook.editEntry")}>
        <div className="space-y-4">
          <div className="space-y-2">
            <label className="text-[11px] font-medium text-fg/70">TITLE</label>
            <input
              value={draft.title || ""}
              onChange={(e) => setDraft({ ...draft, title: e.target.value })}
              placeholder="Name this entry..."
              className="w-full rounded-xl border border-fg/10 bg-surface-el/20 px-3 py-2 text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none"
            />
          </div>

          <div className="flex gap-3">
            <div className="flex flex-1 items-start justify-between gap-4 rounded-xl border border-fg/10 bg-surface-el/90 p-3">
              <div>
                <label className="block text-sm font-semibold text-fg">Enabled</label>
                <p className="mt-0.5 text-xs text-fg/50">Include in prompts</p>
              </div>
              <Switch
                checked={draft.enabled}
                onChange={(next) => setDraft({ ...draft, enabled: next })}
              />
            </div>

            <div className="flex flex-1 items-start justify-between gap-4 rounded-xl border border-fg/10 bg-surface-el/90 p-3">
              <div>
                <label className="block text-sm font-semibold text-fg">Always On</label>
                <p className="mt-0.5 text-xs text-fg/50">No keywords needed</p>
              </div>
              <Switch
                checked={draft.alwaysActive}
                onChange={(next) => setDraft({ ...draft, alwaysActive: next })}
              />
            </div>
          </div>

          {!draft.alwaysActive && (
            <KeywordTagInput
              keywords={draft.keywords}
              onChange={(keywords) => setDraft({ ...draft, keywords })}
              caseSensitive={draft.caseSensitive}
              onCaseSensitiveChange={(caseSensitive) => setDraft({ ...draft, caseSensitive })}
              onGenerate={() => {
                setKeywordDraftKeywords([]);
                setShowKeywordReview(true);
              }}
              isGenerating={isKeywordGenerating}
              canGenerate={!!draft.content.trim()}
            />
          )}

          <div className="space-y-2">
            <label className="text-[11px] font-medium text-fg/70">
              {t("characters.lorebook.contentLabel")}
            </label>
            <textarea
              value={draft.content}
              onChange={(e) => setDraft({ ...draft, content: e.target.value })}
              placeholder={t("characters.lorebook.contentPlaceholder")}
              rows={8}
              className="w-full resize-none rounded-xl border border-fg/10 bg-surface-el/20 px-3 py-2 text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none"
            />
          </div>

          <button
            onClick={handleSave}
            disabled={!draft.title?.trim() && !draft.content?.trim()}
            className="w-full rounded-xl border border-accent/40 bg-accent/20 px-4 py-3.5 text-sm font-semibold text-accent/70 transition hover:bg-accent/30 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {t("characters.lorebook.saveEntry")}
          </button>
        </div>
      </BottomMenu>

      <BottomMenu
        isOpen={showKeywordReview}
        onClose={() => setShowKeywordReview(false)}
        title="Review Keywords"
      >
        <div className="space-y-4">
          <div className="space-y-2">
            <label className="text-[11px] font-medium text-fg/70">DIRECTION PROMPT</label>
            <textarea
              value={keywordDirectionPrompt}
              onChange={(e) => setKeywordDirectionPrompt(e.target.value)}
              placeholder="Optional guidance, like: focus on aliases, locations, and broad recall terms."
              rows={3}
              className="w-full resize-none rounded-xl border border-fg/10 bg-surface-el/20 px-3 py-2 text-fg placeholder-fg/40 transition focus:border-fg/30 focus:outline-none"
            />
            <p className="text-[11px] leading-relaxed text-fg/45">
              {keywordDraftKeywords.length > 0
                ? "Edits here affect the next regenerate pass."
                : "Optional. Leave blank to use defaults."}
            </p>
          </div>

          {keywordDraftKeywords.length > 0 && (
            <div className="space-y-2">
              <label className="text-[11px] font-medium text-fg/70">MODEL RESPONSE</label>
              <div className="flex flex-wrap gap-2 rounded-xl border border-fg/10 bg-surface-el/20 p-3">
                {keywordDraftKeywords.map((keyword) => (
                  <span
                    key={keyword}
                    className="rounded-full border border-accent/25 bg-accent/10 px-2.5 py-1 text-xs text-accent"
                  >
                    {keyword}
                  </span>
                ))}
              </div>
            </div>
          )}

          {keywordDraftKeywords.length === 0 ? (
            <div className="grid grid-cols-2 gap-2">
              <button
                type="button"
                onClick={() => setShowKeywordReview(false)}
                className="rounded-xl border border-fg/10 bg-surface-el/20 px-3 py-3 text-sm font-medium text-fg/70 transition hover:bg-surface-el/30"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={() => void handleGenerateKeywords()}
                disabled={isKeywordGenerating || !draft.content.trim()}
                className="inline-flex items-center justify-center gap-2 rounded-xl border border-accent/40 bg-accent/20 px-3 py-3 text-sm font-semibold text-accent transition hover:bg-accent/30 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {isKeywordGenerating && <Loader2 className="h-4 w-4 animate-spin" />}
                Generate
              </button>
            </div>
          ) : (
            <div className="grid grid-cols-3 gap-2">
              <button
                type="button"
                onClick={() => setShowKeywordReview(false)}
                className="rounded-xl border border-fg/10 bg-surface-el/20 px-3 py-3 text-sm font-medium text-fg/70 transition hover:bg-surface-el/30"
              >
                Reject
              </button>
              <button
                type="button"
                onClick={() => void handleGenerateKeywords()}
                disabled={isKeywordGenerating || !draft.content.trim()}
                className="inline-flex items-center justify-center gap-2 rounded-xl border border-warning/30 bg-warning/10 px-3 py-3 text-sm font-medium text-warning transition hover:bg-warning/15 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {isKeywordGenerating && <Loader2 className="h-4 w-4 animate-spin" />}
                Regenerate
              </button>
              <button
                type="button"
                onClick={handleAcceptKeywords}
                disabled={!keywordDraftKeywords.length}
                className="rounded-xl border border-accent/40 bg-accent/20 px-3 py-3 text-sm font-semibold text-accent transition hover:bg-accent/30 disabled:cursor-not-allowed disabled:opacity-50"
              >
                Accept
              </button>
            </div>
          )}
        </div>
      </BottomMenu>
    </>
  );
}

export function LorebookEditor() {
  const { t } = useI18n();
  const {
    characterId: characterIdParam,
    groupId: groupIdParam,
    groupSessionId: groupSessionIdParam,
  } = useParams();
  const [searchParams, setSearchParams] = useSearchParams();
  const location = useLocation();
  const navigate = useNavigate();
  const { backOrReplace } = useNavigationManager();
  const characterId = characterIdParam ?? searchParams.get("characterId");
  const groupId = groupIdParam ?? searchParams.get("groupId");
  const groupSessionId = groupSessionIdParam ?? searchParams.get("groupSessionId");

  const activeLorebookId = searchParams.get("lorebookId");
  const currentSessionId = searchParams.get("sessionId");

  const [lorebooks, setLorebooks] = useState<Lorebook[]>([]);
  const [assignedLorebookIds, setAssignedLorebookIds] = useState<Set<string>>(new Set());
  const [entries, setEntries] = useState<LorebookEntry[]>([]);

  const [isLorebooksLoading, setIsLorebooksLoading] = useState(true);
  const [isEntriesLoading, setIsEntriesLoading] = useState(false);

  const [editingEntry, setEditingEntry] = useState<LorebookEntry | null>(null);
  const [showLorebookSettingsMenu, setShowLorebookSettingsMenu] = useState(false);
  const [lorebookNameDraft, setLorebookNameDraft] = useState("");
  const [avatarDraftPath, setAvatarDraftPath] = useState<string | null>(null);
  const [keywordDetectionModeDraft, setKeywordDetectionModeDraft] =
    useState<Lorebook["keywordDetectionMode"]>("recentMessageWindow");
  const [isExporting, setIsExporting] = useState(false);
  const [showExportMenu, setShowExportMenu] = useState(false);
  const avatarInputRef = useRef<HTMLInputElement | null>(null);

  const activeLorebook = useMemo(
    () => lorebooks.find((l) => l.id === activeLorebookId) ?? null,
    [lorebooks, activeLorebookId],
  );
  const buildLorebookSearchParams = (lorebookId: string) => {
    const next = new URLSearchParams();
    next.set("lorebookId", lorebookId);
    if (currentSessionId) {
      next.set("sessionId", currentSessionId);
    }
    return next;
  };
  const target = useMemo(() => {
    if (characterId) return { type: "character" as const, id: characterId };
    if (groupSessionId) return { type: "groupSession" as const, id: groupSessionId };
    if (groupId) return { type: "group" as const, id: groupId };
    return null;
  }, [characterId, groupId, groupSessionId]);

  const pageTitle = activeLorebook ? `Lorebook - ${activeLorebook.name}` : undefined;
  const assignmentCopy = useMemo(() => {
    if (target?.type === "group") {
      return {
        assignmentLabel: t("characters.lorebook.enabledForGroup"),
        enableLabel: t("characters.lorebook.enableForGroup"),
        disableLabel: t("characters.lorebook.disableForGroup"),
        enableDescription: t("characters.lorebook.addToActiveGroup"),
        disableDescription: t("characters.lorebook.removeFromActiveGroup"),
      };
    }
    if (target?.type === "groupSession") {
      return {
        assignmentLabel: t("characters.lorebook.enabledForSession"),
        enableLabel: t("characters.lorebook.enableForSession"),
        disableLabel: t("characters.lorebook.disableForSession"),
        enableDescription: t("characters.lorebook.addToActiveSession"),
        disableDescription: t("characters.lorebook.removeFromActiveSession"),
      };
    }
    return {
      assignmentLabel: t("characters.lorebook.enabledForCharacter"),
      enableLabel: t("characters.lorebook.enableForCharacter"),
      disableLabel: t("characters.lorebook.disableForCharacter"),
      enableDescription: t("characters.lorebook.addToActive"),
      disableDescription: t("characters.lorebook.removeFromActive"),
    };
  }, [t, target]);
  const handleBack = useMemo(() => {
    if (target?.type === "character") {
      return () => backOrReplace("/settings/characters");
    }
    if (target?.type === "group") {
      return () => backOrReplace(Routes.groupSettings(target.id));
    }
    if (target?.type === "groupSession") {
      return () => backOrReplace(Routes.groupChatSettings(target.id));
    }
    return () => backOrReplace(Routes.settings);
  }, [backOrReplace, target]);

  useEffect(() => {
    if (!target) return;
    loadLorebooks();
  }, [target]);

  useEffect(() => {
    if (!activeLorebookId) {
      setEntries([]);
      return;
    }
    loadEntries(activeLorebookId);
  }, [activeLorebookId]);

  const loadLorebooks = async () => {
    if (!target) return;
    try {
      setIsLorebooksLoading(true);
      const [allLorebooks, assignedLorebooks] = await Promise.all([
        listLorebooks(),
        target.type === "character"
          ? listCharacterLorebooks(target.id)
          : target.type === "group"
            ? listGroupLorebooks(target.id)
            : listGroupSessionLorebooks(target.id),
      ]);
      setLorebooks(allLorebooks);
      setAssignedLorebookIds(new Set(assignedLorebooks.map((l) => l.id)));
    } catch (error) {
      console.error("Failed to load lorebooks:", error);
    } finally {
      setIsLorebooksLoading(false);
    }
  };

  const loadEntries = async (lorebookId: string) => {
    try {
      setIsEntriesLoading(true);
      const data = await listLorebookEntries(lorebookId);
      setEntries(data);
    } catch (error) {
      console.error("Failed to load entries:", error);
    } finally {
      setIsEntriesLoading(false);
    }
  };

  const handleCreateLorebook = async (name: string) => {
    if (!target) return;
    try {
      const created = await saveLorebook({ name });
      setLorebooks((prev) => [created, ...prev]);
      const next = new Set(assignedLorebookIds);
      next.add(created.id);
      setAssignedLorebookIds(next);
      if (target.type === "character") {
        await setCharacterLorebooks(target.id, Array.from(next));
      } else if (target.type === "group") {
        await setGroupLorebooks(target.id, Array.from(next));
      } else {
        await setGroupSessionLorebooks(target.id, Array.from(next));
      }
      setSearchParams(buildLorebookSearchParams(created.id));
    } catch (error) {
      console.error("Failed to create lorebook:", error);
    }
  };

  const handleToggleAssignment = async (lorebookId: string, enabled: boolean) => {
    if (!target) return;
    const next = new Set(assignedLorebookIds);
    if (enabled) next.add(lorebookId);
    else next.delete(lorebookId);
    setAssignedLorebookIds(next);
    if (target.type === "character") {
      await setCharacterLorebooks(target.id, Array.from(next));
    } else if (target.type === "group") {
      await setGroupLorebooks(target.id, Array.from(next));
    } else {
      await setGroupSessionLorebooks(target.id, Array.from(next));
    }
  };

  const handleSelectLorebook = (lorebookId: string) => {
    setSearchParams(buildLorebookSearchParams(lorebookId));
  };

  const openLorebookSettings = () => {
    if (!activeLorebook) return;
    setLorebookNameDraft(activeLorebook.name);
    setAvatarDraftPath(activeLorebook.avatarPath ?? null);
    setKeywordDetectionModeDraft(activeLorebook.keywordDetectionMode);
    setShowLorebookSettingsMenu(true);
  };

  const closeLorebookSettings = () => {
    setShowLorebookSettingsMenu(false);
    if (!activeLorebook) return;
    setLorebookNameDraft(activeLorebook.name);
    setAvatarDraftPath(activeLorebook.avatarPath ?? null);
    setKeywordDetectionModeDraft(activeLorebook.keywordDetectionMode);
  };

  const handleAvatarFileChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = () => {
      setAvatarDraftPath(typeof reader.result === "string" ? reader.result : null);
    };
    reader.readAsDataURL(file);
    event.target.value = "";
  };

  const lorebookMetadataReturnPath = `${location.pathname}${location.search}`;

  const handleChooseAvatarFromLibrary = () => {
    if (!activeLorebook) return;
    const draftPayload = {
      lorebookId: activeLorebook.id,
      nameDraft: lorebookNameDraft,
      keywordMode: keywordDetectionModeDraft,
      avatarDraft: avatarDraftPath,
    };
    sessionStorage.setItem(
      `lorebook-metadata-draft:${lorebookMetadataReturnPath}`,
      JSON.stringify(draftPayload),
    );
    navigate("/library/images/pick", {
      state: { returnPath: lorebookMetadataReturnPath, selectionKind: "avatar" },
    });
  };

  useEffect(() => {
    if (isLorebooksLoading || !activeLorebook) return;

    const selectionKey = buildAvatarLibrarySelectionKey(lorebookMetadataReturnPath);
    const rawSelection = sessionStorage.getItem(selectionKey);
    if (!rawSelection) return;
    sessionStorage.removeItem(selectionKey);

    const draftKey = `lorebook-metadata-draft:${lorebookMetadataReturnPath}`;
    const rawDraft = sessionStorage.getItem(draftKey);
    sessionStorage.removeItem(draftKey);

    let parsed: AvatarLibrarySelectionPayload | null = null;
    try {
      parsed = JSON.parse(rawSelection) as AvatarLibrarySelectionPayload;
    } catch {
      return;
    }
    if (!parsed?.filePath) return;

    let draft:
      | {
          lorebookId?: string;
          nameDraft?: string;
          keywordMode?: Lorebook["keywordDetectionMode"];
          avatarDraft?: string | null;
        }
      | null = null;
    if (rawDraft) {
      try {
        draft = JSON.parse(rawDraft);
      } catch {}
    }

    let cancelled = false;
    void (async () => {
      const dataUrl = await convertFilePathToDataUrl(parsed.filePath);
      if (cancelled || !dataUrl) return;
      if (draft && draft.lorebookId === activeLorebook.id) {
        if (typeof draft.nameDraft === "string") setLorebookNameDraft(draft.nameDraft);
        if (draft.keywordMode) setKeywordDetectionModeDraft(draft.keywordMode);
      }
      setAvatarDraftPath(dataUrl);
      setShowLorebookSettingsMenu(true);
    })();

    return () => {
      cancelled = true;
    };
  }, [isLorebooksLoading, activeLorebook, lorebookMetadataReturnPath]);

  const handleSaveLorebookSettings = async () => {
    if (!activeLorebook || !lorebookNameDraft.trim()) return;
    let replacementAvatarPath: string | undefined;

    try {
      let nextAvatarPath = activeLorebook.avatarPath;

      if (!avatarDraftPath) {
        nextAvatarPath = undefined;
      } else if (avatarDraftPath.startsWith("data:")) {
        const storedAvatarPath = await convertToImageRef(avatarDraftPath);
        if (!storedAvatarPath) {
          throw new Error("Failed to save lorebook image");
        }
        nextAvatarPath = storedAvatarPath;
        replacementAvatarPath = storedAvatarPath;
      } else {
        nextAvatarPath = avatarDraftPath;
      }

      const saved = await saveLorebook({
        ...activeLorebook,
        name: lorebookNameDraft.trim(),
        avatarPath: nextAvatarPath,
        keywordDetectionMode: keywordDetectionModeDraft,
      });

      if (activeLorebook.avatarPath && activeLorebook.avatarPath !== saved.avatarPath) {
        await deleteImageRef(activeLorebook.avatarPath);
      }

      setLorebooks((prev) => prev.map((item) => (item.id === saved.id ? saved : item)));
      setLorebookNameDraft(saved.name);
      setAvatarDraftPath(saved.avatarPath ?? null);
      setKeywordDetectionModeDraft(saved.keywordDetectionMode);
      setShowLorebookSettingsMenu(false);
    } catch (error) {
      if (replacementAvatarPath) {
        await deleteImageRef(replacementAvatarPath);
      }
      console.error("Failed to save lorebook settings:", error);
    }
  };

  const handleExportLorebook = async (format: LorebookExportFormat) => {
    if (!activeLorebook || isExporting) return;

    try {
      setIsExporting(true);
      const exportJson =
        format === "usc"
          ? await exportLorebookAsUsc(activeLorebook.id)
          : await exportLorebook(activeLorebook.id);

      await downloadJson(
        exportJson,
        generateLorebookExportFilenameWithFormat(activeLorebook.name, format),
      );
      setShowExportMenu(false);
    } catch (error) {
      console.error("Failed to export lorebook:", error);
      toast.error("Export failed", String(error));
    } finally {
      setIsExporting(false);
    }
  };

  const handleCreateEntry = async () => {
    if (!activeLorebookId) return;
    try {
      const newEntry = await createBlankLorebookEntry(activeLorebookId);
      setEntries((prev) => [...prev, newEntry]);
      setEditingEntry(newEntry);
    } catch (error) {
      console.error("Failed to create entry:", error);
    }
  };

  const handleDeleteLorebook = async (id: string) => {
    try {
      const lorebook = lorebooks.find((item) => item.id === id);
      if (lorebook?.avatarPath) {
        await deleteImageRef(lorebook.avatarPath);
      }
      await deleteLorebook(id);
      setLorebooks((prev) => prev.filter((l) => l.id !== id));
      if (assignedLorebookIds.has(id)) {
        const next = new Set(assignedLorebookIds);
        next.delete(id);
        setAssignedLorebookIds(next);
      }
    } catch (error) {
      console.error("Failed to delete lorebook:", error);
    }
  };

  const handleDeleteEntry = async (id: string) => {
    try {
      await deleteLorebookEntry(id);
      setEntries((prev) => prev.filter((e) => e.id !== id));
    } catch (error) {
      console.error("Failed to delete entry:", error);
    }
  };

  const handleSaveEntry = async (entry: LorebookEntry) => {
    try {
      const saved = await saveLorebookEntry(entry);
      setEntries((prev) => prev.map((e) => (e.id === saved.id ? saved : e)));
    } catch (error) {
      console.error("Failed to save entry:", error);
    }
  };

  const handleToggleEntry = async (entry: LorebookEntry, enabled: boolean) => {
    try {
      const updated = { ...entry, enabled };
      await saveLorebookEntry(updated);
      setEntries((prev) => prev.map((e) => (e.id === entry.id ? { ...e, enabled } : e)));
    } catch (error) {
      console.error("Failed to toggle entry:", error);
    }
  };

  const handleReorderEntries = async (reorderedEntries: LorebookEntry[]) => {
    try {
      const updates = reorderedEntries.map((e, i) => [e.id, i] as [string, number]);
      await reorderLorebookEntries(updates);
      setEntries(reorderedEntries.map((e, i) => ({ ...e, displayOrder: i })));
    } catch (error) {
      console.error("Failed to reorder entries:", error);
    }
  };

  if (!target) {
    return (
      <div className="flex h-full items-center justify-center text-fg/50">
        No lorebook target provided
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col bg-surface pt-[calc(72px+env(safe-area-inset-top))]">
      <TopNav
        currentPath={location.pathname + location.search}
        titleOverride={pageTitle}
        onBackOverride={handleBack}
        rightAction={
          activeLorebook ? (
            <div className="flex items-center gap-1">
              {target?.type === "character" ? (
                <button
                  type="button"
                  onClick={() =>
                    navigate(
                      Routes.characterLorebookGenerate(
                        target.id,
                        activeLorebook.id,
                        currentSessionId,
                      ),
                    )
                  }
                  className="flex items-center px-[0.6em] py-[0.3em] justify-center rounded-full text-fg/70 hover:text-fg hover:bg-fg/10 transition"
                  aria-label="Generate lorebook entry"
                  title="Generate lorebook entry"
                >
                  <Sparkles size={18} className="text-fg" />
                </button>
              ) : null}
              <button
                type="button"
                onClick={openLorebookSettings}
                className="flex items-center px-[0.6em] py-[0.3em] justify-center rounded-full text-fg/70 hover:text-fg hover:bg-fg/10 transition"
                aria-label={t("common.buttons.edit")}
                title={t("common.buttons.edit")}
              >
                <Edit2 size={18} className="text-fg" />
              </button>
              <button
                type="button"
                onClick={() => setShowExportMenu(true)}
                disabled={isExporting}
                className="flex items-center px-[0.6em] py-[0.3em] justify-center rounded-full text-fg/70 hover:text-fg hover:bg-fg/10 transition disabled:cursor-not-allowed disabled:opacity-50"
                aria-label="Export lorebook"
                title="Export lorebook"
              >
                {isExporting ? (
                  <Loader2 size={18} className="animate-spin text-fg" />
                ) : (
                  <Download size={18} className="text-fg" />
                )}
              </button>
            </div>
          ) : null
        }
      />
      <div className="flex-1 min-h-0 overflow-visible">
        {activeLorebookId && activeLorebook ? (
          <>
            <EntryListView
              entries={entries}
              loading={isEntriesLoading}
              onCreateEntry={handleCreateEntry}
              onEditEntry={setEditingEntry}
              onToggleEntry={handleToggleEntry}
              onDeleteEntry={handleDeleteEntry}
              onReorderEntries={handleReorderEntries}
              onOpenPreview={
                entries.length > 0
                  ? () => {
                      if (target.type === "character") {
                        navigate(
                          Routes.characterLorebookPreview(target.id, activeLorebook.id),
                        );
                      } else if (target.type === "group") {
                        navigate(Routes.groupLorebookPreview(target.id, activeLorebook.id));
                      } else {
                        navigate(
                          Routes.groupChatLorebookPreview(target.id, activeLorebook.id),
                        );
                      }
                    }
                  : null
              }
            />
            <EntryEditorMenu
              entry={editingEntry}
              isOpen={Boolean(editingEntry)}
              onClose={() => setEditingEntry(null)}
              onSave={handleSaveEntry}
            />
          </>
        ) : (
          <LorebookListView
            lorebooks={lorebooks}
            assignedLorebookIds={assignedLorebookIds}
            loading={isLorebooksLoading}
            enableLabel={assignmentCopy.enableLabel}
            disableLabel={assignmentCopy.disableLabel}
            enableDescription={assignmentCopy.enableDescription}
            disableDescription={assignmentCopy.disableDescription}
            onSelectLorebook={handleSelectLorebook}
            onToggleAssignment={handleToggleAssignment}
            onCreateLorebook={handleCreateLorebook}
            onDeleteLorebook={handleDeleteLorebook}
          />
        )}
      </div>

      <LorebookExportMenu
        isOpen={showExportMenu}
        onClose={() => {
          if (isExporting) return;
          setShowExportMenu(false);
        }}
        onSelect={(format) => {
          void handleExportLorebook(format);
        }}
        exporting={isExporting}
      />

      {activeLorebook && (
        <LorebookMetadataMenu
          isOpen={showLorebookSettingsMenu}
          onClose={closeLorebookSettings}
          title={t("library.actions.renameLorebook")}
          nameValue={lorebookNameDraft}
          previewName={lorebookNameDraft.trim() || activeLorebook.name}
          namePlaceholder={t("characters.lorebook.enterNamePlaceholder")}
          avatarPath={avatarDraftPath}
          avatarInputRef={avatarInputRef}
          onNameChange={setLorebookNameDraft}
          onNameSubmit={handleSaveLorebookSettings}
          onAvatarFileChange={handleAvatarFileChange}
          onAvatarRemove={() => setAvatarDraftPath(null)}
          onChooseFromLibrary={handleChooseAvatarFromLibrary}
          keywordDetectionMode={keywordDetectionModeDraft}
          onKeywordDetectionModeChange={setKeywordDetectionModeDraft}
          onSave={handleSaveLorebookSettings}
          saveDisabled={
            !lorebookNameDraft.trim() ||
            (lorebookNameDraft.trim() === activeLorebook.name &&
              (avatarDraftPath ?? "") === (activeLorebook.avatarPath ?? "") &&
              keywordDetectionModeDraft === activeLorebook.keywordDetectionMode)
          }
        />
      )}
    </div>
  );
}
