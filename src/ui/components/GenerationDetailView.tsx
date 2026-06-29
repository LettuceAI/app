import { useMemo } from "react";
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  Tooltip,
  Legend,
  ResponsiveContainer,
  CartesianGrid,
} from "recharts";
import { useI18n } from "../../core/i18n/context";
import type { LlmMetricSample, LlmMetricDetail } from "../../core/storage/schemas";

export const GRID_STROKE = "rgba(255,255,255,0.05)";
export const AXIS_TICK = { fill: "rgba(255,255,255,0.4)", fontSize: 10 };
export const AXIS_LABEL = { fill: "rgba(255,255,255,0.45)", fontSize: 10.5 };

export function round(value: number | null | undefined, digits = 0): number {
  if (value == null || !Number.isFinite(value)) return 0;
  const factor = 10 ** digits;
  return Math.round(value * factor) / factor;
}

export function formatBytes(bytes: number | null | undefined): string | null {
  if (bytes == null || !Number.isFinite(bytes) || bytes <= 0) return null;
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(2)} GB`;
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(0)} MB`;
  return `${bytes} B`;
}

export function formatElapsed(ms: number | null | undefined): string | null {
  if (ms == null || !Number.isFinite(ms) || ms <= 0) return null;
  const seconds = ms / 1000;
  if (seconds < 60) return `${seconds.toFixed(1)}s`;
  const m = Math.floor(seconds / 60);
  const s = Math.round(seconds % 60);
  return `${m}m ${s}s`;
}

export function GenerationOverlayChart({ samples }: { samples: LlmMetricSample[] }) {
  const { t } = useI18n();
  const data = useMemo(
    () =>
      samples.map((s) => ({
        s: round(s.tMs / 1000, 1),
        tps: round(s.tps, 1),
        ctx: round(s.ctxFill * 100, 1),
      })),
    [samples],
  );

  if (data.length === 0) {
    return (
      <p className="py-10 text-center text-[12.5px] text-fg/45">
        {t("performance.charts.noSamples")}
      </p>
    );
  }

  return (
    <ResponsiveContainer width="100%" height={264}>
      <LineChart data={data} margin={{ top: 8, right: 8, bottom: 4, left: 0 }}>
        <CartesianGrid stroke={GRID_STROKE} vertical={false} />
        <XAxis dataKey="s" tick={AXIS_TICK} tickLine={false} axisLine={false} unit="s" />
        <YAxis yAxisId="tps" tick={AXIS_TICK} tickLine={false} axisLine={false} width={36} />
        <YAxis
          yAxisId="ctx"
          orientation="right"
          domain={[0, 100]}
          tick={AXIS_TICK}
          tickLine={false}
          axisLine={false}
          width={36}
        />
        <Tooltip
          contentStyle={{
            background: "var(--color-surface)",
            border: "1px solid rgba(255,255,255,0.1)",
            borderRadius: 8,
            fontSize: 11,
          }}
        />
        <Legend iconType="plainline" wrapperStyle={{ fontSize: 11, paddingTop: 8 }} />
        <Line
          yAxisId="tps"
          type="monotone"
          dataKey="tps"
          stroke="var(--color-accent)"
          strokeWidth={1.5}
          dot={false}
          name={t("performance.charts.legendSpeed")}
        />
        <Line
          yAxisId="ctx"
          type="monotone"
          dataKey="ctx"
          stroke="var(--color-info)"
          strokeWidth={1.5}
          dot={false}
          name={t("performance.charts.legendContext")}
        />
      </LineChart>
    </ResponsiveContainer>
  );
}

export function DetailStats({ detail }: { detail: LlmMetricDetail }) {
  const { t } = useI18n();
  const rows: { label: string; value: string }[] = [];
  const push = (label: string, value: string | number | null | undefined) => {
    if (value == null || value === "") return;
    rows.push({ label, value: String(value) });
  };

  push(
    t("performance.detail.decodeSpeed"),
    typeof detail.decodeTokensPerSecond === "number"
      ? `${round(detail.decodeTokensPerSecond, 1)} tok/s`
      : null,
  );
  push(
    t("performance.detail.firstToken"),
    typeof detail.ttftMs === "number" ? `${round(detail.ttftMs)} ms` : null,
  );
  push(t("performance.detail.elapsed"), formatElapsed(detail.generationElapsedMs));
  push(t("performance.detail.promptTokens"), detail.promptTokens);
  push(t("performance.detail.outputTokens"), detail.completionTokens);
  push(t("performance.detail.totalTokens"), detail.totalTokens);
  push(t("performance.detail.context"), detail.nCtx);
  push(t("performance.detail.backend"), detail.backend);
  push(t("performance.detail.gpuLayers"), detail.gpuLayers);
  push(t("performance.detail.kvType"), detail.kvType);
  push(t("performance.detail.modelSize"), formatBytes(detail.modelSizeBytes));
  push(t("performance.detail.finishReason"), detail.finishReason);

  if (rows.length === 0) return null;

  return (
    <div className="mb-4 grid grid-cols-2 gap-x-4 gap-y-2.5 rounded-xl border border-fg/8 bg-fg/[0.02] px-4 py-3 sm:grid-cols-3">
      {rows.map((r) => (
        <div key={r.label} className="min-w-0">
          <div className="text-[10px] font-semibold uppercase tracking-[0.08em] text-fg/40">
            {r.label}
          </div>
          <div className="mt-0.5 truncate text-[12.5px] font-medium tabular-nums text-fg/85">
            {r.value}
          </div>
        </div>
      ))}
    </div>
  );
}

export function GenerationDetailContent({ detail }: { detail: LlmMetricDetail }) {
  const { t } = useI18n();
  return (
    <div className="pb-4">
      <div className="mb-3 truncate text-[12.5px] font-medium text-fg/80">
        {detail.modelName ?? t("performance.list.unknownModel")}
      </div>
      <DetailStats detail={detail} />
      <GenerationOverlayChart samples={detail.samples} />
    </div>
  );
}
