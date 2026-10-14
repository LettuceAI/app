import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, Sheet, Textarea } from "@lettuceai/crisp";
import type { TimelineMessage } from "@/api/generated/bindings";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { textOf } from "../data/messages";

export interface EditSheetProps {
  message: TimelineMessage | null;
  characterName: string;
  busy: boolean;
  onClose: () => void;
  onSave: (message: TimelineMessage, text: string) => void;
}

export function EditSheet({ message, characterName, busy, onClose, onSave }: EditSheetProps) {
  const { t } = useTranslation("conversations");
  const { t: common } = useTranslation("common");
  const { shown, session } = useClosingValue(message);
  const [text, setText] = useState("");
  const [seen, setSeen] = useState(0);
  if (session !== seen) {
    setSeen(session);
    setText(shown ? textOf(shown) : "");
  }
  return (
    <Sheet
      open={message !== null}
      onClose={onClose}
      size="sm"
      title={t("edit.title")}
      {...(shown?.role === "assistant" ? { description: t("edit.assistantHint", { name: characterName }) } : {})}
      footer={
        <div className="flex gap-2">
          <Button variant="secondary" className="flex-1" onClick={onClose}>{common("actions.cancel")}</Button>
          <Button variant="primary" className="flex-1" disabled={busy || !text.trim() || !shown} onClick={() => shown && onSave(shown, text.trim())}>{t("edit.save")}</Button>
        </div>
      }
    >
      <Textarea data-autofocus rows={7} value={text} onChange={(event) => setText(event.target.value)} aria-label={t("edit.field")} />
    </Sheet>
  );
}
