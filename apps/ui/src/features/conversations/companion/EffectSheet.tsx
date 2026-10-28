import { useQuery } from "@tanstack/react-query";
import { Sprout } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge, cn, icon, ListGroup, ListRow, ListRowButton, Sheet, Skeleton, Text } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { MessageCompanionEffect, TimelineMessage } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { textOf } from "../data/messages";
import { changedAxes, dataLabel, numericEntries, signed } from "./values";
import { companionEffectQuery } from "./queries";
import { useOverlayBack } from "./use-overlay-back";

export function CompanionEffectRow({ api, conversationId, message, onOpen }: { api: ApiClient; conversationId: string; message: TimelineMessage; onOpen: () => void }) {
  const { t, i18n } = useTranslation("conversations");
  const effect = useQuery(companionEffectQuery(api, conversationId, message.id));
  if (effect.isPending) return <Skeleton className="h-10 w-full" />;
  if (effect.isError) return <FailureState error={effect.error} onRetry={() => void effect.refetch()} size="sm" />;
  if (effect.data === null) return null;
  const value = effect.data;
  const parts = changedAxes(value.relationship).slice(0, 2).map(([axis, delta]) => t("companion.effect.axisSummary", { value: signed(delta, i18n.language), axis: t(`companion.axes.${axis}.label`).toLowerCase() }));
  const memories = value.memories_added.length + value.memories_updated.length + value.memories_superseded.length;
  if (memories > 0) parts.push(t("companion.effect.memoryCount", { count: memories }));
  const description = value.status === "processing" ? t("companion.effect.processing") : value.status === "failed" ? t("companion.effect.failed") : value.status === "invalidated" ? t("companion.effect.invalidated") : value.summary ?? (parts.join(", ") || t("companion.effect.none"));
  return <ListGroup><ListRowButton icon={<Sprout size={icon.lg} />} title={t("companion.effect.title")} description={description} onClick={onOpen} /></ListGroup>;
}

function EffectDetails({ effect }: { effect: MessageCompanionEffect }) {
  const { t, i18n } = useTranslation("conversations");
  if (effect.status === "processing") return <div className="space-y-5 pb-2" role="status" aria-label={t("companion.effect.processing")}><Skeleton className="h-20 w-full" /><Skeleton className="h-20 w-full" /><Skeleton className="h-12 w-full" /></div>;
  if (effect.status === "failed" || effect.status === "invalidated") return <Text variant="secondary">{t(effect.status === "failed" ? "companion.effect.failed" : "companion.effect.invalidated")}</Text>;
  const bond = changedAxes(effect.relationship);
  const felt = numericEntries(effect.felt).filter(([, value]) => value !== 0);
  const remembered = [...effect.memories_added, ...effect.memories_updated, ...effect.memories_superseded];
  return <div className="space-y-5 pb-2">
    {bond.length > 0 && <ListGroup label={t("companion.bond")}>{bond.map(([axis, value]) => <ListRow key={axis} title={t(`companion.axes.${axis}.label`)} className="py-2.5" trailing={<Text as="span" variant="body" className={cn("tabular-nums", (axis === "tension" ? value < 0 : value > 0) ? "text-accent" : "text-danger")}>{signed(value, i18n.language)}</Text>} />)}</ListGroup>}
    {felt.length > 0 && <ListGroup label={t("companion.effect.feels")}>{felt.map(([emotion, value]) => <ListRow key={emotion} title={t(`companion.emotions.${emotion}`)} className="py-2.5" trailing={<Text as="span" variant="body" className={cn("tabular-nums", value > 0 ? "text-fg" : "text-fg-3")}>{signed(value, i18n.language)}</Text>} />)}</ListGroup>}
    {(effect.signals_added.length > 0 || effect.signals_removed.length > 0) && <div className="flex flex-wrap items-center gap-2 px-1">
      {effect.signals_added.length > 0 && <><Text as="span" variant="caption" className="text-sm text-fg-3">{t("companion.effect.pickedUp")}</Text>{effect.signals_added.map((signal) => <Badge key={signal} tone="accent">{dataLabel(signal)}</Badge>)}</>}
      {effect.signals_removed.length > 0 && <><Text as="span" variant="caption" className="ml-2 text-sm text-fg-3">{t("companion.effect.letGo")}</Text>{effect.signals_removed.map((signal) => <Badge key={signal}>{dataLabel(signal)}</Badge>)}</>}
    </div>}
    {remembered.length > 0 && <ListGroup label={t("companion.effect.remembered")}>{remembered.map((text, index) => <Text key={index} variant="body" className="px-4 py-3 text-base leading-snug text-fg">{text}</Text>)}</ListGroup>}
    {bond.length === 0 && felt.length === 0 && remembered.length === 0 && effect.signals_added.length === 0 && effect.signals_removed.length === 0 && <Text variant="hint">{t("companion.effect.none")}</Text>}
  </div>;
}

export function EffectSheet({ api, conversationId, message, open, onClose }: { api: ApiClient; conversationId: string; message: TimelineMessage; open: boolean; onClose: () => void }) {
  const { t } = useTranslation("conversations");
  const effect = useQuery({ ...companionEffectQuery(api, conversationId, message.id), enabled: open });
  useOverlayBack(open, onClose);
  return <Sheet open={open} onClose={onClose} placement="auto" size="md" title={t("companion.effect.title")} description={textOf(message) ? <span className="line-clamp-2">{t("companion.effect.quote", { text: textOf(message) })}</span> : undefined}>
    {effect.isPending ? <Skeleton className="h-20 w-full" /> : effect.isError ? <FailureState error={effect.error} onRetry={() => void effect.refetch()} /> : effect.data === null ? <Text variant="hint">{t("companion.effect.none")}</Text> : <EffectDetails effect={effect.data} />}
  </Sheet>;
}
