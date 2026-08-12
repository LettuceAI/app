import { useEffect, useMemo, useState } from "react";
import {
  ChevronLeft,
  ChevronRight,
  Loader2,
  Search,
  X,
  RefreshCw,
} from "lucide-react";
import { useUsageTracking, RequestUsage } from "../../../core/usage";
import {
  UsageRequestDetailSheet,
  formatCompactNumber,
  formatCurrency,
  getEffectiveTotalCost,
  getOperationColor,
  getOperationLabel,
  getRelativeTime,
} from "./UsageActivityShared";
import { useI18n } from "../../../core/i18n/context";
import { cn } from "../../design-tokens";

const PAGE_SIZE = 50;

type OpFilter = string | "all";

export function UsageActivityPage() {
  const { t } = useI18n();
  const { queryRecords } = useUsageTracking();
  const [records, setRecords] = useState<RequestUsage[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [page, setPage] = useState(1);
  const [query, setQuery] = useState("");
  const [opFilter, setOpFilter] = useState<OpFilter>("all");
  const [selectedRequest, setSelectedRequest] = useState<RequestUsage | null>(null);

  const load = async (mode: "initial" | "refresh" = "initial") => {
    if (mode === "initial") setLoading(true);
    else setRefreshing(true);
    try {
      const all = await queryRecords({});
      setRecords(all.sort((a, b) => b.timestamp - a.timestamp));
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  };

  useEffect(() => {
    void load("initial");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Operation type counts (for filter chips)
  const opCounts = useMemo(() => {
    const map = new Map<string, number>();
    for (const r of records) {
      map.set(r.operationType, (map.get(r.operationType) ?? 0) + 1);
    }
    return Array.from(map.entries()).sort((a, b) => b[1] - a[1]);
  }, [records]);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return records.filter((r) => {
      if (opFilter !== "all" && r.operationType !== opFilter) return false;
      if (!needle) return true;
      return (
        (r.characterName ?? "").toLowerCase().includes(needle) ||
        (r.modelName ?? "").toLowerCase().includes(needle) ||
        (r.providerLabel ?? "").toLowerCase().includes(needle) ||
        r.operationType.toLowerCase().includes(needle)
      );
    });
  }, [records, query, opFilter]);

  const totalPages = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));

  useEffect(() => {
    if (page > totalPages) setPage(totalPages);
  }, [page, totalPages]);

  useEffect(() => {
    setPage(1);
  }, [query, opFilter]);

  const pageRecords = useMemo(() => {
    const start = (page - 1) * PAGE_SIZE;
    return filtered.slice(start, start + PAGE_SIZE);
  }, [page, filtered]);

  const totals = useMemo(() => {
    let cost = 0;
    let tokens = 0;
    for (const r of filtered) {
      cost += getEffectiveTotalCost(r);
      tokens += r.totalTokens ?? 0;
    }
    return { cost, tokens };
  }, [filtered]);

  const start = filtered.length === 0 ? 0 : (page - 1) * PAGE_SIZE + 1;
  const end = Math.min(page * PAGE_SIZE, filtered.length);

  return (
    <div className="mx-auto flex w-full max-w-[1280px] flex-col gap-5 px-5 pb-24 pt-5 sm:px-8">
      <UsageRequestDetailSheet
        request={selectedRequest}
        isOpen={selectedRequest !== null}
        onClose={() => setSelectedRequest(null)}
      />

      {/* Header */}
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-[16px] font-semibold tracking-tight text-fg">
            {t("usageAnalytics.activity.title")}
          </h1>
          <p className="mt-1 text-[12.5px] text-fg/50">
            {t("usageAnalytics.activity.recordsCount", {
              count: records.length.toLocaleString(),
            })}
          </p>
        </div>
        <div className="flex items-center gap-1.5 text-[12px] text-fg/55">
          <span className="tabular-nums text-fg/75">
            {filtered.length.toLocaleString()}
          </span>
          <span className="text-fg/40">{t("usageAnalytics.activity.records")}</span>
          <span className="px-0.5 text-fg/25">·</span>
          <span className="font-medium tabular-nums text-fg/75">
            {formatCompactNumber(totals.tokens)}
          </span>
          <span className="text-fg/40">{t("usageAnalytics.activity.tokens")}</span>
          <span className="px-0.5 text-fg/25">·</span>
          <span className="font-medium tabular-nums text-accent">
            {formatCurrency(totals.cost)}
          </span>
        </div>
      </header>

      {/* Toolbar */}
      <div className="flex flex-wrap items-center gap-3">
        <div className="relative min-w-0 flex-1">
          <Search
            size={14}
            className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-fg/40"
          />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("usageAnalytics.activity.searchPlaceholder")}
            className="h-9 w-full rounded-lg border border-fg/10 bg-fg/[0.02] pl-9 pr-9 text-[13px] text-fg outline-none transition placeholder:text-fg/35 focus:border-accent/40 focus:bg-fg/[0.05] focus:ring-1 focus:ring-accent/20"
          />
          {query && (
            <button
              onClick={() => setQuery("")}
              className="absolute right-2 top-1/2 -translate-y-1/2 rounded p-1 text-fg/40 transition hover:bg-fg/10 hover:text-fg/70"
              aria-label={t("usageAnalytics.activity.clearSearch")}
            >
              <X size={12} />
            </button>
          )}
        </div>
        <button
          onClick={() => void load("refresh")}
          className="inline-flex h-9 items-center gap-1.5 rounded-md border border-fg/8 bg-fg/[0.02] px-3 text-[12px] font-medium text-fg/65 transition hover:border-fg/15 hover:bg-fg/5 hover:text-fg disabled:opacity-30"
          disabled={refreshing}
        >
          <RefreshCw size={12} className={refreshing ? "animate-spin" : ""} />
          {t("common.buttons.refresh")}
        </button>
      </div>

      {/* Operation chips — wrap to multiple rows */}
      {opCounts.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          <OpChip
            label={t("usageAnalytics.activity.all")}
            count={records.length}
            active={opFilter === "all"}
            onClick={() => setOpFilter("all")}
          />
          {opCounts.map(([op, count]) => {
            const color = getOperationColor(op);
            const active = opFilter === op;
            return (
              <OpChip
                key={op}
                label={getOperationLabel(op, t)}
                count={count}
                active={active}
                color={color}
                onClick={() => setOpFilter(active ? "all" : op)}
              />
            );
          })}
        </div>
      )}

      {/* Body */}
      {loading ? (
        <div className="flex flex-col items-center justify-center gap-3 py-24">
          <Loader2 className="h-5 w-5 animate-spin text-fg/30" />
          <p className="text-[12px] text-fg/40">
            {t("usageAnalytics.activity.loading")}
          </p>
        </div>
      ) : filtered.length === 0 ? (
        <div className="rounded-xl border border-dashed border-fg/10 bg-fg/[0.02] px-4 py-16 text-center">
          <div className="mx-auto mb-3 flex h-11 w-11 items-center justify-center rounded-lg border border-fg/10 bg-fg/[0.04]">
            <Search size={16} className="text-fg/45" />
          </div>
          <div className="text-[13px] font-medium text-fg">
            {t("common.labels.none")}
          </div>
          <p className="mx-auto mt-1 max-w-md text-[12.5px] text-fg/55">
            {query || opFilter !== "all"
              ? t("usageAnalytics.activity.noMatch")
              : t("usageAnalytics.page.startChatting")}
          </p>
        </div>
      ) : (
        <section className="overflow-hidden rounded-xl border border-fg/8 bg-fg/[0.02]">
          {/* Pagination header */}
          <header className="flex flex-wrap items-center justify-between gap-3 border-b border-fg/8 px-4 py-2.5">
            <span className="text-[11.5px] tabular-nums text-fg/55">
              {t("usageAnalytics.activity.rangeOfTotal", {
                start,
                end,
                total: filtered.length,
              })}
            </span>
            {totalPages > 1 && (
              <Pager
                page={page}
                totalPages={totalPages}
                onPrev={() => setPage((v) => Math.max(1, v - 1))}
                onNext={() => setPage((v) => Math.min(totalPages, v + 1))}
              />
            )}
          </header>

          {/* Column header (desktop) */}
          <div className="hidden grid-cols-[minmax(0,1.5fr)_92px_minmax(0,1fr)_88px_92px_92px_16px] items-center gap-3 border-b border-fg/[0.05] bg-fg/[0.015] px-4 py-2 text-[10px] font-semibold uppercase tracking-[0.12em] text-fg/35 lg:grid">
            <div>{t("common.labels.name")}</div>
            <div>{t("usageAnalytics.activity.opColumn")}</div>
            <div>{t("usageAnalytics.activity.modelColumn")}</div>
            <div className="text-right">{t("usageAnalytics.page.tokens")}</div>
            <div className="text-right">{t("usageAnalytics.activity.costColumn")}</div>
            <div className="text-right">{t("usageAnalytics.activity.timeColumn")}</div>
            <div />
          </div>

          {/* Rows */}
          <ul>
            {pageRecords.map((r) => (
              <ActivityRow
                key={r.id}
                request={r}
                onClick={setSelectedRequest}
                t={t}
              />
            ))}
          </ul>

          {/* Footer pager */}
          {totalPages > 1 && (
            <footer className="flex items-center justify-between gap-3 border-t border-fg/8 px-4 py-3">
              <span className="text-[11.5px] tabular-nums text-fg/45">
                {t("usageAnalytics.activity.pageOf", { page, total: totalPages })}
              </span>
              <Pager
                page={page}
                totalPages={totalPages}
                onPrev={() => setPage((v) => Math.max(1, v - 1))}
                onNext={() => setPage((v) => Math.min(totalPages, v + 1))}
                showLabels
                t={t}
              />
            </footer>
          )}
        </section>
      )}
    </div>
  );
}

