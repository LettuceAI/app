import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Button, Field, NumberInput } from "@lettuceai/crisp";
import { useTranslation } from "react-i18next";

export function ReplyHelperNumber({ value, label, min, step = 1, onCommit }: {
  value: number;
  label: string;
  min: number;
  step?: number;
  onCommit: (value: number) => Promise<unknown | null>;
}) {
  const { t } = useTranslation("settings");
  const [draft, setDraft] = useState<number | null>(null);
  const [failed, setFailed] = useState(false);
  const current = useRef<number | null>(null);
  const version = useRef(0);
  const inFlight = useRef(new Set<number>());
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const mounted = useRef(true);
  const flush = async () => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    const next = current.current;
    const edit = version.current;
    if (next === null || inFlight.current.has(edit)) return;
    if (next === value && inFlight.current.size === 0) {
      current.current = null;
      if (mounted.current) { setDraft(null); setFailed(false); }
      return;
    }
    inFlight.current.add(edit);
    const result = await onCommit(next);
    inFlight.current.delete(edit);
    if (!mounted.current || edit !== version.current) return;
    if (result !== null) { current.current = null; setDraft(null); setFailed(false); }
    else setFailed(true);
  };
  const latest = useRef(flush);
  useLayoutEffect(() => { latest.current = flush; });
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; void latest.current(); };
  }, []);
  return <div className="w-36 shrink-0 space-y-1" onBlur={(event) => { if (!event.currentTarget.contains(event.relatedTarget)) void flush(); }} onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); void flush(); } }}>
    <Field label={<span className="sr-only">{label}</span>} className="space-y-0" error={failed ? t("replyHelper.notSaved") : undefined}><NumberInput value={draft ?? value} min={min} max={2 ** 32 - 1} step={step} precision={0} onChange={(next) => {
      version.current++;
      current.current = next;
      setDraft(next);
      setFailed(false);
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => void latest.current(), 800);
    }} /></Field>
    {failed && <Button variant="ghost" size="sm" onClick={() => void flush()}>{t("replyHelper.retry")}</Button>}
  </div>;
}
