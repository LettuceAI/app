import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { NumberInput } from "@lettuceai/crisp";

/** A number field that keeps what is typed locally and hands it over when focus leaves or a short pause follows. */
export function CommitNumber({ value, min, max, step, precision, onCommit }: { value: number; min: number; max: number; step?: number; precision?: number; onCommit: (value: number) => Promise<unknown> | void }) {
  const [draft, setDraft] = useState<number | null>(null);
  const draftRef = useRef<number | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const flush = () => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    const next = draftRef.current;
    draftRef.current = null;
    if (next === null || next === value) {
      setDraft(null);
      return;
    }
    void Promise.resolve(onCommit(next)).finally(() => setDraft(null));
  };
  const latest = useRef(flush);
  useLayoutEffect(() => {
    latest.current = flush;
  });
  useEffect(() => () => latest.current(), []);
  return (
    <div className="w-32" onBlur={flush}>
      <NumberInput
        value={draft ?? value}
        min={min}
        max={max}
        {...(step !== undefined ? { step } : {})}
        {...(precision !== undefined ? { precision } : {})}
        onChange={(next) => {
          draftRef.current = next;
          setDraft(next);
          if (timer.current) clearTimeout(timer.current);
          timer.current = setTimeout(() => latest.current(), 800);
        }}
      />
    </div>
  );
}
