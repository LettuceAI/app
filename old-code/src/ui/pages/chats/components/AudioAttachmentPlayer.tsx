import { useEffect, useRef, useState } from "react";
import { Pause, Play, FileAudio } from "lucide-react";
import { cn, radius, interactive } from "../../../design-tokens";

function formatTime(value: number): string {
  if (!Number.isFinite(value) || value < 0) value = 0;
  const minutes = Math.floor(value / 60);
  const seconds = Math.floor(value % 60);
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

interface AudioAttachmentPlayerProps {
  src?: string | null;
  filename?: string | null;
  fallbackLabel: string;
  className?: string;
  buttonClassName?: string;
}

export function AudioAttachmentPlayer({
  src,
  filename,
  fallbackLabel,
  className,
  buttonClassName,
}: AudioAttachmentPlayerProps) {
  const audioRef = useRef<HTMLAudioElement>(null);
  const [playing, setPlaying] = useState(false);
  const [current, setCurrent] = useState(0);
  const [duration, setDuration] = useState(0);
  const ready = !!src;

  useEffect(() => {
    setPlaying(false);
    setCurrent(0);
    setDuration(0);
  }, [src]);

  const togglePlay = () => {
    const el = audioRef.current;
    if (!el) return;
    if (playing) {
      el.pause();
    } else {
      void el.play().catch(() => setPlaying(false));
    }
  };

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
    <div className={cn("flex items-center gap-3", className)}>
      <audio
        ref={audioRef}
        src={src ?? undefined}
        preload="metadata"
        onPlay={() => setPlaying(true)}
        onPause={() => setPlaying(false)}
        onEnded={() => setPlaying(false)}
        onTimeUpdate={(event) => setCurrent(event.currentTarget.currentTime)}
        onLoadedMetadata={(event) => setDuration(event.currentTarget.duration)}
        className="hidden"
      />
      <button
        type="button"
        onClick={togglePlay}
        disabled={!ready}
        className={cn(
          "flex h-9 w-9 shrink-0 items-center justify-center",
          radius.full,
          buttonClassName ?? "bg-fg/15 text-fg",
          interactive.transition.fast,
          interactive.active.scale,
          "hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-50",
        )}
        aria-label={playing ? "Pause" : "Play"}
      >
        {playing ? (
          <Pause size={15} fill="currentColor" />
        ) : (
          <Play size={15} fill="currentColor" className="ml-0.5" />
        )}
      </button>

      <div className="flex min-w-0 flex-1 flex-col gap-1.5">
        <div className="flex items-center gap-1.5">
          <FileAudio className="h-3.5 w-3.5 shrink-0 opacity-60" />
          <span className="truncate text-xs font-medium">{filename || fallbackLabel}</span>
        </div>
        <div
          onClick={handleSeek}
          className={cn("relative h-1.5 cursor-pointer overflow-hidden bg-fg/15", radius.full)}
        >
          <div
            className={cn("absolute inset-y-0 left-0 bg-fg/65", radius.full)}
            style={{ width: `${progress}%` }}
          />
        </div>
      </div>

      <span className="shrink-0 text-[11px] tabular-nums opacity-55">
        {formatTime(current)} / {formatTime(duration)}
      </span>
    </div>
  );
}
