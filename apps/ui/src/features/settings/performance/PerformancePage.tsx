import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Activity, Gauge, Hash, RefreshCw, Timer, Trash2 } from "lucide-react";
import {
  Button,
  Chart,
  confirm,
  EmptyState,
  icon,
  KeyValue,
  ListGroup,
  ListRowButton,
  LoadingState,
  Page,
  Pagination,
  Panel,
  Sheet,
  StatTile,
  toast,
} from "@lettuceai/crisp";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi, useSettingsLayout } from "../data";
import { GenerationChart, SpeedVsContextChart } from "./charts";
import { metricDetailQuery, metricsListQuery, samplesOf, summaryOf, type MetricSummary } from "./data";

const LIST_PAGE_SIZE = 10;

const detailKeys = {
  decodeSpeed: "detail.decodeSpeed",
  nativePromptSpeed: "detail.nativePromptSpeed",
  nativeGenerationSpeed: "detail.nativeGenerationSpeed",
  firstToken: "detail.firstToken",
  elapsed: "detail.elapsed",
  nativeGenerationTime: "detail.nativeGenerationTime",
  appOverhead: "detail.appOverhead",
  promptTokens: "detail.promptTokens",
  outputTokens: "detail.outputTokens",
  totalTokens: "detail.totalTokens",
  context: "detail.context",
  batch: "detail.batch",
  microbatch: "detail.microbatch",
  backend: "detail.backend",
  gpuLayers: "detail.gpuLayers",
  kvType: "detail.kvType",
  modelSize: "detail.modelSize",
  finishReason: "detail.finishReason",
} as const;

const modelLabel = (path: string | null) => (path ? (path.split(/[\\/]/).pop() ?? path) : null);

function useFormatters() {
  const { i18n } = useTranslation();
  return useMemo(() => {
    const decimal = new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 1 });
    const whole = new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 0 });
    const relative = new Intl.RelativeTimeFormat(i18n.language, { numeric: "auto" });
    const size = (bytes: number) => {
      const units = ["byte", "kilobyte", "megabyte", "gigabyte", "terabyte"] as const;
      let value = bytes;
      let unit = 0;
      while (value >= 1024 && unit < units.length - 1) {
        value /= 1024;
        unit += 1;
      }
      return new Intl.NumberFormat(i18n.language, { style: "unit", unit: units[unit], unitDisplay: "short", maximumFractionDigits: 1 }).format(value);
    };
    const ago = (at: number) => {
      const minutes = Math.round((at - Date.now()) / 60000);
      if (Math.abs(minutes) < 60) return relative.format(minutes, "minute");
      const hours = Math.round(minutes / 60);
      if (Math.abs(hours) < 24) return relative.format(hours, "hour");
      return relative.format(Math.round(hours / 24), "day");
    };
    const seconds = (ms: number) => new Intl.NumberFormat(i18n.language, { style: "unit", unit: "second", unitDisplay: "short", maximumFractionDigits: 1 }).format(ms / 1000);
    return { decimal: (value: number) => decimal.format(value), whole: (value: number) => whole.format(value), size, ago, seconds };
  }, [i18n.language]);
}

