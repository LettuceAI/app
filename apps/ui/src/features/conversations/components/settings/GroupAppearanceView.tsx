import { useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Button, LoadingState, useCompactLayout } from "@lettuceai/crisp";
import type { GroupDraft_Serialize } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { GroupAppearanceEditor } from "@/features/groups";
import { FailureState } from "@/shared/ui/FailureState";
import type { PanelCtx } from "./context";

export function GroupAppearanceView({ ctx }: { ctx: PanelCtx }) {
  const { t } = useTranslation("groups");
  const phone = useCompactLayout();
  const groupId = ctx.view.source.type === "group" ? ctx.view.source.group_id : null;
  const key = [...queryKeys.groups.all, "appearance", groupId];
  const query = useQuery({ queryKey: key, queryFn: () => ctx.api.call("groupGet", { group_id: groupId! }), enabled: groupId !== null });
  const [draft, setDraft] = useState<GroupDraft_Serialize | null>(null);
  const [error, setError] = useState<unknown>(null);
  const attempt = useRef<{ payload: string; id: string } | null>(null);
  if (!query.data) return query.isError ? <FailureState error={query.error} onRetry={() => void query.refetch()} /> : <LoadingState />;
  const saved = query.data;
  const value = draft ?? saved.draft;
  const dirty = JSON.stringify(value) !== JSON.stringify(saved.draft);
  const save = () => void ctx.run(async () => {
    const payload = JSON.stringify([value, saved.revision]);
    if (attempt.current?.payload !== payload) attempt.current = { payload, id: crypto.randomUUID() };
    try {
      const next = await ctx.api.call("groupSave", { group_id: saved.id, expected_revision: saved.revision, draft: value, client_operation_id: attempt.current.id });
      ctx.queryClient.setQueryData(key, next);
      setDraft(null); setError(null); attempt.current = null;
      await Promise.all([ctx.queryClient.invalidateQueries({ queryKey: queryKeys.groups.all }), ctx.queryClient.invalidateQueries({ queryKey: queryKeys.conversations.all })]);
    } catch (failure) { setError(failure); throw failure; }
  });
  return <div className="space-y-5">
    <fieldset disabled={ctx.busy} className="min-w-0"><GroupAppearanceEditor draft={value} onChange={setDraft} phone={phone} /></fieldset>
    {error !== null && <FailureState error={error} onRetry={() => void ctx.run(async () => {
      const fresh = await query.refetch({ throwOnError: true });
      if (fresh.data) setDraft({ ...fresh.data.draft, presentation: value.presentation, presentation_overrides: value.presentation_overrides ?? null });
      setError(null); attempt.current = null;
    })} />}
    {ctx.footerTarget && createPortal(<div className="flex items-center justify-end gap-2"><span className="mr-auto text-xs text-fg-3" role="status">{t(dirty ? "edit.unsaved" : "edit.saved")}</span>{dirty && <><Button variant="ghost" disabled={ctx.busy} onClick={() => { setDraft(null); setError(null); }}>{t("create.discard")}</Button><Button disabled={ctx.busy || error !== null} loading={ctx.busy} onClick={save}>{t("edit.save")}</Button></>}</div>, ctx.footerTarget)}
  </div>;
}
