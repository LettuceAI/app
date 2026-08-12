import { useState, useEffect, useMemo, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { motion, AnimatePresence } from "framer-motion";
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  CartesianGrid,
} from "recharts";
import {
  useUsageTracking,
  RequestUsage,
  UsageFilter,
  AppActiveUsageSummary,
} from "../../../core/usage";
import {
  Download,
  Activity,
  Clock,
  Filter,
  ChevronRight,
  Calendar,
  RefreshCw,
  X,
  Loader2,
  ChevronDown,
  Gauge,
} from "lucide-react";
import { BottomMenu } from "../../components";
import { useI18n } from "../../../core/i18n/context";
import {
  SectionCard,
  StatTile,
  UsageRequestDetailSheet,
  formatCompactNumber,
  formatCurrency,
  getEffectiveTotalCost,
  getOperationColor,
  getOperationLabel,
  getRelativeTime,
} from "./UsageActivityShared";
import { cn } from "../../design-tokens";
import { NanoGptUsagePanel } from "./NanoGptUsagePanel";

// =============================================================================
// Helpers
// =============================================================================

function formatDurationMs(durationMs: number): string {
  if (durationMs <= 0) return "0s";
  const totalSeconds = Math.floor(durationMs / 1000);
  const days = Math.floor(totalSeconds / 86400);
  const hours = Math.floor((totalSeconds % 86400) / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m ${seconds}s`;
  return `${seconds}s`;
}

function dayKeyFromDate(date: Date): string {
  const y = date.getFullYear();
  const m = `${date.getMonth() + 1}`.padStart(2, "0");
  const d = `${date.getDate()}`.padStart(2, "0");
  return `${y}-${m}-${d}`;
}

type DatePreset = "today" | "week" | "month" | "all" | "custom";
type ViewMode = "dashboard" | "appTime" | "nanogpt";

function getDateRange(preset: DatePreset): { start: Date; end: Date } {
  const now = new Date();
  const end = new Date(now);
  end.setHours(23, 59, 59, 999);
  const start = new Date(now);
  start.setHours(0, 0, 0, 0);
  switch (preset) {
    case "today":
      break;
    case "week":
      start.setDate(start.getDate() - 6);
      break;
    case "month":
      start.setDate(start.getDate() - 29);
      break;
    case "all":
      start.setFullYear(start.getFullYear() - 10);
      break;
    case "custom":
      break;
  }
  return { start, end };
}

// =============================================================================
// Chart tooltip
// =============================================================================

function makeChartTooltip(formatter: (value: number) => string) {
  return function ChartTooltip({ active, payload, label }: any) {
    if (!active || !payload?.length) return null;
    return (
      <div className="rounded-lg border border-fg/15 bg-surface/95 px-3 py-2 shadow-xl backdrop-blur-md">
        <p className="text-[10px] font-medium uppercase tracking-[0.1em] text-fg/45">{label}</p>
        <div className="mt-1.5 space-y-1">
          {payload.map((p: any, i: number) => (
            <div key={i} className="flex items-center justify-between gap-4">
              <div className="flex items-center gap-1.5">
                <span
                  className="h-1.5 w-1.5 rounded-full"
                  style={{ backgroundColor: p.color }}
                />
                <span className="text-[11px] text-fg/55">{p.name}</span>
              </div>
              <span className="text-[12px] font-medium tabular-nums text-fg">
                {formatter(p.value)}
              </span>
            </div>
          ))}
        </div>
      </div>
    );
  };
}

const TokenTooltip = makeChartTooltip(formatCompactNumber);
const TimeTooltip = makeChartTooltip(formatDurationMs);

// =============================================================================
// Atoms
// =============================================================================

function RankedBarList({
  items,
  emptyLabel,
  unit,
}: {
  items: Array<{ id: string; name: string; tokens: number; cost: number }>;
  emptyLabel: string;
  unit: string;
}) {
  if (items.length === 0) {
    return (
      <div className="py-8 text-center text-[12px] text-fg/40">{emptyLabel}</div>
    );
  }
  const max = Math.max(...items.map((i) => i.tokens), 1);
  return (
    <ul className="space-y-2.5">
      {items.map((item, idx) => {
        const pct = (item.tokens / max) * 100;
        return (
          <li key={item.id} className="space-y-1">
            <div className="flex items-baseline justify-between gap-3">
              <div className="flex min-w-0 items-center gap-2">
                <span className="w-4 shrink-0 text-[10.5px] font-semibold tabular-nums text-fg/35">
                  {idx + 1}
                </span>
                <span className="truncate text-[12.5px] font-medium text-fg/85">
                  {item.name}
                </span>
              </div>
              <div className="shrink-0 text-right">
                <span className="text-[12px] font-medium tabular-nums text-fg">
                  {formatCompactNumber(item.tokens)}
                </span>
                <span className="ml-1 text-[10.5px] text-fg/40">{unit}</span>
                {item.cost > 0 && (
                  <span className="ml-2 text-[11px] tabular-nums text-accent/85">
                    {formatCurrency(item.cost)}
                  </span>
                )}
              </div>
            </div>
            <div className="h-1 overflow-hidden rounded-full bg-fg/[0.06]">
              <div
                className="h-full rounded-full bg-accent/55"
                style={{ width: `${pct}%` }}
              />
            </div>
          </li>
        );
      })}
    </ul>
  );
}

function PresetPills<T extends string>({
  value,
  onChange,
  onCustom,
  options,
  customLabel,
  customActive,
}: {
  value: T;
  onChange: (v: T) => void;
  onCustom: () => void;
  options: Array<{ key: T; label: string }>;
  customLabel: string;
  customActive: boolean;
}) {
  return (
    <div className="inline-flex items-center gap-0.5 rounded-lg border border-fg/8 bg-fg/[0.025] p-0.5">
      {options.map(({ key, label }) => {
        const active = value === key;
        return (
          <button
            key={key}
            onClick={() => onChange(key)}
            className={cn(
              "rounded-md px-2.5 py-1 text-[11.5px] font-medium transition-colors",
              active
                ? "bg-fg/10 text-fg"
                : "text-fg/45 hover:bg-fg/5 hover:text-fg/80",
            )}
          >
            {label}
          </button>
        );
      })}
      <button
        onClick={onCustom}
        className={cn(
          "rounded-md px-2.5 py-1 text-[11.5px] font-medium transition-colors",
          customActive
            ? "bg-fg/10 text-fg"
            : "text-fg/45 hover:bg-fg/5 hover:text-fg/80",
        )}
      >
        {customLabel}
      </button>
    </div>
  );
}

function IconButton({
  icon,
  label,
  onClick,
  disabled,
  active,
  spinning,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  disabled?: boolean;
  active?: boolean;
  spinning?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={label}
      aria-label={label}
      className={cn(
        "inline-flex h-8 w-8 items-center justify-center rounded-md border transition",
        active
          ? "border-accent/30 bg-accent/10 text-accent"
          : "border-fg/8 bg-fg/[0.02] text-fg/55 hover:border-fg/15 hover:bg-fg/5 hover:text-fg",
        disabled && "cursor-not-allowed opacity-30",
        spinning && "[&>svg]:animate-spin",
      )}
    >
      {icon}
    </button>
  );
}

// =============================================================================
// Activity row (dense desktop grid + mobile two-line)
// =============================================================================

function ActivityRow({
  request,
  onClick,
  t,
}: {
  request: RequestUsage;
  onClick: (r: RequestUsage) => void;
  t: (k: any, v?: any) => string;
}) {
  const opColor = getOperationColor(request.operationType);
  const opBg = opColor.includes("var")
    ? `color-mix(in srgb, ${opColor}, transparent 88%)`
    : `${opColor}1f`;
  return (
    <button
      type="button"
      onClick={() => onClick(request)}
      className="group block w-full border-b border-fg/[0.05] px-3 py-2.5 text-left transition hover:bg-fg/[0.025]"
    >
      <div className="grid items-center gap-3 lg:grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)_88px_88px_92px_16px]">
        {/* Name + op */}
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <span
              className="inline-flex h-1.5 w-1.5 shrink-0 rounded-full"
              style={{ backgroundColor: opColor }}
            />
            <span className="truncate text-[13px] font-medium text-fg">
              {request.characterName || t("usageAnalytics.shared.unknown")}
            </span>
            <span
              className="hidden shrink-0 rounded px-1.5 py-0.5 text-[9.5px] font-semibold uppercase tracking-wider sm:inline-flex"
              style={{ color: opColor, backgroundColor: opBg }}
            >
              {getOperationLabel(request.operationType, t)}
            </span>
          </div>
          <div className="mt-0.5 flex items-center gap-2 text-[11px] text-fg/45 lg:hidden">
            <span className="truncate">{request.modelName ?? "—"}</span>
            <span className="opacity-30">·</span>
            <span className="shrink-0">
              {formatCompactNumber(request.totalTokens || 0)} tok
            </span>
            <span className="opacity-30">·</span>
            <span className="shrink-0 font-medium text-accent/90">
              {formatCurrency(getEffectiveTotalCost(request))}
            </span>
            <span className="opacity-30">·</span>
            <span className="shrink-0">{getRelativeTime(request.timestamp, t)}</span>
          </div>
        </div>

        {/* Model */}
        <div className="hidden min-w-0 truncate text-[12px] text-fg/65 lg:block">
          {request.modelName ?? "—"}
        </div>

        {/* Tokens */}
        <div className="hidden text-right text-[12px] tabular-nums text-fg/75 lg:block">
          {formatCompactNumber(request.totalTokens || 0)}
        </div>

        {/* Cost */}
        <div className="hidden text-right text-[12px] font-medium tabular-nums text-accent lg:block">
          {formatCurrency(getEffectiveTotalCost(request))}
        </div>

        {/* Time */}
        <div className="hidden text-right text-[11.5px] text-fg/45 lg:block">
          {getRelativeTime(request.timestamp, t)}
        </div>

        {/* Chevron */}
        <ChevronRight
          size={14}
          className="hidden shrink-0 text-fg/25 transition group-hover:text-fg/55 lg:block"
        />
      </div>
    </button>
  );
}

// =============================================================================
// Main page
// =============================================================================

export function UsagePage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const { queryRecords, exportCSV, saveCSV, getAppActiveUsage } = useUsageTracking();

  const [viewMode, setViewMode] = useState<ViewMode>("dashboard");

  // Dashboard
  const [datePreset, setDatePreset] = useState<DatePreset>("month");
  const [records, setRecords] = useState<RequestUsage[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [exporting, setExporting] = useState(false);

  // App time
  const [appActiveUsage, setAppActiveUsage] = useState<AppActiveUsageSummary>({
    totalMs: 0,
    byDayMs: {},
  });
  const [appTimePreset, setAppTimePreset] =
    useState<DatePreset>("week");

  // Sheets
  const [showFilters, setShowFilters] = useState(false);
  const [showCustomDatePicker, setShowCustomDatePicker] = useState(false);
  const [selectedRequest, setSelectedRequest] = useState<RequestUsage | null>(null);

  // Custom date range (shared)
  const [customStartDate, setCustomStartDate] = useState<Date>(() => {
    const d = new Date();
    d.setDate(d.getDate() - 30);
    d.setHours(0, 0, 0, 0);
    return d;
  });
  const [customEndDate, setCustomEndDate] = useState<Date>(() => {
    const d = new Date();
    d.setHours(23, 59, 59, 999);
    return d;
  });

  // Filters
  const [selectedModel, setSelectedModel] = useState<string | null>(null);
  const [selectedCharacter, setSelectedCharacter] = useState<string | null>(null);

  const resolveDashboardRange = useCallback(() => {
    if (datePreset === "custom") return { start: customStartDate, end: customEndDate };
    return getDateRange(datePreset);
  }, [datePreset, customStartDate, customEndDate]);

  const loadDashboard = useCallback(
    async (mode: "initial" | "refresh" = "initial") => {
      if (mode === "initial") setLoading(true);
      else setRefreshing(true);
      const { start, end } = resolveDashboardRange();
      const filter: UsageFilter = {
        startTimestamp: start.getTime(),
        endTimestamp: end.getTime(),
      };
      try {
        const next = await queryRecords(filter);
        if (next) setRecords(next);
      } finally {
        setLoading(false);
        setRefreshing(false);
      }
    },
    [resolveDashboardRange, queryRecords],
  );

  const loadAppUsage = useCallback(async () => {
    const summary = await getAppActiveUsage();
    if (summary) setAppActiveUsage(summary);
  }, [getAppActiveUsage]);

  useEffect(() => {
    if (viewMode === "dashboard") void loadDashboard("initial");
  }, [viewMode, datePreset, customStartDate, customEndDate, loadDashboard]);

  useEffect(() => {
    void loadAppUsage();
  }, [loadAppUsage]);

  useEffect(() => {
    if (viewMode !== "appTime") return;
    void loadAppUsage();
    const interval = window.setInterval(() => void loadAppUsage(), 15000);
    return () => window.clearInterval(interval);
  }, [viewMode, loadAppUsage]);

  // ---- Derived ----

  const filteredRecords = useMemo(() => {
    let list = records;
    if (selectedModel) list = list.filter((r) => r.modelId === selectedModel);
    if (selectedCharacter)
      list = list.filter((r) => r.characterId === selectedCharacter);
    return [...list].sort((a, b) => b.timestamp - a.timestamp);
  }, [records, selectedModel, selectedCharacter]);

  const { modelOptions, characterOptions, chartData } = useMemo(() => {
    const modelMap = new Map<string, { name: string; tokens: number; cost: number }>();
    const charMap = new Map<string, { name: string; tokens: number; cost: number }>();
    const dailyMap = new Map<
      string,
      { input: number; output: number; cost: number; date: Date }
    >();

    for (const r of filteredRecords) {
      if (r.modelId && r.modelName) {
        const e = modelMap.get(r.modelId) || { name: r.modelName, tokens: 0, cost: 0 };
        e.tokens += r.totalTokens || 0;
        e.cost += getEffectiveTotalCost(r);
        modelMap.set(r.modelId, e);
      }
      if (r.characterId && r.characterName) {
        const e = charMap.get(r.characterId) || { name: r.characterName, tokens: 0, cost: 0 };
        e.tokens += r.totalTokens || 0;
        e.cost += getEffectiveTotalCost(r);
        charMap.set(r.characterId, e);
      }
      const d = new Date(r.timestamp);
      const k = dayKeyFromDate(d);
      const day = dailyMap.get(k) || { input: 0, output: 0, cost: 0, date: d };
      day.input += r.promptTokens || 0;
      day.output += r.completionTokens || 0;
      day.cost += getEffectiveTotalCost(r);
      dailyMap.set(k, day);
    }

    const modelOptions = Array.from(modelMap.entries())
      .map(([id, d]) => ({ id, ...d }))
      .sort((a, b) => b.tokens - a.tokens);
    const characterOptions = Array.from(charMap.entries())
      .map(([id, d]) => ({ id, ...d }))
      .sort((a, b) => b.tokens - a.tokens);

    const days =
      datePreset === "today"
        ? 1
        : datePreset === "week"
          ? 7
          : datePreset === "month"
            ? 30
            : undefined;
    const sorted = Array.from(dailyMap.entries()).sort(
      (a, b) => a[1].date.getTime() - b[1].date.getTime(),
    );
    const chartData = (days !== undefined ? sorted.slice(-days) : sorted).map(
      ([, d]) => ({
        label: d.date.toLocaleDateString(undefined, { month: "short", day: "numeric" }),
        input: d.input,
        output: d.output,
        cost: d.cost,
      }),
    );

    return { modelOptions, characterOptions, chartData };
  }, [filteredRecords, datePreset]);

  const stats = useMemo(() => {
    const totals = filteredRecords.reduce(
      (acc, r) => {
        acc.tokens += r.totalTokens || 0;
        acc.cost += getEffectiveTotalCost(r);
        acc.requests += 1;
        return acc;
      },
      { tokens: 0, cost: 0, requests: 0 },
    );
    return {
      ...totals,
      avgPerRequest: totals.requests > 0 ? totals.tokens / totals.requests : 0,
    };
  }, [filteredRecords]);

  const appTime = useMemo(() => {
    const byDay = appActiveUsage.byDayMs ?? {};
    const today = new Date();

    const sumPrev = (offsetDays: number, n: number) => {
      let sum = 0;
      for (let i = offsetDays; i < offsetDays + n; i += 1) {
        const d = new Date(today);
        d.setDate(today.getDate() - i);
        sum += byDay[dayKeyFromDate(d)] ?? 0;
      }
      return sum;
    };
    const sumFrom = (start: Date, n: number) => {
      let sum = 0;
      const d = new Date(start);
      d.setHours(0, 0, 0, 0);
      for (let i = 0; i < n; i += 1) {
        sum += byDay[dayKeyFromDate(d)] ?? 0;
        d.setDate(d.getDate() + 1);
      }
      return sum;
    };

    const todayMs = sumPrev(0, 1);
    const yesterdayMs = sumPrev(1, 1);
    const avg30Ms = sumPrev(1, 30) / 30;

    const customKeys = (() => {
      const keys: string[] = [];
      const d = new Date(customStartDate);
      d.setHours(0, 0, 0, 0);
      const end = new Date(customEndDate);
      end.setHours(0, 0, 0, 0);
      while (d <= end) {
        keys.push(dayKeyFromDate(d));
        d.setDate(d.getDate() + 1);
      }
      return keys;
    })();

    const days =
      appTimePreset === "today"
        ? 1
        : appTimePreset === "week"
          ? 7
          : appTimePreset === "month"
            ? 30
            : appTimePreset === "custom"
              ? customKeys.length
              : undefined;

    const allKeys = Object.keys(byDay).sort();
    const chartKeys =
      days === undefined
        ? allKeys
        : appTimePreset === "custom"
          ? customKeys
          : Array.from({ length: days }, (_, i) => {
              const d = new Date(today);
              d.setDate(today.getDate() - (days - 1 - i));
              return dayKeyFromDate(d);
            });

    const byDayChart = chartKeys.map((key) => {
      const d = key.includes("-") ? new Date(`${key}T00:00:00`) : new Date();
      return {
        label: d.toLocaleDateString(undefined, { month: "short", day: "numeric" }),
        ms: byDay[key] ?? 0,
      };
    });

    const rangeTotalMs = byDayChart.reduce((s, x) => s + x.ms, 0);
    const selectedDays = Math.max(byDayChart.length, 1);
    const dailyAvgMs = rangeTotalMs / selectedDays;

    const prevTotalMs =
      days === undefined
        ? 0
        : appTimePreset === "custom"
          ? (() => {
              const prev = new Date(customStartDate);
              prev.setHours(0, 0, 0, 0);
              prev.setDate(prev.getDate() - selectedDays);
              return sumFrom(prev, selectedDays);
            })()
          : sumPrev(days, days);
    const deltaPct =
      prevTotalMs > 0
        ? ((rangeTotalMs - prevTotalMs) / prevTotalMs) * 100
        : rangeTotalMs > 0
          ? 100
          : 0;

    return {
      todayMs,
      yesterdayMs,
      avg30Ms,
      rangeTotalMs,
      selectedDays,
      dailyAvgMs,
      deltaPct,
      byDayChart,
    };
  }, [appActiveUsage, appTimePreset, customStartDate, customEndDate]);

  const handleExport = async () => {
    setExporting(true);
    try {
      const { start, end } = resolveDashboardRange();
      const csv = await exportCSV({
        startTimestamp: start.getTime(),
        endTimestamp: end.getTime(),
      });
      if (csv) {
        const fileName = `usage-${datePreset}-${new Date().toISOString().split("T")[0]}.csv`;
        const path = await saveCSV(csv, fileName);
        if (path) alert(t("usageAnalytics.page.exportedTo", { path }));
      }
    } catch (e) {
      console.error("Export failed:", e);
    } finally {
      setExporting(false);
    }
  };

  const activeFilterCount = [selectedModel, selectedCharacter].filter(Boolean).length;

  const presetOptions = useMemo(
    () => [
      { key: "today" as const, label: t("usageAnalytics.page.today") },
      { key: "week" as const, label: t("usageAnalytics.page.last7Days") },
      { key: "month" as const, label: t("usageAnalytics.page.last30Days") },
      { key: "all" as const, label: t("usageAnalytics.page.all") },
    ],
    [t],
  );

  const periodLabel =
    datePreset === "today"
      ? t("usageAnalytics.page.today")
      : datePreset === "week"
        ? t("usageAnalytics.page.last7d")
        : datePreset === "month"
          ? t("usageAnalytics.page.last30d")
          : datePreset === "custom"
            ? t("usageAnalytics.page.custom")
            : t("usageAnalytics.page.allTime");

  const tabs: Array<{ key: ViewMode; label: string; icon: React.ReactNode }> = [
    {
      key: "dashboard",
      label: t("usageAnalytics.page.dashboard"),
      icon: <Activity size={12} />,
    },
    {
      key: "appTime",
      label: t("usageAnalytics.page.appTime"),
      icon: <Clock size={12} />,
    },
    {
      key: "nanogpt",
      label: t("usageAnalytics.page.nanoGpt.tab"),
      icon: <Gauge size={12} />,
    },
  ];

  return (
    <div className="mx-auto flex w-full max-w-[1280px] flex-col gap-5 px-5 pb-24 pt-5 sm:px-8">
      <UsageRequestDetailSheet
        request={selectedRequest}
        isOpen={selectedRequest !== null}
        onClose={() => setSelectedRequest(null)}
      />

      {/* ---- Header ---- */}
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-[16px] font-semibold tracking-tight text-fg">
            {t("common.nav.usageAnalytics")}
          </h1>
          <p className="mt-1 text-[12.5px] text-fg/50">
            {viewMode === "dashboard"
              ? t("usageAnalytics.page.tokenConsumptionOverTime")
              : viewMode === "appTime"
                ? t("usageAnalytics.page.usageDurationPerDay")
                : t("usageAnalytics.page.nanoGpt.subtitle")}
          </p>
        </div>

        <div className="flex items-center gap-1 rounded-full border border-fg/8 bg-fg/[0.025] p-0.5">
          {tabs.map(({ key, label, icon }) => {
            const active = viewMode === key;
            return (
              <button
                key={key}
                onClick={() => setViewMode(key)}
                className={cn(
                  "relative flex items-center gap-1.5 rounded-full px-3.5 py-1.5 text-[12px] font-medium transition-colors",
                  active ? "text-fg" : "text-fg/55 hover:text-fg/85",
                )}
              >
                {active && (
                  <motion.span
                    layoutId="usageTabIndicator"
                    className="absolute inset-0 rounded-full bg-fg/[0.09] ring-1 ring-inset ring-fg/15"
                    transition={{ type: "spring", stiffness: 420, damping: 34, mass: 0.6 }}
                  />
                )}
                <span className="relative z-10 flex items-center gap-1.5">
                  {icon}
                  {label}
                </span>
              </button>
            );
          })}
        </div>
      </header>

      {/* ---- Toolbar ---- */}
      {viewMode !== "nanogpt" && (
        <div className="flex flex-wrap items-center justify-between gap-3">
          <PresetPills
            value={viewMode === "dashboard" ? datePreset : appTimePreset}
            onChange={(v) =>
              viewMode === "dashboard" ? setDatePreset(v) : setAppTimePreset(v)
            }
            onCustom={() => setShowCustomDatePicker(true)}
            options={presetOptions}
            customLabel={t("usageAnalytics.page.custom")}
            customActive={
              viewMode === "dashboard"
                ? datePreset === "custom"
                : appTimePreset === "custom"
            }
          />

          <div className="flex items-center gap-2">
            {viewMode === "dashboard" && (
              <button
                onClick={() => setShowFilters(true)}
                className={cn(
                  "inline-flex items-center gap-1.5 rounded-md border px-2.5 py-1.5 text-[11.5px] font-medium transition",
                  activeFilterCount > 0
                    ? "border-accent/30 bg-accent/10 text-accent"
                    : "border-fg/8 bg-fg/[0.02] text-fg/60 hover:border-fg/15 hover:bg-fg/5 hover:text-fg",
                )}
              >
                <Filter size={12} />
                {activeFilterCount > 0
                  ? t("usageAnalytics.page.filtersCount", { count: activeFilterCount })
                  : t("usageAnalytics.page.filters")}
              </button>
            )}

            {viewMode === "dashboard" && (
              <IconButton
                icon={<Download size={14} />}
                label={t("common.buttons.export")}
                onClick={() => void handleExport()}
                disabled={exporting || filteredRecords.length === 0}
                spinning={exporting}
              />
            )}

            <IconButton
              icon={<RefreshCw size={14} />}
              label={t("common.buttons.refresh")}
              onClick={() => {
                if (viewMode === "dashboard") void loadDashboard("refresh");
                else void loadAppUsage();
              }}
              disabled={refreshing}
              spinning={refreshing}
            />
          </div>
        </div>
      )}

      {/* ---- Body ---- */}
      <AnimatePresence mode="wait">
        {viewMode === "dashboard" ? (
          <motion.div
            key="dashboard"
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -4 }}
            transition={{ duration: 0.15 }}
            className="flex flex-col gap-5"
          >
            {/* KPI strip */}
            <div className="grid grid-cols-2 gap-2.5 lg:grid-cols-4">
              <StatTile
                label={t("usageAnalytics.page.totalCost")}
                value={formatCurrency(stats.cost)}
                sub={periodLabel}
                highlight
              />
              <StatTile
                label={t("usageAnalytics.page.tokens")}
                value={formatCompactNumber(stats.tokens)}
                sub={t("usageAnalytics.page.avgShort", {
                  value: formatCompactNumber(Math.round(stats.avgPerRequest)),
                })}
              />
              <StatTile
                label={t("usageAnalytics.page.requests")}
                value={stats.requests.toLocaleString()}
                sub={t("usageAnalytics.page.recordsCount", {
                  count: filteredRecords.length,
                })}
              />
              <StatTile
                label={t("usageAnalytics.page.period")}
                value={periodLabel}
                sub={
                  datePreset === "custom"
                    ? `${customStartDate.toLocaleDateString()} → ${customEndDate.toLocaleDateString()}`
                    : ""
                }
              />
            </div>

            {/* Loading or empty */}
            {loading ? (
              <div className="flex items-center justify-center py-20">
                <Loader2 className="h-5 w-5 animate-spin text-fg/30" />
              </div>
            ) : filteredRecords.length === 0 ? (
              <EmptyState
                icon={<Calendar size={18} className="text-fg/45" />}
                title={t("common.labels.none")}
                description={t("usageAnalytics.page.startChatting")}
              />
            ) : (
              <>
                {/* Chart */}
                {chartData.length > 1 && (
                  <SectionCard
                    title={t("usageAnalytics.page.usageTrend")}
                    subtitle={t("usageAnalytics.page.tokenConsumptionOverTime")}
                    right={
                      <div className="flex items-center gap-3 text-[10.5px] uppercase tracking-[0.1em] text-fg/45">
                        <span className="flex items-center gap-1.5">
                          <span className="h-1.5 w-1.5 rounded-full bg-info" />
                          {t("usageAnalytics.page.input")}
                        </span>
                        <span className="flex items-center gap-1.5">
                          <span className="h-1.5 w-1.5 rounded-full bg-accent" />
                          {t("usageAnalytics.page.output")}
                        </span>
                      </div>
                    }
                  >
                    <div className="h-56">
                      <ResponsiveContainer width="100%" height="100%">
                        <AreaChart data={chartData} margin={{ top: 4, right: 4, left: -16, bottom: 0 }}>
                          <defs>
                            <linearGradient id="usageInputGrad" x1="0" y1="0" x2="0" y2="1">
                              <stop offset="0%" stopColor="var(--color-info)" stopOpacity={0.28} />
                              <stop offset="100%" stopColor="var(--color-info)" stopOpacity={0} />
                            </linearGradient>
                            <linearGradient id="usageOutputGrad" x1="0" y1="0" x2="0" y2="1">
                              <stop offset="0%" stopColor="var(--color-accent)" stopOpacity={0.3} />
                              <stop offset="100%" stopColor="var(--color-accent)" stopOpacity={0} />
                            </linearGradient>
                          </defs>
                          <CartesianGrid
                            stroke="rgba(255,255,255,0.04)"
                            vertical={false}
                          />
                          <XAxis
                            dataKey="label"
                            tick={{ fill: "rgba(255,255,255,0.32)", fontSize: 10 }}
                            axisLine={false}
                            tickLine={false}
                            dy={6}
                          />
                          <YAxis
                            tick={{ fill: "rgba(255,255,255,0.32)", fontSize: 10 }}
                            axisLine={false}
                            tickLine={false}
                            tickFormatter={formatCompactNumber}
                            width={48}
                          />
                          <Tooltip
                            content={<TokenTooltip />}
                            cursor={{ stroke: "rgba(255,255,255,0.08)", strokeWidth: 1 }}
                          />
                          <Area
                            type="monotone"
                            dataKey="input"
                            name={t("usageAnalytics.page.input")}
                            stroke="var(--color-info)"
                            fill="url(#usageInputGrad)"
                            strokeWidth={1.75}
                          />
                          <Area
                            type="monotone"
                            dataKey="output"
                            name={t("usageAnalytics.page.output")}
                            stroke="var(--color-accent)"
                            fill="url(#usageOutputGrad)"
                            strokeWidth={1.75}
                          />
                        </AreaChart>
                      </ResponsiveContainer>
                    </div>
                  </SectionCard>
                )}

                {/* Breakdowns */}
                <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
                  <SectionCard
                    title={t("usageAnalytics.page.byModel")}
                    subtitle={`${modelOptions.length} ${t("usageAnalytics.page.tokens").toLowerCase()}`}
                  >
                    <RankedBarList
                      items={modelOptions.slice(0, 5)}
                      emptyLabel={t("common.labels.none")}
                      unit={t("usageAnalytics.page.tokens").toLowerCase()}
                    />
                  </SectionCard>
                  <SectionCard
                    title={t("usageAnalytics.page.byCharacter")}
                    subtitle={`${characterOptions.length} ${t("usageAnalytics.page.active").toLowerCase()}`}
                  >
                    <RankedBarList
                      items={characterOptions.slice(0, 5)}
                      emptyLabel={t("common.labels.none")}
                      unit={t("usageAnalytics.page.tokens").toLowerCase()}
                    />
                  </SectionCard>
                </div>

                {/* Recent Activity */}
                <SectionCard
                  title={t("usageAnalytics.page.recentActivity")}
                  subtitle={t("usageAnalytics.page.recordsCount", {
                    count: filteredRecords.length,
                  })}
                  right={
                    filteredRecords.length > 8 && (
                      <button
                        onClick={() => navigate("/settings/usage/activity")}
                        className="inline-flex items-center gap-1 text-[11.5px] font-medium text-accent transition hover:opacity-80"
                      >
                        {t("usageAnalytics.page.viewAll")}
                        <ChevronRight size={12} />
                      </button>
                    )
                  }
                  bodyClassName="!p-0"
                >
                  {/* Desktop column header */}
                  <div className="hidden grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)_88px_88px_92px_16px] items-center gap-3 border-b border-fg/[0.05] px-3 py-2 text-[10px] font-semibold uppercase tracking-[0.12em] text-fg/35 lg:grid">
                    <div>{t("common.labels.name")}</div>
                    <div>{t("usageAnalytics.page.modelColumn")}</div>
                    <div className="text-right">{t("usageAnalytics.page.tokens")}</div>
                    <div className="text-right">{t("usageAnalytics.page.totalCost")}</div>
                    <div className="text-right">{t("usageAnalytics.page.timeColumn")}</div>
                    <div />
                  </div>
                  {filteredRecords.slice(0, 8).map((r) => (
                    <ActivityRow
                      key={r.id}
                      request={r}
                      onClick={setSelectedRequest}
                      t={t}
                    />
                  ))}
                </SectionCard>
              </>
            )}
          </motion.div>
        ) : viewMode === "appTime" ? (
          <motion.div
            key="appTime"
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -4 }}
            transition={{ duration: 0.15 }}
            className="flex flex-col gap-5"
          >
            <div className="grid grid-cols-2 gap-2.5 lg:grid-cols-4">
              <StatTile
                label={t("usageAnalytics.page.periodTotal")}
                value={formatDurationMs(appTime.rangeTotalMs)}
                sub={t("usageAnalytics.page.daysActive", {
                  count: appTime.selectedDays,
                })}
                trend={{ value: appTime.deltaPct, isUp: appTime.deltaPct >= 0 }}
                highlight
              />
              <StatTile
                label={t("usageAnalytics.page.dailyAvg")}
                value={formatDurationMs(appTime.dailyAvgMs)}
                sub={t("usageAnalytics.page.selectedPeriod")}
              />
              <StatTile
                label={t("usageAnalytics.page.today")}
                value={formatDurationMs(appTime.todayMs)}
                sub={t("usageAnalytics.page.yesterdayValue", {
                  value: formatDurationMs(appTime.yesterdayMs),
                })}
              />
              <StatTile
                label={t("usageAnalytics.page.thirtyDayAvg")}
                value={formatDurationMs(appTime.avg30Ms)}
              />
            </div>

            <SectionCard
              title={t("usageAnalytics.page.appTimeTrend")}
              subtitle={t("usageAnalytics.page.usageDurationPerDay")}
              right={
                <span className="rounded-md border border-fg/8 bg-fg/[0.04] px-2 py-0.5 text-[10.5px] font-medium tabular-nums text-fg/55">
                  {t("usageAnalytics.page.totalValue", {
                    value: formatDurationMs(appTime.rangeTotalMs),
                  })}
                </span>
              }
            >
              <div className="h-56">
                <ResponsiveContainer width="100%" height="100%">
                  <AreaChart
                    data={appTime.byDayChart}
                    margin={{ top: 4, right: 4, left: -16, bottom: 0 }}
                  >
                    <defs>
                      <linearGradient id="appTimeGrad" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0%" stopColor="var(--color-accent)" stopOpacity={0.3} />
                        <stop offset="100%" stopColor="var(--color-accent)" stopOpacity={0} />
                      </linearGradient>
                    </defs>
                    <CartesianGrid stroke="rgba(255,255,255,0.04)" vertical={false} />
                    <XAxis
                      dataKey="label"
                      tick={{ fill: "rgba(255,255,255,0.32)", fontSize: 10 }}
                      axisLine={false}
                      tickLine={false}
                      dy={6}
                    />
                    <YAxis
                      tick={{ fill: "rgba(255,255,255,0.32)", fontSize: 10 }}
                      axisLine={false}
                      tickLine={false}
                      tickFormatter={(v) => formatDurationMs(v)}
                      width={56}
                    />
                    <Tooltip
                      content={<TimeTooltip />}
                      cursor={{ stroke: "rgba(255,255,255,0.08)", strokeWidth: 1 }}
                    />
                    <Area
                      type="monotone"
                      dataKey="ms"
                      name={t("usageAnalytics.page.activeTime")}
                      stroke="var(--color-accent)"
                      fill="url(#appTimeGrad)"
                      strokeWidth={1.75}
                    />
                  </AreaChart>
                </ResponsiveContainer>
              </div>
            </SectionCard>
          </motion.div>
        ) : (
          <motion.div
            key="nanogpt"
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -4 }}
            transition={{ duration: 0.15 }}
          >
            <NanoGptUsagePanel />
          </motion.div>
        )}
      </AnimatePresence>

      {/* ---- Filter sheet ---- */}
      <BottomMenu
        isOpen={showFilters}
        onClose={() => setShowFilters(false)}
        title={t("usageAnalytics.page.filters")}
        includeExitIcon={false}
      >
        <div className="space-y-5 pb-6">
          <FilterGroup
            label={t("usageAnalytics.page.model")}
            options={modelOptions.slice(0, 12)}
            selected={selectedModel}
            onToggle={(id) => setSelectedModel((p) => (p === id ? null : id))}
          />
          <FilterGroup
            label={t("usageAnalytics.page.character")}
            options={characterOptions.slice(0, 12)}
            selected={selectedCharacter}
            onToggle={(id) => setSelectedCharacter((p) => (p === id ? null : id))}
          />
          <div className="flex gap-2 pt-1">
            <button
              onClick={() => {
                setSelectedModel(null);
                setSelectedCharacter(null);
              }}
              className="flex-1 rounded-lg border border-fg/10 bg-fg/[0.02] py-2.5 text-[12.5px] font-medium text-fg/60 transition hover:bg-fg/5 hover:text-fg"
            >
              {t("usageAnalytics.page.clearAll")}
            </button>
            <button
              onClick={() => setShowFilters(false)}
              className="flex-1 rounded-lg bg-accent py-2.5 text-[12.5px] font-semibold text-black transition hover:brightness-110"
            >
              {t("usageAnalytics.page.applyFilters")}
            </button>
          </div>
        </div>
      </BottomMenu>

      {/* ---- Custom date sheet ---- */}
      <BottomMenu
        isOpen={showCustomDatePicker}
        onClose={() => setShowCustomDatePicker(false)}
        title={t("usageAnalytics.page.customRange")}
        includeExitIcon={false}
      >
        <div className="space-y-5 pb-6">
          <div className="grid grid-cols-2 gap-3">
            <DateField
              label={t("usageAnalytics.page.startDate")}
              value={customStartDate}
              onChange={(d) => {
                d.setHours(0, 0, 0, 0);
                setCustomStartDate(d);
              }}
              max={customEndDate}
            />
            <DateField
              label={t("usageAnalytics.page.endDate")}
              value={customEndDate}
              onChange={(d) => {
                d.setHours(23, 59, 59, 999);
                setCustomEndDate(d);
              }}
              min={customStartDate}
              maxToday
            />
          </div>
          <div className="flex gap-2">
            <button
              onClick={() => setShowCustomDatePicker(false)}
              className="flex-1 rounded-lg border border-fg/10 bg-fg/[0.02] py-2.5 text-[12.5px] font-medium text-fg/60 transition hover:bg-fg/5 hover:text-fg"
            >
              {t("common.buttons.cancel")}
            </button>
            <button
              onClick={() => {
                if (viewMode === "appTime") setAppTimePreset("custom");
                else setDatePreset("custom");
                setShowCustomDatePicker(false);
              }}
              className="flex-1 rounded-lg bg-accent py-2.5 text-[12.5px] font-semibold text-black transition hover:brightness-110"
            >
              {t("usageAnalytics.page.applyRange")}
            </button>
          </div>
        </div>
      </BottomMenu>

    </div>
  );
}

// =============================================================================
// Sub-components used by the sheets
// =============================================================================

function FilterGroup({
  label,
  options,
  selected,
  onToggle,
}: {
  label: string;
  options: Array<{ id: string; name: string; tokens: number }>;
  selected: string | null;
  onToggle: (id: string) => void;
}) {
  const { t } = useI18n();
  return (
    <div>
      <div className="mb-2 flex items-center justify-between">
        <span className="text-[10px] font-semibold uppercase tracking-[0.12em] text-fg/40">
          {label}
        </span>
        {selected && (
          <button
            onClick={() => onToggle(selected)}
            className="inline-flex items-center gap-1 text-[11px] text-fg/45 transition hover:text-fg/75"
          >
            <X size={11} />
            {t("usageAnalytics.page.clear")}
          </button>
        )}
      </div>
      {options.length === 0 ? (
        <div className="rounded-lg border border-dashed border-fg/10 bg-fg/[0.02] py-4 text-center text-[12px] text-fg/40">
          —
        </div>
      ) : (
        <div className="max-h-56 space-y-1 overflow-y-auto pr-1">
          {options.map((o) => {
            const active = selected === o.id;
            return (
              <button
                key={o.id}
                onClick={() => onToggle(o.id)}
                className={cn(
                  "flex w-full items-center justify-between gap-3 rounded-md border px-3 py-2 text-left transition",
                  active
                    ? "border-accent/30 bg-accent/10 text-accent"
                    : "border-fg/8 bg-fg/[0.02] text-fg/70 hover:border-fg/15 hover:bg-fg/5",
                )}
              >
                <span className="truncate text-[12.5px] font-medium">{o.name}</span>
                <span className="shrink-0 text-[11px] tabular-nums text-fg/45">
                  {formatCompactNumber(o.tokens)}
                </span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

function DateField({
  label,
  value,
  onChange,
  min,
  max,
  maxToday,
}: {
  label: string;
  value: Date;
  onChange: (d: Date) => void;
  min?: Date;
  max?: Date;
  maxToday?: boolean;
}) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-[10px] font-semibold uppercase tracking-[0.12em] text-fg/40">
        {label}
      </span>
      <div className="relative">
        <input
          type="date"
          value={value.toISOString().split("T")[0]}
          onChange={(e) => onChange(new Date(e.target.value))}
          min={min ? min.toISOString().split("T")[0] : undefined}
          max={
            maxToday
              ? new Date().toISOString().split("T")[0]
              : max
                ? max.toISOString().split("T")[0]
                : undefined
          }
          className="h-10 w-full rounded-lg border border-fg/10 bg-fg/[0.02] px-3 text-[13px] text-fg outline-none transition focus:border-accent/40 focus:bg-fg/[0.05] focus:ring-1 focus:ring-accent/20"
        />
        <ChevronDown
          size={12}
          className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-fg/30"
        />
      </div>
    </label>
  );
}

function EmptyState({
  icon,
  title,
  description,
}: {
  icon: React.ReactNode;
  title: string;
  description: string;
}) {
  return (
    <div className="rounded-xl border border-dashed border-fg/10 bg-fg/[0.02] px-4 py-12 text-center">
      <div className="mx-auto mb-3 flex h-11 w-11 items-center justify-center rounded-lg border border-fg/10 bg-fg/[0.04]">
        {icon}
      </div>
      <div className="text-[13px] font-medium text-fg">{title}</div>
      <p className="mx-auto mt-1 max-w-md text-[12.5px] text-fg/55">{description}</p>
    </div>
  );
}