// =============================================================================
// Subcomponents
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
  const opLabel = getOperationLabel(request.operationType, t);
  return (
    <li>
      <button
        type="button"
        onClick={() => onClick(request)}
        className="group block w-full border-b border-fg/[0.05] px-4 py-2.5 text-left transition last:border-b-0 hover:bg-fg/[0.025]"
      >
        <div className="grid items-center gap-3 lg:grid-cols-[minmax(0,1.5fr)_minmax(0,120px)_minmax(0,1fr)_88px_92px_92px_16px]">
          {/* Name */}
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <span
                className="inline-flex h-1.5 w-1.5 shrink-0 rounded-full"
                style={{ backgroundColor: opColor }}
              />
              <span className="truncate text-[13px] font-medium text-fg">
                {request.characterName || t("usageAnalytics.shared.unknown")}
              </span>
              {/* Op badge inline on small screens */}
              <span
                className="shrink-0 rounded px-1.5 py-0.5 text-[9.5px] font-semibold uppercase tracking-wider lg:hidden"
                style={{ color: opColor, backgroundColor: opBg }}
              >
                {opLabel}
              </span>
            </div>
            <div className="mt-0.5 flex items-center gap-2 text-[11px] text-fg/45 lg:hidden">
              <span className="truncate">{request.modelName ?? "—"}</span>
              <span className="opacity-30">·</span>
              <span className="shrink-0 tabular-nums">
                {formatCompactNumber(request.totalTokens || 0)} tok
              </span>
              <span className="opacity-30">·</span>
              <span className="shrink-0 font-medium tabular-nums text-accent/90">
                {formatCurrency(getEffectiveTotalCost(request))}
              </span>
              <span className="opacity-30">·</span>
              <span className="shrink-0">{getRelativeTime(request.timestamp, t)}</span>
            </div>
          </div>

          {/* Op (desktop) */}
          <div className="hidden min-w-0 lg:block">
            <span
              className="inline-block max-w-full truncate rounded px-1.5 py-0.5 align-middle text-[9.5px] font-semibold uppercase tracking-wider"
              style={{ color: opColor, backgroundColor: opBg }}
              title={opLabel}
            >
              {opLabel}
            </span>
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

          <ChevronRight
            size={14}
            className="hidden shrink-0 text-fg/25 transition group-hover:text-fg/55 lg:block"
          />
        </div>
      </button>
    </li>
  );
}

