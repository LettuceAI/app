import { useState } from "react";
import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge, Button, icon, IconButton, MediaTile, Sheet, Textarea } from "@lettuceai/crisp";
import type { TimelineMessage } from "@/api/generated/bindings";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { mediaOf, textOf } from "../data/messages";

export interface EditSheetProps {
  message: TimelineMessage | null;
  characterName: string;
  busy: boolean;
  onClose: () => void;
  onSave: (message: TimelineMessage, text: string, keepMedia: string[]) => void;
}

export function EditSheet({ message, characterName, busy, onClose, onSave }: EditSheetProps) {
  const { t } = useTranslation("conversations");
  const { t: common } = useTranslation("common");
  const { shown, session } = useClosingValue(message);
  const [text, setText] = useState("");
  const [removed, setRemoved] = useState<readonly string[]>([]);
  const [seen, setSeen] = useState(0);
  if (session !== seen) {
    setSeen(session);
    setText(shown ? textOf(shown) : "");
    setRemoved([]);
  }
  const media = shown ? mediaOf(shown).filter((item) => !removed.includes(item.id)) : [];
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
          <Button variant="primary" className="flex-1" disabled={busy || !text.trim() || !shown} onClick={() => shown && onSave(shown, text.trim(), media.map((item) => item.id))}>{t("edit.save")}</Button>
        </div>
      }
    >
      <div className="space-y-3">
        <Textarea data-autofocus rows={7} value={text} onChange={(event) => setText(event.target.value)} aria-label={t("edit.field")} />
        {media.length > 0 && (
          <div className="grid grid-cols-3 gap-2">
            {media.map((item) => {
              const name = item.name ?? t(item.kind === "audio" || item.kind === "video" || item.kind === "document" ? `message.${item.kind}` : "message.image");
              return (
                <div key={item.id} className="relative">
                  {item.url && (item.kind === "image" || item.kind === null) ? (
                    <MediaTile src={item.url} alt={name} className="aspect-square" />
                  ) : (
                    <div className="flex aspect-square items-center justify-center rounded-lg border border-line p-2 text-center">
                      <Badge tone="neutral">{name}</Badge>
                    </div>
                  )}
                  <div className="absolute right-1 top-1">
                    <IconButton size="sm" variant="overlay" label={t("edit.removeMedia", { name })} disabled={busy} onClick={() => setRemoved((current) => [...current, item.id])}>
                      <X size={icon.sm} />
                    </IconButton>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </Sheet>
  );
}
