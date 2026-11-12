import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Activity, Clock, Gauge, Hash, RefreshCw, Timer, Trash2, Zap } from "lucide-react";
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
  Table,
  toast,
} from "@lettuceai/crisp";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { SelectMenuField } from "@/shared/ui/SelectMenuField";
import { useSettingsApi, useSettingsLayout } from "../data";
import { GenerationChart, SpeedVsPromptChart } from "./charts";
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
  cachedPromptTokens: "detail.cachedPromptTokens",
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
    const day = new Intl.DateTimeFormat(i18n.language, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });
    return { decimal: (value: number) => decimal.format(value), whole: (value: number) => whole.format(value), size, ago, seconds, date: (at: number) => day.format(at) };
  }, [i18n.language]);
}

const ALL_MODELS = "__all__";

function median(values: readonly (number | null)[]): number | null {
  const sorted = values.filter((value): value is number => value !== null && value > 0).sort((a, b) => a - b);
  if (sorted.length === 0) return null;
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[middle]! : (sorted[middle - 1]! + sorted[middle]!) / 2;
}

function DetailGroup({ label, items }: { label: string; items: readonly { label: string; value: string }[] }) {
  if (items.length === 0) return null;
  return (
    <section className="space-y-2">
      <h3 className="text-xs font-medium text-fg-3">{label}</h3>
      <KeyValue items={items} />
    </section>
  );
}

function DetailSheet({ id, metrics, onClose }: { id: string | null; metrics: readonly MetricSummary[]; onClose: () => void }) {
  const { t } = useTranslation("performance");
  const api = useSettingsApi();
  const format = useFormatters();
  const [shown, setShown] = useState(id);
  if (id !== null && id !== shown) setShown(id);
  const detail = useQuery(metricDetailQuery(api, shown));
  const metric = detail.data ? summaryOf(detail.data) : null;
  const siblings = metric ? metrics.filter((run) => modelKey(run) === modelKey(metric)) : [];
  const speed = (value: number | null) => (value === null ? null : t("units.speed", { value: format.decimal(value) }));
  const ms = (value: number | null) => (value === null ? null : t("units.ms", { value: format.whole(value) }));
  const time = (value: number | null) => (value === null ? null : format.seconds(value));
  const count = (value: number | null) => (value === null ? null : format.whole(value));
  const rows = (entries: readonly [keyof typeof detailKeys, string | number | null][]) =>
    entries.flatMap(([key, value]) => (value === null || value === "" ? [] : [{ label: t(detailKeys[key]), value: String(value) }]));
  const typical = (pick: (run: MetricSummary) => number | null, show: (value: number | null) => string | null) => {
    if (siblings.length < 2) return undefined;
    const value = show(median(siblings.map(pick)));
    return value === null ? undefined : t("detail.modelMedian", { value });
  };
  return (
    <Sheet
      open={id !== null}
      onClose={onClose}
      size="lg"
      title={metric ? (modelLabel(metric.modelName) ?? t("list.unknownModel")) : t("charts.detailTitle")}
      description={metric ? [format.date(metric.createdAt), metric.backend].filter(Boolean).join(" · ") : undefined}
    >
      {detail.isPending ? (
        <LoadingState />
      ) : detail.isError ? (
        <FailureState size="sm" error={detail.error} onRetry={() => void detail.refetch()} />
      ) : !detail.data || !metric ? (
        <p className="py-8 text-center text-sm text-fg-3">{t("charts.noSamples")}</p>
      ) : (
        <div className="space-y-6 pb-1">
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <StatTile icon={<Gauge size={icon.sm} />} label={t("detail.decodeSpeed")} value={speed(metric.decodeTokensPerSecond) ?? "-"} sub={typical((run) => run.decodeTokensPerSecond, speed)} />
            <StatTile icon={<Zap size={icon.sm} />} label={t("stats.promptSpeed")} value={speed(metric.nativePromptEvalTokensPerSecond) ?? "-"} sub={typical((run) => run.nativePromptEvalTokensPerSecond, speed)} />
            <StatTile icon={<Timer size={icon.sm} />} label={t("detail.firstToken")} value={ms(metric.ttftMs) ?? "-"} sub={typical((run) => run.ttftMs, ms)} />
            <StatTile icon={<Clock size={icon.sm} />} label={t("detail.elapsed")} value={time(metric.generationElapsedMs) ?? "-"} sub={typical((run) => run.generationElapsedMs, time)} />
          </div>
          <section className="space-y-2">
            <h3 className="text-xs font-medium text-fg-3">{t("detail.timeline")}</h3>
            <GenerationChart samples={samplesOf(detail.data)} />
          </section>
          <div className="grid grid-cols-1 gap-6 sm:grid-cols-2">
            <DetailGroup
              label={t("detail.groupTokens")}
              items={rows([
                ["promptTokens", count(metric.promptTokens)],
                ["cachedPromptTokens", count(metric.cachedPromptTokens)],
                ["outputTokens", count(metric.completionTokens)],
                ["totalTokens", count(metric.totalTokens)],
                ["finishReason", metric.finishReason],
              ])}
            />
            <DetailGroup
              label={t("detail.groupTime")}
              items={rows([
                ["nativeGenerationSpeed", speed(metric.nativeGenerationTokensPerSecond)],
                ["nativeGenerationTime", time(metric.nativeGenerationComputeMs)],
                ["appOverhead", time(metric.appGenerationOverheadMs)],
              ])}
            />
          </div>
          <DetailGroup
            label={t("detail.groupSetup")}
            items={rows([
              ["backend", metric.backend],
              ["gpuLayers", metric.gpuLayers],
              ["context", count(metric.nCtx)],
              ["batch", count(metric.nBatch)],
              ["microbatch", count(metric.nUbatch)],
              ["kvType", metric.kvType],
              ["modelSize", metric.modelSizeBytes === null ? null : format.size(metric.modelSizeBytes)],
            ])}
          />
        </div>
      )}
    </Sheet>
  );
}

