import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, Cpu } from "lucide-react";
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
import type { ModelView, ProviderAccountView } from "@/api/generated/bindings";
import { ModelMark } from "@/entities/provider";
import { FailureState } from "@/shared/ui/FailureState";

export type CompanionPickerId = "primary" | "fallback" | "prompt";
export type WriterModel = { model: ModelView; account: ProviderAccountView };
export function CompanionPicker({
  picker,
  open,
  onClose,
  models,
  loading,
  failure,
  retry,
  value,
  defaultName,
  onChoose,
  onAddModel,
}: {
  picker: Exclude<CompanionPickerId, "prompt"> | null;
  open: boolean;
  onClose: () => void;
  models: WriterModel[];
  loading: boolean;
  failure: unknown;
  retry: () => void;
  value: string | null;
  defaultName: string;
  onChoose: (id: string | null) => void;
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
  const foundModels = models.filter(
    ({ model, account }) =>
      !q ||
      `${model.display_name} ${model.external_model_id} ${account.label}`
        .toLocaleLowerCase()
        .includes(q),
  );
  const groups = [...new Map(foundModels.map(({ account }) => [account.id, account])).values()];
  const mark = <Check size={icon.md} className="text-accent" />;
  return (
    <Sheet
      open={open}
      onClose={onClose}
      title={t(picker === "fallback" ? "backupModel" : "writerModel")}
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
          {models.length > 8 && (
            <SearchField
              value={query}
              onChange={setQuery}
              placeholder={t("searchModels")}
              aria-label={t("searchModels")}
            />
          )}
          <ListGroup label={t("defaultGroup")}>
            <ListRowButton
              icon={<Cpu size={icon.lg} />}
              title={t(picker === "fallback" ? "none" : "appDefault")}
              description={picker === "fallback" ? t("noBackupBody") : defaultName}
              chevron={false}
              aria-pressed={value === null}
              trailing={value === null ? mark : undefined}
              onClick={() => onChoose(null)}
            />
          </ListGroup>
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
        </div>
      )}
    </Sheet>
  );
}
