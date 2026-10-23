import { useState } from "react";
import type { UseQueryResult } from "@tanstack/react-query";
import { Check, Cpu, FileText } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, EmptyState, icon, ListGroup, ListRowButton, SearchField, Sheet, Skeleton } from "@lettuceai/crisp";
import type { ModelView, PromptSummary, ProviderAccountView } from "@/api/generated/bindings";
import { ModelMark } from "@/entities/provider";
import { FailureState } from "@/shared/ui/FailureState";

export type ReplyPicker = "model" | "roleplay" | "conversational";
export type ReplyHelperModel = { model: ModelView; account: ProviderAccountView };

export function ReplyHelperPicker({ open, picker, phone, models, modelLoading, modelFailure, defaultName, prompts, value, onClose, onChange, onManage, onAddModel }: {
  open: boolean;
  picker: ReplyPicker | null;
  phone: boolean;
  models: ReplyHelperModel[];
  modelLoading: boolean;
  modelFailure: { error: unknown; retry: () => void } | null;
  defaultName: string;
  prompts: UseQueryResult<PromptSummary[], Error>;
  value: string | null;
  onClose: () => void;
  onChange: (id: string | null) => void;
  onManage: () => void;
  onAddModel: () => void;
}) {
  const { t } = useTranslation("settings");
  const [query, setQuery] = useState("");
  const [previous, setPrevious] = useState({ open, picker });
  if (previous.open !== open || previous.picker !== picker) {
    setPrevious({ open, picker });
    if (open) setQuery("");
  }
  const q = query.trim().toLocaleLowerCase();
  const foundModels = models.filter(({ model, account }) => !q || `${model.display_name} ${model.external_model_id} ${account.label}`.toLocaleLowerCase().includes(q));
  const groups = [...new Map(foundModels.map(({ account }) => [account.id, account])).values()];
  const customPrompts = (prompts.data ?? []).filter((prompt) => prompt.origin.type !== "built_in");
  const foundPrompts = customPrompts.filter((prompt) => !q || prompt.name.toLocaleLowerCase().includes(q));
  const builtIn = prompts.data?.find((prompt) => prompt.origin.type === "built_in");
  const defaultSelected = value === null || value === builtIn?.id;
  const title = t(picker === "model" ? "replyHelper.model" : picker === "roleplay" ? "replyHelper.roleplayPrompt" : "replyHelper.conversationalPrompt");
  const loading = picker === "model" ? modelLoading : prompts.isPending;
  const failure = picker === "model" ? modelFailure : prompts.isError ? { error: prompts.error, retry: () => void prompts.refetch() } : null;
  const mark = <Check size={icon.md} className="text-accent" />;
  return <Sheet open={open} onClose={onClose} title={title} {...(phone ? { detents: [0.6, 0.92] } : {})}>
    {failure ? <FailureState error={failure.error} onRetry={failure.retry} /> : loading ? <div className="space-y-3" role="status" aria-label={t("replyHelper.loading")}>{[0, 1, 2].map((index) => <Skeleton key={index} shape="block" className="h-16 w-full" />)}</div> : <div className="space-y-5">
      {(picker === "model" ? models.length : customPrompts.length) > 8 && <SearchField value={query} onChange={setQuery} placeholder={t(picker === "model" ? "replyHelper.modelSearch" : "replyHelper.promptSearch")} aria-label={t(picker === "model" ? "replyHelper.modelSearch" : "replyHelper.promptSearch")} />}
      {picker === "model" ? <>
        <ListGroup label={t("replyHelper.defaultGroup")}><ListRowButton icon={<Cpu size={icon.lg} />} title={t("replyHelper.appDefault")} description={defaultName} chevron={false} aria-pressed={value === null} trailing={value === null ? mark : undefined} onClick={() => onChange(null)} /></ListGroup>
        {groups.map((account) => <ListGroup key={account.id} label={account.provider_kind === "llamacpp" ? t("replyHelper.onDevice") : account.label}>{foundModels.filter((choice) => choice.account.id === account.id).map(({ model }) => <ListRowButton key={model.id} icon={<ModelMark modelId={model.external_model_id} providerKind={account.provider_kind} name={model.display_name} />} title={model.display_name} description={account.enabled ? account.label : t("replyHelper.providerDisabled", { provider: account.label })} chevron={false} aria-pressed={value === model.id} trailing={value === model.id ? mark : undefined} disabled={!account.enabled} onClick={() => onChange(model.id)} />)}</ListGroup>)}
        {models.length === 0 ? <EmptyState title={t("replyHelper.noModels")} description={t("replyHelper.noModelsBody")} action={<Button variant="secondary" onClick={onAddModel}>{t("replyHelper.addModel")}</Button>} /> : foundModels.length === 0 ? <EmptyState title={t("replyHelper.noMatches")} /> : null}
      </> : <>
        <ListGroup label={t("replyHelper.app")}><ListRowButton icon={<FileText size={icon.lg} />} title={t("replyHelper.builtIn")} description={builtIn?.name ?? t("replyHelper.builtinMissing")} chevron={false} aria-pressed={defaultSelected} trailing={defaultSelected ? mark : undefined} onClick={() => onChange(null)} /></ListGroup>
        {foundPrompts.length > 0 && <ListGroup label={t("replyHelper.yours")}>{foundPrompts.map((prompt) => <ListRowButton key={prompt.id} icon={<FileText size={icon.lg} />} title={prompt.name} chevron={false} aria-pressed={value === prompt.id} trailing={value === prompt.id ? mark : undefined} onClick={() => onChange(prompt.id)} />)}</ListGroup>}
        {q && foundPrompts.length === 0 && <EmptyState title={t("replyHelper.noMatches")} />}
        <ListGroup><ListRowButton icon={<FileText size={icon.lg} />} title={t("replyHelper.managePrompts")} description={t("replyHelper.managePromptsBody")} onClick={onManage} /></ListGroup>
      </>}
    </div>}
  </Sheet>;
}