function OpChip({
  label,
  count,
  active,
  color,
  onClick,
}: {
  label: string;
  count: number;
  active: boolean;
  color?: string;
  onClick: () => void;
}) {
  const dotColor = color ?? "var(--color-fg)";
  return (
    <button
      onClick={onClick}
      className={cn(
        "inline-flex shrink-0 items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11.5px] font-medium transition",
        active
          ? "border-fg/20 bg-fg/10 text-fg"
          : "border-fg/8 bg-fg/[0.025] text-fg/55 hover:border-fg/15 hover:bg-fg/5 hover:text-fg/85",
      )}
    >
      {color && (
        <span
          className="h-1.5 w-1.5 shrink-0 rounded-full"
          style={{ backgroundColor: dotColor }}
        />
      )}
      <span>{label}</span>
      <span
        className={cn(
          "rounded px-1.5 text-[10px] tabular-nums leading-[1.4]",
          active ? "bg-fg/12 text-fg/75" : "bg-fg/5 text-fg/40",
        )}
      >
        {count.toLocaleString()}
      </span>
    </button>
  );
}

function Pager({
  page,
  totalPages,
  onPrev,
  onNext,
  showLabels,
  t,
}: {
  page: number;
  totalPages: number;
  onPrev: () => void;
  onNext: () => void;
  showLabels?: boolean;
  t?: (k: any, v?: any) => string;
}) {
  return (
    <div className="flex items-center gap-1.5">
      <button
        onClick={onPrev}
        disabled={page === 1}
        className="inline-flex h-7 items-center gap-1 rounded-md border border-fg/8 bg-fg/[0.02] px-2 text-[11.5px] font-medium text-fg/65 transition hover:border-fg/15 hover:bg-fg/5 hover:text-fg disabled:cursor-not-allowed disabled:opacity-25"
      >
        <ChevronLeft size={13} />
        {showLabels && t && (
          <span className="hidden sm:inline">
            {t("usageAnalytics.activity.previous")}
          </span>
        )}
      </button>
      <span className="px-1 text-[11.5px] tabular-nums text-fg/55">
        {page}/{totalPages}
      </span>
      <button
        onClick={onNext}
        disabled={page === totalPages}
        className="inline-flex h-7 items-center gap-1 rounded-md border border-fg/8 bg-fg/[0.02] px-2 text-[11.5px] font-medium text-fg/65 transition hover:border-fg/15 hover:bg-fg/5 hover:text-fg disabled:cursor-not-allowed disabled:opacity-25"
      >
        {showLabels && t && (
          <span className="hidden sm:inline">
            {t("usageAnalytics.activity.next")}
          </span>
        )}
        <ChevronRight size={13} />
      </button>
    </div>
  );
}
