import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Check, Cpu, FileText, Globe, LifeBuoy } from "lucide-react";
import { Alert, icon, ListGroup, ListRowButton, Skeleton, Text } from "@lettuceai/crisp";
import type { SettingsCompanionSoulWriterSettings } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { modelsQuery } from "@/features/conversations";
import { promptsQuery } from "@/features/characters";
import { FailureState } from "@/shared/ui/FailureState";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { useSettingsApi } from "../data";
import { CompanionPicker, type CompanionPickerId, type WriterModel } from "./CompanionPicker";

const fields = {
  primary: "model_profile_id",
  fallback: "fallback_model_profile_id",
  prompt: "prompt_id",
} as const;
export function WriterTab({
  value,
  defaultModelId,
  picker,
  onOpen,
  onClose,
  onChange,
}: {
  value: SettingsCompanionSoulWriterSettings;
  defaultModelId: string | null;
  picker: CompanionPickerId | null;
  onOpen: (picker: CompanionPickerId) => void;
  onClose: () => void;
  onChange: (patch: Partial<SettingsCompanionSoulWriterSettings>) => Promise<boolean>;
}) {
  const { t } = useTranslation("companions");
  const api = useSettingsApi();
  const navigate = useNavigate();
  const models = useQuery(modelsQuery(api));
  const accounts = useQuery({
    queryKey: queryKeys.providers.accounts(),
    queryFn: () => api.call("providerAccountsList"),
  });
  const prompts = useQuery(promptsQuery(api, "companion_soul_writer"));
  const shown = useClosingValue(picker).shown;
  const byAccount = new Map((accounts.data ?? []).map((account) => [account.id, account]));
  const choices: WriterModel[] = (models.data?.models ?? []).flatMap((model) => {
    const account = byAccount.get(model.provider_account_id);
    return account &&
      model.kind === "chat" &&
      model.input_scopes.includes("text") &&
      model.output_scopes.includes("text")
      ? [{ model, account }]
      : [];
  });
  const defaultChoice = choices.find(({ model }) => model.id === defaultModelId);
  const defaultName =
    defaultModelId === null
      ? t("noDefault")
      : defaultChoice?.account.enabled
        ? defaultChoice.model.display_name
        : t("defaultUnavailable");
  const primary = choices.find(({ model }) => model.id === value.model_profile_id);
  const fallback = choices.find(({ model }) => model.id === value.fallback_model_profile_id);
  const modelLine = (choice: WriterModel | undefined) =>
    !choice
      ? t("modelUnavailable")
      : !choice.account.enabled
        ? t("providerDisabled", { provider: choice.account.label })
        : t("modelIdentity", { provider: choice.account.label, name: choice.model.display_name });
  const prompt = prompts.data?.find((prompt) => prompt.id === value.prompt_id);
  const modelFailure = [models, accounts].find((query) => query.isError);
  const loading = models.isPending || accounts.isPending;
  const choose = (id: string | null) => {
    if (picker) {
      void onChange({ [fields[picker]]: id });
      onClose();
    }
  };
  return (
    <>
      <div className="space-y-7">
        <Text variant="secondary" className="px-1">
          {t("writerIntro")}
        </Text>
        <section className="space-y-2">
          <ListGroup label={t("models")}>
            <ListRowButton
              icon={
                (primary ?? (value.model_profile_id === null ? defaultChoice : undefined))?.account
                  .provider_kind === "llamacpp" ? (
                  <Cpu size={icon.lg} />
                ) : (
                  <Globe size={icon.lg} />
                )
              }
              title={t("writerModel")}
              description={
                loading && !modelFailure ? (
                  <Skeleton className="h-4 w-40" />
                ) : value.model_profile_id === null ? (
                  t("appDefaultModel", { name: defaultName })
                ) : (
                  modelLine(primary)
                )
              }
              onClick={() => onOpen("primary")}
            />
            <ListRowButton
              icon={<LifeBuoy size={icon.lg} />}
              title={t("backupModel")}
              description={
                loading && !modelFailure ? (
                  <Skeleton className="h-4 w-40" />
                ) : value.fallback_model_profile_id === null ? (
                  t("none")
                ) : (
                  modelLine(fallback)
                )
              }
              onClick={() => onOpen("fallback")}
            />
          </ListGroup>
          <Text variant="hint" className="px-1">
            {t("backupHint")}
          </Text>
          {modelFailure && (
            <FailureState
              error={modelFailure.error}
              onRetry={() => void modelFailure.refetch()}
              size="sm"
            />
          )}
        </section>
        <section className="space-y-2">
          <ListGroup label={t("formatGroup")}>
            {(["json", "xml"] as const).map((format) => (
              <ListRowButton
                key={format}
                title={t(`formats.${format}.title`)}
                description={t(`formats.${format}.body`)}
                tone={format === value.structured_fallback_format ? "accent" : "neutral"}
                chevron={false}
                aria-pressed={format === value.structured_fallback_format}
                trailing={
                  format === value.structured_fallback_format ? (
                    <Check size={icon.md} className="text-accent" />
                  ) : undefined
                }
                onClick={() => void onChange({ structured_fallback_format: format })}
              />
            ))}
          </ListGroup>
          <Text variant="hint" className="px-1">
            {t("formatHint")}
          </Text>
        </section>
        <section className="space-y-2">
          <ListGroup label={t("promptGroup")}>
            <ListRowButton
              icon={<FileText size={icon.lg} />}
              title={t("writerPrompt")}
              description={
                prompts.isPending ? (
                  <Skeleton className="h-4 w-32" />
                ) : value.prompt_id === null || prompt?.origin.type === "built_in" ? (
                  t("builtIn")
                ) : (
                  (prompt?.name ?? t("promptUnavailable"))
                )
              }
              onClick={() => onOpen("prompt")}
            />
          </ListGroup>
          <Text variant="hint" className="px-1">
            {t("promptHint")}
          </Text>
          {prompts.isError && (
            <FailureState error={prompts.error} onRetry={() => void prompts.refetch()} size="sm" />
          )}
        </section>
        <Alert tone="info" title={t("whereTitle")} description={t("whereBody")} />
      </div>
      <CompanionPicker
        open={picker !== null}
        picker={shown}
        models={choices}
        prompts={prompts.data ?? []}
        loading={shown === "prompt" ? prompts.isPending : loading}
        failure={shown === "prompt" ? prompts.error : (modelFailure?.error ?? null)}
        retry={() => {
          if (shown === "prompt") void prompts.refetch();
          else {
            void models.refetch();
            void accounts.refetch();
          }
        }}
        value={shown ? value[fields[shown]] : null}
        defaultName={defaultName}
        onClose={onClose}
        onChoose={choose}
        onManage={() =>
          void navigate({
            to: "/settings/$section",
            params: { section: "prompts" },
            search: {},
            replace: true,
          })
        }
        onAddModel={() => void navigate({ to: "/settings/models/new", replace: true })}
      />
    </>
  );
}
