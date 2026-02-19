import { useState, useEffect, useMemo } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  PieChart,
  Pie,
  Cell,
} from "recharts";
import {
  useUsageTracking,
  RequestUsage,
  UsageFilter,
  AppActiveUsageSummary,
} from "../../../core/usage";
import {
  Download,
  TrendingUp,
  TrendingDown,
  Zap,
  DollarSign,
  Activity,
  Clock,
  Filter,
  X,
  ChevronRight,
  Calendar,
} from "lucide-react";
import { BottomMenu } from "../../components";

// ============================================================================
// Utilities
// ============================================================================

function formatCurrency(value: number): string {
  if (value === 0) return "$0.00";
  if (value < 0.01) return `$${value.toFixed(4)}`;
  if (value < 1) return `$${value.toFixed(3)}`;
  return `$${value.toFixed(2)}`;
}

function formatNumber(value: number): string {
  if (value === 0) return "0";
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
  return value.toLocaleString();
}

function formatCompactNumber(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(0)}K`;
  return value.toString();
}

function getRelativeTime(timestamp: number): string {
  const now = Date.now();
  const diff = now - timestamp;
  const minutes = Math.floor(diff / 60000);
  const hours = Math.floor(diff / 3600000);
  const days = Math.floor(diff / 86400000);

  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m ago`;
  if (hours < 24) return `${hours}h ago`;
  if (days < 7) return `${days}d ago`;
  return new Date(timestamp).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

function getOperationColor(type: string): string {
  const colors: Record<string, string> = {
    chat: "var(--color-info)",
    regenerate: "var(--color-secondary)",
    continue: "#22d3ee",
    summary: "var(--color-warning)",
    memory_manager: "var(--color-accent)",
  };
  return colors[type.toLowerCase()] || "#94a3b8";
}

function getOperationLabel(type: string): string {
  const labels: Record<string, string> = {
    chat: "Chat",
    regenerate: "Regen",
    continue: "Continue",
    summary: "Summary",
    memory_manager: "Memory",
  };
  return labels[type.toLowerCase()] || type;
}

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
  if (seconds > 0) return `${seconds}s`;
  return `${minutes}m`;
}

function dayKeyFromDate(date: Date): string {
  const y = date.getFullYear();
  const m = `${date.getMonth() + 1}`.padStart(2, "0");
  const d = `${date.getDate()}`.padStart(2, "0");
  return `${y}-${m}-${d}`;
}

// ============================================================================
// Date Range Presets
// ============================================================================

type DatePreset = "today" | "week" | "month" | "all" | "custom";

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
      start.setDate(start.getDate() - 7);
      break;
    case "month":
      start.setDate(start.getDate() - 30);
      break;
    case "all":
      start.setFullYear(start.getFullYear() - 10);
      break;
    case "custom":
      // Custom range will be handled separately
      break;
  }

  return { start, end };
}

// ============================================================================
// Chart Tooltip
// ============================================================================

const ChartTooltip = ({ active, payload, label }: any) => {
  if (!active || !payload?.length) return null;

  return (
    <div className="max-w-[70vw] rounded-lg border border-fg/15 bg-surface/70 backdrop-blur-md px-2.5 py-2 shadow-xl">
      <p className="text-[10px] font-medium text-fg/60 mb-1">{label}</p>
      {payload.map((p: any, i: number) => (
        <div key={i} className="flex items-center gap-1.5 text-[11px]">
          <span className="h-2 w-2 rounded-full" style={{ backgroundColor: p.color }} />
          <span className="text-fg/60">{p.name}:</span>
          <span className="text-fg font-medium">{formatNumber(p.value)}</span>
        </div>
      ))}
    </div>
  );
};

const AppTimeTooltip = ({ active, payload, label }: any) => {
  if (!active || !payload?.length) return null;

  return (
    <div className="max-w-[70vw] rounded-lg border border-fg/15 bg-surface/70 backdrop-blur-md px-2.5 py-2 shadow-xl">
      <p className="text-[10px] font-medium text-fg/60 mb-1">{label}</p>
      {payload.map((p: any, i: number) => (
        <div key={i} className="flex items-center gap-1.5 text-[11px]">
          <span className="h-2 w-2 rounded-full" style={{ backgroundColor: p.color }} />
          <span className="text-fg/60">{p.name}:</span>
          <span className="text-fg font-medium">{formatDurationMs(p.value)}</span>
        </div>
      ))}
    </div>
  );
};

// ============================================================================
// Stat Card
// ============================================================================