function DetailSheet({ id, onClose }: { id: string | null; onClose: () => void }) {
  const { t } = useTranslation("performance");
  const api = useSettingsApi();
  const format = useFormatters();
  const [shown, setShown] = useState(id);
  if (id !== null && id !== shown) setShown(id);
  const detail = useQuery(metricDetailQuery(api, shown));
  const metric = detail.data ? summaryOf(detail.data) : null;
  const rows: { label: string; value: string }[] = [];
  const push = (key: keyof typeof detailKeys, value: string | number | null) => {
    if (value !== null && value !== "") rows.push({ label: t(detailKeys[key]), value: String(value) });
  };
  if (metric) {
    const speed = (value: number | null) => (value === null ? null : t("units.speed", { value: format.decimal(value) }));
    const time = (value: number | null) => (value === null ? null : format.seconds(value));
    push("decodeSpeed", speed(metric.decodeTokensPerSecond));
    push("nativePromptSpeed", speed(metric.nativePromptEvalTokensPerSecond));
    push("nativeGenerationSpeed", speed(metric.nativeGenerationTokensPerSecond));
    push("firstToken", metric.ttftMs === null ? null : t("units.ms", { value: format.whole(metric.ttftMs) }));
    push("elapsed", time(metric.generationElapsedMs));
    push("nativeGenerationTime", time(metric.nativeGenerationComputeMs));
    push("appOverhead", time(metric.appGenerationOverheadMs));
    push("promptTokens", metric.promptTokens === null ? null : format.whole(metric.promptTokens));
    push("outputTokens", metric.completionTokens === null ? null : format.whole(metric.completionTokens));
    push("totalTokens", metric.totalTokens === null ? null : format.whole(metric.totalTokens));
    push("context", metric.nCtx === null ? null : format.whole(metric.nCtx));
    push("batch", metric.nBatch === null ? null : format.whole(metric.nBatch));
    push("microbatch", metric.nUbatch === null ? null : format.whole(metric.nUbatch));
    push("backend", metric.backend);
    push("gpuLayers", metric.gpuLayers);
    push("kvType", metric.kvType);
    push("modelSize", metric.modelSizeBytes === null ? null : format.size(metric.modelSizeBytes));
    push("finishReason", metric.finishReason);
  }
  return (
    <Sheet open={id !== null} onClose={onClose} size="lg" title={t("charts.detailTitle")} description={metric ? (modelLabel(metric.modelName) ?? t("list.unknownModel")) : undefined}>
      {detail.isPending ? (
        <LoadingState />
      ) : detail.isError ? (
        <FailureState size="sm" error={detail.error} onRetry={() => void detail.refetch()} />
      ) : !detail.data ? (
        <p className="py-8 text-center text-sm text-fg-3">{t("charts.noSamples")}</p>
      ) : (
        <div className="space-y-5 pb-1">
          {rows.length > 0 && <KeyValue items={rows} />}
          <GenerationChart samples={samplesOf(detail.data)} />
        </div>
      )}
    </Sheet>
  );
}

function Overview({ metrics, onOpen }: { metrics: readonly MetricSummary[]; onOpen: (id: string) => void }) {
  const { t } = useTranslation("performance");
  const api = useSettingsApi();
  const format = useFormatters();
  const [page, setPage] = useState(1);
  const latest = useQuery(metricDetailQuery(api, metrics[0]?.id ?? null));
  const latestSamples = samplesOf(latest.data);
  const stats = useMemo(() => {
    const speeds = metrics.flatMap((metric) => (metric.decodeTokensPerSecond !== null && metric.decodeTokensPerSecond > 0 ? [metric.decodeTokensPerSecond] : []));
    const ttfts = metrics.flatMap((metric) => (metric.ttftMs !== null && metric.ttftMs > 0 ? [metric.ttftMs] : []));
    const average = (values: number[]) => (values.length ? values.reduce((sum, value) => sum + value, 0) / values.length : 0);
    return { runs: metrics.length, avgSpeed: average(speeds), peakSpeed: speeds.length ? Math.max(...speeds) : 0, avgTtft: average(ttfts) };
  }, [metrics]);
  const speedByRun = useMemo(
    () =>
      [...metrics]
        .filter((metric) => metric.decodeTokensPerSecond !== null)
        .reverse()
        .map((metric, index) => ({ run: index + 1, tps: Math.round((metric.decodeTokensPerSecond ?? 0) * 10) / 10 })),
    [metrics],
  );
  const speedVsContext = useMemo(
    () =>
      metrics.flatMap((metric) => {
        const ctx = metric.nCtx ?? metric.totalTokens ?? 0;
        const tps = Math.round((metric.decodeTokensPerSecond ?? 0) * 10) / 10;
        return ctx > 0 && tps > 0 ? [{ ctx, tps }] : [];
      }),
    [metrics],
  );
  const pageCount = Math.max(1, Math.ceil(metrics.length / LIST_PAGE_SIZE));
  const current = Math.min(page, pageCount);
  const shown = metrics.slice((current - 1) * LIST_PAGE_SIZE, current * LIST_PAGE_SIZE);
  const speed = (value: number) => t("units.speed", { value: format.decimal(value) });
  return (
    <>
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <StatTile icon={<Hash size={icon.sm} />} label={t("stats.runs")} value={format.whole(stats.runs)} />
        <StatTile icon={<Gauge size={icon.sm} />} label={t("stats.avgSpeed")} value={speed(stats.avgSpeed)} />
        <StatTile icon={<Activity size={icon.sm} />} label={t("stats.peakSpeed")} value={speed(stats.peakSpeed)} />
        <StatTile icon={<Timer size={icon.sm} />} label={t("stats.avgTtft")} value={t("units.ms", { value: format.whole(stats.avgTtft) })} />
      </div>
      <div className="grid grid-cols-1 gap-5 lg:grid-cols-2">
        <Panel title={t("charts.speedByRun")} description={t("charts.speedByRunSub")}>
          <Chart data={speedByRun} x="run" kind="area" height={224} series={[{ key: "tps", label: t("charts.speedAxis") }]} format={format.decimal} />
        </Panel>
        <Panel title={t("charts.speedVsContext")} description={t("charts.speedVsContextSub")}>
          <SpeedVsContextChart points={speedVsContext} />
        </Panel>
      </div>
      {latestSamples.length > 0 && (
        <Panel title={t("charts.latestTitle")} description={t("charts.latestSub")}>
          <GenerationChart samples={latestSamples} />
        </Panel>
      )}
      <ListGroup label={t("list.title")}>
        {shown.map((metric) => (
          <ListRowButton
            key={metric.id}
            title={<span className="block truncate">{modelLabel(metric.modelName) ?? t("list.unknownModel")}</span>}
            description={[
              format.ago(metric.createdAt),
              metric.backend,
              metric.completionTokens === null ? null : t("units.tokens", { value: format.whole(metric.completionTokens) }),
            ]
              .filter(Boolean)
              .join(" · ")}
            trailing={
              <span className="flex shrink-0 items-center gap-3 tabular-nums">
                {metric.decodeTokensPerSecond !== null && <span className="text-sm font-medium text-fg">{speed(metric.decodeTokensPerSecond)}</span>}
                {metric.ttftMs !== null && <span className="hidden text-xs text-fg-3 sm:inline">{t("units.ms", { value: format.whole(metric.ttftMs) })}</span>}
              </span>
            }
            onClick={() => onOpen(metric.id)}
          />
        ))}
      </ListGroup>
      <Pagination page={current} pageCount={pageCount} onChange={setPage} />
    </>
  );
}

