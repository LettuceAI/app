import { useCallback, useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  CalendarClock,
  Gauge,
  LayoutGrid,
  List,
  Loader2,
  RefreshCw,
  ShieldCheck,
} from "lucide-react";

import { useI18n } from "../../../core/i18n/context";
import { readSettings } from "../../../core/storage";
import type { ProviderCredential } from "../../../core/storage";
import {
  fetchNanoGptSubscriptionUsage,
  onNanoGptQuota,
  parseNanoGptTimestamp,
  primaryNanoGptQuota,
  quotaPercent,
  type NanoGptQuotaWindow,
  type NanoGptSubscriptionUsage,
} from "../../../core/usage/nanogpt";
import { useUsageTracking, type RequestUsage, type UsageStats } from "../../../core/usage";
import { cn } from "../../design-tokens";
import {
  ActivityItem,
  SectionCard,
  StatTile,
  UsageRequestDetailSheet,
  formatCompactNumber,
  formatCurrency,
} from "./UsageActivityShared";

const NANOGPT_PROVIDER_ID = "nanogpt";
const DAY_MS = 86_400_000;
const RECORD_PAGE_SIZE = 20;

type LocalRange = "window" | "30d" | "all";
type LocalView = "summary" | "list";

function quotaWindowStart(
  kind: "weekly" | "daily" | "monthly",
  resetAt: Date | null,
): number | null {
  if (!resetAt) return null;
  const start = new Date(resetAt.getTime());
  if (kind === "weekly") start.setDate(start.getDate() - 7);
  else if (kind === "daily") start.setDate(start.getDate() - 1);
  else start.setMonth(start.getMonth() - 1);
  return start.getTime();
}

function quotaToneColor(percent: number | null): string {
  if (percent === null) return "var(--color-accent)";
  if (percent * 100 >= 90) return "var(--color-danger)";
  if (percent * 100 >= 75) return "var(--color-warning)";
  return "var(--color-accent)";
}

function formatQuotaValue(value?: number | null): string {
  if (typeof value !== "number" || !Number.isFinite(value)) return "—";
  return new Intl.NumberFormat(undefined, {
    notation: value >= 100_000 ? "compact" : "standard",
    maximumFractionDigits: value >= 100_000 ? 1 : 0,
  }).format(value);
}

function EmptyState({
  icon,
  title,
  description,
  tone = "muted",
  inset,
}: {
  icon: React.ReactNode;
  title: string;
  description?: string;
  tone?: "muted" | "danger";
  inset?: boolean;
}) {
  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center px-6 text-center",
        inset ? "min-h-40 rounded-lg py-4" : "min-h-56 rounded-xl border border-fg/8 bg-fg/[0.02]",
        inset && tone === "danger" && "bg-danger/[0.04]",
        !inset && tone === "danger" && "border-danger/20 bg-danger/[0.04]",
      )}
    >
      {icon}
      <h3 className="mt-3 text-[13px] font-semibold tracking-tight text-fg">{title}</h3>
      {description && (
        <p className="mt-1 max-w-lg text-[11.5px] leading-relaxed text-fg/45">{description}</p>
      )}
    </div>
  );
}

const RING_SIZE = 132;
const RING_STROKE = 9;
const RING_RADIUS = (RING_SIZE - RING_STROKE) / 2;
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS;

