import { useLayoutEffect, useRef, useState } from "react";
import { NotebookPen } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, icon, Textarea } from "@lettuceai/crisp";

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
  useLayoutEffect(() => {
    if (!field.current) return;
    field.current.style.height = "0px";
    field.current.style.height = `${Math.min(field.current.scrollHeight, 160)}px`;
  }, [draft, editing]);

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

  return (
    <div className="flex items-start gap-2.5 px-3 py-2">
      <NotebookPen size={icon.sm} className="mt-1 shrink-0 text-fg-3" aria-hidden />
      {editing ? (
        <Textarea
          ref={field}
          rows={1}
          value={draft}
          aria-label={t("note.inlineField")}
          disabled={saving}
          className="min-w-0 flex-1 resize-none border-0 bg-transparent p-0 text-sm shadow-none"
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
      ) : (
        <Button variant="ghost" size="sm" className="h-auto min-w-0 flex-1 justify-start whitespace-pre-wrap break-words px-0 text-left font-normal" aria-label={t("note.inlineEdit")} onClick={() => { setDraft(value); setEditing(true); }}>
          {value || t("note.inlinePlaceholder")}
        </Button>
      )}
      {saving && <span className="text-xs text-fg-3">{t("note.saving")}</span>}
    </div>
  );
}
