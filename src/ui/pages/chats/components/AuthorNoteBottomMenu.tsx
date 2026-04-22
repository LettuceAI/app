import { useEffect, useMemo, useState } from "react";
import type { Session } from "../../../../core/storage/schemas";
import { updateSessionAuthorNote } from "../../../../core/storage/repo";
import { BottomMenu } from "../../../components/BottomMenu";
import { cn, radius } from "../../../design-tokens";

interface AuthorNoteBottomMenuProps {
  isOpen: boolean;
  onClose: () => void;
  session: Session | null | undefined;
  onSaved?: (session: Session | null) => void;
}

export function AuthorNoteBottomMenu({
  isOpen,
  onClose,
  session,
  onSaved,
}: AuthorNoteBottomMenuProps) {
  const [draft, setDraft] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) return;
    setDraft(session?.authorNote ?? "");
    setError(null);
  }, [isOpen, session?.authorNote]);

  const trimmedDraft = draft.trim();
  const savedValue = session?.authorNote?.trim() ?? "";
  const isDirty = trimmedDraft !== savedValue;

  const footerText = useMemo(() => {
    if (!trimmedDraft) {
      return savedValue ? "Author note will be removed on save" : "No author note saved";
    }
    return `${trimmedDraft.length} characters`;
  }, [savedValue, trimmedDraft]);

  const handleSave = async () => {
    if (!session?.id || saving) return;
    setSaving(true);
    setError(null);
    try {
      const updated = await updateSessionAuthorNote(session.id, trimmedDraft || null);
      onSaved?.(updated);
      onClose();
    } catch (err) {
      console.error("Failed to save author note:", err);
      setError(typeof err === "string" ? err : "Failed to save author note");
    } finally {
      setSaving(false);
    }
  };

  const handleClear = async () => {
    if (!session?.id || saving) return;
    setDraft("");
    setSaving(true);
    setError(null);
    try {
      const updated = await updateSessionAuthorNote(session.id, null);
      onSaved?.(updated);
      onClose();
    } catch (err) {
      console.error("Failed to clear author note:", err);
      setError(typeof err === "string" ? err : "Failed to clear author note");
    } finally {
      setSaving(false);
    }
  };

  return (
    <BottomMenu
      isOpen={isOpen}
      onClose={() => {
        if (!saving) onClose();
      }}
      title="Author Note"
    >
      <div className="space-y-4">
        <textarea
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          disabled={!session?.id || saving}
          placeholder="Private direction for this chat"
          rows={8}
          className={cn(
            "w-full resize-none border bg-fg/[0.04] px-3 py-3 text-sm leading-relaxed text-fg outline-none",
            "placeholder:text-fg/35 focus:border-fg/25 focus:bg-fg/[0.06]",
            "disabled:cursor-not-allowed disabled:opacity-60",
            radius.md,
          )}
        />

        <div className="flex items-center justify-between gap-3 text-xs text-fg/45">
          <span>{footerText}</span>
          {error ? <span className="text-red-300">{error}</span> : null}
        </div>

        <div className="grid grid-cols-2 gap-2">
          <button
            type="button"
            onClick={() => void handleClear()}
            disabled={!session?.id || saving || !savedValue}
            className={cn(
              "h-11 border px-3 text-sm font-semibold transition",
              radius.md,
              "border-fg/10 bg-fg/[0.04] text-fg/75 hover:border-fg/20 hover:bg-fg/[0.07]",
              "disabled:cursor-not-allowed disabled:opacity-45",
            )}
          >
            Clear
          </button>
          <button
            type="button"
            onClick={() => void handleSave()}
            disabled={!session?.id || saving || !isDirty}
            className={cn(
              "h-11 border px-3 text-sm font-semibold transition",
              radius.md,
              "border-emerald-400/35 bg-emerald-500/20 text-emerald-100 hover:bg-emerald-500/30",
              "disabled:cursor-not-allowed disabled:opacity-45",
            )}
          >
            {saving ? "Saving..." : "Save"}
          </button>
        </div>
      </div>
    </BottomMenu>
  );
}