function QuotaRing({
  percent,
  caption,
  footnote,
}: {
  percent: number | null;
  caption: string;
  footnote?: string;
}) {
  const filled = percent ?? 0;
  return (
    <div className="flex shrink-0 flex-col items-center gap-2.5">
      <div className="relative" style={{ width: RING_SIZE, height: RING_SIZE }}>
        <svg
          width={RING_SIZE}
          height={RING_SIZE}
          viewBox={`0 0 ${RING_SIZE} ${RING_SIZE}`}
          className="-rotate-90"
        >
          <circle
            cx={RING_SIZE / 2}
            cy={RING_SIZE / 2}
            r={RING_RADIUS}
            fill="none"
            stroke="currentColor"
            strokeWidth={RING_STROKE}
            className="text-fg/[0.07]"
          />
          <circle
            cx={RING_SIZE / 2}
            cy={RING_SIZE / 2}
            r={RING_RADIUS}
            fill="none"
            stroke={quotaToneColor(percent)}
            strokeWidth={RING_STROKE}
            strokeLinecap="round"
            strokeDasharray={RING_CIRCUMFERENCE}
            strokeDashoffset={RING_CIRCUMFERENCE * (1 - filled)}
            className="transition-[stroke-dashoffset,stroke] duration-700 ease-out"
            style={{ opacity: filled === 0 ? 0 : 1 }}
          />
        </svg>
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          <div className="flex items-baseline gap-0.5">
            <span className="text-[30px] font-semibold leading-none tabular-nums tracking-[-0.035em] text-fg">
              {percent === null ? "—" : Math.round(percent * 100)}
            </span>
            {percent !== null && (
              <span className="text-[14px] font-semibold leading-none text-fg/40">%</span>
            )}
          </div>
          <span className="mt-1.5 text-[9.5px] font-semibold uppercase tracking-[0.16em] text-fg/38">
            {caption}
          </span>
        </div>
      </div>
      {footnote && <span className="text-[11px] tabular-nums text-fg/40">{footnote}</span>}
    </div>
  );
}

function SecondaryWindow({ label, window }: { label: string; window: NanoGptQuotaWindow }) {
  const percent = quotaPercent(window);
  return (
    <div className="rounded-xl border border-fg/8 bg-fg/[0.025] px-3.5 py-3">
      <div className="flex items-center justify-between gap-3">
        <span className="text-[10px] font-semibold uppercase tracking-[0.12em] text-fg/40">
          {label}
        </span>
        <span className="text-[11.5px] font-semibold tabular-nums text-fg/70">
          {percent === null ? "—" : `${Math.round(percent * 100)}%`}
        </span>
      </div>
      <div className="mt-2.5 h-1.5 overflow-hidden rounded-full bg-fg/8">
        <div
          className="h-full rounded-full transition-[width,background-color] duration-500"
          style={{
            width: `${(percent ?? 0) * 100}%`,
            backgroundColor: quotaToneColor(percent),
          }}
        />
      </div>
      <div className="mt-2 flex justify-between text-[11px] tabular-nums text-fg/45">
        <span>{formatQuotaValue(window.used)}</span>
        <span>{formatQuotaValue(window.limit)}</span>
      </div>
    </div>
  );
}

