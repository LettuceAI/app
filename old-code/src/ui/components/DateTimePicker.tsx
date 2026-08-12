import { useMemo, useState } from "react";
import { ChevronDown, ChevronLeft, ChevronRight, ChevronUp } from "lucide-react";
import { cn, interactive } from "../design-tokens";
import { NumberInput } from "./NumberInput";
import { useI18n, type TranslationKey } from "../../core/i18n/context";

const WEEKDAYS = [
  "components.dateTimePicker.weekdays.mon",
  "components.dateTimePicker.weekdays.tue",
  "components.dateTimePicker.weekdays.wed",
  "components.dateTimePicker.weekdays.thu",
  "components.dateTimePicker.weekdays.fri",
  "components.dateTimePicker.weekdays.sat",
  "components.dateTimePicker.weekdays.sun",
] satisfies TranslationKey[];
const MONTHS = [
  "components.dateTimePicker.months.january",
  "components.dateTimePicker.months.february",
  "components.dateTimePicker.months.march",
  "components.dateTimePicker.months.april",
  "components.dateTimePicker.months.may",
  "components.dateTimePicker.months.june",
  "components.dateTimePicker.months.july",
  "components.dateTimePicker.months.august",
  "components.dateTimePicker.months.september",
  "components.dateTimePicker.months.october",
  "components.dateTimePicker.months.november",
  "components.dateTimePicker.months.december",
] satisfies TranslationKey[];
const MONTHS_SHORT = [
  "components.dateTimePicker.monthsShort.jan",
  "components.dateTimePicker.monthsShort.feb",
  "components.dateTimePicker.monthsShort.mar",
  "components.dateTimePicker.monthsShort.apr",
  "components.dateTimePicker.monthsShort.may",
  "components.dateTimePicker.monthsShort.jun",
  "components.dateTimePicker.monthsShort.jul",
  "components.dateTimePicker.monthsShort.aug",
  "components.dateTimePicker.monthsShort.sep",
  "components.dateTimePicker.monthsShort.oct",
  "components.dateTimePicker.monthsShort.nov",
  "components.dateTimePicker.monthsShort.dec",
] satisfies TranslationKey[];
const MIN_YEAR = 1;
const MAX_YEAR = 275759;

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function wrap(value: number, min: number, max: number): number {
  const range = max - min + 1;
  return (((value - min) % range) + range) % range + min;
}

function daysInMonth(year: number, month0: number): number {
  return new Date(year, month0 + 1, 0).getDate();
}

function composeMs(
  year: number,
  month0: number,
  day: number,
  hour: number,
  minute: number,
): number {
  const safeDay = clamp(day, 1, daysInMonth(year, month0));
  return new Date(year, month0, safeDay, hour, minute, 0, 0).getTime();
}

interface DateTimePickerProps {
  valueMs: number;
  onChange: (ms: number) => void;
  className?: string;
}

