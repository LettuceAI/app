import { useState } from "react";
import { Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Field, icon, Input, ListGroup, ListRowButton, Sheet, toast } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";

export function LorebookSettingsSheet({
  open,
  name,
  deleting,
  onSave,
  onClose,
  onDelete,
}: {
  open: boolean;
  name: string;
  deleting: boolean;
  onSave: (name: string) => Promise<unknown>;
  onClose: () => void;
  onDelete: () => void;
}) {
  const { t } = useTranslation("lorebook-editor");
  const { t: common } = useTranslation();
  const [draftName, setDraftName] = useState(name);
  const commitName = () => {
    const next = draftName.trim();
    setDraftName(next || name);
    if (!next || next === name) return;
    onSave(next).catch((error: unknown) =>
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`)),
    );
  };
  return (
    <Sheet open={open} onClose={onClose} dismissible={!deleting} title={t("settingsTitle")}>
      <div className="space-y-5 pb-1">
        <Field label={t("nameField")}>
          <Input
            value={draftName}
            disabled={deleting}
            onChange={(event) => setDraftName(event.target.value)}
            onBlur={commitName}
            onKeyDown={(event) => {
              if (event.key === "Enter") event.currentTarget.blur();
              else if (event.key === "Escape" && draftName !== name) {
                event.stopPropagation();
                setDraftName(name);
              }
            }}
          />
        </Field>
        <ListGroup>
          <ListRowButton
            icon={<Trash2 size={icon.lg} />}
            tone="danger"
            title={t("deleteLorebook")}
            chevron={false}
            disabled={deleting}
            onClick={onDelete}
          />
        </ListGroup>
      </div>
    </Sheet>
  );
}
