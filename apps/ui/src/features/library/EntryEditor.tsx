import { useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { Radar } from "lucide-react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Button, Field, icon, IconButton, Input, Sheet, Textarea, toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import { createEntry, refreshLibrary, saveEntry } from "./data/actions";
import type { EntryKind, EntryView } from "./data/queries";

type EditableKind = Exclude<EntryKind, "character">;
type EditableView = Exclude<EntryView, { kind: "character" }>;

export function EntryEditor({
  api,
  kind,
  loaded,
  open,
  onClose,
  onCreated,
}: {
  api: ApiClient;
  kind: EditableKind;
  loaded?: EditableView;
  open: boolean;
  onClose: () => void;
  onCreated?: (kind: EditableKind) => void;
}) {
  const { t } = useTranslation("library");
  const { t: common } = useTranslation();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [original] = useState(loaded);
  const [name, setName] = useState(() =>
    original?.kind === "persona" ? original.view.draft.title : (original?.view.lorebook.name ?? ""),
  );
  const [description, setDescription] = useState(() =>
    original?.kind === "persona" ? original.view.draft.description : "",
  );
  const [operationId] = useState(() => crypto.randomUUID());
  const mutation = useMutation({
    mutationFn: async () => {
      if (original) await saveEntry(api, original, name.trim(), description.trim(), operationId);
      else await createEntry(api, kind, name.trim(), description.trim(), operationId);
    },
    onSuccess: () => {
      toast.success(original ? t("toast.saved") : t("toast.created", { name: name.trim() }));
      if (!original) onCreated?.(kind);
      onClose();
      void refreshLibrary(queryClient);
    },
  });
  const submit = () => {
    if (name.trim() && !mutation.isPending) mutation.mutate();
  };
  return (
    <Sheet
      open={open}
      onClose={() => {
        if (!mutation.isPending) onClose();
      }}
      size="sm"
      title={original ? t(`menu.edit_${kind}`) : t(`create.new_${kind}`)}
      rightAction={
        original?.kind === "lorebook" ? (
          <IconButton
            label={t("triggerPreview")}
            disabled={mutation.isPending}
            onClick={() => {
              void navigate({
                to: "/library/lorebooks/$lorebookId/preview",
                params: { lorebookId: original.view.lorebook.id },
                replace: true,
              });
            }}
          >
            <Radar size={icon.md} />
          </IconButton>
        ) : undefined
      }
      footer={
        <div className="flex gap-2">
          <Button
            variant="secondary"
            className="flex-1"
            disabled={mutation.isPending}
            onClick={onClose}
          >
            {t("edit.cancel")}
          </Button>
          <Button
            variant="primary"
            className="flex-1"
            loading={mutation.isPending}
            disabled={!name.trim() || mutation.isPending}
            onClick={submit}
          >
            {original ? t("edit.save") : t("edit.create")}
          </Button>
        </div>
      }
    >
      <div className="space-y-4">
        <Field label={t("edit.name")}>
          <Input
            data-autofocus
            value={name}
            disabled={mutation.isPending}
            onChange={(event) => setName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") submit();
            }}
            placeholder={t(`edit.placeholder_${kind}`)}
          />
        </Field>
        {kind === "persona" && (
          <Field label={t("edit.description")} hint={t(`edit.hint_${kind}`)}>
            <Textarea
              rows={4}
              value={description}
              disabled={mutation.isPending}
              onChange={(event) => setDescription(event.target.value)}
            />
          </Field>
        )}
        {mutation.isError && (
          <p role="alert" className="text-sm text-danger">
            {common(
              `errors.code.${isApiFailure(mutation.error) ? mutation.error.code : "internal"}`,
            )}
          </p>
        )}
      </div>
    </Sheet>
  );
}