export function DateTimePicker({ valueMs, onChange, className }: DateTimePickerProps) {
  const { t } = useI18n();
  const [view, setView] = useState<"days" | "months">("days");

  const current = new Date(valueMs);
  const year = current.getFullYear();
  const month0 = current.getMonth();
  const day = current.getDate();
  const hour = current.getHours();
  const minute = current.getMinutes();

  const cells = useMemo(() => {
    const total = daysInMonth(year, month0);
    const leading = (new Date(year, month0, 1).getDay() + 6) % 7; // Monday-first
    const out: (number | null)[] = [];
    for (let i = 0; i < leading; i += 1) out.push(null);
    for (let d = 1; d <= total; d += 1) out.push(d);
    while (out.length % 7 !== 0) out.push(null);
    return out;
  }, [year, month0]);

  const shiftMonth = (delta: number) => {
    const m = month0 + delta;
    const nextYear = year + Math.floor(m / 12);
    const nextMonth = ((m % 12) + 12) % 12;
    onChange(composeMs(nextYear, nextMonth, day, hour, minute));
  };

  const pickDay = (d: number) => onChange(composeMs(year, month0, d, hour, minute));
  const setHour = (h: number) =>
    onChange(composeMs(year, month0, day, clamp(h, 0, 23), minute));
  const setMinute = (m: number) =>
    onChange(composeMs(year, month0, day, hour, clamp(m, 0, 59)));
  const setYear = (y: number) =>
    onChange(composeMs(clamp(y, MIN_YEAR, MAX_YEAR), month0, day, hour, minute));
  const pickMonth = (m: number) => {
    onChange(composeMs(year, m, day, hour, minute));
    setView("days");
  };

  const cellBase =
    "flex h-7 items-center justify-center rounded-md text-[12px] tabular-nums";

  return (
    <div
      className={cn(
        "flex flex-col gap-2 rounded-lg border border-fg/12 bg-fg/5 p-2",
        className,
      )}
    >
      <div className="flex items-center justify-between gap-1">
        <button
          type="button"
          onClick={() => (view === "days" ? shiftMonth(-1) : setYear(year - 1))}
          aria-label={
            view === "days"
              ? t("components.dateTimePicker.previousMonth")
              : t("components.dateTimePicker.previousYear")
          }
          className={cn(
            "flex h-6 w-6 shrink-0 items-center justify-center rounded-md text-fg/55 hover:bg-fg/10 hover:text-fg/85",
            interactive.transition.fast,
          )}
        >
          <ChevronLeft size={15} />
        </button>
        {view === "days" ? (
          <button
            type="button"
            onClick={() => setView("months")}
            className={cn(
              "rounded-md px-2 py-0.5 text-[12px] font-semibold text-fg/80 hover:bg-fg/10",
              interactive.transition.fast,
            )}
          >
            {t(MONTHS[month0])} {year}
          </button>
        ) : (
          <NumberInput
            value={year}
            min={MIN_YEAR}
            max={MAX_YEAR}
            step={1}
            onChange={(next) => {
              if (next === null) return;
              setYear(next);
            }}
            aria-label={t("components.dateTimePicker.year")}
            className={cn(
              "w-16 rounded-md bg-transparent text-center text-[12px] font-semibold tabular-nums text-fg/80",
              "focus:bg-fg/10 focus:outline-none",
              "[appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none",
            )}
          />
        )}
        <button
          type="button"
          onClick={() => (view === "days" ? shiftMonth(1) : setYear(year + 1))}
          aria-label={
            view === "days"
              ? t("components.dateTimePicker.nextMonth")
              : t("components.dateTimePicker.nextYear")
          }
          className={cn(
            "flex h-6 w-6 shrink-0 items-center justify-center rounded-md text-fg/55 hover:bg-fg/10 hover:text-fg/85",
            interactive.transition.fast,
          )}
        >
          <ChevronRight size={15} />
        </button>
      </div>

      {view === "days" ? (
        <div className="grid grid-cols-7 gap-0.5">
          {WEEKDAYS.map((labelKey) => (
            <div
              key={labelKey}
              className="flex h-5 items-center justify-center text-[9px] font-semibold uppercase tracking-wide text-fg/35"
            >
              {t(labelKey)}
            </div>
          ))}
          {cells.map((cell, index) =>
            cell === null ? (
              <div key={`empty-${index}`} className={cellBase} />
            ) : (
              <button
                key={`day-${cell}`}
                type="button"
                onClick={() => pickDay(cell)}
                className={cn(
                  cellBase,
                  interactive.transition.fast,
                  cell === day
                    ? "bg-accent font-semibold text-black"
                    : "text-fg/70 hover:bg-fg/10 hover:text-fg/90",
                )}
              >
                {cell}
              </button>
            ),
          )}
        </div>
      ) : (
        <div className="grid grid-cols-3 gap-1">
          {MONTHS_SHORT.map((labelKey, index) => (
            <button
              key={labelKey}
              type="button"
              onClick={() => pickMonth(index)}
              className={cn(
                "flex h-8 items-center justify-center rounded-md text-[12px] font-medium",
                interactive.transition.fast,
                index === month0
                  ? "bg-accent font-semibold text-black"
                  : "text-fg/70 hover:bg-fg/10 hover:text-fg/90",
              )}
            >
              {t(labelKey)}
            </button>
          ))}
        </div>
      )}

      <div className="flex items-center justify-center gap-2 border-t border-fg/8 pt-2">
        <TimeStepper
          value={hour}
          min={0}
          max={23}
          onSet={setHour}
          label={t("components.dateTimePicker.hour")}
          increaseLabel={t("components.dateTimePicker.increaseHour")}
          decreaseLabel={t("components.dateTimePicker.decreaseHour")}
        />
        <span className="text-lg font-semibold text-fg/50">:</span>
        <TimeStepper
          value={minute}
          min={0}
          max={59}
          onSet={setMinute}
          label={t("components.dateTimePicker.minute")}
          increaseLabel={t("components.dateTimePicker.increaseMinute")}
          decreaseLabel={t("components.dateTimePicker.decreaseMinute")}
        />
      </div>
    </div>
  );
}

function TimeStepper({
  value,
  min,
  max,
  onSet,
  label,
  increaseLabel,
  decreaseLabel,
}: {
  value: number;
  min: number;
  max: number;
  onSet: (value: number) => void;
  label: string;
  increaseLabel: string;
  decreaseLabel: string;
}) {
  const btn = cn(
    "flex h-5 w-12 items-center justify-center rounded-md text-fg/55 hover:bg-fg/10 hover:text-fg/85",
    interactive.transition.fast,
  );
  return (
    <div className="flex flex-col items-center gap-0.5">
      <button
        type="button"
        onClick={() => onSet(wrap(value + 1, min, max))}
        aria-label={increaseLabel}
        className={btn}
      >
        <ChevronUp size={14} />
      </button>
      <NumberInput
        value={value}
        min={min}
        max={max}
        step={1}
        onChange={(next) => {
          if (next === null) return;
          onSet(next);
        }}
        aria-label={label}
        className={cn(
          "w-12 rounded-md bg-transparent text-center text-base font-semibold tabular-nums text-fg/90",
          "focus:bg-fg/10 focus:outline-none",
          "[appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none",
        )}
      />
      <button
        type="button"
        onClick={() => onSet(wrap(value - 1, min, max))}
        aria-label={decreaseLabel}
        className={btn}
      >
        <ChevronDown size={14} />
      </button>
    </div>
  );
}
