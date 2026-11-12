import { useState } from "react";
import { Check, Plus, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, Field, icon, IconButton, Input } from "@lettuceai/crisp";

type PatternCheck = { kind: "ok"; regex: RegExp; folded: RegExp | null } | { kind: "empty" } | { kind: "invalid" } | { kind: "unsupported" };

/** Same folding as the app's matcher: the Turkish I, İ and ı all count as i. */
const foldCase = (value: string) => value.toLowerCase().replaceAll("i\u0307", "i").replaceAll("ı", "i");

/** Lookarounds and backreferences compile in the browser but not in the app's matcher. */
const unsupported = /\(\?<?[=!]|\\[1-9]/;

function check(pattern: string, caseSensitive: boolean): PatternCheck {
  if (!pattern.trim()) return { kind: "empty" };
  if (unsupported.test(pattern)) return { kind: "unsupported" };
  try {
    if (caseSensitive) return { kind: "ok", regex: new RegExp(pattern.trim(), "u"), folded: null };
    return {
      kind: "ok",
      regex: new RegExp(pattern.trim(), "iu"),
      folded: new RegExp(pattern.trim().replaceAll(/[İı]/g, "i"), "iu"),
    };
  } catch {
    return { kind: "invalid" };
  }
}

/** One regular expression per row, each checked as you type, with a sentence to try them on. */
export function PatternEditor({
  rowLabel,
  addLabel,
  tryable = true,
  value,
  caseSensitive,
  disabled,
  onChange,
}: {
  rowLabel: (number: number) => string;
  addLabel: string;
  tryable?: boolean;
  value: readonly string[];
  caseSensitive: boolean;
  disabled: boolean;
  onChange: (patterns: string[]) => void;
}) {
  const { t } = useTranslation("lorebook-editor");
  const [rows, setRows] = useState<string[]>(() => (value.length ? [...value] : [""]));
  const [sample, setSample] = useState("");
  const write = (next: string[]) => {
    setRows(next);
    onChange(next.map((row) => row.trim()).filter(Boolean));
  };
  const checks = rows.map((row) => check(row, caseSensitive));
  const tried = sample.trim()
    ? checks.map((result) =>
        result.kind === "ok" ? result.regex.test(sample) || Boolean(result.folded?.test(foldCase(sample))) : null,
      )
    : null;
  const fires = tried?.some(Boolean) ?? false;
  return (
    <div className="space-y-3">
      <ul className="space-y-2">
        {rows.map((row, index) => {
          const result = checks[index]!;
          const hit = tried?.[index];
          return (
            <li key={index} className="space-y-1">
              <div className="flex items-center gap-2">
                <Input
                  className="flex-1 font-mono"
                  aria-label={rowLabel(index + 1)}
                  aria-invalid={result.kind === "invalid" || result.kind === "unsupported"}
                  value={row}
                  disabled={disabled}
                  placeholder={t("regexPlaceholder")}
                  spellCheck={false}
                  autoCapitalize="off"
                  autoCorrect="off"
                  onChange={(event) => write(rows.map((current, at) => (at === index ? event.target.value : current)))}
                  trailing={
                    hit === true ? (
                      <Check size={icon.sm} className="text-accent" aria-label={t("tryMatch")} />
                    ) : hit === false ? (
                      <X size={icon.sm} className="text-fg-4" aria-label={t("tryNoMatch")} />
                    ) : undefined
                  }
                />
                <IconButton
                  label={t("removePattern")}
                  variant="ghost"
                  disabled={disabled || (rows.length === 1 && !row)}
                  onClick={() => write(rows.length === 1 ? [""] : rows.filter((_, at) => at !== index))}
                >
                  <X size={icon.md} />
                </IconButton>
              </div>
              {(result.kind === "invalid" || result.kind === "unsupported") && (
                <p role="alert" className="text-xs text-danger">
                  {t(result.kind === "invalid" ? "patternInvalid" : "patternUnsupported")}
                </p>
              )}
            </li>
          );
        })}
      </ul>
      <Button variant="ghost" size="sm" leading={<Plus size={icon.sm} />} disabled={disabled} onClick={() => setRows([...rows, ""])}>
        {addLabel}
      </Button>
      {tryable && <Field
        label={t("tryLabel")}
        hint={sample.trim() ? t(fires ? "tryFires" : "tryQuiet") : t("tryHint")}
      >
        <Input value={sample} disabled={disabled} placeholder={t("tryPlaceholder")} onChange={(event) => setSample(event.target.value)} />
      </Field>}
    </div>
  );
}
