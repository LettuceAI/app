import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { BookOpen, FileUp, User, UserRound } from "lucide-react";
import { useTranslation } from "react-i18next";
import { icon, ListGroup, ListRowButton, Sheet, toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import { importEntry, refreshLibrary } from "./data/actions";
import type { EntryKind } from "./data/queries";

export function LibraryCreateSheet({ api, open, onClose }: { api: ApiClient; open: boolean; onClose: () => void }) {
  const { t } = useTranslation("library");
  const { t: common } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const mutation = useMutation({ mutationFn: () => importEntry(api), onSuccess: (result) => {
    if (!result) return;
    if (result.kind === "imported") {
      if (result.warnings.length) toast.warning(t("toast.warnings", { count: result.warnings.length }));
      else toast.success(t("toast.imported"));
    }
    onClose();
    void refreshLibrary(queryClient);
  } });
  const create = (kind: EntryKind) => {
    onClose();
    if (kind === "character") void navigate({ to: "/characters/new" });
    else void navigate({ to: "/library", search: { create: kind } });
  };
  return (
    <Sheet open={open} onClose={() => { if (!mutation.isPending) onClose(); }} size="sm" title={t("create.title")} description={t("create.description")}>
      <div className="space-y-4 pb-1">
        <ListGroup>
          <ListRowButton icon={<User size={icon.lg} />} tone="info" title={t("kinds.character")} description={t("create.character")} disabled={mutation.isPending} onClick={() => create("character")} />
          <ListRowButton icon={<UserRound size={icon.lg} />} tone="secondary" title={t("kinds.persona")} description={t("create.persona")} disabled={mutation.isPending} onClick={() => create("persona")} />
          <ListRowButton icon={<BookOpen size={icon.lg} />} tone="warning" title={t("kinds.lorebook")} description={t("create.lorebook")} disabled={mutation.isPending} onClick={() => create("lorebook")} />
        </ListGroup>
        <ListGroup><ListRowButton icon={<FileUp size={icon.lg} />} title={t("create.import")} description={t("create.importHint")} chevron={false} disabled={mutation.isPending} onClick={() => mutation.mutate()} /></ListGroup>
        {mutation.isError && <p role="alert" className="text-sm text-danger">{common(`errors.code.${isApiFailure(mutation.error) ? mutation.error.code : "internal"}`)}</p>}
      </div>
    </Sheet>
  );
}
