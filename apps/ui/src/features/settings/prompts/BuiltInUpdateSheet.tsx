import { useMemo, useState } from "react";
import { diffChars, type ChangeObject } from "diff";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Alert, Badge, Button, Segmented, Sheet, Skeleton, Text, toast, type BadgeTone } from "@lettuceai/crisp";
import type { PromptBuiltinChange, PromptBuiltinChangeKind, PromptEntryView, PromptSummary } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { usePromptText } from "./copy";
import { builtInUpdateQuery, choicePayload, defaultChoices, usePromptAction, type UpdateChoices } from "./data";

const KIND_LABEL: Record<PromptBuiltinChangeKind, string> = { added: "New entry", updated: "Updated", conflict: "Changed in both", removed: "Removed from the built-in" };
const KIND_TONE: Record<PromptBuiltinChangeKind, BadgeTone> = { added: "accent", updated: "accent", conflict: "warning", removed: "neutral" };
const KIND_HINT: Record<PromptBuiltinChangeKind, string> = {
  added: "New in the built-in prompt. Keep mine leaves it out.",
  updated: "You haven't changed this entry.",
  conflict: "You changed this entry and so did the built-in prompt.",
  removed: "No longer in the built-in prompt. Use new deletes it from this prompt.",
};

function Side({ label, entry, parts, side, kept }: { label: string; entry: PromptEntryView; parts?: ChangeObject<string>[] | undefined; side: "current" | "new"; kept: boolean }) {
  const { t } = usePromptText();
  const content = parts
    ? parts.filter((part) => (side === "current" ? !part.added : !part.removed)).map((part, index) => part.added || part.removed
      ? <span key={index} className={`rounded-sm ${kept ? "bg-accent/20 text-accent" : "bg-danger/15 text-danger line-through"}`}>{part.value}</span>
      : <span key={index}>{part.value}</span>)
    : entry.content || t("Empty");
  return <div className="min-w-0 space-y-1 rounded-xl bg-fill-2 p-3">
    <Text variant="label">{label}</Text>
    <p className="truncate text-sm font-medium text-fg">{entry.name}</p>
    <p className="max-h-60 overflow-y-auto whitespace-pre-line break-words font-mono text-xs leading-relaxed text-fg-2">{content}</p>
  </div>;
}

function ChangeRow({ change, take, onChange }: { change: PromptBuiltinChange; take: boolean; onChange: (take: boolean) => void }) {
  const { t } = usePromptText();
  const name = change.new?.name ?? change.current?.name ?? change.entry_key;
  const { current, new: next } = change;
  const parts = useMemo(() => current && next ? diffChars(current.content, next.content) : undefined, [current, next]);
  return <div className="space-y-3 rounded-2xl border border-line bg-surface-1 p-3 @xl:p-4">
    <div className="flex flex-wrap items-center gap-2"><p className="min-w-0 flex-1 truncate text-base font-medium text-fg">{name}</p><Badge tone={KIND_TONE[change.kind]}>{t(KIND_LABEL[change.kind])}</Badge></div>
    <Text variant="hint">{t(KIND_HINT[change.kind])}</Text>
    <div className={`grid gap-2 ${change.current && change.new ? "grid-cols-1 @xl:grid-cols-2" : "grid-cols-1"}`}>
      {change.current && <Side label={t("Yours")} entry={change.current} parts={parts} side="current" kept={!take} />}
      {change.new && <Side label={t("New")} entry={change.new} parts={parts} side="new" kept={take} />}
    </div>
    <Segmented size="sm" aria-label={t("Choice for {{name}}", { name })} value={take ? "new" : "mine"} onChange={(value) => onChange(value === "new")} options={[{ value: "mine", label: t("Keep mine") }, { value: "new", label: t("Use new") }]} />
  </div>;
}

export function BuiltInUpdateSheet({ prompt, open, onClose }: { prompt: PromptSummary; open: boolean; onClose: () => void }) {
  const { t } = usePromptText();
  const api = useSettingsApi();
  const client = useQueryClient();
  const action = usePromptAction();
  const update = useQuery({ ...builtInUpdateQuery(api, prompt.id), enabled: open });
  const [choices, setChoices] = useState<UpdateChoices>({});
  const [seed, setSeed] = useState(update.data);
  if (update.data !== seed) {
    setSeed(update.data);
    setChoices(update.data ? defaultChoices(update.data.changes) : {});
  }
  const data = update.data;
  const send = (keepAll: boolean) => {
    if (!data || action.isPending) return;
    const payload = choicePayload(data.changes, keepAll ? {} : choices);
    action.mutate(async () => {
      const view = await api.call("promptBuiltinUpdateApply", { client_operation_id: action.operation(`builtin-update:${prompt.id}:${data.revision}:${JSON.stringify(payload)}`), prompt_id: prompt.id, expected_revision: data.revision, choices: payload }).catch((error: unknown) => {
        void update.refetch();
        throw error;
      });
      client.setQueryData([...queryKeys.prompts.all, "detail", view.prompt.id], view);
      toast.success(t(payload.some((choice) => choice.take_new) ? "Built-in changes applied" : "Kept your version"));
      onClose();
    });
  };
  const empty = !!data && data.changes.length === 0;
  return <Sheet open={open} dismissible={!action.isPending} onClose={onClose} title={t("Built-in update")} description={data ? t("Changes to {{name}} in the app", { name: data.built_in_name }) : undefined}
    footer={<div className="flex items-center gap-2"><Button variant="ghost" disabled={!data || empty || action.isPending} onClick={() => send(true)}>{t("Keep all mine")}</Button><span className="flex-1" /><Button variant="primary" loading={action.isPending} disabled={!data || empty} onClick={() => send(false)}>{t("Apply")}</Button></div>}>
    <div className="space-y-3 @container">
      {update.isPending && <div role="status" aria-label={t("Loading changes")} className="space-y-3">{[0, 1].map((value) => <Skeleton key={value} shape="block" className="h-28 w-full" />)}</div>}
      {update.isError && <FailureState error={update.error} onRetry={() => void update.refetch()} />}
      {empty && <Alert tone="info" title={t("Nothing left to review")} />}
      {data?.changes.map((change) => <ChangeRow key={change.entry_key} change={change} take={choices[change.entry_key] ?? false} onChange={(take) => setChoices((value) => ({ ...value, [change.entry_key]: take }))} />)}
      {action.isError && <FailureState error={action.error} size="sm" />}
    </div>
  </Sheet>;
}
