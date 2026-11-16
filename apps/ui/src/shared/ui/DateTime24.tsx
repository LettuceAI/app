import { useState } from "react";
import { CalendarDays } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Calendar, cn, Popover, PopoverContent, PopoverTrigger } from "@lettuceai/crisp";

export interface DateTime24Props {
  value: Date | null;
  onChange: (value: Date) => void;
  placeholder?: string;
  disabled?: boolean;
  locale?: string;
  min?: Date;
  max?: Date;
  className?: string;
}

const pad = (n: number) => String(n).padStart(2, "0");

/** Date + time field whose time is always 24h, independent of the webview locale, unlike a native `<input type="time">`. */
export function DateTime24({ value, onChange, placeholder, disabled = false, locale, min, max, className }: DateTime24Props) {
  const { t, i18n } = useTranslation();
  const loc = locale ?? i18n.language;
  const hour = value ? value.getHours() : 0;
  const minute = value ? value.getMinutes() : 0;
  const label = value ? `${new Intl.DateTimeFormat(loc, { dateStyle: "medium" }).format(value)}, ${pad(hour)}:${pad(minute)}` : (placeholder ?? "");
  const setParts = (h: number, m: number) => {
    const next = new Date(value ?? new Date());
    next.setHours(h, m, 0, 0);
    onChange(next);
  };
  return (
    <Popover>
      <PopoverTrigger>
        <button
          type="button"
          disabled={disabled}
          className={cn(
            "control-md flex w-full items-center gap-2.5 rounded-xl border border-line bg-fill px-3.5 text-left transition-colors hover:border-line-2 focus-visible:border-accent focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent/35 disabled:cursor-not-allowed disabled:opacity-50",
            className,
          )}
        >
          <CalendarDays size={16} className="shrink-0 text-fg-3" />
          <span className={cn("min-w-0 flex-1 truncate", value ? "text-fg" : "text-fg-4")}>{label}</span>
        </button>
      </PopoverTrigger>
      <PopoverContent className="p-3">
        <Calendar value={value} locale={loc} onChange={(picked) => { const next = new Date(picked); next.setHours(hour, minute, 0, 0); onChange(next); }} {...(min ? { min } : {})} {...(max ? { max } : {})} />
        <div className="mt-3 border-t border-line pt-3">
          <div className="flex items-center justify-center gap-2">
            <TimeField value={hour} max={23} disabled={disabled} label={t("dateTime.hour")} onChange={(h) => setParts(h, minute)} />
            <span className="text-fg-3">{t("dateTime.separator")}</span>
            <TimeField value={minute} max={59} disabled={disabled} label={t("dateTime.minute")} onChange={(m) => setParts(hour, m)} />
          </div>
          <p className="mt-1.5 text-center text-2xs text-fg-3">{t("dateTime.format24")}</p>
        </div>
      </PopoverContent>
    </Popover>
  );
}

function TimeField({ value, max, disabled, label, onChange }: { value: number; max: number; disabled: boolean; label: string; onChange: (value: number) => void }) {
  const [text, setText] = useState<string | null>(null);
  const clamp = (n: number) => Math.min(max, Math.max(0, n));
  const commit = () => {
    if (text === null) return;
    const n = Number.parseInt(text, 10);
    setText(null);
    if (text.trim() !== "" && !Number.isNaN(n)) onChange(clamp(n));
  };
  return (
    <input
      type="text"
      inputMode="numeric"
      aria-label={label}
      disabled={disabled}
      value={text ?? pad(value)}
      onChange={(event) => {
        const raw = event.target.value;
        setText(raw);
        const n = Number.parseInt(raw, 10);
        if (raw.trim() !== "" && !Number.isNaN(n) && n >= 0 && n <= max) onChange(n);
      }}
      onBlur={commit}
      onKeyDown={(event) => { if (event.key === "Enter") commit(); }}
      className="control-md w-14 rounded-xl border border-line bg-fill text-center text-base tabular-nums text-fg outline-none transition-colors hover:border-line-2 focus:border-accent focus:ring-1 focus:ring-accent/35 disabled:cursor-not-allowed disabled:opacity-50"
    />
  );
}
