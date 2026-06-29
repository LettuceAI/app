import { useCallback, useEffect, useMemo, useState } from "react";
import { Activity, Gauge, Timer, Hash, Trash2, RefreshCw, Loader2 } from "lucide-react";
import {
  AreaChart,
  Area,
  ScatterChart,
  Scatter,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  CartesianGrid,
} from "recharts";
import { useI18n } from "../../../core/i18n/context";
import { BottomMenu } from "../../components/BottomMenu";
import { confirmBottomMenu } from "../../components/ConfirmBottomMenu";
import { toast } from "../../components/toast";
import { listLlmMetrics, getLlmMetric, clearLlmMetrics } from "../../../core/storage/metrics";
import type { LlmMetricSummary, LlmMetricDetail } from "../../../core/storage/schemas";
import {
  round,
  GRID_STROKE,
  AXIS_TICK,
  AXIS_LABEL,
  GenerationOverlayChart,
  GenerationDetailContent,
} from "../../components/GenerationDetailView";

function relativeTime(ms: number): string {
  const diff = Date.now() - ms;
  const minutes = Math.floor(diff / 60000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.floor(hours / 24)}d ago`;
}

function StatCard({
  icon,
  label,
  value,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
}) {
  return (
    <div className="rounded-xl border border-fg/8 bg-fg/[0.02] px-4 py-3">
      <div className="flex items-center gap-2 text-fg/45">
        {icon}
        <span className="text-[10.5px] font-semibold uppercase tracking-[0.1em]">{label}</span>
      </div>
      <div className="mt-1.5 text-[18px] font-semibold tabular-nums text-fg">{value}</div>
    </div>
  );
}

function ChartCard({
  title,
  subtitle,
  children,
}: {
  title: string;
  subtitle: string;
  children: React.ReactNode;
}) {
  return (
    <section className="rounded-xl border border-fg/8 bg-fg/[0.02]">
      <header className="border-b border-fg/8 px-4 py-3">
        <h3 className="text-[13px] font-semibold tracking-tight text-fg">{title}</h3>
        <p className="mt-0.5 text-[11.5px] text-fg/45">{subtitle}</p>
      </header>
      <div className="px-2 py-3">{children}</div>
    </section>
  );
}

export default function PerformancePage() {
  const { t } = useI18n();
  const [metrics, setMetrics] = useState<LlmMetricSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [latestDetail, setLatestDetail] = useState<LlmMetricDetail | null>(null);
  const [selected, setSelected] = useState<LlmMetricDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const list = await listLlmMetrics();
      setMetrics(list);
      setLatestDetail(list.length > 0 ? await getLlmMetric(list[0].id) : null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const openDetail = useCallback(async (id: string) => {
    setDetailLoading(true);
    try {
      setSelected(await getLlmMetric(id));
    } finally {
      setDetailLoading(false);
    }
  }, []);

  const handleClear = useCallback(async () => {
    const confirmed = await confirmBottomMenu({
      title: t("performance.clearConfirmTitle"),
      message: t("performance.clearConfirmBody"),
      confirmLabel: t("performance.clear"),
      destructive: true,
    });
    if (!confirmed) return;
    await clearLlmMetrics();
    toast.success(t("performance.cleared"));
    await load();
  }, [t, load]);

  const stats = useMemo(() => {
    const speeds = metrics
      .map((m) => m.decodeTokensPerSecond)
      .filter((v): v is number => typeof v === "number" && v > 0);
    const ttfts = metrics
      .map((m) => m.ttftMs)
      .filter((v): v is number => typeof v === "number" && v > 0);
    const avg = (xs: number[]) => (xs.length ? xs.reduce((s, x) => s + x, 0) / xs.length : 0);
    return {
      runs: metrics.length,
      avgSpeed: avg(speeds),
      peakSpeed: speeds.length ? Math.max(...speeds) : 0,
      avgTtft: avg(ttfts),
    };
  }, [metrics]);

  const speedByRun = useMemo(
    () =>
      [...metrics]
        .filter((m) => typeof m.decodeTokensPerSecond === "number")
        .sort((a, b) => a.createdAt - b.createdAt)
        .map((m, i) => ({ idx: i + 1, tps: round(m.decodeTokensPerSecond, 1) })),
    [metrics],
  );

  const speedVsContext = useMemo(
    () =>
      metrics
        .map((m) => ({
          ctx: m.nCtx ?? m.totalTokens ?? 0,
          tps: round(m.decodeTokensPerSecond, 1),
        }))
        .filter((p) => p.ctx > 0 && p.tps > 0),
    [metrics],
  );

  return (
    <div
      className="mx-auto flex w-full max-w-[1280px] flex-col gap-6 overflow-hidden px-5 pt-5 sm:px-8"
      style={{ height: "calc(100dvh - var(--topnav-h, 72px))" }}
    >
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-[16px] font-semibold tracking-tight text-fg">
            {t("performance.title")}
          </h1>
          <p className="mt-1 text-[12.5px] text-fg/50">{t("performance.subtitle")}</p>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => void load()}
            className="flex items-center gap-1.5 rounded-lg border border-fg/10 bg-fg/[0.03] px-3 py-1.5 text-[12px] font-medium text-fg/70 transition-colors hover:text-fg"
          >
            <RefreshCw className="h-3.5 w-3.5" />
            {t("performance.refresh")}
          </button>
          {metrics.length > 0 && (
            <button
              onClick={() => void handleClear()}
              className="flex items-center gap-1.5 rounded-lg border border-danger/20 bg-danger/[0.06] px-3 py-1.5 text-[12px] font-medium text-danger/90 transition-colors hover:bg-danger/10"
            >
              <Trash2 className="h-3.5 w-3.5" />
              {t("performance.clear")}
            </button>
          )}
        </div>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto pb-10">
        {loading ? (
          <div className="flex items-center justify-center py-20 text-fg/40">
            <Loader2 className="h-5 w-5 animate-spin" />
          </div>
        ) : metrics.length === 0 ? (
          <div className="rounded-xl border border-dashed border-fg/10 bg-fg/[0.02] px-4 py-16 text-center">
            <div className="mx-auto mb-3 flex h-11 w-11 items-center justify-center rounded-lg border border-fg/10 bg-fg/[0.04]">
              <Activity className="h-5 w-5 text-fg/40" />
            </div>
            <p className="mx-auto max-w-md text-[12.5px] text-fg/55">{t("performance.empty")}</p>
          </div>
        ) : (
          <div className="flex flex-col gap-5">
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
              <StatCard
                icon={<Hash className="h-3.5 w-3.5" />}
                label={t("performance.stats.runs")}
                value={String(stats.runs)}
              />
              <StatCard
                icon={<Gauge className="h-3.5 w-3.5" />}
                label={t("performance.stats.avgSpeed")}
                value={`${round(stats.avgSpeed, 1)} tok/s`}
              />
              <StatCard
                icon={<Activity className="h-3.5 w-3.5" />}
                label={t("performance.stats.peakSpeed")}
                value={`${round(stats.peakSpeed, 1)} tok/s`}
              />
              <StatCard
                icon={<Timer className="h-3.5 w-3.5" />}
                label={t("performance.stats.avgTtft")}
                value={`${round(stats.avgTtft)} ms`}
              />
            </div>

            <div className="grid grid-cols-1 gap-5 lg:grid-cols-2">
              <ChartCard
                title={t("performance.charts.speedByRun")}
                subtitle={t("performance.charts.speedByRunSub")}
              >
                <ResponsiveContainer width="100%" height={216}>
                  <AreaChart data={speedByRun} margin={{ top: 8, right: 12, bottom: 18, left: 6 }}>
                    <defs>
                      <linearGradient id="perfSpeedGrad" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0%" stopColor="var(--color-accent)" stopOpacity={0.3} />
                        <stop offset="100%" stopColor="var(--color-accent)" stopOpacity={0} />
                      </linearGradient>
                    </defs>
                    <CartesianGrid stroke={GRID_STROKE} vertical={false} />
                    <XAxis
                      dataKey="idx"
                      tick={AXIS_TICK}
                      tickLine={false}
                      axisLine={false}
                      label={{
                        value: t("performance.charts.runAxis"),
                        position: "insideBottom",
                        offset: -8,
                        ...AXIS_LABEL,
                      }}
                    />
                    <YAxis
                      tick={AXIS_TICK}
                      tickLine={false}
                      axisLine={false}
                      width={44}
                      label={{
                        value: t("performance.charts.speedAxis"),
                        angle: -90,
                        position: "insideLeft",
                        ...AXIS_LABEL,
                      }}
                    />
                    <Tooltip
                      contentStyle={{
                        background: "var(--color-surface)",
                        border: "1px solid rgba(255,255,255,0.1)",
                        borderRadius: 8,
                        fontSize: 11,
                      }}
                    />
                    <Area
                      type="monotone"
                      dataKey="tps"
                      stroke="var(--color-accent)"
                      strokeWidth={1.5}
                      fill="url(#perfSpeedGrad)"
                      name={t("performance.charts.speedAxis")}
                    />
                  </AreaChart>
                </ResponsiveContainer>
              </ChartCard>

              <ChartCard
                title={t("performance.charts.speedVsContext")}
                subtitle={t("performance.charts.speedVsContextSub")}
              >
                <ResponsiveContainer width="100%" height={216}>
                  <ScatterChart margin={{ top: 8, right: 12, bottom: 18, left: 6 }}>
                    <CartesianGrid stroke={GRID_STROKE} />
                    <XAxis
                      type="number"
                      dataKey="ctx"
                      name="context"
                      tick={AXIS_TICK}
                      tickLine={false}
                      axisLine={false}
                      label={{
                        value: t("performance.charts.contextSizeAxis"),
                        position: "insideBottom",
                        offset: -8,
                        ...AXIS_LABEL,
                      }}
                    />
                    <YAxis
                      type="number"
                      dataKey="tps"
                      name="tok/s"
                      tick={AXIS_TICK}
                      tickLine={false}
                      axisLine={false}
                      width={44}
                      label={{
                        value: t("performance.charts.speedAxis"),
                        angle: -90,
                        position: "insideLeft",
                        ...AXIS_LABEL,
                      }}
                    />
                    <Tooltip
                      cursor={{ stroke: GRID_STROKE }}
                      contentStyle={{
                        background: "var(--color-surface)",
                        border: "1px solid rgba(255,255,255,0.1)",
                        borderRadius: 8,
                        fontSize: 11,
                      }}
                    />
                    <Scatter data={speedVsContext} fill="var(--color-info)" />
                  </ScatterChart>
                </ResponsiveContainer>
              </ChartCard>
            </div>

            {latestDetail && latestDetail.samples.length > 0 && (
              <ChartCard
                title={t("performance.charts.latestTitle")}
                subtitle={t("performance.charts.latestSub")}
              >
                <GenerationOverlayChart samples={latestDetail.samples} />
              </ChartCard>
            )}

            <section className="rounded-xl border border-fg/8 bg-fg/[0.02]">
              <header className="border-b border-fg/8 px-4 py-3">
                <h3 className="text-[13px] font-semibold tracking-tight text-fg">
                  {t("performance.list.title")}
                </h3>
              </header>
              <ul className="divide-y divide-fg/5">
                {metrics.map((m) => (
                  <li key={m.id}>
                    <button
                      onClick={() => void openDetail(m.id)}
                      className="flex w-full items-center justify-between gap-3 px-4 py-3 text-left transition-colors hover:bg-fg/[0.03]"
                    >
                      <div className="min-w-0">
                        <div className="truncate text-[12.5px] font-medium text-fg/85">
                          {m.modelName ?? t("performance.list.unknownModel")}
                        </div>
                        <div className="mt-0.5 text-[11px] text-fg/40">
                          {relativeTime(m.createdAt)}
                          {m.backend ? ` · ${m.backend}` : ""}
                          {typeof m.completionTokens === "number"
                            ? ` · ${m.completionTokens} tok`
                            : ""}
                        </div>
                      </div>
                      <div className="flex shrink-0 items-center gap-4 tabular-nums">
                        <span className="text-[12.5px] font-semibold text-fg">
                          {t("performance.list.speed", {
                            value: round(m.decodeTokensPerSecond, 1),
                          })}
                        </span>
                        <span className="hidden text-[11.5px] text-fg/45 sm:inline">
                          {t("performance.list.ttft", { value: round(m.ttftMs) })}
                        </span>
                      </div>
                    </button>
                  </li>
                ))}
              </ul>
            </section>
          </div>
        )}
      </div>

      <BottomMenu
        isOpen={Boolean(selected) || detailLoading}
        onClose={() => setSelected(null)}
        title={t("performance.charts.detailTitle")}
      >
        <div className="text-white">
          {detailLoading ? (
            <div className="flex items-center justify-center py-12 text-fg/40">
              <Loader2 className="h-5 w-5 animate-spin" />
            </div>
          ) : !selected ? (
            <p className="py-10 text-center text-[12.5px] text-fg/45">
              {t("performance.charts.noSamples")}
            </p>
          ) : (
            <GenerationDetailContent detail={selected} />
          )}
        </div>
      </BottomMenu>
    </div>
  );
}
