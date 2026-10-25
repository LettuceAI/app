import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Button, EmptyState, Field, ListGroup, SelectMenu, Skeleton, Text } from "@lettuceai/crisp";
import type { UsageRow } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { requestFilters, requestsQuery, statsQuery } from "./data";
import { operationLabel, operations } from "./format";
import { UsageRows } from "./UsageRows";
export function Requests({
  status,
  operation,
  model,
  phone,
  now,
  onFilter,
  onOpen,
}: {
  status: string;
  operation: string;
  model: string;
  phone: boolean;
  now: number;
  onFilter: (filters: {
    usageStatus?: string;
    usageOperation?: string;
    usageModel?: string;
  }) => void;
  onOpen: (row: UsageRow) => void;
}) {
  const { t } = useTranslation("usage");
  const api = useSettingsApi();
  const models = useQuery(statsQuery(api, "all", "all", "model", now));
  const features = useQuery(statsQuery(api, "all", "all", "operation", now));
  const query = useInfiniteQuery(requestsQuery(api, requestFilters(status, operation, model)));
  const rows = query.data?.pages.flatMap((page) => page.items) ?? [];
  const modelChoices = (models.data?.groups ?? []).flatMap((group) =>
    group.key ? [{ value: group.key, label: group.label ?? t("unknownModel") }] : [],
  );
  const operationChoices = [
    ...new Set([
      ...operations,
      ...(features.data?.groups ?? []).flatMap((group) => (group.key ? [group.key] : [])),
    ]),
  ];
  const selectors = [
    {
      label: t("result"),
      value: status,
      key: "usageStatus" as const,
      options: [
        { value: "all", label: t("anyResult") },
        ...(["succeeded", "failed", "cancelled", "interrupted", "pending"] as const).map(
          (value) => ({ value, label: t(`statuses.${value}`) }),
        ),
      ],
    },
    {
      label: t("feature"),
      value: operation,
      key: "usageOperation" as const,
      options: [
        { value: "all", label: t("allFeatures") },
        ...operationChoices.map((value) => ({ value, label: operationLabel(value, t) })),
      ],
    },
    {
      label: t("model"),
      value: model,
      key: "usageModel" as const,
      options: [
        { value: "all", label: t("allModels") },
        ...modelChoices,
        ...(model !== "all" && !modelChoices.some((item) => item.value === model)
          ? [{ value: model, label: t("unknownModel") }]
          : []),
      ],
    },
  ];
  return (
    <div className="space-y-5">
      <div className={phone ? "grid gap-2" : "grid grid-cols-3 gap-2"}>
        {selectors.map((item) => (
          <Field
            key={item.key}
            label={<span className="sr-only">{item.label}</span>}
            className="space-y-0"
          >
            <SelectMenu
              className="w-full"
              value={item.value}
              onChange={(value) => onFilter({ [item.key]: value ?? "all" })}
              options={item.options}
            />
          </Field>
        ))}
      </div>
      {[models, features].map(
        (item, index) =>
          item.isError && (
            <FailureState
              key={index}
              error={item.error}
              onRetry={() => void item.refetch()}
              size="sm"
            />
          ),
      )}
      {[status, operation, model].some((value) => value !== "all") && (
        <div className="flex items-center justify-between gap-3 px-1">
          <Text variant="hint">{t("loadedMatches", { count: rows.length })}</Text>
          <Button
            size="sm"
            variant="ghost"
            onClick={() =>
              onFilter({ usageStatus: "all", usageOperation: "all", usageModel: "all" })
            }
          >
            {t("clearFilters")}
          </Button>
        </div>
      )}
      {query.isPending ? (
        <div role="status" aria-label={t("loadingRequests")} className="space-y-2">
          {[0, 1, 2, 3].map((value) => (
            <Skeleton key={value} shape="block" className="h-16 w-full" />
          ))}
        </div>
      ) : query.isError && !query.data ? (
        <FailureState error={query.error} onRetry={() => void query.refetch()} />
      ) : rows.length === 0 ? (
        <EmptyState title={t("noMatches")} description={t("noMatchesBody")} />
      ) : (
        <>
          {query.isError && !query.isFetchNextPageError && (
            <FailureState error={query.error} onRetry={() => void query.refetch()} size="sm" />
          )}
          <ListGroup>
            <UsageRows rows={rows} onOpen={onOpen} />
          </ListGroup>
          {query.isFetchNextPageError && (
            <FailureState
              error={query.error}
              onRetry={() => void query.fetchNextPage()}
              size="sm"
            />
          )}
          {query.hasNextPage && (
            <div className="flex justify-center">
              <Button
                variant="secondary"
                loading={query.isFetchingNextPage}
                onClick={() => void query.fetchNextPage()}
              >
                {t("showMore")}
              </Button>
            </div>
          )}
        </>
      )}
    </div>
  );
}
