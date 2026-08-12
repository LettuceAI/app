import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { NotebookPen } from "lucide-react";
import type { Session } from "../../../../core/storage/schemas";
import { updateSessionAuthorNote } from "../../../../core/storage/repo";
import { getPlatform } from "../../../../core/utils/platform";
import { cn } from "../../../design-tokens";
import { useI18n } from "../../../../core/i18n/context";

interface InlineAuthorNoteBarProps {
  session: Session | null | undefined;
  onSaved?: (session: Session | null) => void;
}

export function InlineAuthorNoteBar({
  session,
  onSaved,
}: InlineAuthorNoteBarProps) {
  const savedValue = session?.authorNote ?? "";
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(savedValue);
  const [saving, setSaving] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const isDesktop = useMemo(() => getPlatform().type === "desktop", []);
  const { t } = useI18n();

  useEffect(() => {
    if (!editing) {
      setDraft(savedValue);
    }
  }, [savedValue, editing]);

  useLayoutEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "0px";
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
  }, [draft, editing]);

  const handleBeginEdit = () => {
    if (!session?.id) return;
    setDraft(savedValue);
    setEditing(true);
    requestAnimationFrame(() => {
      textareaRef.current?.focus();
      const len = textareaRef.current?.value.length ?? 0;
      textareaRef.current?.setSelectionRange(len, len);
    });
  };

  const commit = async () => {
    if (!session?.id) {
      setEditing(false);
      return;
    }
    const next = draft.trim();
    if (next === savedValue.trim()) {
      setEditing(false);
      return;
    }
    setSaving(true);
    try {
      const updated = await updateSessionAuthorNote(session.id, next || null);
      onSaved?.(updated);
    } catch (err) {
      console.error("Failed to save author note:", err);
    } finally {
      setSaving(false);
      setEditing(false);
    }
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      setDraft(savedValue);
      setEditing(false);
      return;
    }
    if (isDesktop && event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      void commit();
    }
  };

  const showPlaceholder = !editing && savedValue.length === 0;

  return (
    <div className="group flex items-start gap-2.5 p-2">
      <div
        className="flex h-5 w-11 shrink-0 items-center justify-center text-fg/45"
        aria-hidden
      >
        <NotebookPen className="h-4 w-4" />
      </div>

      {editing ? (
        <textarea
          ref={textareaRef}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onBlur={() => void commit()}
          onKeyDown={handleKeyDown}
          disabled={saving}
          rows={1}
          className={cn(
            "m-0 block min-w-0 flex-1 resize-none overflow-hidden border-0 bg-transparent p-0 font-sans",
            "text-[13px] leading-5 text-fg",
            "focus:outline-none focus:ring-0",
          )}
        />
      ) : (
        <button
          type="button"
          onClick={handleBeginEdit}
          className={cn(
            "min-w-0 flex-1 whitespace-pre-wrap break-words text-left",
            "text-[13px] leading-5",
            showPlaceholder ? "text-fg/35" : "text-fg/85",
            "group-hover:text-fg",
          )}
        >
          {showPlaceholder ? t("chats.authorNote.addPlaceholder") : savedValue}
        </button>
      )}

      {saving && <span className="shrink-0 text-[10px] text-fg/35">{t("chats.authorNote.savingShort")}</span>}
    </div>
  );
}
