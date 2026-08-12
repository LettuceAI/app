import { forwardRef, useEffect, useState } from "react";
import type { InputHTMLAttributes } from "react";

type BaseProps = Omit<
  InputHTMLAttributes<HTMLInputElement>,
  "value" | "onChange" | "type" | "inputMode" | "min" | "max" | "step"
>;

export interface NumberInputProps extends BaseProps {
  value: number | null;
  onChange: (next: number | null) => void;
  min?: number;
  max?: number;
  step?: number;
  /** Number of decimal places to display. Defaults to 0 (integer). */
  decimals?: number;
}

function formatValue(value: number | null, decimals: number): string {
  if (value === null || !Number.isFinite(value)) return "";
  return decimals > 0 ? value.toFixed(decimals) : String(value);
}

/**
 * Number input that keeps a local string draft while the user is typing and
 * only clamps + commits the value on blur / Enter. Press Escape to revert.
 *
 * Solves the common React pitfall where `value` is recomputed every keystroke
 * (e.g. `Math.min(max, Math.max(min, Number(e.target.value)))`), which traps
 * the user at the min/max boundary as soon as they start typing.
 */
export const NumberInput = forwardRef<HTMLInputElement, NumberInputProps>(function NumberInput(
  { value, onChange, min, max, step, decimals = 0, onKeyDown, onBlur, ...rest },
  ref,
) {
  const formatted = formatValue(value, decimals);
  const [draft, setDraft] = useState(formatted);

  useEffect(() => {
    setDraft(formatted);
  }, [formatted]);

  const commit = () => {
    const trimmed = draft.trim();
    if (!trimmed) {
      if (value !== null) {
        onChange(null);
      } else {
        setDraft(formatted);
      }
      return;
    }
    const next = Number(trimmed);
    if (!Number.isFinite(next)) {
      setDraft(formatted);
      return;
    }
    let clamped = next;
    if (min !== undefined) clamped = Math.max(min, clamped);
    if (max !== undefined) clamped = Math.min(max, clamped);
    if (clamped !== value) {
      onChange(clamped);
    } else {
      setDraft(formatted);
    }
  };

  return (
    <input
      ref={ref}
      type="number"
      inputMode={decimals > 0 ? "decimal" : "numeric"}
      min={min}
      max={max}
      step={step}
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={(e) => {
        commit();
        onBlur?.(e);
      }}
      onKeyDown={(e) => {
        onKeyDown?.(e);
        if (e.defaultPrevented) return;
        if (e.key === "Enter") {
          e.currentTarget.blur();
        } else if (e.key === "Escape") {
          setDraft(formatted);
          e.currentTarget.blur();
        }
      }}
      {...rest}
    />
  );
});
