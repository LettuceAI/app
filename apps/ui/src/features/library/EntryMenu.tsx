import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { BookOpen, Download, MessageSquarePlus, Pencil, Trash2, UserCheck } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, Button, icon, ListGroup, ListRowButton, Sheet, toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { PersonaDefaultView } from "@/api/generated/bindings";
import { useChatNavigation } from "@/features/conversations";
import { FailureState } from "@/shared/ui/FailureState";
import { deleteEntry, exportEntry, refreshLibrary } from "./data/actions";
import { entryQuery, type Entry } from "./data/queries";
import { EntryEditor } from "./EntryEditor";

export function EntryMenu({ api, entry, defaultPersona, open, onClose }: { api: ApiClient; entry: Entry; defaultPersona?: PersonaDefaultView | undefined; open: boolean; onClose: () => void }) {
  const { t } = useTranslation("library");
  const { t: common } = useTranslation();
  const queryClient = useQueryClient();
  const detail = useQuery({ ...entryQuery(api, entry), enabled: open });
  const [mode, setMode] = useState<"menu" | "edit" | "delete">("menu");
  const [operationId] = useState(() => crypto.randomUUID());
  const chats = useChatNavigation();
  const mutation = useMutation({
    mutationFn: async (action: "delete" | "export" | "default") => {
      if (action === "export") {
        const result = await exportEntry(api, entry);
        if (!result) return false;
        if (result.warnings.length) toast.warning(t("toast.warnings", { count: result.warnings.length }));
        else toast.success(t("toast.exported"));
      } else if (action === "delete" && detail.data) {
        await deleteEntry(api, detail.data, operationId);
        toast.success(t("toast.deleted", { name: entry.name }));
      } else if (action === "default" && defaultPersona) {
        await api.call("personaDefaultSet", { persona_id: entry.id, expected_revision: defaultPersona.revision, client_operation_id: operationId });
        toast.success(t("toast.default", { name: entry.name }));
      } else return false;
      return true;
    },
    onSuccess: (done) => { if (done) { onClose(); void refreshLibrary(queryClient); } },
  });
  const busy = mutation.isPending;
  const ready = Boolean(detail.data) && !detail.isError;
  if (mode === "edit" && detail.data) return <EntryEditor api={api} kind={entry.kind} loaded={detail.data} open={open} onClose={onClose} />;
  return (
    <Sheet open={open} onClose={() => { if (!busy) onClose(); }} size="sm" title={mode === "delete" ? t("delete.title", { name: entry.name }) : entry.name}
      description={mode === "menu" ? entry.kind === "persona" && entry.isDefault ? t("defaultPersona") : t(`kinds.${entry.kind}`) : undefined}
      leftAction={mode === "menu" ? entry.kind === "lorebook" ? <span className="flex h-10 w-10 items-center justify-center rounded-xl border border-line bg-fill-2 text-warning"><BookOpen size={icon.md} /></span> : <Avatar src={entry.image} name={entry.name} size="md" /> : undefined}
      footer={mode === "delete" ? <div className="flex gap-2"><Button variant="secondary" className="flex-1" disabled={busy} onClick={onClose}>{t("edit.cancel")}</Button><Button variant="danger" className="flex-1" loading={busy} disabled={busy || !ready} onClick={() => mutation.mutate("delete")}>{t("delete.confirm")}</Button></div> : undefined}
    >
      {mode === "delete" ? <p className="text-base text-fg-2">{t(`delete.${entry.kind}`)}</p> : <div className="space-y-4 pb-1">
        <ListGroup>
          {entry.kind === "character" && <ListRowButton icon={<MessageSquarePlus size={icon.lg} />} tone="accent" title={t("menu.start")} chevron={false} disabled={busy} onClick={() => { onClose(); void chats.openCharacter(entry.id, null); }} />}
          <ListRowButton icon={<Pencil size={icon.lg} />} title={t(`menu.edit_${entry.kind}`)} description={detail.data?.kind === "lorebook" ? t("entries", { count: detail.data.view.entries.length }) : undefined} disabled={busy || !ready} onClick={() => setMode("edit")} />
          {entry.kind === "persona" && !entry.isDefault && <ListRowButton icon={<UserCheck size={icon.lg} />} title={t("menu.default")} description={t("menu.defaultHint")} chevron={false} disabled={busy || !defaultPersona} onClick={() => mutation.mutate("default")} />}
          <ListRowButton icon={<Download size={icon.lg} />} title={t("menu.export")} description={t(`menu.export_${entry.kind}`)} chevron={false} disabled={busy} onClick={() => mutation.mutate("export")} />
        </ListGroup>
        <ListGroup><ListRowButton icon={<Trash2 size={icon.lg} />} tone="danger" title={t(`menu.delete_${entry.kind}`)} chevron={false} disabled={busy || !ready} onClick={() => setMode("delete")} /></ListGroup>
        {detail.isError && <FailureState error={detail.error} onRetry={() => void detail.refetch()} size="sm" />}
      </div>}
      {mutation.isError && <p role="alert" className="mt-3 text-sm text-danger">{common(`errors.code.${isApiFailure(mutation.error) ? mutation.error.code : "internal"}`)}</p>}
    </Sheet>
  );
}