interface ModelRow {
  model: string;
  runs: number;
  decode: number | null;
  prompt: number | null;
  firstToken: number | null;
}

const modelKey = (metric: MetricSummary) => metric.modelName ?? "";

function modelRows(metrics: readonly MetricSummary[]): ModelRow[] {
  const groups = new Map<string, MetricSummary[]>();
  for (const metric of metrics) groups.set(modelKey(metric), [...(groups.get(modelKey(metric)) ?? []), metric]);
  return [...groups].map(([model, runs]) => ({
    model,
    runs: runs.length,
    decode: median(runs.map((run) => run.decodeTokensPerSecond)),
    prompt: median(runs.map((run) => run.nativePromptEvalTokensPerSecond)),
    firstToken: median(runs.map((run) => run.ttftMs)),
  }));
}

function Overview({ metrics, scope, onScope, onOpen }: { metrics: readonly MetricSummary[]; scope: string; onScope: (scope: string) => void; onOpen: (id: string) => void }) {
  const { t } = useTranslation("performance");
  const { t: look } = useTranslation("customization");
  const api = useSettingsApi();
  const format = useFormatters();
  const [page, setPage] = useState(1);
  const runs = useMemo(() => (scope === ALL_MODELS ? metrics : metrics.filter((metric) => modelKey(metric) === scope)), [metrics, scope]);
  const latest = useQuery(metricDetailQuery(api, runs[0]?.id ?? null));
  const latestSamples = samplesOf(latest.data);
  const decode = median(runs.map((run) => run.decodeTokensPerSecond));
  const prompt = median(runs.map((run) => run.nativePromptEvalTokensPerSecond));
  const firstToken = median(runs.map((run) => run.ttftMs));
  const models = useMemo(() => (scope === ALL_MODELS ? modelRows(metrics) : []), [metrics, scope]);
  const overTime = useMemo(
    () =>
      [...runs]
        .reverse()
        .flatMap((run) => (run.decodeTokensPerSecond === null ? [] : [{ at: format.date(run.createdAt), tps: Math.round(run.decodeTokensPerSecond * 10) / 10 }])),
    [runs, format],
  );
  const speedVsPrompt = useMemo(
    () =>
      runs.flatMap((run) =>
        run.promptTokens !== null && run.promptTokens > 0 && run.decodeTokensPerSecond !== null && run.decodeTokensPerSecond > 0
          ? [{ prompt: run.promptTokens, tps: Math.round(run.decodeTokensPerSecond * 10) / 10 }]
          : [],
      ),
    [runs],
  );
  const pageCount = Math.max(1, Math.ceil(runs.length / LIST_PAGE_SIZE));
  const current = Math.min(page, pageCount);
  const shown = runs.slice((current - 1) * LIST_PAGE_SIZE, current * LIST_PAGE_SIZE);
  const speed = (value: number | null) => (value === null ? "-" : t("units.speed", { value: format.decimal(value) }));
  const ms = (value: number | null) => (value === null ? "-" : t("units.ms", { value: format.whole(value) }));
  const medianLabel = t("stats.median");
  return (
    <>
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <StatTile icon={<Gauge size={icon.sm} />} label={t("detail.decodeSpeed")} value={speed(decode)} sub={medianLabel} />
        <StatTile icon={<Zap size={icon.sm} />} label={t("stats.promptSpeed")} value={speed(prompt)} sub={medianLabel} />
        <StatTile icon={<Timer size={icon.sm} />} label={t("detail.firstToken")} value={ms(firstToken)} sub={medianLabel} />
        <StatTile icon={<Hash size={icon.sm} />} label={t("stats.runs")} value={format.whole(runs.length)} />
      </div>
      {models.length > 1 && (
        <Panel title={t("models.title")} description={t("models.description")}>
          <Table
            rows={models}
            rowKey={(row) => row.model}
            onRowClick={(row) => onScope(row.model)}
            columns={[
              { key: "model", header: t("models.model"), cell: (row) => <span className="block max-w-[18rem] truncate">{modelLabel(row.model) ?? t("list.unknownModel")}</span> },
              { key: "runs", header: t("stats.runs"), numeric: true, align: "right", cell: (row) => format.whole(row.runs) },
              { key: "decode", header: t("detail.decodeSpeed"), numeric: true, align: "right", cell: (row) => speed(row.decode) },
              { key: "prompt", header: t("stats.promptSpeed"), numeric: true, align: "right", cell: (row) => speed(row.prompt) },
              { key: "firstToken", header: t("detail.firstToken"), numeric: true, align: "right", cell: (row) => ms(row.firstToken) },
            ]}
          />
        </Panel>
      )}
      <div className="grid grid-cols-1 gap-5 lg:grid-cols-2">
        <Panel title={t("charts.speedOverTime")} description={t("charts.speedOverTimeSub")}>
          <Chart data={overTime} x="at" kind="area" height={224} series={[{ key: "tps", label: t("charts.speedAxis") }]} format={format.decimal} />
        </Panel>
        <Panel title={t("charts.speedVsPrompt")} description={t("charts.speedVsPromptSub")}>
          <SpeedVsPromptChart points={speedVsPrompt} />
        </Panel>
      </div>
      {latestSamples.length > 0 && (
        <Panel title={t("charts.latestTitle")} description={t("charts.latestSub")}>
          <GenerationChart samples={latestSamples} />
        </Panel>
      )}
      <ListGroup label={t("list.title")}>
        {shown.map((run) => (
          <ListRowButton
            key={run.id}
            title={<span className="block truncate">{modelLabel(run.modelName) ?? t("list.unknownModel")}</span>}
            description={[
              format.ago(run.createdAt),
              run.backend,
              run.promptTokens === null ? null : look("metricInput", { value: format.whole(run.promptTokens) }),
              run.completionTokens === null ? null : look("metricOutput", { value: format.whole(run.completionTokens) }),
            ]
              .filter(Boolean)
              .join(" · ")}
            trailing={
              <span className="flex shrink-0 items-center gap-3 tabular-nums">
                {run.decodeTokensPerSecond !== null && <span className="text-sm font-medium text-fg">{speed(run.decodeTokensPerSecond)}</span>}
                {run.ttftMs !== null && <span className="hidden text-xs text-fg-3 sm:inline">{ms(run.ttftMs)}</span>}
              </span>
            }
            onClick={() => onOpen(run.id)}
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
  const format = useFormatters();
  const metrics = useQuery(metricsListQuery(api));
  const [open, setOpen] = useState<string | null>(null);
  const [chosen, setChosen] = useState<string | null>(null);
  const clear = useMutation({
    mutationFn: () => api.call("llmMetricsClear", { client_operation_id: crypto.randomUUID() }),
    onSuccess: async () => {
      toast.success(t("cleared"));
      setChosen(null);
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
  const models = useMemo(() => {
    const counts = new Map<string, number>();
    for (const metric of items) counts.set(modelKey(metric), (counts.get(modelKey(metric)) ?? 0) + 1);
    return [...counts];
  }, [items]);
  const fallback = items[0] ? modelKey(items[0]) : ALL_MODELS;
  const scope = chosen !== null && (chosen === ALL_MODELS || models.some(([model]) => model === chosen)) ? chosen : fallback;
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
        <>
          {models.length > 1 && (
            <div className="max-w-sm">
              <SelectMenuField
                aria-label={t("filter.label")}
                value={scope}
                onChange={(value) => setChosen(value ?? fallback)}
                options={[
                  { value: ALL_MODELS, label: t("filter.all"), meta: format.whole(items.length) },
                  ...models.map(([model, count]) => ({ value: model, label: modelLabel(model) ?? t("list.unknownModel"), meta: format.whole(count) })),
                ]}
              />
            </div>
          )}
          <Overview key={scope} metrics={items} scope={scope} onScope={setChosen} onOpen={setOpen} />
        </>
      )}
      <DetailSheet id={open} metrics={items} onClose={() => setOpen(null)} />
    </Page>
  );
}
