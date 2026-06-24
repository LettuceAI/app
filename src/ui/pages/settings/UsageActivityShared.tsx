import { Zap, ChevronRight } from "lucide-react";
import { BottomMenu } from "../../components";
import { RequestUsage } from "../../../core/usage";
import { useI18n } from "../../../core/i18n/context";
import { typography, cn } from "../../design-tokens";

export function formatCurrency(value: number): string {
  if (value === 0) return "$0.00";
  if (value < 0.01) return `$${value.toFixed(4)}`;
  if (value < 1) return `$${value.toFixed(3)}`;
  return `$${value.toFixed(2)}`;
}

export function formatCompactNumber(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(0)}K`;
  return value.toString();
}

export function getEffectiveTotalCost(request: RequestUsage): number {
  return request.cost?.totalCost ?? request.apiCost ?? 0;
}

export function getRelativeTime(
  timestamp: number,
  t: (key: any, params?: Record<string, string | number>) => string,
): string {
  const now = Date.now();
  const diff = now - timestamp;
  const minutes = Math.floor(diff / 60000);
  const hours = Math.floor(diff / 3600000);
  const days = Math.floor(diff / 86400000);

  if (minutes < 1) return t("usageAnalytics.shared.relativeTime.justNow");
  if (minutes < 60) return t("usageAnalytics.shared.relativeTime.minutesAgo", { count: minutes });
  if (hours < 24) return t("usageAnalytics.shared.relativeTime.hoursAgo", { count: hours });
  if (days < 7) return t("usageAnalytics.shared.relativeTime.daysAgo", { count: days });
  return new Date(timestamp).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

function normalizeOpType(type: string): string {
  return type
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .replace(/([A-Z])([A-Z][a-z])/g, "$1_$2")
    .replace(/-/g, "_")
    .toLowerCase();
}

export function getOperationColor(type: string): string {
  const colorsMap: Record<string, string> = {
    chat: "var(--color-info)",
    regenerate: "var(--color-secondary)",
    continue: "#22d3ee",
    summary: "var(--color-warning)",
    memory_manager: "var(--color-accent)",
    image_generation: "var(--color-accent)",
    ai_creator: "var(--color-secondary)",
    a_i_creator: "var(--color-secondary)",
    reply_helper: "var(--color-warning)",
    group_chat_message: "var(--color-info)",
    group_chat_regenerate: "var(--color-secondary)",
    group_chat_continue: "#22d3ee",
    group_chat_decision_maker: "var(--color-warning)",
  };
  return colorsMap[normalizeOpType(type)] || colorsMap[type.toLowerCase()] || "#94a3b8";
}

export function getOperationLabel(
  type: string,
  t: (key: any, params?: Record<string, string | number>) => string,
): string {
  const labels: Record<string, string> = {
    chat: t("usageAnalytics.shared.operations.chat"),
    regenerate: t("usageAnalytics.shared.operations.regenerate"),
    continue: t("usageAnalytics.shared.operations.continue"),
    summary: t("usageAnalytics.shared.operations.summary"),
    memory_manager: t("usageAnalytics.shared.operations.memoryManager"),
    image_generation: t("usageAnalytics.shared.operations.imageGeneration"),
    ai_creator: t("usageAnalytics.shared.operations.aiCreator"),
    a_i_creator: t("usageAnalytics.shared.operations.aiCreator"),
    reply_helper: t("usageAnalytics.shared.operations.replyHelper"),
    group_chat_message: t("usageAnalytics.shared.operations.groupChatMessage"),
    group_chat_regenerate: t("usageAnalytics.shared.operations.groupChatRegenerate"),
    group_chat_continue: t("usageAnalytics.shared.operations.groupChatContinue"),
    group_chat_decision_maker: t("usageAnalytics.shared.operations.groupChatDecisionMaker"),
  };
  return labels[normalizeOpType(type)] || labels[type.toLowerCase()] || type;
}

function parseMetadataNumber(metadata: RequestUsage["metadata"], key: string): number | null {
  const raw = metadata?.[key];
  if (!raw) return null;
  const parsed = Number(raw);
  return Number.isFinite(parsed) ? parsed : null;
}

function DetailStat({
  label,
  value,
  highlight,
}: {
  label: string;
  value: string;
  highlight?: boolean;
}) {
  return (
    <div
      className={cn(
        "rounded-md border px-2.5 py-2",
        highlight
          ? "border-accent/25 bg-accent/[0.06]"
          : "border-fg/8 bg-fg/[0.025]",
      )}
    >
      <div className="text-[9.5px] font-semibold uppercase tracking-[0.12em] text-fg/40">
        {label}
      </div>
      <div
        className={cn(
          "mt-0.5 text-[13.5px] font-medium tabular-nums",
          highlight ? "text-accent" : "text-fg",
        )}
      >
        {value}
      </div>
    </div>
  );
}

function MetaChip({ children }: { children: React.ReactNode }) {
  return (
    <span className="inline-flex items-center gap-1 rounded-md border border-fg/10 bg-fg/[0.03] px-2 py-0.5 text-[10.5px] font-medium text-fg/60">
      {children}
    </span>
  );
}

export function ActivityItem({
  request,
  onClick,
  showChevron = false,
}: {
  request: RequestUsage;
  onClick?: (request: RequestUsage) => void;
  showChevron?: boolean;
}) {
  const { t } = useI18n();
  const clickable = Boolean(onClick);
  const opColor = getOperationColor(request.operationType);
  const outputImageCount = parseMetadataNumber(request.metadata, "output_image_count");
  const usageLabel =
    outputImageCount && (request.totalTokens ?? 0) === 0
      ? t("usageAnalytics.shared.outputImages", { count: formatCompactNumber(outputImageCount) })
      : t("usageAnalytics.shared.tokens", { count: formatCompactNumber(request.totalTokens || 0) });

  // Robust background color calculation that works with both hex and var()
  const bgStyle = opColor.includes("var")
    ? `color-mix(in srgb, ${opColor}, transparent 88%)`
    : `${opColor}18`;

  return (
    <button
      type="button"
      disabled={!clickable}
      onClick={() => onClick?.(request)}
      className={cn(
        "flex w-full items-center gap-3 px-3 py-3 text-left transition-all duration-200",
        clickable ? "hover:bg-fg/5 active:scale-[0.99]" : "",
      )}
    >
      <div
        className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl"
        style={{ backgroundColor: bgStyle }}
      >
        <Zap className="h-4 w-4" style={{ color: opColor }} />
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className={cn(typography.body.size, typography.body.weight, "truncate text-fg")}>
            {request.characterName || t("usageAnalytics.shared.unknown")}
          </span>
          <span
            className="rounded-lg px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider"
            style={{
              backgroundColor: bgStyle,
              color: opColor,
            }}
          >
            {getOperationLabel(request.operationType, t)}
          </span>
        </div>
        <div className={cn(typography.caption.size, "mt-0.5 flex items-center gap-2 text-fg/40")}>
          <span>{usageLabel}</span>
          <span className="opacity-30">·</span>
          <span>{getRelativeTime(request.timestamp, t)}</span>
        </div>
      </div>
      <div className="shrink-0 text-right">
        <p className={cn(typography.body.size, typography.body.weight, "text-accent")}>
          {formatCurrency(getEffectiveTotalCost(request))}
        </p>
        <p className={cn(typography.overline.size, "mt-0.5 truncate text-fg/30")}>
          {request.modelName}
        </p>
      </div>
      {showChevron && <ChevronRight className="h-4 w-4 shrink-0 text-fg/20" />}
    </button>
  );
}

export function UsageRequestDetailSheet({
  request,
  isOpen,
  onClose,
}: {
  request: RequestUsage | null;
  isOpen: boolean;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const cachedPromptTokens =
    request?.cachedPromptTokens ??
    parseMetadataNumber(request?.metadata, "cached_prompt_tokens") ??
    parseMetadataNumber(request?.metadata, "openrouter_cached_prompt_tokens");
  const cacheWriteTokens =
    request?.cacheWriteTokens ??
    parseMetadataNumber(request?.metadata, "cache_write_tokens") ??
    parseMetadataNumber(request?.metadata, "openrouter_cache_write_tokens");
  const webSearchRequests =
    request?.webSearchRequests ??
    parseMetadataNumber(request?.metadata, "web_search_requests") ??
    parseMetadataNumber(request?.metadata, "openrouter_web_search_requests");
  const apiCost =
    request?.apiCost ??
    parseMetadataNumber(request?.metadata, "api_cost") ??
    parseMetadataNumber(request?.metadata, "openrouter_api_cost");
  const inputImageCount = parseMetadataNumber(request?.metadata, "input_image_count");
  const outputImageCount = parseMetadataNumber(request?.metadata, "output_image_count");

  return (
    <BottomMenu
      isOpen={isOpen}
      onClose={onClose}
      title={request ? getOperationLabel(request.operationType, t) : t("usageAnalytics.shared.requestDetails")}
      includeExitIcon={false}
    >
      {request && (() => {
        const opColor = getOperationColor(request.operationType);
        const tokenStats: Array<{ label: string; value: number }> = [
          { label: t("usageAnalytics.shared.stats.prompt"), value: request.promptTokens ?? 0 },
          { label: t("usageAnalytics.shared.stats.completion"), value: request.completionTokens ?? 0 },
          { label: t("usageAnalytics.shared.stats.reasoning"), value: request.reasoningTokens ?? 0 },
          { label: t("usageAnalytics.shared.stats.image"), value: request.imageTokens ?? 0 },
          { label: t("usageAnalytics.shared.stats.audio"), value: request.audioTokens ?? 0 },
          { label: t("usageAnalytics.shared.stats.memory"), value: request.memoryTokens ?? 0 },
          { label: t("usageAnalytics.shared.stats.summary"), value: request.summaryTokens ?? 0 },
        ];
        if (inputImageCount !== null) tokenStats.push({ label: t("usageAnalytics.shared.stats.inputImages"), value: inputImageCount });
        if (outputImageCount !== null) tokenStats.push({ label: t("usageAnalytics.shared.stats.outputImages"), value: outputImageCount });
        if (cachedPromptTokens !== null) tokenStats.push({ label: t("usageAnalytics.shared.stats.cachedPrompt"), value: cachedPromptTokens });
        if (cacheWriteTokens !== null) tokenStats.push({ label: t("usageAnalytics.shared.stats.cacheWrite"), value: cacheWriteTokens });
        if (webSearchRequests !== null) tokenStats.push({ label: t("usageAnalytics.shared.stats.webSearches"), value: webSearchRequests });
        const visibleTokenStats = tokenStats.filter((s) => s.value > 0);

        const extraCosts: Array<{ label: string; value: number }> = [];
        if ((request.cost?.cacheReadCost ?? 0) > 0)
          extraCosts.push({ label: t("usageAnalytics.shared.stats.cacheRead"), value: request.cost!.cacheReadCost! });
        if ((request.cost?.cacheWriteCost ?? 0) > 0)
          extraCosts.push({ label: t("usageAnalytics.shared.stats.cacheWrite"), value: request.cost!.cacheWriteCost! });
        if ((request.cost?.reasoningCost ?? 0) > 0)
          extraCosts.push({ label: t("usageAnalytics.shared.stats.reasoning"), value: request.cost!.reasoningCost! });
        if ((request.cost?.requestCost ?? 0) > 0)
          extraCosts.push({ label: t("usageAnalytics.shared.stats.requestFee"), value: request.cost!.requestCost! });
        if ((request.cost?.webSearchCost ?? 0) > 0)
          extraCosts.push({ label: t("usageAnalytics.shared.stats.webSearch"), value: request.cost!.webSearchCost! });
        if (apiCost !== null)
          extraCosts.push({ label: t("usageAnalytics.shared.stats.providerTotal"), value: apiCost });

        const totalCost = getEffectiveTotalCost(request);
        const ts = new Date(request.timestamp);

        return (
          <div className="space-y-5 pb-6">
            {/* Hero */}
            <div className="rounded-xl border border-fg/8 bg-fg/[0.02] p-4">
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span
                      className="inline-flex h-1.5 w-1.5 shrink-0 rounded-full"
                      style={{ backgroundColor: opColor }}
                    />
                    <h2 className="truncate text-[15px] font-semibold tracking-tight text-fg">
                      {request.characterName || t("usageAnalytics.shared.unknown")}
                    </h2>
                  </div>
                  <p className="mt-0.5 truncate font-mono text-[11.5px] text-fg/50">
                    {request.modelName ?? "—"}
                  </p>
                </div>
                <div className="shrink-0 text-right">
                  <div className="text-[15px] font-semibold tabular-nums text-accent">
                    {formatCurrency(totalCost)}
                  </div>
                  <div className="text-[10.5px] tabular-nums text-fg/45">
                    {t("usageAnalytics.shared.tokensSuffix", {
                      count: (request.totalTokens ?? 0).toLocaleString(),
                    })}
                  </div>
                </div>
              </div>
              <div className="mt-3 flex flex-wrap gap-1.5">
                <MetaChip>{ts.toLocaleString()}</MetaChip>
                <MetaChip>{request.providerLabel || request.providerId}</MetaChip>
                <MetaChip>
                  {request.finishReason || t("usageAnalytics.shared.noStopReason")}
                </MetaChip>
              </div>
            </div>

            {/* Token usage */}
            {visibleTokenStats.length > 0 && (
              <section>
                <div className="mb-2 flex items-baseline justify-between">
                  <h3 className="text-[10px] font-semibold uppercase tracking-[0.12em] text-fg/40">
                    {t("usageAnalytics.shared.tokenUsage")}
                  </h3>
                  <span className="text-[11.5px] tabular-nums text-fg/55">
                    {t("usageAnalytics.shared.totalSuffix", {
                      count: (request.totalTokens ?? 0).toLocaleString(),
                    })}
                  </span>
                </div>
                <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-4">
                  {visibleTokenStats.map((s) => (
                    <DetailStat key={s.label} label={s.label} value={s.value.toLocaleString()} />
                  ))}
                </div>
              </section>
            )}

            {/* Cost */}
            <section>
              <div className="mb-2 flex items-baseline justify-between">
                <h3 className="text-[10px] font-semibold uppercase tracking-[0.12em] text-fg/40">
                  {t("usageAnalytics.shared.estimatedCost")}
                </h3>
              </div>
              <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
                <DetailStat
                  label={t("usageAnalytics.shared.stats.prompt")}
                  value={formatCurrency(request.cost?.promptCost || 0)}
                />
                <DetailStat
                  label={t("usageAnalytics.shared.stats.completion")}
                  value={formatCurrency(request.cost?.completionCost || 0)}
                />
                <DetailStat
                  label={t("usageAnalytics.shared.stats.total")}
                  value={formatCurrency(totalCost)}
                  highlight
                />
                {extraCosts.map((c) => (
                  <DetailStat key={c.label} label={c.label} value={formatCurrency(c.value)} />
                ))}
              </div>
            </section>
          </div>
        );
      })()}
    </BottomMenu>
  );
}
