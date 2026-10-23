import { useCallback, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { useLocation, useNavigate, useRouter, useSearch } from "@tanstack/react-router";
import { Check, Cpu, FileText, Globe, Hash, History, MessageCircle, PenLine, Theater, Zap } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge, icon, ListGroup, ListRow, ListRowButton, Page, Skeleton, Switch, Text } from "@lettuceai/crisp";
import type { SettingsHelpMeReplySettings } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { promptsQuery } from "@/features/characters";
import { modelsQuery } from "@/features/conversations";
import { FailureState } from "@/shared/ui/FailureState";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { useSettingsApi, useSettingsLayout } from "../data";
import { useSettingsWrite } from "../use-settings-write";
import { ReplyHelperNumber } from "./ReplyHelperNumber";
import { ReplyHelperPicker, type ReplyHelperModel, type ReplyPicker } from "./ReplyHelperPicker";

type Style = SettingsHelpMeReplySettings["style"];
const styles = ["roleplay", "conversational"] as const;
const fields = { roleplay: "roleplay_prompt_id", conversational: "conversational_prompt_id" } as const;
const kinds = { roleplay: "reply_helper_roleplay", conversational: "reply_helper_conversational" } as const;

export function ReplyHelperPage() {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const { phone, back } = useSettingsLayout();
  const mainRef = useRef<HTMLElement>(null);
  const navigate = useNavigate();
  const router = useRouter();
  const picker = useSearch({ from: "/settings/$section", select: (search) => search.replyPicker ?? null });
  const shown = useClosingValue(picker).shown;
  const pushed = useLocation({ select: (location) => location.state.replyPickerPushed });
  const settings = useQuery({ queryKey: queryKeys.settings.detail(), queryFn: () => api.call("settingsGet") });
  const value = settings.data?.global.help_me_reply;
  const enabled = value?.enabled ?? false;
  const models = useQuery({ ...modelsQuery(api), enabled });
  const accounts = useQuery({ queryKey: queryKeys.providers.accounts(), queryFn: () => api.call("providerAccountsList"), enabled });
  const roleplay = useQuery({ ...promptsQuery(api, kinds.roleplay), enabled });
  const conversational = useQuery({ ...promptsQuery(api, kinds.conversational), enabled });
  const prompts = { roleplay, conversational };
  const write = useSettingsWrite(api);
  const change = useCallback((patch: Partial<SettingsHelpMeReplySettings>) => write((latest) => ({ section: "help_me_reply", value: { ...latest.global.help_me_reply, ...patch } })), [write]);
  const byAccount = new Map((accounts.data ?? []).map((account) => [account.id, account]));
  const choices: ReplyHelperModel[] = (models.data?.models ?? []).flatMap((model) => {
    const account = byAccount.get(model.provider_account_id);
    return model.kind === "chat" && model.input_scopes.includes("text") && model.output_scopes.includes("text") && account ? [{ model, account }] : [];
  });
  const selectedModel = choices.find((choice) => choice.model.id === value?.model_profile_id);
  const defaultModel = choices.find((choice) => choice.model.id === settings.data?.default_model_profile_id);
  const defaultName = settings.data?.default_model_profile_id === null ? t("replyHelper.noDefault") : defaultModel?.account.enabled ? defaultModel.model.display_name : t("replyHelper.defaultUnavailable");
  const modelDescription = value?.model_profile_id === null ? t("replyHelper.defaultModel", { name: defaultName }) : !selectedModel ? t("replyHelper.modelUnavailable") : !selectedModel.account.enabled ? t("replyHelper.modelDisabled") : t("replyHelper.modelIdentity", { provider: selectedModel.account.label, name: selectedModel.model.display_name });
  const modelFailure = [models, accounts].find((query) => query.isError);
  const modelLoading = models.isPending || accounts.isPending;
  const close = () => {
    if (pushed) router.history.back();
    else void navigate({ to: "/settings/$section", params: { section: "replyHelper" }, search: {}, replace: true, resetScroll: false });
  };
  const open = (next: ReplyPicker) => void navigate({ to: "/settings/$section", params: { section: "replyHelper" }, search: { replyPicker: next }, state: { replyPickerPushed: true }, resetScroll: false });
  const choose = (id: string | null) => {
    if (!picker) return;
    void change(picker === "model" ? { model_profile_id: id } : { [fields[picker]]: id });
    close();
  };
  const promptRow = (style: Style) => {
    const query = prompts[style];
    const id = value?.[fields[style]] ?? null;
    const selected = query.data?.find((prompt) => prompt.id === id);
    const description = query.isPending ? <Skeleton className="h-4 w-32" /> : id === null ? t("replyHelper.builtIn") : selected?.name ?? t("replyHelper.promptUnavailable");
    return <ListRowButton key={style} icon={<FileText size={icon.lg} />} title={t(style === "roleplay" ? "replyHelper.roleplayPrompt" : "replyHelper.conversationalPrompt")} description={description} trailing={value?.style === style ? <Badge tone="accent" dot>{t("replyHelper.inUse")}</Badge> : undefined} onClick={() => open(style)} />;
  };
  return <>
    <Page mainRef={mainRef} title={t("sections.replyHelper.title")} meta={phone ? undefined : t("replyHelper.meta")} {...back} contentClassName="max-w-3xl space-y-7">
      {settings.isError && <FailureState error={settings.error} onRetry={() => void settings.refetch()} />}
      {!value ? settings.isError ? null : <div className="space-y-7" role="status" aria-label={t("replyHelper.loading")}>{[1, 2, 2, 2, 2].map((count, index) => <ListGroup key={index}>{Array.from({ length: count }, (_, row) => <ListRow key={row} title={<Skeleton className="h-5 w-32" />} description={<Skeleton className="h-4 w-56 max-w-full" />} trailing={<Skeleton className="h-6 w-10" />} />)}</ListGroup>)}</div> : <>
        <ListGroup><ListRow icon={<PenLine size={icon.lg} />} title={t("sections.replyHelper.title")} description={t("replyHelper.enabledBody")} trailing={<Switch checked={value.enabled} onChange={(next) => void change({ enabled: next })} aria-label={t("sections.replyHelper.title")} />} /></ListGroup>
        {value.enabled && <>
          <section className="space-y-2">
            <ListGroup label={t("replyHelper.style")}>{styles.map((style) => <ListRowButton key={style} icon={style === "roleplay" ? <Theater size={icon.lg} /> : <MessageCircle size={icon.lg} />} tone={style === value.style ? "accent" : "neutral"} title={t(`replyHelper.${style}`)} description={t(`replyHelper.${style}Hint`)} chevron={false} aria-pressed={style === value.style} trailing={style === value.style ? <Check size={icon.md} className="text-accent" /> : undefined} onClick={() => void change({ style })} />)}</ListGroup>
            <Text variant="hint">{t("replyHelper.example", { text: t(`replyHelper.${value.style}Example`) })}</Text>
          </section>
          <ListGroup label={t("replyHelper.modelGroup")}>
            <ListRowButton icon={selectedModel?.account.provider_kind === "llamacpp" || (value.model_profile_id === null && defaultModel?.account.provider_kind === "llamacpp") ? <Cpu size={icon.lg} /> : <Globe size={icon.lg} />} title={t("replyHelper.model")} description={modelLoading && !modelFailure ? <Skeleton className="h-4 w-40" /> : modelDescription} onClick={() => open("model")} />
            <ListRow icon={<Zap size={icon.lg} />} title={t("replyHelper.streaming")} description={t("replyHelper.streamingBody")} trailing={<Switch checked={value.streaming} onChange={(next) => void change({ streaming: next })} aria-label={t("replyHelper.streaming")} />} />
          </ListGroup>
          {modelFailure && <FailureState error={modelFailure.error} onRetry={() => void modelFailure.refetch()} />}
          <ListGroup label={t("replyHelper.lengthGroup")}>
            <ListRow icon={<Hash size={icon.lg} />} title={t("replyHelper.maxLength")} description={t("replyHelper.maxLengthBody")} trailing={<ReplyHelperNumber value={value.max_output_tokens} label={t("replyHelper.maxLength")} min={1} step={50} onCommit={(next) => change({ max_output_tokens: next })} />} />
            <ListRow icon={<History size={icon.lg} />} title={t("replyHelper.history")} description={t(value.history_count === 0 ? "replyHelper.historyDefault" : "replyHelper.historyBody")} trailing={<ReplyHelperNumber value={value.history_count} label={t("replyHelper.history")} min={0} onCommit={(next) => change({ history_count: next })} />} />
          </ListGroup>
          <section className="space-y-2"><ListGroup label={t("replyHelper.prompts")}>{styles.map(promptRow)}</ListGroup><Text variant="hint">{t("replyHelper.promptsHint")}</Text>{styles.map((style) => prompts[style].isError && <FailureState key={style} error={prompts[style].error} onRetry={() => void prompts[style].refetch()} />)}</section>
          <Text variant="hint">{t("replyHelper.globalHint")}</Text>
        </>}
      </>}
    </Page>
    <ReplyHelperPicker open={picker !== null && enabled} picker={shown} phone={phone} models={choices} modelLoading={modelLoading} modelFailure={modelFailure ? { error: modelFailure.error, retry: () => void modelFailure.refetch() } : null} defaultName={defaultName} prompts={shown && shown !== "model" ? prompts[shown] : roleplay} value={shown === "model" ? value?.model_profile_id ?? null : shown ? value?.[fields[shown]] ?? null : null} onClose={close} onChange={choose} onManage={() => void navigate({ to: "/settings/$section", params: { section: "prompts" }, search: {}, replace: true })} onAddModel={() => void navigate({ to: "/settings/models/new", replace: true })} />
  </>;
}
