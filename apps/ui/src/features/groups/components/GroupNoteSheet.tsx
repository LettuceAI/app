import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Chip, Field, toast } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import { NoteSheetShell } from "@/features/characters";
import { newGroupNoteDraft, type GroupNoteDraft } from "../data/notes";

export interface NoteMember {
  id: string;
  name: string;
}

export function GroupNoteSheet({ initial, members, open, onClose, onSubmit, onRemove }: {
  initial: GroupNoteDraft | null;
  members: NoteMember[];
  open: boolean;
  onClose: () => void;
  onSubmit: (draft: GroupNoteDraft) => Promise<void>;
  onRemove?: (() => Promise<void>) | undefined;
}) {
  const { t } = useTranslation("groups");
  const { t: common } = useTranslation("common");
  const [draft, setDraft] = useState(() => initial ?? newGroupNoteDraft(Date.now()));
  const [busy, setBusy] = useState<"save" | "remove" | null>(null);
  const run = async (kind: "save" | "remove", task: () => Promise<void>) => {
    setBusy(kind);
    try {
      await task();
      onClose();
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      setBusy(null);
    }
  };
  const toggle = (id: string, on: boolean) => setDraft((current) => ({ ...current, visibleTo: on ? [...current.visibleTo, id] : current.visibleTo.filter((entry) => entry !== id) }));
  return <NoteSheetShell
    open={open}
    isNew={!initial}
    draft={draft}
    onChange={(patch) => setDraft((current) => ({ ...current, ...patch }))}
    busy={busy !== null}
    saving={busy === "save"}
    onSave={() => void run("save", () => onSubmit(draft))}
    onClose={onClose}
    onDelete={initial && onRemove ? () => void run("remove", onRemove) : undefined}
    copy={{ contentHint: t("notes.contentHint"), contentPlaceholder: t("notes.contentPlaceholder"), deleteBody: (label) => t("notes.deleteBody", { label }) }}
    extra={<Field label={t("notes.whoSees")} hint={draft.visibleTo.length === 0 ? t("notes.everyone") : t("notes.whoSeesHint")}>
      <div className="flex flex-wrap gap-2" role="group" aria-label={t("notes.whoSees")}>
        {members.map((member) => <Chip key={member.id} selected={draft.visibleTo.includes(member.id)} disabled={busy !== null} onClick={() => toggle(member.id, !draft.visibleTo.includes(member.id))}>{member.name}</Chip>)}
      </div>
    </Field>}
  />;
}