function StatCard({
  icon: Icon,
  label,
  value,
  subValue,
  trend,
  highlight,
}: {
  icon: any;
  label: string;
  value: string;
  subValue?: string;
  trend?: { value: number; isUp: boolean } | null;
  highlight?: boolean;
}) {
  return (
    <div
      className={`rounded-2xl border p-4 ${
        highlight
          ? "border-accent/30 bg-linear-to-br from-accent/20 via-accent/10 to-transparent"
          : "border-fg/10 bg-fg/5"
      }`}
    >
      <div className="flex items-center gap-2 mb-2">
        <div className={`p-1.5 rounded-lg ${highlight ? "bg-accent/20" : "bg-fg/5"}`}>
          <Icon className={`h-3.5 w-3.5 ${highlight ? "text-accent" : "text-fg/50"}`} />
        </div>
        <span className="text-[11px] font-medium text-fg/50 uppercase tracking-wide">
          {label}
        </span>
      </div>
      <div className="flex items-end justify-between">
        <div>
          <p className={`text-2xl font-bold ${highlight ? "text-accent/90" : "text-fg"}`}>
            {value}
          </p>
          {subValue && <p className="text-[11px] text-fg/40 mt-0.5">{subValue}</p>}
        </div>
        {trend && trend.value > 0 && (
          <div
            className={`flex items-center gap-0.5 rounded-full px-1.5 py-0.5 text-[10px] font-medium ${
              trend.isUp ? "bg-accent/15 text-accent" : "bg-danger/15 text-danger"
            }`}
          >
            {trend.isUp ? <TrendingUp className="h-3 w-3" /> : <TrendingDown className="h-3 w-3" />}
            {trend.value.toFixed(0)}%
          </div>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// Activity Item
// ============================================================================

function ActivityItem({ request }: { request: RequestUsage }) {
  return (
    <div className="flex items-center gap-3 px-3 py-2.5 rounded-xl hover:bg-fg/5 transition-colors">
      <div
        className="h-8 w-8 rounded-full flex items-center justify-center shrink-0"
        style={{ backgroundColor: `${getOperationColor(request.operationType)}20` }}
      >
        <Zap className="h-3.5 w-3.5" style={{ color: getOperationColor(request.operationType) }} />
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-fg truncate">
            {request.characterName || "Unknown"}
          </span>
          <span
            className="text-[10px] font-medium px-1.5 py-0.5 rounded-full"
            style={{
              backgroundColor: `${getOperationColor(request.operationType)}20`,
              color: getOperationColor(request.operationType),
            }}
          >
            {getOperationLabel(request.operationType)}
          </span>
        </div>
        <div className="flex items-center gap-2 text-[11px] text-fg/40">
          <span>{formatCompactNumber(request.totalTokens || 0)} tokens</span>
          <span>·</span>
          <span>{getRelativeTime(request.timestamp)}</span>
        </div>
      </div>
      <div className="text-right shrink-0">
        <p className="text-sm font-medium text-accent">
          {formatCurrency(request.cost?.totalCost || 0)}
        </p>
        <p className="text-[10px] text-fg/30">{request.modelName}</p>
      </div>
    </div>
  );
}

// ============================================================================
// Main Component
// ============================================================================

export function UsagePage() {
  const { queryRecords, exportCSV, saveCSV, getAppActiveUsage } = useUsageTracking();
  const [appActiveUsage, setAppActiveUsage] = useState<AppActiveUsageSummary>({
    totalMs: 0,
    byDayMs: {},
  });

  // View mode
  const [viewMode, setViewMode] = useState<"dashboard" | "appTime">("dashboard");

  // Dashboard state
  const [datePreset, setDatePreset] = useState<DatePreset>("month");
  const [records, setRecords] = useState<RequestUsage[]>([]);
  const [loading, setLoading] = useState(true);
  const [showFilters, setShowFilters] = useState(false);
  const [showAllActivity, setShowAllActivity] = useState(false);
  const [exporting, setExporting] = useState(false);

  // Custom date range
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
  const [showCustomDatePicker, setShowCustomDatePicker] = useState(false);
  const [appTimePreset, setAppTimePreset] = useState<"today" | "week" | "month" | "all" | "custom">(
    "week",
  );

  // Filters
  const [selectedModel, setSelectedModel] = useState<string | null>(null);
  const [selectedCharacter, setSelectedCharacter] = useState<string | null>(null);

  // Load dashboard data
  const loadDashboardData = async () => {
    setLoading(true);
    let start: Date, end: Date;

    if (datePreset === "custom") {
      start = customStartDate;
      end = customEndDate;
    } else {
      const range = getDateRange(datePreset);
      start = range.start;
      end = range.end;
    }

    const filter: UsageFilter = {
      startTimestamp: start.getTime(),
      endTimestamp: end.getTime(),
    };
    const newRecords = await queryRecords(filter);
    if (newRecords) setRecords(newRecords);
    setLoading(false);
  };

  const loadAppUsageInfo = async () => {
    const summary = await getAppActiveUsage();
    if (!summary) return;
    setAppActiveUsage(summary);
  };

  useEffect(() => {
    if (viewMode === "dashboard") {
      loadDashboardData();
    }
  }, [datePreset, viewMode, customStartDate, customEndDate]);

  useEffect(() => {
    loadAppUsageInfo();
  }, []);

  useEffect(() => {
    if (viewMode !== "appTime") return;
    loadAppUsageInfo();
    const interval = window.setInterval(() => {
      loadAppUsageInfo();
    }, 15000);
    return () => window.clearInterval(interval);
  }, [viewMode]);

  // Filtered records for dashboard
  const filteredRecords = useMemo(() => {
    let list = records;
    if (selectedModel) list = list.filter((r) => r.modelId === selectedModel);
    if (selectedCharacter) list = list.filter((r) => r.characterId === selectedCharacter);
    return list.sort((a, b) => b.timestamp - a.timestamp);
  }, [records, selectedModel, selectedCharacter]);

  // Dashboard derived data
  const { modelOptions, characterOptions, chartData, topModels } = useMemo(() => {
    const modelMap = new Map<string, { name: string; tokens: number; cost: number }>();
    const charMap = new Map<string, { name: string; tokens: number; cost: number }>();
    const dailyMap = new Map<string, { input: number; output: number; cost: number; date: Date }>();

    for (const r of filteredRecords) {
      if (r.modelId && r.modelName) {
        const existing = modelMap.get(r.modelId) || { name: r.modelName, tokens: 0, cost: 0 };
        existing.tokens += r.totalTokens || 0;
        existing.cost += r.cost?.totalCost || 0;
        modelMap.set(r.modelId, existing);
      }
      if (r.characterId && r.characterName) {
        const existing = charMap.get(r.characterId) || {
          name: r.characterName,
          tokens: 0,
          cost: 0,
        };
        existing.tokens += r.totalTokens || 0;
        existing.cost += r.cost?.totalCost || 0;
        charMap.set(r.characterId, existing);
      }
      const recordDate = new Date(r.timestamp);
      // Use date string as key (YYYY-MM-DD format ensures uniqueness)
      const dateKey = recordDate.toISOString().split("T")[0];
      const dayData = dailyMap.get(dateKey) || { input: 0, output: 0, cost: 0, date: recordDate };
      dayData.input += r.promptTokens || 0;
      dayData.output += r.completionTokens || 0;
      dayData.cost += r.cost?.totalCost || 0;
      dailyMap.set(dateKey, dayData);
    }

    const modelOptions = Array.from(modelMap.entries())
      .map(([id, data]) => ({ id, ...data }))
      .sort((a, b) => b.tokens - a.tokens);

    const characterOptions = Array.from(charMap.entries())
      .map(([id, data]) => ({ id, ...data }))
      .sort((a, b) => b.tokens - a.tokens);

    // Determine how many days to show based on the date preset
    const daysToShow =
      datePreset === "today"
        ? 1
        : datePreset === "week"
          ? 7
          : datePreset === "month"
            ? 30
            : datePreset === "all"
              ? undefined // Show all data
              : datePreset === "custom"
                ? undefined // Show all data for custom range
                : 14; // Default fallback

    const sortedEntries = Array.from(dailyMap.entries()).sort(
      (a, b) => a[1].date.getTime() - b[1].date.getTime(),
    );

    const chartData = (
      daysToShow !== undefined ? sortedEntries.slice(-daysToShow) : sortedEntries
    ).map(([, data]) => ({
      label: data.date.toLocaleDateString(undefined, { month: "short", day: "numeric" }),
      input: data.input,
      output: data.output,
      cost: data.cost,
    }));

    const topModels = modelOptions.slice(0, 5);

    return { modelOptions, characterOptions, chartData, topModels };
  }, [filteredRecords, datePreset]);

  // Dashboard stats
  const displayStats = useMemo(() => {
    const totals = filteredRecords.reduce(
      (acc, r) => {
        acc.tokens += r.totalTokens || 0;
        acc.cost += r.cost?.totalCost || 0;
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

  const appTimeStats = useMemo(() => {
    const byDay = appActiveUsage.byDayMs ?? {};
    const today = new Date();
    const sumPreviousDays = (startOffsetDays: number, days: number) => {
      let sum = 0;
      for (let i = startOffsetDays; i < startOffsetDays + days; i += 1) {
        const d = new Date(today);
        d.setDate(today.getDate() - i);
        const key = dayKeyFromDate(d);
        sum += byDay[key] ?? 0;
      }
      return sum;
    };
    const todayMs = sumPreviousDays(0, 1);
    const yesterdayMs = sumPreviousDays(1, 1);
    const avg3Ms = sumPreviousDays(1, 3) / 3;
    const avg7Ms = sumPreviousDays(1, 7) / 7;
    const avg30Ms = sumPreviousDays(1, 30) / 30;

    const sumRangeFromDate = (start: Date, days: number) => {
      let sum = 0;
      const d = new Date(start);
      d.setHours(0, 0, 0, 0);
      for (let i = 0; i < days; i += 1) {
        const key = dayKeyFromDate(d);
        sum += byDay[key] ?? 0;
        d.setDate(d.getDate() + 1);
      }
      return sum;
    };

    const customRangeKeys = (() => {
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

    const daysToShow =
      appTimePreset === "today"
        ? 1
        : appTimePreset === "week"
          ? 7
          : appTimePreset === "month"
            ? 30
            : appTimePreset === "custom"
              ? customRangeKeys.length
              : undefined;
    const allDayKeysSorted = Object.keys(byDay).sort();
    const chartKeys =
      daysToShow === undefined
        ? allDayKeysSorted
        : appTimePreset === "custom"
          ? customRangeKeys
          : Array.from({ length: daysToShow }, (_, idx) => {
              const d = new Date(today);
              d.setDate(today.getDate() - (daysToShow - 1 - idx));
              return dayKeyFromDate(d);
            });

    const byDayChart = chartKeys.map((key) => {
      const d = key.includes("-") ? new Date(`${key}T00:00:00`) : new Date();
      return {
        label: d.toLocaleDateString(undefined, { month: "short", day: "numeric" }),
        ms: byDay[key] ?? 0,
      };
    });
    const rangeTotalMs = byDayChart.reduce((sum, item) => sum + item.ms, 0);
    const selectedDays = Math.max(byDayChart.length, 1);
    const dailyAvgInRangeMs = rangeTotalMs / selectedDays;
    const prevRangeTotalMs =
      daysToShow === undefined
        ? 0
        : appTimePreset === "custom"
          ? (() => {
              const prevStart = new Date(customStartDate);
              prevStart.setHours(0, 0, 0, 0);
              prevStart.setDate(prevStart.getDate() - selectedDays);
              return sumRangeFromDate(prevStart, selectedDays);
            })()
          : sumPreviousDays(daysToShow, daysToShow);
    const rangeDeltaMs = rangeTotalMs - prevRangeTotalMs;
    const rangeDeltaPct =
      prevRangeTotalMs > 0 ? (rangeDeltaMs / prevRangeTotalMs) * 100 : rangeTotalMs > 0 ? 100 : 0;

    return {
      todayMs,
      yesterdayMs,
      avg3Ms,
      avg7Ms,
      avg30Ms,
      rangeTotalMs,
      selectedDays,
      dailyAvgInRangeMs,
      prevRangeTotalMs,
      rangeDeltaMs,
      rangeDeltaPct,
      byDayChart,
    };
  }, [appActiveUsage, appTimePreset, customStartDate, customEndDate]);

  // Export handler
  const handleExport = async () => {
    setExporting(true);
    try {
      const { start, end } = getDateRange(datePreset);
      const csv = await exportCSV({
        startTimestamp: start.getTime(),
        endTimestamp: end.getTime(),
      });
      if (csv) {
        const fileName = `usage-${datePreset}-${new Date().toISOString().split("T")[0]}.csv`;
        const path = await saveCSV(csv, fileName);
        if (path) alert(`Exported to: ${path}`);
      }
    } catch (e) {
      console.error("Export failed:", e);
    } finally {
      setExporting(false);
    }
  };

  const COLORS = ["var(--color-accent)", "var(--color-info)", "var(--color-secondary)", "#f472b6", "var(--color-warning)"];
  const activeFilterCount = [selectedModel, selectedCharacter].filter(Boolean).length;

  return (
    <div className="min-h-screen bg-surface pb-24">
      {/* Filters Bottom Sheet */}
      <BottomMenu
        isOpen={showFilters}
        onClose={() => setShowFilters(false)}
        title="Filters"
        includeExitIcon={false}
      >
        <div className="space-y-4 pb-4">
          <div>
            <label className="text-xs font-medium text-fg/60 uppercase tracking-wide mb-2 block">
              Model
            </label>
            <div className="space-y-1 max-h-48 overflow-y-auto">
              {modelOptions.slice(0, 8).map((m) => (
                <button
                  key={m.id}
                  onClick={() => setSelectedModel(selectedModel === m.id ? null : m.id)}
                  className={`w-full flex items-center justify-between px-3 py-2.5 rounded-lg transition ${
                    selectedModel === m.id
                      ? "bg-accent/20 text-accent/90"
                      : "bg-fg/5 text-fg/70 hover:bg-fg/10"
                  }`}
                >
                  <span className="truncate">{m.name}</span>
                  <span className="text-xs text-fg/40">{formatCompactNumber(m.tokens)}</span>
                </button>
              ))}
            </div>
          </div>

          <div>
            <label className="text-xs font-medium text-fg/60 uppercase tracking-wide mb-2 block">
              Character
            </label>
            <div className="space-y-1 max-h-48 overflow-y-auto">
              {characterOptions.slice(0, 8).map((c) => (
                <button
                  key={c.id}
                  onClick={() => setSelectedCharacter(selectedCharacter === c.id ? null : c.id)}
                  className={`w-full flex items-center justify-between px-3 py-2.5 rounded-lg transition ${
                    selectedCharacter === c.id
                      ? "bg-accent/20 text-accent/90"
                      : "bg-fg/5 text-fg/70 hover:bg-fg/10"
                  }`}
                >
                  <span className="truncate">{c.name}</span>
                  <span className="text-xs text-fg/40">{formatCompactNumber(c.tokens)}</span>
                </button>
              ))}
            </div>
          </div>

          <div className="flex gap-2 pt-2">
            <button
              onClick={() => {
                setSelectedModel(null);
                setSelectedCharacter(null);
              }}
              className="flex-1 py-2.5 rounded-xl border border-fg/10 text-fg/60 text-sm font-medium hover:bg-fg/5 transition"
            >
              Clear All
            </button>
            <button
              onClick={() => setShowFilters(false)}
              className="flex-1 py-2.5 rounded-xl bg-accent/20 border border-accent/30 text-accent/90 text-sm font-medium hover:bg-accent/30 transition"
            >
              Apply
            </button>
          </div>
        </div>
      </BottomMenu>

      {/* All Activity Bottom Sheet */}
      <BottomMenu
        isOpen={showAllActivity}
        onClose={() => setShowAllActivity(false)}
        title="Recent Activity"
        includeExitIcon={false}
      >
        <div className="space-y-1 pb-4 max-h-[60vh] overflow-y-auto">
          {filteredRecords.slice(0, 50).map((r) => (
            <ActivityItem key={r.id} request={r} />
          ))}
          {filteredRecords.length === 0 && (
            <p className="text-center text-fg/40 py-8 text-sm">No activity in this period</p>
          )}
        </div>
      </BottomMenu>

      {/* Custom Date Range Bottom Sheet */}
      <BottomMenu
        isOpen={showCustomDatePicker}
        onClose={() => setShowCustomDatePicker(false)}
        title="Custom Date Range"
        includeExitIcon={false}
      >
        <div className="space-y-4 pb-4">
          <div>
            <label className="block text-xs font-medium text-fg/60 mb-2">Start Date</label>
            <input
              type="date"
              value={customStartDate.toISOString().split("T")[0]}
              onChange={(e) => {
                const newDate = new Date(e.target.value);
                newDate.setHours(0, 0, 0, 0);
                setCustomStartDate(newDate);
              }}
              max={customEndDate.toISOString().split("T")[0]}
              className="w-full px-3 py-2.5 rounded-xl bg-fg/5 border border-fg/10 text-fg text-sm focus:outline-none focus:border-accent/50 focus:bg-fg/10 transition"
            />
          </div>

          <div>
            <label className="block text-xs font-medium text-fg/60 mb-2">End Date</label>
            <input
              type="date"
              value={customEndDate.toISOString().split("T")[0]}
              onChange={(e) => {
                const newDate = new Date(e.target.value);
                newDate.setHours(23, 59, 59, 999);
                setCustomEndDate(newDate);
              }}
              min={customStartDate.toISOString().split("T")[0]}
              max={new Date().toISOString().split("T")[0]}
              className="w-full px-3 py-2.5 rounded-xl bg-fg/5 border border-fg/10 text-fg text-sm focus:outline-none focus:border-accent/50 focus:bg-fg/10 transition"
            />
          </div>

          <div className="flex gap-2 pt-2">
            <button
              onClick={() => setShowCustomDatePicker(false)}
              className="flex-1 py-2.5 rounded-xl border border-fg/10 text-fg/60 text-sm font-medium hover:bg-fg/5 transition"
            >
              Cancel
            </button>
            <button
              onClick={() => {
                if (viewMode === "appTime") {
                  setAppTimePreset("custom");
                } else {
                  setDatePreset("custom");
                }
                setShowCustomDatePicker(false);
              }}
              className="flex-1 py-2.5 rounded-xl bg-accent/20 border border-accent/30 text-accent/90 text-sm font-medium hover:bg-accent/30 transition"
            >
              Apply
            </button>
          </div>
        </div>
      </BottomMenu>

      <div className="max-w-5xl mx-auto px-4 py-4">
        {/* View Mode Toggle */}
        <div className="flex items-center gap-2 mb-4">
          <div className="flex-1 flex items-center gap-1 p-1 rounded-xl bg-fg/5 border border-fg/10">
            <button
              onClick={() => setViewMode("dashboard")}
              className={`flex-1 flex items-center justify-center gap-2 px-3 py-2 rounded-lg text-xs font-medium transition-all ${
                viewMode === "dashboard"
                  ? "bg-fg/10 text-fg"
                  : "text-fg/50 hover:text-fg/70"
              }`}
            >
              <Activity className="h-3.5 w-3.5" />
              Dashboard
            </button>
            <button
              onClick={() => setViewMode("appTime")}
              className={`flex-1 flex items-center justify-center gap-2 px-3 py-2 rounded-lg text-xs font-medium transition-all ${
                viewMode === "appTime"
                  ? "bg-fg/10 text-fg"
                  : "text-fg/50 hover:text-fg/70"
              }`}
            >
              <Clock className="h-3.5 w-3.5" />
              App Time
            </button>
          </div>
        </div>

        {/* ================================================================== */}
        {/* DASHBOARD VIEW */}
        {/* ================================================================== */}
        {viewMode === "dashboard" && (
          <>
            {/* Header Row */}
            <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between mb-4">
              <div className="flex flex-wrap items-center gap-1 p-1 rounded-xl bg-fg/5 border border-fg/10">
                {[
                  { key: "today", label: "Today" },
                  { key: "week", label: "7 Days" },
                  { key: "month", label: "30 Days" },
                  { key: "all", label: "All" },
                ].map(({ key, label }) => (
                  <button
                    key={key}
                    onClick={() => setDatePreset(key as DatePreset)}
                    className={`px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
                      datePreset === key
                        ? "bg-fg/10 text-fg"
                        : "text-fg/50 hover:text-fg/70"
                    }`}
                  >
                    {label}
                  </button>
                ))}
                <button
                  onClick={() => setShowCustomDatePicker(true)}
                  className={`px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
                    datePreset === "custom"
                      ? "bg-fg/10 text-fg"
                      : "text-fg/50 hover:text-fg/70"
                  }`}
                >
                  Custom
                </button>
              </div>

              <div className="flex w-full items-center gap-2 sm:w-auto sm:justify-end">
                <button
                  onClick={() => setShowFilters(true)}
                  className={`flex-1 sm:flex-none flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition ${
                    activeFilterCount > 0
                      ? "bg-accent/20 text-accent/90 border border-accent/30"
                      : "bg-fg/5 text-fg/60 border border-fg/10 hover:bg-fg/10"
                  }`}
                >
                  <Filter className="h-3.5 w-3.5" />
                  {activeFilterCount > 0 ? `${activeFilterCount}` : "Filter"}
                </button>
                <button
                  onClick={handleExport}
                  disabled={exporting || records.length === 0}
                  className="flex-1 sm:flex-none flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-lg bg-fg/5 border border-fg/10 text-fg/60 text-xs font-medium hover:bg-fg/10 transition disabled:opacity-50"
                >
                  <Download className="h-3.5 w-3.5" />
                </button>
              </div>
            </div>

            {/* Active Filters */}
            <AnimatePresence>
              {activeFilterCount > 0 && (
                <motion.div
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: "auto" }}
                  exit={{ opacity: 0, height: 0 }}
                  className="flex items-center gap-2 mb-4 overflow-hidden"
                >
                  {selectedModel && (
                    <button
                      onClick={() => setSelectedModel(null)}
                      className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-accent/15 border border-accent/30 text-accent/90 text-xs"
                    >
                      {modelOptions.find((m) => m.id === selectedModel)?.name}
                      <X className="h-3 w-3" />
                    </button>
                  )}
                  {selectedCharacter && (
                    <button
                      onClick={() => setSelectedCharacter(null)}
                      className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-accent/15 border border-accent/30 text-accent/90 text-xs"
                    >
                      {characterOptions.find((c) => c.id === selectedCharacter)?.name}
                      <X className="h-3 w-3" />
                    </button>
                  )}
                </motion.div>
              )}
            </AnimatePresence>

            {loading ? (
              <div className="flex items-center justify-center py-20">
                <div className="h-8 w-8 animate-spin rounded-full border-2 border-fg/20 border-t-accent" />
              </div>
            ) : (
              <div className="space-y-4">
                {/* Stats Row */}
                <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
                  <StatCard
                    icon={DollarSign}
                    label="Total Cost"
                    value={formatCurrency(displayStats.cost)}
                    highlight
                  />
                  <StatCard
                    icon={Zap}
                    label="Tokens"
                    value={formatNumber(displayStats.tokens)}
                    subValue={`${formatNumber(Math.round(displayStats.avgPerRequest))} avg`}
                  />
                  <StatCard
                    icon={Activity}
                    label="Requests"
                    value={displayStats.requests.toLocaleString()}
                  />
                  <StatCard
                    icon={Clock}
                    label="Period"
                    value={
                      datePreset === "today"
                        ? "Today"
                        : datePreset === "week"
                          ? "7 Days"
                          : datePreset === "month"
                            ? "30 Days"
                            : datePreset === "custom"
                              ? `${customStartDate.toLocaleDateString()} - ${customEndDate.toLocaleDateString()}`
                              : "All Time"
                    }
                    subValue={`${filteredRecords.length} records`}
                  />
                </div>

                {/* Chart */}
                {chartData.length > 1 && (
                  <motion.div
                    initial={{ opacity: 0, y: 8 }}
                    animate={{ opacity: 1, y: 0 }}
                    className="rounded-2xl border border-fg/10 bg-fg/5 p-4"
                  >
                    <div className="flex items-center justify-between mb-4">
                      <h3 className="text-sm font-semibold text-fg">Usage Trend</h3>
                      <div className="flex items-center gap-3 text-[11px]">
                        <div className="flex items-center gap-1.5">
                          <span className="h-2 w-2 rounded-full bg-info" />
                          <span className="text-fg/50">Input</span>
                        </div>
                        <div className="flex items-center gap-1.5">
                          <span className="h-2 w-2 rounded-full bg-accent" />
                          <span className="text-fg/50">Output</span>
                        </div>
                      </div>
                    </div>
                    <div className="h-48">
                      <ResponsiveContainer width="100%" height="100%">
                        <AreaChart
                          data={chartData}
                          margin={{ top: 5, right: 5, left: -20, bottom: 0 }}
                        >
                          <defs>
                            <linearGradient id="inputGrad" x1="0" y1="0" x2="0" y2="1">
                              <stop offset="5%" stopColor="var(--color-info)" stopOpacity={0.3} />
                              <stop offset="95%" stopColor="var(--color-info)" stopOpacity={0} />
                            </linearGradient>
                            <linearGradient id="outputGrad" x1="0" y1="0" x2="0" y2="1">
                              <stop offset="5%" stopColor="var(--color-accent)" stopOpacity={0.3} />
                              <stop offset="95%" stopColor="var(--color-accent)" stopOpacity={0} />
                            </linearGradient>
                          </defs>
                          <XAxis
                            dataKey="label"
                            tick={{ fill: "rgba(255,255,255,0.4)", fontSize: 10 }}
                            axisLine={{ stroke: "rgba(255,255,255,0.1)" }}
                            tickLine={false}
                          />
                          <YAxis
                            tick={{ fill: "rgba(255,255,255,0.4)", fontSize: 10 }}
                            axisLine={false}
                            tickLine={false}
                            tickFormatter={formatCompactNumber}
                          />
                          <Tooltip content={<ChartTooltip />} />
                          <Area
                            type="monotone"
                            dataKey="input"
                            name="Input"
                            stroke="var(--color-info)"
                            fill="url(#inputGrad)"
                            strokeWidth={2}
                          />
                          <Area
                            type="monotone"
                            dataKey="output"
                            name="Output"
                            stroke="var(--color-accent)"
                            fill="url(#outputGrad)"
                            strokeWidth={2}
                          />
                        </AreaChart>
                      </ResponsiveContainer>
                    </div>
                  </motion.div>
                )}

                {/* Two Column Layout */}
                <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
                  {topModels.length > 0 && (
                    <motion.div
                      initial={{ opacity: 0, y: 8 }}
                      animate={{ opacity: 1, y: 0 }}
                      transition={{ delay: 0.05 }}
                      className="rounded-2xl border border-fg/10 bg-fg/5 p-4"
                    >
                      <h3 className="text-sm font-semibold text-fg mb-4">By Model</h3>
                      <div className="flex gap-4">
                        <div className="w-28 h-28 shrink-0">
                          <ResponsiveContainer width="100%" height="100%">
                            <PieChart>
                              <Pie
                                data={topModels}
                                dataKey="tokens"
                                nameKey="name"
                                cx="50%"
                                cy="50%"
                                innerRadius={25}
                                outerRadius={45}
                                paddingAngle={2}
                              >
                                {topModels.map((_, i) => (
                                  <Cell key={i} fill={COLORS[i % COLORS.length]} />
                                ))}
                              </Pie>
                            </PieChart>
                          </ResponsiveContainer>
                        </div>
                        <div className="flex-1 space-y-2">
                          {topModels.slice(0, 4).map((m, i) => (
                            <div key={m.id} className="flex items-center gap-2">
                              <span
                                className="h-2 w-2 rounded-full shrink-0"
                                style={{ backgroundColor: COLORS[i % COLORS.length] }}
                              />
                              <span className="text-xs text-fg/70 truncate flex-1">
                                {m.name}
                              </span>
                              <span className="text-xs text-fg/40">
                                {formatCompactNumber(m.tokens)}
                              </span>
                            </div>
                          ))}
                        </div>
                      </div>
                    </motion.div>
                  )}

                  {characterOptions.length > 0 && (
                    <motion.div
                      initial={{ opacity: 0, y: 8 }}
                      animate={{ opacity: 1, y: 0 }}
                      transition={{ delay: 0.1 }}
                      className="rounded-2xl border border-fg/10 bg-fg/5 p-4"
                    >
                      <h3 className="text-sm font-semibold text-fg mb-4">By Character</h3>
                      <div className="flex gap-4">
                        <div className="w-28 h-28 shrink-0">
                          <ResponsiveContainer width="100%" height="100%">
                            <PieChart>
                              <Pie
                                data={characterOptions.slice(0, 5)}
                                dataKey="tokens"
                                nameKey="name"
                                cx="50%"
                                cy="50%"
                                innerRadius={25}
                                outerRadius={45}
                                paddingAngle={2}
                              >
                                {characterOptions.slice(0, 5).map((_, i) => (
                                  <Cell key={i} fill={COLORS[i % COLORS.length]} />
                                ))}
                              </Pie>
                            </PieChart>
                          </ResponsiveContainer>
                        </div>
                        <div className="flex-1 space-y-2">
                          {characterOptions.slice(0, 4).map((c, i) => (
                            <div key={c.id} className="flex items-center gap-2">
                              <span
                                className="h-2 w-2 rounded-full shrink-0"
                                style={{ backgroundColor: COLORS[i % COLORS.length] }}
                              />
                              <span className="text-xs text-fg/70 truncate flex-1">
                                {c.name}
                              </span>
                              <span className="text-xs text-fg/40">
                                {formatCompactNumber(c.tokens)}
                              </span>
                            </div>
                          ))}
                        </div>
                      </div>
                    </motion.div>
                  )}
                </div>

                {/* Recent Activity */}
                <motion.div
                  initial={{ opacity: 0, y: 8 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: 0.2 }}
                  className="rounded-2xl border border-fg/10 bg-fg/5 overflow-hidden"
                >
                  <div className="flex items-center justify-between px-4 py-3 border-b border-fg/10">
                    <h3 className="text-sm font-semibold text-fg">Recent Activity</h3>
                    {filteredRecords.length > 5 && (
                      <button
                        onClick={() => setShowAllActivity(true)}
                        className="flex items-center gap-1 text-xs text-accent hover:text-accent/80 transition"
                      >
                        View all
                        <ChevronRight className="h-3.5 w-3.5" />
                      </button>
                    )}
                  </div>
                  <div className="divide-y divide-fg/5">
                    {filteredRecords.slice(0, 5).map((r) => (
                      <ActivityItem key={r.id} request={r} />
                    ))}
                    {filteredRecords.length === 0 && (
                      <div className="py-12 text-center">
                        <Calendar className="h-8 w-8 text-fg/20 mx-auto mb-2" />
                        <p className="text-sm text-fg/50">No activity yet</p>
                        <p className="text-xs text-fg/30 mt-1">
                          Start chatting to see usage data
                        </p>
                      </div>
                    )}
                  </div>
                </motion.div>
              </div>
            )}
          </>
        )}

        {/* ================================================================== */}
        {/* APP TIME VIEW */}
        {/* ================================================================== */}
        {viewMode === "appTime" && (
          <div className="space-y-4">
            <div className="inline-flex flex-wrap items-center gap-1 p-1 rounded-xl bg-fg/5 border border-fg/10">
              {[
                { key: "today", label: "Today" },
                { key: "week", label: "7 Days" },
                { key: "month", label: "30 Days" },
                { key: "all", label: "All" },
                { key: "custom", label: "Custom" },
              ].map(({ key, label }) => (
                <button
                  key={key}
                  onClick={() => {
                    if (key === "custom") {
                      setShowCustomDatePicker(true);
                      return;
                    }
                    setAppTimePreset(key as typeof appTimePreset);
                  }}
                  className={`px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
                    appTimePreset === key
                      ? "bg-fg/10 text-fg"
                      : "text-fg/50 hover:text-fg/70"
                  }`}
                >
                  {label}
                </button>
              ))}
            </div>

            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
              <StatCard
                icon={Clock}
                label="Period Total"
                value={formatDurationMs(appTimeStats.rangeTotalMs)}
                subValue={`${appTimeStats.selectedDays} day${appTimeStats.selectedDays === 1 ? "" : "s"}`}
                trend={{
                  value:
                    appTimeStats.prevRangeTotalMs > 0
                      ? Math.abs(
                          ((appTimeStats.rangeTotalMs - appTimeStats.prevRangeTotalMs) /
                            appTimeStats.prevRangeTotalMs) *
                            100,
                        )
                      : appTimeStats.rangeTotalMs > 0
                        ? 100
                        : 0,
                  isUp: appTimeStats.rangeTotalMs >= appTimeStats.prevRangeTotalMs,
                }}
                highlight
              />
              <StatCard
                icon={Activity}
                label="Daily Avg"
                value={formatDurationMs(appTimeStats.dailyAvgInRangeMs)}
                subValue="in selected period"
              />
              <StatCard
                icon={Activity}
                label="Today"
                value={formatDurationMs(appTimeStats.todayMs)}
                subValue={`Yesterday ${formatDurationMs(appTimeStats.yesterdayMs)}`}
              />
              <StatCard
                icon={Activity}
                label="30-Day Avg"
                value={formatDurationMs(appTimeStats.avg30Ms)}
              />
            </div>

            <motion.div
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              className="rounded-2xl border border-fg/10 bg-fg/5 p-4"
            >
              <div className="flex items-center justify-between mb-4">
                <h3 className="text-sm font-semibold text-fg">App Time Trend</h3>
                <div className="text-[11px] text-fg/45">
                  Total {formatDurationMs(appTimeStats.rangeTotalMs)}
                  {appTimePreset !== "all" && (
                    <span
                      className={`ml-2 ${appTimeStats.rangeDeltaMs >= 0 ? "text-accent" : "text-danger"}`}
                    >
                      {appTimeStats.rangeDeltaMs >= 0 ? "+" : "-"}
                      {formatDurationMs(Math.abs(appTimeStats.rangeDeltaMs))} (
                      {appTimeStats.rangeDeltaMs >= 0 ? "+" : ""}
                      {appTimeStats.rangeDeltaPct.toFixed(0)}%)
                    </span>
                  )}
                </div>
              </div>
              <div className="h-48">
                <ResponsiveContainer width="100%" height="100%">
                  <AreaChart
                    data={appTimeStats.byDayChart}
                    margin={{ top: 5, right: 5, left: -20, bottom: 0 }}
                  >
                    <defs>
                      <linearGradient id="appTimeGrad" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="5%" stopColor="var(--color-accent)" stopOpacity={0.35} />
                        <stop offset="95%" stopColor="var(--color-accent)" stopOpacity={0} />
                      </linearGradient>
                    </defs>
                    <XAxis
                      dataKey="label"
                      tick={{ fill: "rgba(255,255,255,0.4)", fontSize: 10 }}
                      axisLine={{ stroke: "rgba(255,255,255,0.1)" }}
                      tickLine={false}
                    />
                    <YAxis
                      tick={{ fill: "rgba(255,255,255,0.4)", fontSize: 10 }}
                      axisLine={false}
                      tickLine={false}
                      tickFormatter={(v) => formatDurationMs(v)}
                    />
                    <Tooltip content={<AppTimeTooltip />} />
                    <Area
                      type="monotone"
                      dataKey="ms"
                      name="Active Time"
                      stroke="var(--color-accent)"
                      fill="url(#appTimeGrad)"
                      strokeWidth={2}
                    />
                  </AreaChart>
                </ResponsiveContainer>
              </div>
            </motion.div>
          </div>
        )}
      </div>
    </div>
  );
}
