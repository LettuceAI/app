import { CartesianGrid, Legend, Line, LineChart, ResponsiveContainer, Scatter, ScatterChart, Tooltip, XAxis, YAxis } from "recharts";
import { useTranslation } from "react-i18next";
import type { MetricSample } from "./data";

const axis = {
  stroke: "var(--color-fg-4)",
  tickLine: false,
  axisLine: false,
  tick: { fontSize: 11, fill: "var(--color-fg-3)" },
} as const;

const tooltip = {
  contentStyle: {
    background: "var(--color-surface-el)",
    border: "1px solid var(--color-line)",
    borderRadius: 8,
    fontSize: 12,
    color: "var(--color-fg)",
  },
  labelStyle: { color: "var(--color-fg-3)" },
} as const;

/** Decode speed against how long each run's prompt was. */
export function SpeedVsPromptChart({ points }: { points: readonly { prompt: number; tps: number }[] }) {
  const { t } = useTranslation("performance");
  return (
    <div className="h-56 w-full">
      <ResponsiveContainer width="100%" height="100%">
        <ScatterChart margin={{ top: 4, right: 8, bottom: 0, left: -16 }}>
          <CartesianGrid stroke="var(--color-line)" />
          <XAxis type="number" dataKey="prompt" name={t("charts.promptAxis")} {...axis} />
          <YAxis type="number" dataKey="tps" name={t("charts.speedAxis")} width={48} {...axis} />
          <Tooltip cursor={{ stroke: "var(--color-line-2)" }} {...tooltip} />
          <Scatter data={points as { prompt: number; tps: number }[]} fill="var(--color-info)" />
        </ScatterChart>
      </ResponsiveContainer>
    </div>
  );
}

/** One generation second by second: decode speed and how full the context was. */
export function GenerationChart({ samples }: { samples: readonly MetricSample[] }) {
  const { t } = useTranslation("performance");
  const data = samples.map((sample) => ({
    s: Math.round(sample.tMs / 100) / 10,
    tps: Math.round(sample.tps * 10) / 10,
    ctx: Math.round(sample.ctxFill * 1000) / 10,
  }));
  if (data.length === 0) return <p className="py-8 text-center text-sm text-fg-3">{t("charts.noSamples")}</p>;
  return (
    <div className="h-64 w-full">
      <ResponsiveContainer width="100%" height="100%">
        <LineChart data={data} margin={{ top: 4, right: 0, bottom: 0, left: -16 }}>
          <CartesianGrid stroke="var(--color-line)" vertical={false} />
          <XAxis dataKey="s" unit="s" {...axis} />
          <YAxis yAxisId="tps" width={48} {...axis} />
          <YAxis yAxisId="ctx" orientation="right" domain={[0, 100]} width={40} unit="%" {...axis} />
          <Tooltip cursor={{ stroke: "var(--color-line-2)" }} {...tooltip} />
          <Legend iconType="plainline" wrapperStyle={{ fontSize: 12, paddingTop: 8 }} />
          <Line yAxisId="tps" type="monotone" dataKey="tps" name={t("charts.legendSpeed")} stroke="var(--color-accent)" strokeWidth={2} dot={false} />
          <Line yAxisId="ctx" type="monotone" dataKey="ctx" name={t("charts.legendContext")} stroke="var(--color-info)" strokeWidth={2} dot={false} />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
