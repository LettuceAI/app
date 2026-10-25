import { useEffect, useRef, type CSSProperties } from "react";
import {
  ArrowLeft,
  Brain,
  ChevronDown,
  ChevronUp,
  Loader2,
  Search as SearchIcon,
  SlidersHorizontal,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, Button, cn, icon, IconButton, Input, useOnSurface } from "@lettuceai/crisp";
import type { Search } from "../data/use-search";
import { useChatLook } from "@/entities/preferences";

export interface ChatHeaderProps {
  columnStyle?: CSSProperties | undefined;
  name: string;
  avatar: string | null;
  model: { label: string; local: boolean } | null;
  modelKnown: boolean;
  phone: boolean;
  memory: { count: number; busy: boolean } | null;
  search: Search;
  onBack: () => void;
  onOptions: () => void;
  onMemory: () => void;
}

export function ChatHeader({
  name,
  avatar,
  model,
  modelKnown,
  phone,
  memory,
  search,
  onBack,
  onOptions,
  onMemory,
  columnStyle,
}: ChatHeaderProps) {
  const { t } = useTranslation("conversations");
  const { hasBackground, tone } = useOnSurface();
  const look = useChatLook();
  const field = useRef<HTMLInputElement>(null);
  const frame = cn(
    "z-20 box-content flex h-14 shrink-0 items-center px-2 pt-[var(--safe-top)] @xl:px-3",
    hasBackground && look.transparentHeader
      ? tone === "light"
        ? "border-b border-black/10 bg-white/45 backdrop-blur-md"
        : "border-b border-white/10 bg-black/35 backdrop-blur-md"
      : "border-b border-line bg-surface",
  );

  useEffect(() => {
    if (search.open) field.current?.focus({ preventScroll: true });
  }, [search.open]);

  if (search.open) {
    const total = search.hits.length;
    return (
      <header style={columnStyle} className={cn(frame, "gap-2")}>
        <div className="min-w-0 flex-1">
          <Input
            ref={field}
            value={search.text}
            onChange={(event) => search.setText(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Escape") search.stop();
              if (event.key === "Enter") search.step(event.shiftKey ? 1 : -1);
            }}
            placeholder={t("search.placeholder")}
            aria-label={t("search.placeholder")}
            leading={<SearchIcon size={icon.md} />}
            className="h-9"
          />
        </div>
        {search.query && (
          <span className="shrink-0 text-sm tabular-nums text-fg-3" aria-live="polite">
            {total > 0
              ? t("search.position", { index: search.index + 1, total })
              : t("search.none")}
          </span>
        )}
        <IconButton
          label={t("search.previous")}
          size="sm"
          disabled={total === 0}
          onClick={() => search.step(-1)}
        >
          <ChevronUp size={icon.md} />
        </IconButton>
        <IconButton
          label={t("search.next")}
          size="sm"
          disabled={total === 0}
          onClick={() => search.step(1)}
        >
          <ChevronDown size={icon.md} />
        </IconButton>
        <Button variant="ghost" size="sm" onClick={search.stop}>
          {t("search.done")}
        </Button>
      </header>
    );
  }

  const identity = cn(
    "flex min-w-0 flex-1 items-center gap-2.5 rounded-xl px-1.5 py-1 text-left",
    phone && "transition-colors hover:bg-fill",
  );
  const who = (
    <>
      <Avatar src={avatar} name={name} size="sm" />
      <span className="min-w-0">
        <span className="block truncate text-base font-semibold text-fg">{name}</span>
        <span className="flex items-center gap-1.5 text-2xs text-fg-3">
          {model && (
            <span
              className={cn(
                "h-1.5 w-1.5 shrink-0 rounded-full",
                model.local ? "bg-accent" : "bg-info",
              )}
            />
          )}
          {modelKnown && (
            <span className="truncate">{model ? model.label : t("header.noModel")}</span>
          )}
        </span>
      </span>
    </>
  );

  return (
    <header style={columnStyle} className={cn(frame, "gap-1")}>
      <IconButton label={t("header.back")} shape="round" onClick={onBack}>
        <ArrowLeft size={icon.lg} />
      </IconButton>
      {phone ? (
        <button type="button" onClick={onOptions} className={identity}>
          {who}
        </button>
      ) : (
        <div className={identity}>{who}</div>
      )}
      {memory && (
        <IconButton
          label={
            memory.busy ? t("header.memoriesBusy") : t("header.memories", { count: memory.count })
          }
          onClick={onMemory}
          className="relative"
        >
          {memory.busy ? (
            <Loader2 size={icon.lg} className="animate-spin text-accent" />
          ) : (
            <Brain size={icon.lg} />
          )}
          {!memory.busy && memory.count > 0 && (
            <span className="absolute right-0.5 top-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-accent px-1 text-2xs font-semibold leading-none text-on-accent">
              {memory.count}
            </span>
          )}
        </IconButton>
      )}
      <IconButton label={t("header.search")} onClick={search.start}>
        <SearchIcon size={icon.lg} />
      </IconButton>
      {!phone && (
        <IconButton label={t("header.options")} onClick={onOptions}>
          <SlidersHorizontal size={icon.lg} />
        </IconButton>
      )}
    </header>
  );
}