function LocalActivityCard({ windowStart }: { windowStart: number | null }) {
  const { t } = useI18n();
  const { getStats, queryRecords } = useUsageTracking();
  const [range, setRange] = useState<LocalRange>(windowStart ? "window" : "30d");
  const [view, setView] = useState<LocalView>("summary");
  const [stats, setStats] = useState<UsageStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [records, setRecords] = useState<RequestUsage[] | null>(null);
  const [recordsLoading, setRecordsLoading] = useState(false);
  const [visibleCount, setVisibleCount] = useState(RECORD_PAGE_SIZE);
  const [selectedRequest, setSelectedRequest] = useState<RequestUsage | null>(null);

  const effectiveRange: LocalRange = range === "window" && !windowStart ? "30d" : range;

  const startTimestamp = useMemo(() => {
    if (effectiveRange === "all") return undefined;
    if (effectiveRange === "30d") return Date.now() - 30 * DAY_MS;
    return windowStart ?? undefined;
  }, [effectiveRange, windowStart]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    void getStats({
      providerId: NANOGPT_PROVIDER_ID,
      ...(startTimestamp === undefined ? {} : { startTimestamp }),
    })
      .then((result) => {
        if (!cancelled) setStats(result);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [getStats, startTimestamp]);

  useEffect(() => {
    if (view !== "list") return;
    let cancelled = false;
    setRecordsLoading(true);
    void queryRecords({
      providerId: NANOGPT_PROVIDER_ID,
      ...(startTimestamp === undefined ? {} : { startTimestamp }),
    })
      .then((rows) => {
        if (cancelled) return;
        setRecords([...rows].sort((a, b) => b.timestamp - a.timestamp));
        setVisibleCount(RECORD_PAGE_SIZE);
      })
      .finally(() => {
        if (!cancelled) setRecordsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [queryRecords, startTimestamp, view]);

  const ranges: Array<{ id: LocalRange; label: string }> = [
    ...(windowStart
      ? [{ id: "window" as const, label: t("usageAnalytics.page.nanoGpt.localWindowQuota") }]
      : []),
    { id: "30d", label: t("usageAnalytics.page.nanoGpt.localWindow30d") },
    { id: "all", label: t("usageAnalytics.page.nanoGpt.localWindowAll") },
  ];

  const avgTokens = stats && stats.totalRequests > 0 ? stats.totalTokens / stats.totalRequests : 0;

  const views: Array<{ id: LocalView; label: string; icon: React.ReactNode }> = [
    {
      id: "summary",
      label: t("usageAnalytics.page.nanoGpt.localViewSummary"),
      icon: <LayoutGrid size={13} />,
    },
    {
      id: "list",
      label: t("usageAnalytics.page.nanoGpt.localViewList"),
      icon: <List size={13} />,
    },
  ];

  const visibleRecords = records?.slice(0, visibleCount) ?? [];
  const isEmpty =
    view === "list"
      ? records !== null && records.length === 0
      : !stats || stats.totalRequests === 0;
  const isLoading = view === "list" ? recordsLoading && records === null : loading && !stats;

  return (
    <>
      <UsageRequestDetailSheet
        request={selectedRequest}
        isOpen={selectedRequest !== null}
        onClose={() => setSelectedRequest(null)}
      />
      <SectionCard
        title={t("usageAnalytics.page.nanoGpt.localTitle")}
        subtitle={t("usageAnalytics.page.nanoGpt.localSubtitle")}
        right={
          <div className="flex flex-wrap items-center justify-end gap-2">
            <div className="flex items-center gap-1 rounded-lg border border-fg/8 bg-fg/[0.025] p-0.5">
              {ranges.map((option) => (
                <button
                  key={option.id}
                  type="button"
                  onClick={() => setRange(option.id)}
                  className={cn(
                    "rounded-md px-2.5 py-1 text-[11px] font-medium transition",
                    effectiveRange === option.id
                      ? "bg-fg/[0.08] text-fg"
                      : "text-fg/45 hover:text-fg/75",
                  )}
                >
                  {option.label}
                </button>
              ))}
            </div>
            <div className="flex items-center gap-1 rounded-lg border border-fg/8 bg-fg/[0.025] p-0.5">
              {views.map((option) => (
                <button
                  key={option.id}
                  type="button"
                  onClick={() => setView(option.id)}
                  title={option.label}
                  aria-label={option.label}
                  aria-pressed={view === option.id}
                  className={cn(
                    "rounded-md px-2 py-1 transition",
                    view === option.id ? "bg-fg/[0.08] text-fg" : "text-fg/40 hover:text-fg/70",
                  )}
                >
                  {option.icon}
                </button>
              ))}
            </div>
          </div>
        }
        bodyClassName={view === "list" ? "!p-0" : undefined}
      >
        {isLoading ? (
          <div className="flex items-center justify-center py-10">
            <Loader2 className="h-4 w-4 animate-spin text-fg/30" />
          </div>
        ) : isEmpty ? (
          <div className="py-8 text-center text-[12px] text-fg/40">
            {t("usageAnalytics.page.nanoGpt.localEmpty")}
          </div>
        ) : view === "list" ? (
          <>
            <ul className="divide-y divide-fg/[0.06]">
              {visibleRecords.map((request) => (
                <li key={request.id}>
                  <ActivityItem request={request} onClick={setSelectedRequest} showChevron />
                </li>
              ))}
            </ul>
            {records && visibleCount < records.length && (
              <div className="border-t border-fg/[0.06] p-3">
                <button
                  type="button"
                  onClick={() => setVisibleCount((count) => count + RECORD_PAGE_SIZE)}
                  className="w-full rounded-lg border border-fg/8 bg-fg/[0.025] py-2 text-[11.5px] font-medium text-fg/60 transition hover:bg-fg/[0.05] hover:text-fg"
                >
                  {t("usageAnalytics.page.nanoGpt.localShowMore", {
                    count: formatCompactNumber(records.length - visibleCount),
                  })}
                </button>
              </div>
            )}
          </>
        ) : (
          <div className="grid grid-cols-2 gap-2.5 lg:grid-cols-4">
            <StatTile
              label={t("usageAnalytics.page.nanoGpt.localRequests")}
              value={formatCompactNumber(stats?.totalRequests ?? 0)}
              sub={
                stats && stats.failedRequests > 0
                  ? t("usageAnalytics.page.nanoGpt.localFailed", {
                      count: formatCompactNumber(stats.failedRequests),
                    })
                  : undefined
              }
            />
            <StatTile
              label={t("usageAnalytics.page.nanoGpt.localTokens")}
              value={formatCompactNumber(stats?.totalTokens ?? 0)}
            />
            <StatTile
              label={t("usageAnalytics.page.nanoGpt.localCost")}
              value={formatCurrency(stats?.totalCost ?? 0)}
            />
            <StatTile
              label={t("usageAnalytics.page.nanoGpt.localAvgTokens")}
              value={formatCompactNumber(Math.round(avgTokens))}
            />
          </div>
        )}
      </SectionCard>
    </>
  );
}

export function NanoGptUsagePanel() {
  const { t } = useI18n();
  const [credentials, setCredentials] = useState<ProviderCredential[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const [usage, setUsage] = useState<NanoGptSubscriptionUsage | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void readSettings()
      .then((settings) => {
        if (cancelled) return;
        const nanoGptCredentials = settings.providerCredentials.filter(
          (credential) => credential.providerId === "nanogpt",
        );
        setCredentials(nanoGptCredentials);
        setSelectedId((current) =>
          nanoGptCredentials.some((credential) => credential.id === current)
            ? current
            : (nanoGptCredentials[0]?.id ?? ""),
        );
      })
      .catch((reason) => {
        if (!cancelled) setError(String(reason));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const refresh = useCallback(async () => {
    if (!selectedId) return;
    setRefreshing(true);
    setError(null);
    try {
      setUsage(await fetchNanoGptSubscriptionUsage(selectedId));
    } catch (reason) {
      setError(String(reason));
      setUsage(null);
    } finally {
      setRefreshing(false);
    }
  }, [selectedId]);

  useEffect(() => {
    if (selectedId) void refresh();
  }, [refresh, selectedId]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    void onNanoGptQuota(({ usage: incoming }) => {
      if (incoming.credentialId !== selectedId) return;
      setUsage(incoming);
      setError(null);
    }).then((dispose) => {
      if (cancelled) dispose();
      else unlisten = dispose;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [selectedId]);

  const primary = usage ? primaryNanoGptQuota(usage) : null;
  const percent = primary ? quotaPercent(primary.window) : null;
  const resetDate = parseNanoGptTimestamp(primary?.window.resetAt ?? usage?.currentPeriodEnd);
  const resetLabel = resetDate
    ? `${resetDate.toLocaleDateString(undefined, { dateStyle: "medium" })} · ${resetDate.toLocaleTimeString(undefined, { timeStyle: "short" })}`
    : t("usageAnalytics.page.nanoGpt.notReported");
  if (loading) {
    return (
      <div className="flex items-center justify-center py-24">
        <Loader2 className="h-5 w-5 animate-spin text-fg/30" />
      </div>
    );
  }

  if (credentials.length === 0) {
    return (
      <EmptyState
        icon={<Gauge className="h-6 w-6 text-fg/28" />}
        title={t("usageAnalytics.page.nanoGpt.noAccount")}
        description={t("usageAnalytics.page.nanoGpt.noAccountDescription")}
      />
    );
  }

  const headerControls = (
    <div className="flex items-center gap-2">
      {credentials.length > 1 && (
        <select
          value={selectedId}
          onChange={(event) => setSelectedId(event.target.value)}
          className="rounded-md border border-fg/10 bg-surface px-2 py-1.5 text-[11.5px] text-fg outline-none focus:border-accent/40"
        >
          {credentials.map((credential) => (
            <option key={credential.id} value={credential.id}>
              {credential.label}
            </option>
          ))}
        </select>
      )}
      <button
        type="button"
        onClick={() => void refresh()}
        disabled={refreshing}
        className="inline-flex items-center gap-1.5 rounded-md border border-fg/10 bg-fg/[0.03] px-2.5 py-1.5 text-[11.5px] font-medium text-fg/65 transition hover:bg-fg/[0.07] hover:text-fg disabled:opacity-45"
      >
        <RefreshCw size={12} className={cn(refreshing && "animate-spin")} />
        {t("common.buttons.refresh")}
      </button>
    </div>
  );

  const selectedLabel =
    credentials.find((credential) => credential.id === selectedId)?.label ?? credentials[0].label;

  return (
    <div className="flex flex-col gap-4">
      <SectionCard
        title={selectedLabel}
        subtitle={t("usageAnalytics.page.nanoGpt.authoritative")}
        right={headerControls}
      >
        {error ? (
          <EmptyState
            inset
            tone="danger"
            icon={<AlertTriangle className="h-6 w-6 text-danger/70" />}
            title={t("usageAnalytics.page.nanoGpt.fetchFailed")}
            description={error}
          />
        ) : refreshing && !usage ? (
          <div className="flex items-center justify-center py-16">
            <Loader2 className="h-5 w-5 animate-spin text-fg/30" />
          </div>
        ) : usage && primary ? (
          <div className="flex flex-col items-center gap-6 sm:flex-row sm:items-center">
            <QuotaRing
              percent={percent}
              caption={t("usageAnalytics.page.nanoGpt.used")}
              footnote={`${formatQuotaValue(primary.window.used)} / ${formatQuotaValue(primary.window.limit)}`}
            />

            <div className="min-w-0 flex-1">
              <h4 className="text-[13px] font-semibold tracking-tight text-fg">
                {t(`usageAnalytics.page.nanoGpt.${primary.kind}`)}
              </h4>

              <div className="mt-3 grid grid-cols-1 gap-2.5 sm:grid-cols-3">
                <StatTile
                  label={t("usageAnalytics.page.nanoGpt.consumed")}
                  value={formatQuotaValue(primary.window.used)}
                />
                <StatTile
                  label={t("usageAnalytics.page.nanoGpt.remaining")}
                  value={formatQuotaValue(primary.window.remaining)}
                />
                <StatTile
                  label={t("usageAnalytics.page.nanoGpt.limit")}
                  value={formatQuotaValue(primary.window.limit)}
                />
              </div>

              <p className="mt-3 truncate text-[11.5px] text-fg/45">
                <CalendarClock size={13} className="mr-1.5 inline align-[-0.18em] text-fg/35" />
                {t("usageAnalytics.page.nanoGpt.resets")} {resetLabel}
              </p>
            </div>
          </div>
        ) : usage ? (
          <EmptyState
            inset
            icon={<Gauge className="h-6 w-6 text-fg/30" />}
            title={t("usageAnalytics.page.nanoGpt.noQuota")}
            description={t("usageAnalytics.page.nanoGpt.noQuotaDescription")}
          />
        ) : null}
      </SectionCard>

      {!error &&
        usage &&
        primary &&
        (usage.daily || usage.monthly) &&
        primary.kind === "weekly" && (
          <div className="grid gap-3 sm:grid-cols-2">
            {usage.daily && (
              <SecondaryWindow
                label={t("usageAnalytics.page.nanoGpt.daily")}
                window={usage.daily}
              />
            )}
            {usage.monthly && (
              <SecondaryWindow
                label={t("usageAnalytics.page.nanoGpt.monthly")}
                window={usage.monthly}
              />
            )}
          </div>
        )}

      <LocalActivityCard windowStart={primary ? quotaWindowStart(primary.kind, resetDate) : null} />

      {!error && (
        <div className="flex items-start gap-2.5 rounded-xl border border-fg/8 bg-fg/[0.02] px-3.5 py-3 text-[11.5px] leading-relaxed text-fg/45">
          <ShieldCheck className="mt-0.5 h-4 w-4 shrink-0 text-accent/70" />
          {t("usageAnalytics.page.nanoGpt.warningInfo")}
        </div>
      )}
    </div>
  );
}
