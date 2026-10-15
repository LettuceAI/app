import { useLayoutEffect, useRef, useState } from "react";
import { NotebookPen } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, icon, Spinner, Textarea } from "@lettuceai/crisp";

export function InlineAuthorNoteBar({ saved, enterSaves, onSave }: { saved: string | null; enterSaves: boolean; onSave: (note: string) => Promise<boolean> }) {
  const { t } = useTranslation("conversations");
  const value = saved ?? "";
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);
  const [saving, setSaving] = useState(false);
  const pending = useRef(false);
  const field = useRef<HTMLTextAreaElement>(null);

  useLayoutEffect(() => {
    if (!editing || !field.current) return;
    field.current.focus();
    field.current.setSelectionRange(field.current.value.length, field.current.value.length);
  }, [editing]);

  const commit = async () => {
    if (pending.current) return;
    const next = draft.trim();
    if (next === value.trim()) {
      setEditing(false);
      return;
    }
    pending.current = true;
    setSaving(true);
    try {
      if (await onSave(next)) setEditing(false);
    } finally {
      pending.current = false;
      setSaving(false);
    }
  };

  if (!editing) {
    return (
      <Button
        variant="ghost"
        size="sm"
        className="grid h-auto w-full grid-cols-[minmax(0,1fr)] justify-stretch rounded-lg px-1 py-0 text-left font-normal pointer-coarse:-my-2"
        aria-label={t("note.inlineEdit")}
        title={value || t("note.title")}
        onClick={() => { setDraft(value); setEditing(true); }}
      >
        <span className="flex min-w-0 items-center gap-2">
          <NotebookPen size={icon.sm} className="shrink-0 text-fg-3" aria-hidden />
          <span className="min-w-0 flex-1 truncate text-sm text-fg-3">{value || t("note.inlinePlaceholder")}</span>
        </span>
      </Button>
    );
  }

  return (
    <div className="flex min-w-0 items-start gap-2">
      <NotebookPen size={icon.sm} className="mt-3 shrink-0 text-fg-3" aria-hidden />
      <Textarea
        ref={field}
        rows={1}
        autoGrow
        maxRows={4}
        value={draft}
        aria-label={t("note.inlineField")}
        placeholder={t("note.inlinePlaceholder")}
        disabled={saving}
        className="min-w-0 flex-1 text-sm"
        onChange={(event) => setDraft(event.target.value)}
        onBlur={() => void commit()}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            setDraft(value);
            setEditing(false);
          } else if (enterSaves && event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
            event.preventDefault();
            void commit();
          }
        }}
      />
      {saving && <span role="status" aria-label={t("note.saving")} className="mt-3 shrink-0"><Spinner size="sm" /></span>}
    </div>
  );
}