export function PerformancePage() {
  const { t } = useTranslation("performance");
  const { t: settings } = useTranslation("settings");
  const { t: common } = useTranslation();
  const api = useSettingsApi();
  const { phone, back } = useSettingsLayout();
  const client = useQueryClient();
  const metrics = useQuery(metricsListQuery(api));
  const [open, setOpen] = useState<string | null>(null);
  const clear = useMutation({
    mutationFn: () => api.call("llmMetricsClear", { client_operation_id: crypto.randomUUID() }),
    onSuccess: async () => {
      toast.success(t("cleared"));
      await client.invalidateQueries({ queryKey: queryKeys.llmMetrics.all });
    },
    onError: () => toast.error(common("errors.code.internal")),
  });
  const askClear = async () => {
    const confirmed = await confirm({
      title: t("clearConfirmTitle"),
      message: t("clearConfirmBody"),
      confirmLabel: t("clear"),
      cancelLabel: common("actions.cancel"),
      destructive: true,
    });
    if (confirmed) clear.mutate();
  };
  const items = metrics.data ?? [];
  return (
    <Page
      title={settings("sections.performance.title")}
      meta={phone ? undefined : t("subtitle")}
      {...back}
      contentClassName="w-full space-y-5"
      actions={
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="sm" leading={<RefreshCw size={icon.sm} />} loading={metrics.isFetching && !metrics.isPending} onClick={() => void metrics.refetch()}>
            {t("refresh")}
          </Button>
          {items.length > 0 && (
            <Button variant="ghost" size="sm" className="text-danger" leading={<Trash2 size={icon.sm} />} loading={clear.isPending} onClick={() => void askClear()}>
              {t("clear")}
            </Button>
          )}
        </div>
      }
    >
      {metrics.isPending ? (
        <LoadingState />
      ) : metrics.isError ? (
        <FailureState error={metrics.error} onRetry={() => void metrics.refetch()} />
      ) : items.length === 0 ? (
        <EmptyState icon={<Activity size={icon.lg} />} title={t("emptyTitle")} description={t("empty")} />
      ) : (
        <Overview metrics={items} onOpen={setOpen} />
      )}
      <DetailSheet id={open} onClose={() => setOpen(null)} />
    </Page>
  );
}
