import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  Alert,
  Button,
  EmptyState,
  ListGroup,
  ListRow,
  PartitionBar,
  Sheet,
  Skeleton,
  StatTile,
} from "@lettuceai/crisp";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { detailQuery, rowTokens } from "./data";
import { money, number, rowTitle, timestamp } from "./format";
export function RequestDetail({
  id,
  phone,
  onClose,
}: {
  id: string | null;
  phone: boolean;
  onClose: () => void;
}) {
  const { t, i18n } = useTranslation("usage");
  const api = useSettingsApi();
  const query = useQuery(detailQuery(api, id ?? ""));
  const row = query.data;
  const count = (value: number | null) =>
    value === null ? t("notReported") : number(value, i18n.language);
  const cost = (value: number | null) =>
    value === null ? t("notReported") : money(value, i18n.language);
  const input = row?.prompt_tokens;
  const cached = row?.cached_prompt_tokens;
  const written = row?.cache_write_tokens;
  const output = row?.completion_tokens;
  const reasoning = row?.reasoning_tokens;
  const partitions =
    row &&
    input !== null &&
    input !== undefined &&
    output !== null &&
    output !== undefined &&
    (cached ?? 0) + (written ?? 0) <= input &&
    (reasoning ?? 0) <= output
      ? [
          {
            id: "input",
            label: t(cached !== null || written !== null ? "uncachedInput" : "input"),
            value: input - (cached ?? 0) - (written ?? 0),
            tone: "info" as const,
          },
          ...(cached
            ? [{ id: "cached", label: t("cachedInput"), value: cached, tone: "secondary" as const }]
            : []),
          ...(written
            ? [{ id: "written", label: t("writtenInput"), value: written, tone: "info" as const }]
            : []),
          ...(reasoning
            ? [
                {
                  id: "reasoning",
                  label: t("reasoning"),
                  value: reasoning,
                  tone: "warning" as const,
                },
              ]
            : []),
          {
            id: "reply",
            label: t(reasoning ? "reply" : "output"),
            value: output - (reasoning ?? 0),
            tone: "accent" as const,
          },
        ]
      : null;
  const priceParts = row
    ? (
        [
          {
            key: "input",
            value: row.prompt_cost,
            description: [
              row.cache_read_cost !== null
                ? t("cacheReadCost", { value: cost(row.cache_read_cost) })
                : null,
              row.cache_write_cost !== null
                ? t("cacheWriteCost", { value: cost(row.cache_write_cost) })
                : null,
            ]
              .filter(Boolean)
              .join(" · "),
          },
          { key: "output", value: row.completion_cost },
          { key: "reasoning", value: row.reasoning_cost },
          { key: "requestFee", value: row.request_cost },
          { key: "webSearch", value: row.web_search_cost },
          { key: "providerReported", value: row.api_cost },
        ] as const
      ).filter((item) => item.value !== null)
    : [];
  return (
    <Sheet
      open={id !== null}
      onClose={onClose}
      placement={phone ? "bottom" : "side"}
      title={row ? rowTitle(row, t) : t("requestDetail")}
      description={
        row
          ? [
              row.model_name ?? t("unknownModel"),
              row.provider_label ?? t("unknownProvider"),
              timestamp(row.timestamp, i18n.language),
              row.finish_reason ? t("endedWith", { reason: row.finish_reason }) : null,
            ]
              .filter(Boolean)
              .join(" · ")
          : undefined
      }
    >
      {query.isError ? (
        <FailureState error={query.error} onRetry={() => void query.refetch()} />
      ) : query.isPending ? (
        <div role="status" aria-label={t("loadingDetail")} className="space-y-4">
          <Skeleton className="h-24 w-full" />
          <Skeleton className="h-44 w-full" />
        </div>
      ) : !row ? (
        <EmptyState
          title={t("missingRequest")}
          action={<Button onClick={onClose}>{t("close")}</Button>}
        />
      ) : (
        <div className="space-y-6 pb-2">
          {row.status === "failed" && (
            <Alert
              tone="danger"
              title={t("requestFailed")}
              description={row.error_message ?? t("statuses.failed")}
            />
          )}
          <div className="grid grid-cols-2 gap-2.5">
            <StatTile
              label={t("tokens")}
              value={count(rowTokens(row))}
              sub={t(row.total_tokens !== null ? "providerCounts" : "inputPlusOutput")}
            />
            <StatTile
              label={t("cost")}
              value={row.total_cost === null ? t("noPrice") : cost(row.total_cost)}
              sub={t(
                row.api_cost !== null
                  ? "reportedByProvider"
                  : row.prompt_cost !== null
                    ? "savedPrice"
                    : row.provider_kind === "llamacpp" || row.provider_kind === "llama_cpp"
                      ? "ranLocally"
                      : "priceNotReported",
              )}
              highlight={row.total_cost !== null}
            />
          </div>
          {partitions && (
            <PartitionBar
              aria-label={t("tokenPartition")}
              segments={partitions}
              labels="legend"
              format={(value) => number(value, i18n.language)}
            />
          )}
          <ListGroup label={t("tokens")}>
            <ListRow
              title={t("input")}
              description={
                row.prompt_tokens === null
                  ? t("notReported")
                  : [
                      row.cached_prompt_tokens === null
                        ? t("cacheNotReported")
                        : t("cacheReadTokens", { value: count(row.cached_prompt_tokens) }),
                      row.cache_write_tokens !== null
                        ? t("cacheWriteTokens", { value: count(row.cache_write_tokens) })
                        : null,
                    ]
                      .filter(Boolean)
                      .join(" · ")
              }
              trailing={<span className="tabular-nums text-fg">{count(row.prompt_tokens)}</span>}
            />
            <ListRow
              title={t("output")}
              description={
                row.reasoning_tokens === null
                  ? t("reasoningNotReported")
                  : t("reasoningDetail", {
                      reasoning: count(row.reasoning_tokens),
                      reply:
                        row.completion_tokens !== null &&
                        row.reasoning_tokens <= row.completion_tokens
                          ? count(row.completion_tokens - row.reasoning_tokens)
                          : t("notReported"),
                    })
              }
              trailing={
                <span className="tabular-nums text-fg">{count(row.completion_tokens)}</span>
              }
            />
            {(row.memory_tokens !== null || row.summary_tokens !== null) && (
              <ListRow
                title={t("memoryPrompt")}
                description={[
                  row.memory_tokens !== null
                    ? t("memoryTokens", { value: count(row.memory_tokens) })
                    : null,
                  row.summary_tokens !== null
                    ? t("summaryTokens", { value: count(row.summary_tokens) })
                    : null,
                ]
                  .filter(Boolean)
                  .join(" · ")}
                trailing={<span className="text-sm text-fg-3">{t("partOfInput")}</span>}
              />
            )}{" "}
            {(
              [
                { key: "webSearch", value: row.web_search_requests },
                { key: "imageTokens", value: row.image_tokens },
                { key: "audioTokens", value: row.audio_tokens },
                { key: "inputImages", value: row.input_image_count },
                { key: "outputImages", value: row.output_image_count },
              ] as const
            )
              .filter((item) => item.value !== null)
              .map((item) => (
                <ListRow
                  key={item.key}
                  title={t(item.key)}
                  trailing={<span className="tabular-nums text-fg">{count(item.value)}</span>}
                />
              ))}
          </ListGroup>
          {priceParts.length > 0 && (
            <ListGroup label={t("cost")}>
              {priceParts.map((part) => (
                <ListRow
                  key={part.key}
                  title={t(part.key)}
                  description={"description" in part ? part.description : undefined}
                  trailing={<span className="tabular-nums text-fg">{cost(part.value)}</span>}
                />
              ))}
            </ListGroup>
          )}
        </div>
      )}
    </Sheet>
  );
}
