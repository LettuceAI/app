import { useState } from "react";
import { Sparkles } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, cn, Field, icon, Sheet, Textarea } from "@lettuceai/crisp";
import type { MemorySummaryPartsView, MemorySummaryView } from "@/api/generated/bindings";
import { partTier } from "./summary-parts";
import { SummaryPartsPanel } from "./SummaryPartsPanel";

export function SummaryCard({ summary, parts = null, onOpen, companion = false }: { summary: MemorySummaryView | null; parts?: MemorySummaryPartsView | null; onOpen: () => void; companion?: boolean }) {
  const { t } = useTranslation("conversations");
  const text = summary?.text ?? "";
  const tokens = summary?.token_count ?? null;
  const shown = parts?.parts ?? [];
  const chapters = shown.filter((part) => partTier(part) === "chapter").length;
  const arcs = shown.filter((part) => partTier(part) === "arc").length;
  return (
    <button type="button" onClick={onOpen} className="group block w-full rounded-xl border border-line bg-surface-1 p-4 text-left transition-colors hover:bg-fill">
      <div className="flex items-center justify-between gap-3">
        <span className="inline-flex items-center gap-2 text-base font-medium text-fg">
          <Sparkles size={icon.md} className="text-fg-3" />
          {t("memory.summary.title")}
        </span>
        {tokens !== null && <span className="text-sm text-fg-3">{t("memory.tokens", { count: tokens })}</span>}
      </div>
      {shown.length > 0 && (
        <p className="mt-1 text-xs text-fg-3">
          {[t("memory.summary.parts.count", { count: shown.length }), ...(chapters > 0 ? [t("memory.summary.parts.chapters", { count: chapters })] : []), ...(arcs > 0 ? [t("memory.summary.parts.arcs", { count: arcs })] : [])].join(" · ")}
        </p>
      )}
      <p className={cn("mt-2 text-sm leading-relaxed", companion ? "line-clamp-4" : "line-clamp-2 @4xl:line-clamp-5", text ? "text-fg-2" : "text-fg-3")}>{text || t("memory.summary.empty")}</p>
    </button>
  );
}

export interface SummarySheetProps {
  open: boolean;
  text: string;
  parts?: MemorySummaryPartsView | null;
  conversationId?: string;
  textOnly?: boolean;
  dynamic: boolean;
  busy: boolean;
  onClose: () => void;
  onSave: (text: string, close: boolean) => void;
  onClear: () => void;
}

export function SummarySheet({ open, text, parts = null, conversationId, textOnly = false, dynamic, busy, onClose, onSave, onClear }: SummarySheetProps) {
  const { t } = useTranslation("conversations");
  const chapters = dynamic && parts !== null;
  const [mode, setMode] = useState<"parts" | "text">(chapters && !textOnly ? "parts" : "text");
  const [draft, setDraft] = useState(text);
  const trimmed = draft.trim();
  const showParts = chapters && mode === "parts";
  const switchMode = () => {
    setDraft(text);
    setMode(showParts ? "text" : "parts");
  };
  return (
    <Sheet
      open={open}
      onClose={() => {
        if (!busy) onClose();
      }}
      placement="auto"
      size="md"
      title={t("memory.summary.title")}
      description={t(chapters ? "memory.summary.parts.description" : "memory.summary.description")}
      footer={
        <div className="flex items-center gap-2">
          {!showParts && (
            <Button variant="ghost" disabled={busy || text === ""} onClick={onClear}>
              {t("memory.summary.clear")}
            </Button>
          )}
          {chapters && !textOnly && !showParts && (
            <Button variant="ghost" disabled={busy} onClick={switchMode}>
              {t("memory.summary.parts.asParts")}
            </Button>
          )}
          <span className="flex-1" />
          <Button variant="ghost" disabled={busy} onClick={onClose}>
            {t(showParts ? "memory.summary.parts.done" : "memory.summary.cancel")}
          </Button>
          {!showParts && (
            <Button loading={busy} disabled={busy || trimmed === "" || trimmed === text} onClick={() => onSave(trimmed, true)}>
              {t("memory.summary.save")}
            </Button>
          )}
        </div>
      }
    >
      {showParts ? (
        <SummaryPartsPanel view={parts} conversationId={conversationId} busy={busy} onSave={(next) => onSave(next, false)} onClear={onClear} onWriteText={switchMode} />
      ) : (
        <div className="pb-2">
          <Field label={t("memory.summary.field")} {...(dynamic ? { hint: t(chapters ? "memory.summary.parts.textHint" : "memory.summary.dynamicHint") } : {})}>
            <Textarea value={draft} disabled={busy} rows={7} onChange={(event) => setDraft(event.target.value)} />
          </Field>
        </div>
      )}
    </Sheet>
  );
}
