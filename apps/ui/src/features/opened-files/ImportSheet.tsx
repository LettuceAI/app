import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { BookOpen, User, UserRound } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Alert, Button, icon, ListGroup, ListRow, Sheet, toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { FileSource, ImportPreview_Serialize } from "@/api/generated/bindings";
import { commitImport, refreshLibrary } from "@/features/library";

type Kind = "character" | "persona" | "lorebook";

function detail(preview: ImportPreview_Serialize, fallback: string, entries: (count: number) => string): string {
  const { content } = preview;
  if (content.kind === "lorebook") return entries(content.entries.length);
  if (content.kind === "character") return content.content.description?.trim() || fallback;
  if (content.kind === "persona") return content.content.description.trim() || fallback;
  return fallback;
}

export function ImportSheet({ api, source, preview, onClose }: { api: ApiClient; source: FileSource; preview: ImportPreview_Serialize; onClose: () => void }) {
  const { t } = useTranslation("opened-files");
  const { t: common } = useTranslation();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [operationId] = useState(() => crypto.randomUUID());
  const kind = preview.content.kind as Kind;
  const Icon = kind === "character" ? User : kind === "persona" ? UserRound : BookOpen;
  const commit = useMutation({
    mutationFn: () => commitImport(api, source, preview.digest, operationId),
    onSuccess: (result) => {
      onClose();
      void refreshLibrary(queryClient);
      if (result.kind !== "imported") {
        toast.success(t("import.started"));
        return;
      }
      const action = {
        label: t("import.open"),
        onClick: () => {
          if (kind === "character") void navigate({ to: "/characters/$characterId/edit", params: { characterId: result.id } });
          else void navigate({ to: "/library", search: { kind, item: result.id } });
        },
      };
      if (result.warnings.length > 0) toast.warning(t("import.warnings", { count: result.warnings.length }), undefined, { action });
      else toast.success(t("import.done", { name: preview.name }), undefined, { action });
    },
  });
  const close = () => {
    if (!commit.isPending) onClose();
  };
  return (
    <Sheet
      open
      onClose={close}
      size="sm"
      title={t(`import.title.${kind}`)}
      footer={<div className="flex gap-2">
        <Button variant="secondary" className="flex-1" disabled={commit.isPending} onClick={close}>{t("cancel")}</Button>
        <Button variant="primary" className="flex-1" loading={commit.isPending} onClick={() => commit.mutate()}>{commit.isError ? t("retry") : t("import.confirm")}</Button>
      </div>}
    >
      <div className="space-y-4 pb-1">
        <ListGroup>
          <ListRow icon={<Icon size={icon.lg} />} title={preview.name} description={detail(preview, t(`import.kind.${kind}`), (count) => t("import.entries", { count }))} />
        </ListGroup>
        {commit.isError && <Alert tone="danger" title={common(`errors.code.${isApiFailure(commit.error) ? commit.error.code : "internal"}`)} />}
      </div>
    </Sheet>
  );
}
