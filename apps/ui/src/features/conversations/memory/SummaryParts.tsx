import { useState } from "react";
import { Sparkles } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, cn, Field, icon, Sheet, Textarea } from "@lettuceai/crisp";
import type { MemorySummaryView } from "@/api/generated/bindings";

export function SummaryCard({ summary, onOpen }: { summary: MemorySummaryView | null; onOpen: () => void }) {
  const { t } = useTranslation("conversations");
  const text = summary?.text ?? "";
  const tokens = summary?.token_count ?? null;
  return (
    <button type="button" onClick={onOpen} className="group block w-full rounded-xl border border-line bg-surface-1 p-4 text-left transition-colors hover:bg-fill">
      <div className="flex items-center justify-between gap-3">
        <span className="inline-flex items-center gap-2 text-base font-medium text-fg">
          <Sparkles size={icon.md} className="text-fg-3" />
          {t("memory.summary.title")}
        </span>
        {tokens !== null && <span className="text-sm text-fg-3">{t("memory.tokens", { count: tokens })}</span>}
      </div>
      <p className={cn("mt-2 line-clamp-2 text-sm leading-relaxed @4xl:line-clamp-5", text ? "text-fg-2" : "text-fg-3")}>{text || t("memory.summary.empty")}</p>
    </button>
  );
}

export interface SummarySheetProps {
  open: boolean;
  text: string;
  dynamic: boolean;
  busy: boolean;
  onClose: () => void;
  onSave: (text: string) => void;
  onClear: () => void;
}

export function SummarySheet({ open, text, dynamic, busy, onClose, onSave, onClear }: SummarySheetProps) {
  const { t } = useTranslation("conversations");
  const [draft, setDraft] = useState(text);
  const trimmed = draft.trim();
  return (
    <Sheet
      open={open}
      onClose={() => {
        if (!busy) onClose();
      }}
      placement="auto"
      size="md"
      title={t("memory.summary.title")}
      description={t("memory.summary.description")}
      footer={
        <div className="flex items-center gap-2">
          <Button variant="ghost" disabled={busy || text === ""} onClick={onClear}>
            {t("memory.summary.clear")}
          </Button>
          <span className="flex-1" />
          <Button variant="ghost" disabled={busy} onClick={onClose}>
            {t("memory.summary.cancel")}
          </Button>
          <Button loading={busy} disabled={busy || trimmed === "" || trimmed === text} onClick={() => onSave(trimmed)}>
            {t("memory.summary.save")}
          </Button>
        </div>
      }
    >
      <div className="pb-2">
        <Field label={t("memory.summary.field")} {...(dynamic ? { hint: t("memory.summary.dynamicHint") } : {})}>
          <Textarea value={draft} disabled={busy} rows={7} onChange={(event) => setDraft(event.target.value)} />
        </Field>
      </div>
    </Sheet>
  );
}
