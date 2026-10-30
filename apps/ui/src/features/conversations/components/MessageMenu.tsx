import { useQuery } from "@tanstack/react-query";
import { Copy, GitBranch, History, Pencil, Pin, PinOff, RefreshCw, SlidersHorizontal, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { icon, ListGroup, ListRowButton, Skeleton } from "@lettuceai/crisp";
import { object } from "@/entities/preferences";
import type { ApiClient } from "@/api/client";
import type { TimelineMessage } from "@/api/generated/bindings";
import { statsOf } from "../data/messages";
import { metricsQuery, usageQuery } from "../data/queries";
import { MessagePromptDetails, SpeakerSelectionReason } from "./MessagePromptDetails";
import { MessageSources } from "./MessageSources";
import { CompanionEffectRow } from "../companion/EffectSheet";

export interface MessageMenuProps {
  companion?: boolean;
  onCompanionEffect?: () => void;
  api: ApiClient;
  conversationId: string;
  message: TimelineMessage;
  after: number;
  busy: boolean;
  canRegenerate: boolean;
  onCopy: () => void;
  onEdit: () => void;
  onRegenerate: () => void;
  onRegenerateOptions: () => void;
  canEdit: boolean;
  onPin: () => void;
  onRewind: () => void;
  onBranch?: () => void;
  groupBranch?: boolean;
  onDelete: () => void;
}

export function MessageStats({ api, conversationId, message }: { api: ApiClient; conversationId: string; message: TimelineMessage }) {
  const { t, i18n } = useTranslation("conversations");
  const metric = useQuery({ ...metricsQuery(api, conversationId, message.id), enabled: message.role === "assistant" });
  const usage = useQuery({ ...usageQuery(api, conversationId, message.id), enabled: message.role === "assistant" });
  if (message.role !== "assistant") return null;
  if (metric.isPending || usage.isPending) {
    return (
      <div className="space-y-1.5 px-1" aria-hidden="true">
        <Skeleton height={14} width="80%" />
      </div>
    );
  }
  const stats = statsOf(metric.data?.summary, usage.data);
  if (!stats) return null;
  const number = new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 1 });
  const total =
    stats.promptTokens !== null && stats.completionTokens !== null
      ? stats.promptTokens + stats.completionTokens
      : (usage.data?.total_tokens ?? null);
  const mtp = object(object(metric.data?.summary).mtpStats);
  const timing = [
    ...(stats.firstTokenSeconds !== null
      ? [{ key: "ttft", title: t("menu.stats.firstToken"), value: t("menu.stats.firstTokenValue", { value: number.format(stats.firstTokenSeconds * 1000) }) }]
      : []),
    ...(stats.tokensPerSecond !== null ? [{ key: "speed", title: t("menu.stats.speed"), value: t("menu.stats.speedValue", { value: number.format(stats.tokensPerSecond) }) }] : []),
    ...(typeof mtp.tokensPerRound === "number"
      ? [{
          key: "mtp",
          title: t("menu.stats.mtp"),
          value: t("menu.stats.mtpValue", { value: mtp.tokensPerRound.toFixed(2) }) + (typeof mtp.draftAcceptance === "number" ? ` · ${Math.round(mtp.draftAcceptance * 100)}%` : ""),
        }]
      : []),
  ];
  return (
    <div className="space-y-1.5 px-1 text-xs text-fg-3">
      <div className="flex items-center gap-x-3">
        {(stats.promptTokens !== null || stats.completionTokens !== null) && (
          <span className="flex shrink-0 items-center gap-2 border-r border-line pr-3 tabular-nums">
            {stats.promptTokens !== null && <span title={t("menu.stats.prompt")}>{t("menu.stats.promptValue", { value: number.format(stats.promptTokens) })}</span>}
            {stats.completionTokens !== null && <span title={t("menu.stats.reply")}>{t("menu.stats.replyValue", { value: number.format(stats.completionTokens) })}</span>}
          </span>
        )}
        <span className="min-w-0 flex-1 truncate text-fg-2">{stats.model}</span>
        {total !== null && (
          <span className="shrink-0 tabular-nums">
            {number.format(total)} <span className="text-2xs uppercase opacity-60">{t("menu.stats.total")}</span>
          </span>
        )}
      </div>
      {timing.length > 0 && (
        <div className="flex items-center gap-3 text-2xs tabular-nums">
          {timing.map((item) => <span key={item.key} title={item.title}>{item.value}</span>)}
        </div>
      )}
    </div>
  );
}

