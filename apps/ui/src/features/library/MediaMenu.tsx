import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Download, Trash2, User } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, icon, ListGroup, ListRowButton, relativeTime, Sheet, Spinner, toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { MediaLibraryItem } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { applyAvatar, mediaName, refreshLibrary, saveMedia } from "./data/actions";
import type { Entry } from "./data/queries";

export function MediaMenu({ api, item, entries, open, onClose }: { api: ApiClient; item: MediaLibraryItem; entries: Entry[]; open: boolean; onClose: () => void }) {
  const { t, i18n } = useTranslation("library");
  const { t: common } = useTranslation();
  const queryClient = useQueryClient();
  const [deleting, setDeleting] = useState(false);
  const [operationId] = useState(() => crypto.randomUUID());
  const name = mediaName(item);
  const references = useQuery({
    queryKey: [...queryKeys.media.library(), "references", item.asset.asset_id],
    queryFn: () => api.call("mediaReferences", { asset_id: item.asset.asset_id }),
    enabled: open && item.role === "image",
  });
  const owners = entries.filter((entry) => entry.kind !== "lorebook" && references.data?.some((reference) => reference.kind === entry.kind && reference.owner_id === entry.id));
  const mutation = useMutation({
    mutationFn: async (action: { type: "save" } | { type: "delete" } | { type: "avatar"; entry: Entry }) => {
      if (action.type === "save") {
        if (!await saveMedia(api, item)) return false;
        toast.success(t("toast.mediaSaved"));
      } else if (action.type === "delete") {
        await api.call("mediaLibraryRemove", { asset_id: item.asset.asset_id, client_operation_id: operationId });
        toast.success(t("toast.deleted", { name }));
      } else if (action.entry.kind !== "lorebook") {
        await applyAvatar(api, item, { kind: action.entry.kind, id: action.entry.id });
        toast.success(t("toast.avatar"));
      }
      return true;
    },
    onSuccess: (done) => { if (done) { onClose(); void refreshLibrary(queryClient); } },
  });
  const busy = mutation.isPending;
  return (
    <Sheet open={open} onClose={() => { if (!busy) onClose(); }} size="sm" title={deleting ? t("delete.title", { name }) : name}
      description={deleting ? undefined : [item.width && item.height ? `${item.width} × ${item.height}` : null, relativeTime(item.created_at, undefined, i18n.language)].filter(Boolean).join(" · ")}
      footer={deleting ? <div className="flex gap-2"><Button variant="secondary" className="flex-1" disabled={busy} onClick={onClose}>{t("edit.cancel")}</Button><Button variant="danger" className="flex-1" loading={busy} disabled={busy} onClick={() => mutation.mutate({ type: "delete" })}>{t("delete.confirm")}</Button></div> : undefined}
    >
      {deleting ? <p className="text-base text-fg-2">{t(`delete.${item.role}`)}</p> : <div className="space-y-4 pb-1">
        {item.role === "image" && <img src={item.asset.url} alt={name} className="aspect-square w-full rounded-xl border border-line object-cover" />}
        {item.role === "image" && references.isPending && <div className="flex justify-center py-2"><Spinner /></div>}
        {references.isError && <FailureState error={references.error} onRetry={() => void references.refetch()} size="sm" />}
        <ListGroup>
          {item.role === "image" && owners.map((entry) => <ListRowButton key={`${entry.kind}:${entry.id}`} icon={<User size={icon.lg} />} title={t("menu.avatar", { name: entry.name })} chevron={false} disabled={busy} onClick={() => mutation.mutate({ type: "avatar", entry })} />)}
          <ListRowButton icon={<Download size={icon.lg} />} title={t("menu.save")} chevron={false} disabled={busy} onClick={() => mutation.mutate({ type: "save" })} />
        </ListGroup>
        <ListGroup><ListRowButton icon={<Trash2 size={icon.lg} />} tone="danger" title={item.role === "image" ? t("menu.deleteImage") : t("menu.deleteAudio")} chevron={false} disabled={busy} onClick={() => setDeleting(true)} /></ListGroup>
      </div>}
      {mutation.isError && <p role="alert" className="mt-3 text-sm text-danger">{common(`errors.code.${isApiFailure(mutation.error) ? mutation.error.code : "internal"}`)}</p>}
    </Sheet>
  );
}
