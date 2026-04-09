import { Zap, ChevronRight } from "lucide-react";
import { BottomMenu } from "../../components";
import { RequestUsage } from "../../../core/usage";
import { typography, components, cn } from "../../design-tokens";

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

export function getRelativeTime(timestamp: number): string {
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

export function getOperationColor(type: string): string {
  const colorsMap: Record<string, string> = {
    chat: "var(--color-info)",
    regenerate: "var(--color-secondary)",
    continue: "#22d3ee",
    summary: "var(--color-warning)",
    memory_manager: "var(--color-accent)",
    image_generation: "var(--color-accent)",
    group_chat_message: "var(--color-info)",
    group_chat_regenerate: "var(--color-secondary)",
    group_chat_continue: "#22d3ee",
    group_chat_decision_maker: "var(--color-warning)",
  };
  return colorsMap[type.toLowerCase()] || "#94a3b8";
}

export function getOperationLabel(type: string): string {
  const labels: Record<string, string> = {
    chat: "Chat",
    regenerate: "Regen",
    continue: "Continue",
    summary: "Summary",
    memory_manager: "Memory",
    image_generation: "Image Gen",
    group_chat_message: "Group Chat",
    group_chat_regenerate: "Group Regen",
    group_chat_continue: "Group Continue",
    group_chat_decision_maker: "Decision Maker",
  };
  return labels[type.toLowerCase()] || type;
}

function parseMetadataNumber(metadata: RequestUsage["metadata"], key: string): number | null {
  const raw = metadata?.[key];
  if (!raw) return null;
  const parsed = Number(raw);
  return Number.isFinite(parsed) ? parsed : null;
}

function DetailStat({ label, value }: { label: string; value: string }) {
  return (
    <div className={cn("px-3 py-2.5", components.card.base, "bg-fg/5 border-fg/10")}>
      <div
        className={cn(
          typography.overline.size,
          typography.overline.weight,
          typography.overline.tracking,
          typography.overline.transform,
          "text-fg/40",
        )}
      >
        {label}
      </div>
      <div className={cn(typography.body.size, typography.body.weight, "mt-1 text-fg")}>
        {value}
      </div>
    </div>
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
  const clickable = Boolean(onClick);
  const opColor = getOperationColor(request.operationType);

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
            {request.characterName || "Unknown"}
          </span>
          <span
            className="rounded-lg px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider"
            style={{
              backgroundColor: bgStyle,
              color: opColor,
            }}
          >
            {getOperationLabel(request.operationType)}
          </span>
        </div>
        <div className={cn(typography.caption.size, "mt-0.5 flex items-center gap-2 text-fg/40")}>
          <span>{formatCompactNumber(request.totalTokens || 0)} tokens</span>
          <span className="opacity-30">·</span>
          <span>{getRelativeTime(request.timestamp)}</span>
        </div>
      </div>
      <div className="shrink-0 text-right">
        <p className={cn(typography.body.size, typography.body.weight, "text-accent")}>
          {formatCurrency(request.cost?.totalCost || 0)}
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

  return (
    <BottomMenu
      isOpen={isOpen}
      onClose={onClose}
      title={request ? getOperationLabel(request.operationType) : "Request Details"}
      includeExitIcon={false}
    >
      {request && (
        <div className="space-y-6 pb-8">
          <div className={cn("p-5", components.card.base, "bg-fg/5 border-fg/10")}>
            <div className={cn(typography.h2.size, typography.h2.weight, "text-fg")}>
              {request.characterName}
            </div>
            <div className={cn(typography.body.size, "mt-1 text-fg/50")}>{request.modelName}</div>
            <div className="mt-4 flex flex-wrap gap-3">
              <div className="px-2 py-1 rounded-md bg-fg/5 text-[10px] font-medium text-fg/40 uppercase tracking-wider border border-fg/5">
                {new Date(request.timestamp).toLocaleTimeString()}
              </div>
              <div className="px-2 py-1 rounded-md bg-fg/5 text-[10px] font-medium text-fg/40 uppercase tracking-wider border border-fg/5">
                {request.providerLabel || request.providerId}
              </div>
              <div className="px-2 py-1 rounded-md bg-fg/5 text-[10px] font-medium text-fg/40 uppercase tracking-wider border border-fg/5">
                {request.finishReason || "No stop reason"}
              </div>
            </div>
          </div>

          <div className="space-y-3">
            <div
              className={cn(
                typography.overline.size,
                typography.overline.weight,
                typography.overline.tracking,
                "text-fg/40 ml-1",
              )}
            >
              Token Usage
            </div>
            <div className="grid grid-cols-2 gap-2.5">
              <DetailStat label="Prompt" value={(request.promptTokens ?? 0).toLocaleString()} />
              <DetailStat
                label="Completion"
                value={(request.completionTokens ?? 0).toLocaleString()}
              />
              <DetailStat label="Total" value={(request.totalTokens ?? 0).toLocaleString()} />
              <DetailStat
                label="Reasoning"
                value={(request.reasoningTokens ?? 0).toLocaleString()}
              />
              <DetailStat label="Memory" value={(request.memoryTokens ?? 0).toLocaleString()} />
              <DetailStat label="Summary" value={(request.summaryTokens ?? 0).toLocaleString()} />
              {cachedPromptTokens !== null && (
                <DetailStat label="Cached Prompt" value={cachedPromptTokens.toLocaleString()} />
              )}
              {cacheWriteTokens !== null && (
                <DetailStat label="Cache Write" value={cacheWriteTokens.toLocaleString()} />
              )}
              {webSearchRequests !== null && (
                <DetailStat label="Web Searches" value={webSearchRequests.toLocaleString()} />
              )}
            </div>
          </div>

          <div className="space-y-3">
            <div
              className={cn(
                typography.overline.size,
                typography.overline.weight,
                typography.overline.tracking,
                "text-fg/40 ml-1",
              )}
            >
              Estimated Cost
            </div>
            <div className="grid grid-cols-3 gap-2.5">
              <DetailStat label="Prompt" value={formatCurrency(request.cost?.promptCost || 0)} />
              <DetailStat
                label="Completion"
                value={formatCurrency(request.cost?.completionCost || 0)}
              />
              <DetailStat label="Total" value={formatCurrency(request.cost?.totalCost || 0)} />
            </div>
            {(request.cost?.cacheReadCost ||
              request.cost?.cacheWriteCost ||
              request.cost?.reasoningCost ||
              request.cost?.requestCost ||
              request.cost?.webSearchCost ||
              apiCost !== null) && (
              <div className="grid grid-cols-2 gap-2.5">
                {(request.cost?.cacheReadCost ?? 0) > 0 && (
                  <DetailStat
                    label="Cache Read"
                    value={formatCurrency(request.cost?.cacheReadCost || 0)}
                  />
                )}
                {(request.cost?.cacheWriteCost ?? 0) > 0 && (
                  <DetailStat
                    label="Cache Write"
                    value={formatCurrency(request.cost?.cacheWriteCost || 0)}
                  />
                )}
                {(request.cost?.reasoningCost ?? 0) > 0 && (
                  <DetailStat
                    label="Reasoning"
                    value={formatCurrency(request.cost?.reasoningCost || 0)}
                  />
                )}
                {(request.cost?.requestCost ?? 0) > 0 && (
                  <DetailStat
                    label="Request Fee"
                    value={formatCurrency(request.cost?.requestCost || 0)}
                  />
                )}
                {(request.cost?.webSearchCost ?? 0) > 0 && (
                  <DetailStat
                    label="Web Search"
                    value={formatCurrency(request.cost?.webSearchCost || 0)}
                  />
                )}
                {apiCost !== null && (
                  <DetailStat label="Provider Total" value={formatCurrency(apiCost)} />
                )}
              </div>
            )}
          </div>
        </div>
      )}
    </BottomMenu>
  );
}
