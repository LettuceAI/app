import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Check, Cpu } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Collapsible,
  EmptyState,
  icon,
  ListGroup,
  ListRowButton,
  SearchField,
  Sheet,
  Skeleton,
  useCompactLayout,
} from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import { queryKeys } from "@/api/query-keys";
import { ModelMark } from "@/entities/provider";
import { FailureState } from "@/shared/ui/FailureState";
import {
  groupByProvider,
  meetsRequirement,
  modelChoices,
  searchModels,
  type ModelRequirement,
} from "./model-choice";

const searchFrom = 8;
const onDeviceKind = "llamacpp";

export interface ModelChoiceSheetProps {
  api: ApiClient;
  open: boolean;
  onClose: () => void;
  title: string;
  value: string | null;
  /** Return false to keep the sheet open (a failed save or a declined confirmation). */
  onChange: (id: string | null) => boolean | void | Promise<boolean | void>;
  requirement?: ModelRequirement;
  /** The first row, choosing null. Omit for no such row. `selected` overrides `value === null` for the check mark. */
  nullChoice?: { title: string; description?: string; selected?: boolean };
  busy?: boolean;
  /** Offers "Add a model" in the empty state. */
  onAddModel?: () => void;
}

/** The one picker for a configured model: grouped by provider, filtered by what the caller needs. */
export function ModelChoiceSheet({
  api,
  open,
  onClose,
  title,
  value,
  onChange,
  requirement,
  nullChoice,
  busy = false,
  onAddModel,
}: ModelChoiceSheetProps) {
  const { t, i18n } = useTranslation("common");
  const compact = useCompactLayout();
  const models = useQuery({
    queryKey: queryKeys.models.list(),
    queryFn: () => api.call("modelsList"),
    enabled: open,
  });
  const accounts = useQuery({
    queryKey: queryKeys.providers.accounts(),
    queryFn: () => api.call("providerAccountsList"),
    enabled: open,
  });
  const [query, setQuery] = useState("");
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const [wasOpen, setWasOpen] = useState(open);
  if (open !== wasOpen) {
    setWasOpen(open);
    if (open) {
      setQuery("");
      setCollapsed(new Set());
    }
  }
  const failed = [models, accounts].find((entry) => entry.isError);
  const choices = modelChoices(models.data?.models ?? [], accounts.data ?? []).filter((choice) =>
    meetsRequirement(choice, requirement),
  );
  const found = searchModels(choices, query);
  const groups = groupByProvider(found);
  const count = new Intl.NumberFormat(i18n.language);
  const choose = async (id: string | null) => {
    if (busy) return;
    if ((await onChange(id)) !== false) onClose();
  };
  const search = (next: string) => {
    setQuery(next);
    setCollapsed(new Set());
  };
  const toggle = (id: string, expanded: boolean) =>
    setCollapsed((current) => {
      const next = new Set(current);
      if (expanded) next.delete(id);
      else next.add(id);
      return next;
    });
  const mark = <Check size={icon.md} className="text-accent" />;
  const nullSelected = nullChoice?.selected ?? value === null;
  return (
    <Sheet
      open={open}
      onClose={onClose}
      size="sm"
      title={title}
      dismissible={!busy}
      {...(compact ? { detents: [0.6, 0.92] } : {})}
    >
      {failed ? (
        <FailureState error={failed.error} onRetry={() => void failed.refetch()} />
      ) : models.isPending || accounts.isPending ? (
        <div role="status" aria-label={t("model.choice.loading")} className="space-y-3">
          {[0, 1, 2].map((index) => (
            <Skeleton key={index} shape="block" className="h-16 w-full" />
          ))}
        </div>
      ) : (
        <div className="space-y-5 pb-1">
          {choices.length > searchFrom && (
            <SearchField
              value={query}
              onChange={search}
              placeholder={t("model.choice.search")}
              aria-label={t("model.choice.search")}
            />
          )}
          {nullChoice && (
            <ListGroup>
              <ListRowButton
                icon={<Cpu size={icon.lg} />}
                title={nullChoice.title}
                {...(nullChoice.description === undefined ? {} : { description: nullChoice.description })}
                chevron={false}
                aria-pressed={nullSelected}
                trailing={nullSelected ? mark : undefined}
                disabled={busy}
                onClick={() => void choose(null)}
              />
            </ListGroup>
          )}
          {groups.map(({ account, choices: rows }) => (
            <Collapsible
              key={account.id}
              variant="plain"
              contentClassName="pl-0"
              open={!collapsed.has(account.id)}
              onOpenChange={(expanded) => toggle(account.id, expanded)}
              title={account.provider_kind === onDeviceKind ? t("model.choice.onDevice") : account.label}
              trailing={<span className="text-sm text-fg-3">{count.format(rows.length)}</span>}
            >
              <ListGroup>
                {rows.map(({ model }) => (
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
                    {...(account.enabled ? {} : { description: t("model.choice.providerOff") })}
                    chevron={false}
                    aria-pressed={value === model.id}
                    trailing={value === model.id ? mark : undefined}
                    disabled={busy || !account.enabled}
                    onClick={() => void choose(model.id)}
                  />
                ))}
              </ListGroup>
            </Collapsible>
          ))}
          {choices.length === 0 ? (
            <EmptyState
              size="sm"
              title={t("model.choice.empty")}
              {...(onAddModel
                ? {
                    action: (
                      <Button variant="secondary" onClick={onAddModel}>
                        {t("model.choice.add")}
                      </Button>
                    ),
                  }
                : {})}
            />
          ) : found.length === 0 ? (
            <EmptyState size="sm" title={t("model.choice.noMatches")} />
          ) : null}
        </div>
      )}
    </Sheet>
  );
}
