import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  BookOpen,
  Download,
  FileAudio,
  Loader2,
  MessageSquareText,
  MoreVertical,
  Pause,
  Play,
  Sparkles,
  Trash2,
} from "lucide-react";
import {
  deleteAudioLibraryItem,
  downloadAudioLibraryItem,
  listAudioLibraryItems,
  loadAudioLibraryItemData,
} from "../../../core/storage/repo";
import type { AudioLibraryItem } from "../../../core/storage/repo";
import { openDocs } from "../../../core/utils/docs";
import { useNavigate } from "react-router-dom";
import { useI18n } from "../../../core/i18n/context";
import { cn, typography, interactive } from "../../design-tokens";
import { BottomMenu, MenuButton } from "../../components";
import { confirmBottomMenu } from "../../components/ConfirmBottomMenu";
import { Routes } from "../../navigation";

type AudioFilter = "All" | "Generated" | "Uploaded";

function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 KB";
  const units = ["B", "KB", "MB", "GB"];
  const exponent = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  const value = bytes / Math.pow(1024, exponent);
  return `${value >= 10 || exponent === 0 ? Math.round(value) : value.toFixed(1)} ${units[exponent]}`;
}

function formatDate(ms: number): string {
  if (!ms) return "";
  return new Date(ms).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

function formatTime(value: number): string {
  if (!Number.isFinite(value) || value < 0) value = 0;
  const minutes = Math.floor(value / 60);
  const seconds = Math.floor(value % 60);
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

export function AudioLibraryPanel() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [items, setItems] = useState<AudioLibraryItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState<AudioFilter>("All");

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const data = await listAudioLibraryItems();
        if (!cancelled) setItems(data);
      } catch (error) {
        console.error("Failed to load audio library:", error);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const counts = useMemo(
    () => ({
      All: items.length,
      Generated: items.filter((item) => item.source === "tts").length,
      Uploaded: items.filter((item) => item.source === "upload").length,
    }),
    [items],
  );

  const filtered = useMemo(() => {
    if (filter === "Generated") return items.filter((item) => item.source === "tts");
    if (filter === "Uploaded") return items.filter((item) => item.source === "upload");
    return items;
  }, [items, filter]);

  const [menuItem, setMenuItem] = useState<AudioLibraryItem | null>(null);
  const [downloadingId, setDownloadingId] = useState<string | null>(null);

  const openMenu = useCallback((item: AudioLibraryItem) => setMenuItem(item), []);

  const handleOpenInChat = useCallback(
    (item: AudioLibraryItem) => {
      if (!item.characterId || !item.sessionId || !item.messageId) return;
      setMenuItem(null);
      navigate(
        Routes.chatSession(item.characterId, item.sessionId, {
          jumpToMessage: item.messageId,
        }),
      );
    },
    [navigate],
  );

  const handleDownload = useCallback(async (item: AudioLibraryItem) => {
    setDownloadingId(item.id);
    try {
      await downloadAudioLibraryItem(item);
    } catch (error) {
      console.error("Failed to download audio:", error);
    } finally {
      setDownloadingId((current) => (current === item.id ? null : current));
      setMenuItem(null);
    }
  }, []);

  const handleDelete = useCallback(
    async (item: AudioLibraryItem) => {
      setMenuItem(null);
      const confirmed = await confirmBottomMenu({
        title: t("library.audio.actions.deleteConfirmTitle"),
        message: t("library.audio.actions.deleteConfirmDescription"),
        confirmLabel: t("library.audio.actions.delete"),
        destructive: true,
      });
      if (!confirmed) return;
      try {
        await deleteAudioLibraryItem(item);
        setItems((prev) => prev.filter((entry) => entry.id !== item.id));
      } catch (error) {
        console.error("Failed to delete audio:", error);
      }
    },
    [t],
  );

  if (loading) {
    return (
      <div className="grid grid-cols-1 gap-3 pb-24 sm:grid-cols-2">
        {Array.from({ length: 6 }).map((_, index) => (
          <div
            key={index}
            className="h-[72px] animate-pulse rounded-2xl border border-fg/10 bg-fg/5"
          />
        ))}
      </div>
    );
  }

  return (
    <div className="pb-24">
      <div className="mb-4 flex flex-wrap gap-2">
        {(["All", "Generated", "Uploaded"] as AudioFilter[]).map((option) => (
          <button
            key={option}
            type="button"
            onClick={() => setFilter(option)}
            className={cn(
              "shrink-0 rounded-xl border px-3.5 py-1.5 text-sm font-medium transition",
              filter === option
                ? "border-fg/15 bg-fg/10 text-fg"
                : "border-fg/10 bg-surface-el/40 text-fg/60 hover:bg-fg/5 hover:text-fg",
            )}
          >
            {option === "All"
              ? t("library.audio.filters.all")
              : option === "Generated"
                ? t("library.audio.filters.generated")
                : t("library.audio.filters.uploaded")}
            <span className="ml-1.5 text-fg/40">{counts[option]}</span>
          </button>
        ))}
      </div>

      {filtered.length === 0 ? (
        <div className="flex flex-col items-center justify-center px-6 py-20 text-center">
          <div className="mb-4 flex h-14 w-14 items-center justify-center rounded-full border border-fg/10 bg-fg/5">
            <FileAudio className="h-6 w-6 text-fg/40" />
          </div>
          <h3 className={cn(typography.body.size, "font-semibold text-fg")}>
            {t("library.audio.empty.title")}
          </h3>
          <p className={cn(typography.bodySmall.size, "mt-1 max-w-xs text-fg/55")}>
            {t("library.audio.empty.description")}
          </p>
          <button
            onClick={() => void openDocs("textToSpeech")}
            className="mt-4 inline-flex items-center gap-1.5 text-xs font-medium text-fg/40 transition active:scale-95 hover:text-fg/70"
          >
            <BookOpen className="h-3.5 w-3.5" />
            {t("common.buttons.learnMore")}
          </button>
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          {filtered.map((item) => (
            <AudioCard key={item.id} item={item} onOpenMenu={openMenu} />
          ))}
        </div>
      )}

      <BottomMenu
        isOpen={menuItem !== null}
        onClose={() => setMenuItem(null)}
        title={menuItem ? (menuItem.source === "tts" ? t("library.audio.generatedTitle") : menuItem.filename) : ""}
      >
        <div className="space-y-2">
          {menuItem?.characterId && menuItem?.sessionId && menuItem?.messageId && (
            <MenuButton
              icon={MessageSquareText}
              title={t("library.audio.actions.openInChat")}
              description={menuItem.characterName ?? menuItem.sessionTitle ?? undefined}
              onClick={() => {
                if (menuItem) handleOpenInChat(menuItem);
              }}
            />
          )}
          <MenuButton
            icon={Download}
            title={t("library.audio.actions.download")}
            loading={menuItem ? downloadingId === menuItem.id : false}
            onClick={() => {
              if (menuItem) void handleDownload(menuItem);
            }}
          />
          <MenuButton
            icon={Trash2}
            title={t("library.audio.actions.delete")}
            color="from-red-500 to-rose-500"
            onClick={() => {
              if (menuItem) void handleDelete(menuItem);
            }}
          />
        </div>
      </BottomMenu>
    </div>
  );
}

const AudioCard = memo(
  ({ item, onOpenMenu }: { item: AudioLibraryItem; onOpenMenu: (item: AudioLibraryItem) => void }) => {
  const { t } = useI18n();
  const audioRef = useRef<HTMLAudioElement>(null);
  const wantPlayRef = useRef(false);
  const [src, setSrc] = useState<string | null>(null);
  const [loadingSrc, setLoadingSrc] = useState(false);
  const [playing, setPlaying] = useState(false);
  const [current, setCurrent] = useState(0);
  const [duration, setDuration] = useState(0);

  const isTts = item.source === "tts";
  const title = isTts ? t("library.audio.generatedTitle") : item.filename;
  const subtitle = isTts
    ? t("library.audio.generatedSubtitle")
    : item.characterName || item.sessionTitle || t("library.audio.uploadedSubtitle");

  const accentBtn = isTts ? "bg-secondary text-black" : "bg-accent text-black";
  const accentFill = isTts ? "bg-secondary" : "bg-accent";

  const handleToggle = useCallback(async () => {
    if (!src) {
      if (loadingSrc) return;
      wantPlayRef.current = true;
      setLoadingSrc(true);
      try {
        const data = await loadAudioLibraryItemData(item.storagePath);
        setSrc(data);
      } catch (error) {
        console.error("Failed to load audio:", error);
        wantPlayRef.current = false;
        setLoadingSrc(false);
      }
      return;
    }
    const el = audioRef.current;
    if (!el) return;
    if (playing) el.pause();
    else void el.play().catch(() => setPlaying(false));
  }, [src, loadingSrc, playing, item.storagePath]);

  const handleSeek = (event: React.MouseEvent<HTMLDivElement>) => {
    const el = audioRef.current;
    if (!el || !duration) return;
    const rect = event.currentTarget.getBoundingClientRect();
    const ratio = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
    el.currentTime = ratio * duration;
    setCurrent(el.currentTime);
  };

  const progress = duration > 0 ? (current / duration) * 100 : 0;

  return (
    <div className="flex items-center gap-3.5 rounded-2xl border border-fg/10 bg-surface-el/40 p-3.5 transition-colors hover:border-fg/20">
      <audio
        ref={audioRef}
        src={src ?? undefined}
        preload="metadata"
        className="hidden"
        onPlay={() => setPlaying(true)}
        onPause={() => setPlaying(false)}
        onEnded={() => setPlaying(false)}
        onTimeUpdate={(e) => setCurrent(e.currentTarget.currentTime)}
        onLoadedMetadata={(e) => setDuration(e.currentTarget.duration)}
        onCanPlay={() => {
          if (wantPlayRef.current) {
            wantPlayRef.current = false;
            setLoadingSrc(false);
            void audioRef.current?.play().catch(() => setPlaying(false));
          }
        }}
      />

      <button
        type="button"
        onClick={handleToggle}
        className={cn(
          "flex h-12 w-12 shrink-0 items-center justify-center rounded-full",
          accentBtn,
          interactive.transition.fast,
          interactive.active.scale,
          "hover:brightness-110",
        )}
        aria-label={playing ? t("library.audio.pause") : t("library.audio.play")}
      >
        {loadingSrc ? (
          <Loader2 className="h-5 w-5 animate-spin" />
        ) : playing ? (
          <Pause className="h-5 w-5" fill="currentColor" />
        ) : (
          <Play className="ml-0.5 h-5 w-5" fill="currentColor" />
        )}
      </button>

      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <div className="flex items-center gap-2">
          <h3 className={cn(typography.body.size, "min-w-0 flex-1 truncate font-semibold text-fg")}>
            {title}
          </h3>
          <span
            className={cn(
              "shrink-0 rounded-full border border-fg/15 bg-surface-el/60 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.06em]",
              isTts ? "text-secondary" : "text-info",
            )}
          >
            {isTts ? t("library.audio.badges.tts") : t("library.audio.badges.chat")}
          </span>
        </div>

        {src ? (
          <div className="flex items-center gap-2">
            <div
              onClick={handleSeek}
              className="relative h-1.5 flex-1 cursor-pointer overflow-hidden rounded-full bg-fg/15"
            >
              <div
                className={cn("absolute inset-y-0 left-0 rounded-full", accentFill)}
                style={{ width: `${progress}%` }}
              />
            </div>
            <span className="shrink-0 text-[11px] tabular-nums text-fg/45">
              {formatTime(current)} / {formatTime(duration)}
            </span>
          </div>
        ) : (
          <div className="flex items-center gap-1.5 text-fg/55">
            {isTts ? (
              <Sparkles className="h-3 w-3 shrink-0" />
            ) : (
              <MessageSquareText className="h-3 w-3 shrink-0" />
            )}
            <span className="truncate text-xs">{subtitle}</span>
          </div>
        )}

        <div className="flex items-center gap-1.5 text-[11px] text-fg/40">
          <span>{formatBytes(item.sizeBytes)}</span>
          {item.updatedAt > 0 && (
            <>
              <span className="text-fg/20">·</span>
              <span>{formatDate(item.updatedAt)}</span>
            </>
          )}
        </div>
      </div>

      <button
        type="button"
        onClick={() => onOpenMenu(item)}
        className={cn(
          "flex h-9 w-9 shrink-0 items-center justify-center self-center rounded-full text-fg/45",
          interactive.transition.fast,
          "hover:bg-fg/10 hover:text-fg",
        )}
        aria-label={t("library.audio.actions.menu")}
      >
        <MoreVertical className="h-5 w-5" />
      </button>
    </div>
  );
  },
);

AudioCard.displayName = "AudioCard";