export function MessageMenu({ api, conversationId, message, after, busy, canEdit, canRegenerate, onCopy, onEdit, onRegenerate, onRegenerateOptions, onPin, onRewind, onBranch, groupBranch = false, onDelete, companion = false, onCompanionEffect }: MessageMenuProps) {
  const { t } = useTranslation("conversations");
  const scene = message.role === "scene";
  return (
    <div className="space-y-3 pb-1">
      <MessageStats api={api} conversationId={conversationId} message={message} />
      {groupBranch && message.role === "assistant" && <SpeakerSelectionReason api={api} conversationId={conversationId} messageId={message.id} />}
      {companion && message.role === "assistant" && onCompanionEffect && <CompanionEffectRow key={message.id} api={api} conversationId={conversationId} message={message} onOpen={onCompanionEffect} />}
      {message.role === "assistant" && <MessageSources key={message.id} api={api} conversationId={conversationId} messageId={message.id} />}
      <ListGroup>
        {!scene && <ListRowButton icon={<Copy size={icon.lg} />} title={t("menu.copy")} chevron={false} onClick={onCopy} />}
        {canEdit && <ListRowButton icon={<Pencil size={icon.lg} />} title={t("menu.edit")} onClick={onEdit} />}
        {canRegenerate && (
          <>
          <ListRowButton
            icon={<RefreshCw size={icon.lg} />}
            tone="secondary"
            title={t("menu.regenerate")}
            description={busy ? t("menu.regenerateBusy") : t("menu.regenerateHint")}
            chevron={false}
            disabled={busy}
            onClick={onRegenerate}
          />
          <ListRowButton
            icon={<SlidersHorizontal size={icon.lg} />}
            title={t("regeneration.title")}
            description={busy ? t("menu.regenerateBusy") : t(groupBranch ? "regeneration.groupHint" : "regeneration.hint")}
            disabled={busy}
            onClick={onRegenerateOptions}
          />
          </>
        )}
        {!scene && (
          <ListRowButton
            icon={message.pinned ? <PinOff size={icon.lg} /> : <Pin size={icon.lg} />}
            tone="info"
            title={message.pinned ? t("menu.unpin") : t("menu.pin")}
            {...(message.pinned ? {} : { description: t("menu.pinHint") })}
            chevron={false}
            onClick={onPin}
          />
        )}
      </ListGroup>
      {message.role === "assistant" && <MessagePromptDetails api={api} conversationId={conversationId} messageId={message.id} />}
      {(after > 0 || !scene) && (
        <ListGroup>
          {after > 0 && (
            <ListRowButton
              icon={<History size={icon.lg} />}
              title={t("menu.rewind")}
              description={t("menu.rewindHint", { count: after })}
              chevron={false}
              onClick={onRewind}
            />
          )}
          {!scene && onBranch && (
            <ListRowButton
              icon={<GitBranch size={icon.lg} />}
              title={t(groupBranch ? "menu.branchGroup" : "menu.branch")}
              description={busy ? t("menu.regenerateBusy") : t(groupBranch ? "menu.branchGroupHint" : "menu.branchHint")}
              chevron={false}
              disabled={busy}
              onClick={onBranch}
            />
          )}
          {!scene && (
            <ListRowButton
              icon={<Trash2 size={icon.lg} />}
              tone="danger"
              title={<span className="text-danger">{t("menu.delete")}</span>}
              {...(message.pinned ? { description: t("menu.deleteHint") } : {})}
              chevron={false}
              disabled={message.pinned}
              onClick={onDelete}
            />
          )}
        </ListGroup>
      )}
    </div>
  );
}
