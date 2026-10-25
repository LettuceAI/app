import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, Cpu, FileText } from "lucide-react";
import {
  Button,
  EmptyState,
  icon,
  ListGroup,
  ListRowButton,
  SearchField,
  Sheet,
  Skeleton,
} from "@lettuceai/crisp";
import type { ModelView, PromptSummary, ProviderAccountView } from "@/api/generated/bindings";
import { ModelMark } from "@/entities/provider";
import { FailureState } from "@/shared/ui/FailureState";

export type CompanionPickerId = "primary" | "fallback" | "prompt";
export type WriterModel = { model: ModelView; account: ProviderAccountView };
export function CompanionPicker({
  picker,
  open,
  onClose,
  models,
  prompts,
  loading,
  failure,
  retry,
  value,
  defaultName,
  onChoose,
  onManage,
  onAddModel,
}: {
  picker: CompanionPickerId | null;
  open: boolean;
  onClose: () => void;
  models: WriterModel[];
  prompts: PromptSummary[];
  loading: boolean;
  failure: unknown;
  retry: () => void;
  value: string | null;
  defaultName: string;
  onChoose: (id: string | null) => void;
  onManage: () => void;
  onAddModel: () => void;
}) {
  const { t } = useTranslation("companions");
  const [query, setQuery] = useState("");
  const [previous, setPrevious] = useState({ open, picker });
  if (previous.open !== open || previous.picker !== picker) {
    setPrevious({ open, picker });
    if (open) setQuery("");
  }
  const q = query.trim().toLocaleLowerCase();
  const custom = prompts.filter((prompt) => prompt.origin.type !== "built_in");
  const foundPrompts = custom.filter((prompt) => !q || prompt.name.toLocaleLowerCase().includes(q));
  const foundModels = models.filter(
    ({ model, account }) =>
      !q ||
      `${model.display_name} ${model.external_model_id} ${account.label}`
        .toLocaleLowerCase()
        .includes(q),
  );
  const groups = [...new Map(foundModels.map(({ account }) => [account.id, account])).values()];
  const builtin = prompts.find((prompt) => prompt.origin.type === "built_in");
  const selectedDefault = value === null || (picker === "prompt" && value === builtin?.id);
  const mark = <Check size={icon.md} className="text-accent" />;
  return (
    <Sheet
      open={open}
      onClose={onClose}
      title={t(
        picker === "primary"
          ? "writerModel"
          : picker === "fallback"
            ? "backupModel"
            : "writerPrompt",
      )}
    >
      {failure ? (
        <FailureState error={failure} onRetry={retry} />
      ) : loading ? (
        <div role="status" aria-label={t("loadingChoices")} className="space-y-3">
          {[0, 1, 2].map((index) => (
            <Skeleton key={index} shape="block" className="h-16 w-full" />
          ))}
        </div>
      ) : (
        <div className="space-y-5">
          {(picker === "prompt" ? custom.length : models.length) > 8 && (
            <SearchField
              value={query}
              onChange={setQuery}
              placeholder={t(picker === "prompt" ? "searchPrompts" : "searchModels")}
              aria-label={t(picker === "prompt" ? "searchPrompts" : "searchModels")}
            />
          )}
          <ListGroup label={t(picker === "prompt" ? "app" : "defaultGroup")}>
            <ListRowButton
              icon={picker === "prompt" ? <FileText size={icon.lg} /> : <Cpu size={icon.lg} />}
              title={t(
                picker === "primary" ? "appDefault" : picker === "fallback" ? "none" : "builtIn",
              )}
              description={
                picker === "primary"
                  ? defaultName
                  : picker === "fallback"
                    ? t("noBackupBody")
                    : (builtin?.name ?? t("builtinMissing"))
              }
              chevron={false}
              aria-pressed={selectedDefault}
              trailing={selectedDefault ? mark : undefined}
              onClick={() => onChoose(null)}
            />
          </ListGroup>
          {picker === "prompt" ? (
            <>
              {foundPrompts.length > 0 && (
                <ListGroup label={t("yours")}>
                  {foundPrompts.map((prompt) => (
                    <ListRowButton
                      key={prompt.id}
                      icon={<FileText size={icon.lg} />}
                      title={prompt.name}
                      chevron={false}
                      aria-pressed={value === prompt.id}
                      trailing={value === prompt.id ? mark : undefined}
                      onClick={() => onChoose(prompt.id)}
                    />
                  ))}
                </ListGroup>
              )}
              {q && foundPrompts.length === 0 && <EmptyState title={t("noMatches")} />}
              <ListGroup>
                <ListRowButton
                  icon={<FileText size={icon.lg} />}
                  title={t("managePrompts")}
                  description={t("managePromptsBody")}
                  onClick={onManage}
                />
              </ListGroup>
            </>
          ) : (
            <>
              {groups.map((account) => (
                <ListGroup
                  key={account.id}
                  label={account.provider_kind === "llamacpp" ? t("onDevice") : account.label}
                >
                  {foundModels
                    .filter((choice) => choice.account.id === account.id)
                    .map(({ model }) => (
                      <ListRowButton
                        key={model.id}
                        icon={
                          <ModelMark
                            modelId={model.external_model_id}
                            providerKind={account.provider_kind}
                            name={model.display_name}
                          />
                        }
                        title={model.display_name}
                        description={
                          account.enabled
                            ? account.label
                            : t("providerDisabled", { provider: account.label })
                        }
                        disabled={!account.enabled}
                        chevron={false}
                        aria-pressed={value === model.id}
                        trailing={value === model.id ? mark : undefined}
                        onClick={() => onChoose(model.id)}
                      />
                    ))}
                </ListGroup>
              ))}
              {models.length === 0 ? (
                <EmptyState
                  title={t("noModels")}
                  description={t("noModelsBody")}
                  action={
                    <Button variant="secondary" onClick={onAddModel}>
                      {t("addModel")}
                    </Button>
                  }
                />
              ) : foundModels.length === 0 ? (
                <EmptyState title={t("noMatches")} />
              ) : null}
            </>
          )}
        </div>
      )}
    </Sheet>
  );
}
