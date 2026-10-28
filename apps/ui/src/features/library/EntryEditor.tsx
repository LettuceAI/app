import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Button, Field, Input, Sheet, toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import { createEntry, refreshLibrary } from "./data/actions";

type EditableKind = "lorebook";

export function EntryEditor({
  api,
  kind,
  open,
  onClose,
  onCreated,
}: {
  api: ApiClient;
  kind: EditableKind;
  open: boolean;
  onClose: () => void;
  onCreated?: (kind: EditableKind) => void;
}) {
  const { t } = useTranslation("library");
  const { t: common } = useTranslation();
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [operationId] = useState(() => crypto.randomUUID());
  const mutation = useMutation({
    mutationFn: () => createEntry(api, name.trim(), operationId),
    onSuccess: () => {
      toast.success(t("toast.created", { name: name.trim() }));
      onCreated?.(kind);
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
      title={t(`create.new_${kind}`)}
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
            {t("edit.create")}
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
