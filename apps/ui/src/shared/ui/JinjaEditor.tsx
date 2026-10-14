import { useMemo, useRef } from "react";
import { jinjaTokens } from "./jinja-tokens";

export function JinjaEditor({ value, onChange, label, readOnly = false }: { value: string; onChange?: (value: string) => void; label: string; readOnly?: boolean }) {
  const tokens = useMemo(() => jinjaTokens(value), [value]);
  const highlight = useRef<HTMLPreElement>(null);
  const styles = "m-0 box-border h-full w-full touch-auto overflow-auto whitespace-pre-wrap break-words p-4 font-mono text-xs leading-6 [tab-size:2]";
  return <div data-no-drag className="relative min-h-0 min-w-0 flex-1 overflow-hidden rounded-xl border border-line bg-surface-2 focus-within:border-accent focus-within:ring-1 focus-within:ring-accent/35">
    <pre ref={highlight} className={styles} aria-hidden={!readOnly || undefined} aria-label={readOnly ? label : undefined} tabIndex={readOnly ? 0 : undefined}>
      <code>{tokens.map((token, index) => <span key={index} className={token.className}>{token.text}</span>)}{value.endsWith("\n") ? " " : null}</code>
    </pre>
    {!readOnly && <textarea aria-label={label} value={value} spellCheck={false} autoCapitalize="off" autoCorrect="off" wrap="soft" onChange={(event) => onChange?.(event.target.value)} onScroll={(event) => {
      if (highlight.current) { highlight.current.scrollTop = event.currentTarget.scrollTop; highlight.current.scrollLeft = event.currentTarget.scrollLeft; }
    }} onKeyDown={(event) => {
      if (event.key !== "Tab" || !onChange) return;
      event.preventDefault();
      const input = event.currentTarget;
      const start = input.selectionStart; const end = input.selectionEnd;
      onChange(`${value.slice(0, start)}  ${value.slice(end)}`);
      requestAnimationFrame(() => input.setSelectionRange(start + 2, start + 2));
    }} className={`${styles} absolute inset-0 resize-none bg-transparent text-transparent caret-fg outline-none selection:bg-accent/25`} />}
  </div>;
}
