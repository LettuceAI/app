import { useQuery } from "@tanstack/react-query";
import { Copy, GitBranch, History, Pencil, Pin, PinOff, RefreshCw, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { icon, ListGroup, ListRowButton, Skeleton } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { TimelineMessage } from "@/api/generated/bindings";
import { statsOf } from "../data/messages";
import { metricsQuery } from "../data/queries";
import { MessagePromptDetails } from "./MessagePromptDetails";

export interface MessageMenuProps {
  api: ApiClient;
  conversationId: string;
  message: TimelineMessage;
  after: number;
  busy: boolean;
  canRegenerate: boolean;
  onCopy: () => void;
  onEdit: () => void;
  onRegenerate: () => void;
  canEdit: boolean;
  onPin: () => void;
  onRewind: () => void;
  onBranch?: () => void;
  onDelete: () => void;
}

export function MessageStats({ api, conversationId, message }: { api: ApiClient; conversationId: string; message: TimelineMessage }) {
  const { t, i18n } = useTranslation("conversations");
  const metric = useQuery({ ...metricsQuery(api, conversationId, message.id), enabled: message.role === "assistant" });
  if (message.role !== "assistant") return null;
  if (metric.isPending) {
    return (
      <div className="space-y-2.5 px-1" aria-hidden="true">
        <Skeleton height={20} width="55%" />
        <Skeleton height={34} width="100%" />
      </div>
    );
  }
  const stats = statsOf(metric.data?.summary);
  if (!stats) return null;
  const number = new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 1 });
  const items = [
    ...(stats.completionTokens !== null ? [{ label: t("menu.stats.reply"), value: t("menu.stats.replyTokens", { count: stats.completionTokens }) }] : []),
    ...(stats.tokensPerSecond !== null ? [{ label: t("menu.stats.speed"), value: t("menu.stats.speedValue", { value: number.format(stats.tokensPerSecond) }) }] : []),
    ...(stats.firstTokenSeconds !== null
      ? [{ label: t("menu.stats.firstToken"), value: t("menu.stats.firstTokenValue", { value: new Intl.NumberFormat(i18n.language, { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(stats.firstTokenSeconds) }) }]
      : []),
    ...(stats.promptTokens !== null ? [{ label: t("menu.stats.prompt"), value: t("menu.stats.promptTokens", { count: stats.promptTokens }) }] : []),
  ];
  return (
    <div className="space-y-2.5 px-1">
      {stats.model && <p className="truncate text-sm text-fg-2">{stats.model}</p>}
      {items.length > 0 && (
        <dl className="flex flex-wrap gap-x-6 gap-y-2">
          {items.map((item) => (
            <div key={item.label}>
              <dt className="text-2xs text-fg-3">{item.label}</dt>
              <dd className="text-sm tabular-nums text-fg">{item.value}</dd>
            </div>
          ))}
        </dl>
      )}
    </div>
  );
}

export function MessageMenu({ api, conversationId, message, after, busy, canEdit, canRegenerate, onCopy, onEdit, onRegenerate, onPin, onRewind, onBranch, onDelete }: MessageMenuProps) {
  const { t } = useTranslation("conversations");
  const scene = message.role === "scene";
  return (
    <div className="space-y-3 pb-1">
      <MessageStats api={api} conversationId={conversationId} message={message} />
      <ListGroup>
        {!scene && <ListRowButton icon={<Copy size={icon.lg} />} title={t("menu.copy")} chevron={false} onClick={onCopy} />}
        {canEdit && <ListRowButton icon={<Pencil size={icon.lg} />} title={t("menu.edit")} onClick={onEdit} />}
        {canRegenerate && (
          <ListRowButton
            icon={<RefreshCw size={icon.lg} />}
            tone="secondary"
            title={t("menu.regenerate")}
            description={busy ? t("menu.regenerateBusy") : t("menu.regenerateHint")}
            chevron={false}
            disabled={busy}
            onClick={onRegenerate}
          />
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
              title={t("menu.branch")}
              description={busy ? t("menu.regenerateBusy") : t("menu.branchHint")}
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
