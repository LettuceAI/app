import { useState } from "react";
import { Check, Cpu } from "lucide-react";
import { useTranslation } from "react-i18next";
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

export type ReplyPicker = "model" | "roleplay" | "conversational";
export type ReplyHelperModel = { model: ModelView; account: ProviderAccountView };

export function ReplyHelperPicker({
  open,
  phone,
  models,
  modelLoading,
  modelFailure,
  defaultName,
  value,
  onClose,
  onChange,
  onAddModel,
}: {
  open: boolean;
  phone: boolean;
  models: ReplyHelperModel[];
  modelLoading: boolean;
  modelFailure: { error: unknown; retry: () => void } | null;
  defaultName: string;
  value: string | null;
  onClose: () => void;
  onChange: (id: string | null) => void;
  onAddModel: () => void;
}) {
  const { t } = useTranslation("settings");
  const [query, setQuery] = useState("");
  const [previousOpen, setPreviousOpen] = useState(open);
  if (previousOpen !== open) {
    setPreviousOpen(open);
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
      title={t("replyHelper.model")}
      {...(phone ? { detents: [0.6, 0.92] } : {})}
    >
      {modelFailure ? (
        <FailureState error={modelFailure.error} onRetry={modelFailure.retry} />
      ) : modelLoading ? (
        <div className="space-y-3" role="status" aria-label={t("replyHelper.loading")}>
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
              placeholder={t("replyHelper.modelSearch")}
              aria-label={t("replyHelper.modelSearch")}
            />
          )}
          <ListGroup label={t("replyHelper.defaultGroup")}>
            <ListRowButton
              icon={<Cpu size={icon.lg} />}
              title={t("replyHelper.appDefault")}
              description={defaultName}
              chevron={false}
              aria-pressed={value === null}
              trailing={value === null ? mark : undefined}
              onClick={() => onChange(null)}
            />
          </ListGroup>
          {groups.map((account) => (
            <ListGroup
              key={account.id}
              label={
                account.provider_kind === "llamacpp" ? t("replyHelper.onDevice") : account.label
              }
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
                        : t("replyHelper.providerDisabled", { provider: account.label })
                    }
                    chevron={false}
                    aria-pressed={value === model.id}
                    trailing={value === model.id ? mark : undefined}
                    disabled={!account.enabled}
                    onClick={() => onChange(model.id)}
                  />
                ))}
            </ListGroup>
          ))}
          {models.length === 0 ? (
            <EmptyState
              title={t("replyHelper.noModels")}
              description={t("replyHelper.noModelsBody")}
              action={
                <Button variant="secondary" onClick={onAddModel}>
                  {t("replyHelper.addModel")}
                </Button>
              }
            />
          ) : foundModels.length === 0 ? (
            <EmptyState title={t("replyHelper.noMatches")} />
          ) : null}
        </div>
      )}
    </Sheet>
  );
}
