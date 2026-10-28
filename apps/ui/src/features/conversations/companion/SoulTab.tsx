import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Lock, LockOpen, Pencil, Sprout, Trash2, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge, Button, cn, Dialog, icon, IconButton, Panel, relativeTime, Text, toast, Tooltip } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { CompanionSoulView, SoulCategory, SoulFactView } from "@/api/generated/bindings";
import { createOperationIds } from "../memory/operation-ids";
import { SectionHead } from "./presentation";
import { companionSoulQuery } from "./queries";
import { useOverlayBack } from "./use-overlay-back";

const soulCategories: readonly SoulCategory[] = ["essence", "traits", "backstory", "appearance", "goals", "likes", "voice", "relational_style", "vulnerabilities", "fears", "habits", "boundaries"];

export function SoulTab({ api, conversationId, soul, onSources }: { api: ApiClient; conversationId: string; soul: CompanionSoulView; onSources: (fact: SoulFactView) => void }) {
  const { t, i18n } = useTranslation("conversations");
  const { t: common } = useTranslation("common");
  const navigate = useNavigate();
  const client = useQueryClient();
  const [ids] = useState(createOperationIds);
  const [pending, setPending] = useState<string | null>(null);
  const [confirmation, setConfirmation] = useState<SoulFactView | "clear" | null>(null);
  useOverlayBack(confirmation !== null, () => setConfirmation(null));
  const query = companionSoulQuery(api, conversationId, soul.character_id);
  const active = soul.growth.facts.filter((fact) => fact.superseded_by === null);
  const categoryText = (category: SoulCategory) => [soul.config.soul[category], ...soul.config.authored_facts.filter((fact) => fact.category === category && fact.superseded_by === null).map((fact) => fact.value)].filter((value) => value.trim()).join("\n");
  const perform = async (fact: SoulFactView | "clear", lock: boolean) => {
    if (pending !== null) return;
    const key = `${fact === "clear" ? "clear" : fact.id}:${lock ? "lock" : "remove"}:${fact === "clear" ? "" : !fact.locked}:${soul.growth.revision}`;
    setPending(key);
    const before = client.getQueryData(query.queryKey);
    if (lock && fact !== "clear") client.setQueryData(query.queryKey, { ...soul, growth: { ...soul.growth, facts: soul.growth.facts.map((entry) => entry.id === fact.id ? { ...entry, locked: !fact.locked } : entry) } });
    const base = { character_id: soul.character_id, conversation_id: conversationId, client_operation_id: ids.idFor(key) };
    try {
      if (fact === "clear") await api.call("companionSoulGrowthClear", base);
      else if (lock) await api.call("companionSoulGrowthLock", { ...base, fact_id: fact.id, locked: !fact.locked });
      else await api.call("companionSoulGrowthRemove", { ...base, fact_id: fact.id });
      ids.settle(key);
      if (!lock) {
        setConfirmation(null);
        toast.success(t(fact === "clear" ? "companion.soul.cleared" : "companion.soul.removed"));
      }
    } catch (error) {
      if (before) client.setQueryData(query.queryKey, before);
      if (isApiFailure(error) && error.code === "conflict") ids.settle(key);
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      await client.invalidateQueries({ queryKey: query.queryKey });
      setPending(null);
    }
  };
  return <div className="space-y-7">
    <section>
      <SectionHead title={t("companion.soul.title")} right={<Button size="sm" variant="ghost" leading={<Pencil size={icon.sm} />} className="-my-1" onClick={() => void navigate({ to: "/characters/$characterId/edit", params: { characterId: soul.character_id }, search: { tab: "soul" } })}>{t("companion.soul.edit")}</Button>} />
      <div className="grid gap-2 @2xl:grid-cols-2 @5xl:grid-cols-3">{soulCategories.map((category) => {
        const text = categoryText(category);
        const core = category === "essence" || category === "traits" || category === "backstory";
        return <Panel key={category} className="gap-0">
          <Text variant="caption" className="flex items-center gap-1.5 text-xs font-medium uppercase tracking-wide text-fg-3">{t(`companion.soul.categories.${category}`)}{core && <Tooltip content={t("companion.soul.core")}><Lock size={icon.xs} aria-label={t("companion.soul.core")} /></Tooltip>}</Text>
          <Text variant="body" className={cn("mt-1 whitespace-pre-line text-sm leading-relaxed", text ? "text-fg" : "italic text-fg-4")}>{text || t("companion.soul.notWritten")}</Text>
        </Panel>;
      })}</div>
    </section>
    <section>
      <SectionHead title={t("companion.soul.growth")} right={soul.growth.facts.length > 0 ? <Button size="sm" variant="ghost" leading={<Trash2 size={icon.sm} />} className="-my-1" disabled={pending !== null} onClick={() => setConfirmation("clear")}>{t("companion.soul.clear")}</Button> : t("companion.soul.zero")} />
      <Text variant="secondary" className="mb-2 px-1 text-sm text-fg-3">{t("companion.soul.intro")}</Text>
      {active.length === 0 ? <Text variant="hint" className="px-1 text-sm italic text-fg-4">{t("companion.soul.empty")}</Text> : <div className="space-y-2">{active.map((fact) => {
        const before = fact.kind === "adjust" ? fact.supersedes.flatMap((id) => {
          const source = [...soul.growth.facts, ...soul.config.authored_facts].find((entry) => entry.id === id);
          return source ? [source.value] : [];
        }) : [];
        return <Panel key={fact.id} className="gap-0">
          <div className="flex items-start gap-2">
            <div className="flex min-w-0 flex-1 flex-wrap items-center gap-1.5 text-xs text-fg-3">
              <Sprout size={icon.xs} className="text-accent" />
              <Text as="span" variant="caption" className="font-medium uppercase tracking-wide text-fg-2">{t(`companion.soul.categories.${fact.category}`)}</Text>
              <Badge>{t(fact.kind === "adjust" ? "companion.soul.adjusted" : "companion.soul.added")}</Badge>
              <Tooltip content={t(`companion.soul.policy.${fact.policy}.hint`)}><span><Badge>{t(`companion.soul.policy.${fact.policy}.label`)}</Badge></span></Tooltip>
              <Text as="span" variant="caption">{relativeTime(fact.created_at, undefined, i18n.language)}</Text>
              {fact.source_memory_ids.length > 0 && <Button variant="ghost" size="sm" className="text-xs underline decoration-fg-4 underline-offset-2" onClick={() => onSources(fact)}>{t("companion.soul.sources", { count: fact.source_memory_ids.length })}</Button>}
              {fact.confidence !== null && <Tooltip content={t("companion.soul.confidence")}><span><Badge>{new Intl.NumberFormat(i18n.language, { style: "percent", maximumFractionDigits: 0 }).format(fact.confidence)}</Badge></span></Tooltip>}
              {fact.locked && <Badge tone="accent"><Lock size={icon.xs} className="mr-1 inline" />{t("companion.soul.locked")}</Badge>}
            </div>
            <Tooltip content={t(fact.policy === "historical" ? "companion.soul.historicalHint" : fact.locked ? "companion.soul.unlockHint" : "companion.soul.lockHint")}><IconButton label={t(fact.locked ? "companion.soul.unlock" : "companion.soul.lock")} size="sm" disabled={fact.policy === "historical" || pending !== null} className={fact.locked ? "text-accent" : undefined} onClick={() => void perform(fact, true)}>{fact.locked ? <Lock size={icon.sm} /> : <LockOpen size={icon.sm} />}</IconButton></Tooltip>
            <IconButton label={t("companion.soul.remove")} size="sm" disabled={pending !== null} onClick={() => setConfirmation(fact)}><X size={icon.sm} /></IconButton>
          </div>
          {before.map((text, index) => <Text key={index} variant="secondary" className="mt-1.5 text-sm leading-relaxed text-fg-4 line-through decoration-fg-4/60">{text}</Text>)}
          <Text variant="body" className="mt-1 text-sm leading-relaxed text-fg">{fact.value}</Text>
        </Panel>;
      })}</div>}
      {soul.growth.superseded_count > 0 && <Text variant="hint" className="mt-2 px-1">{t("companion.soul.retired", { count: soul.growth.superseded_count })}</Text>}
    </section>
    <Dialog open={confirmation !== null} onClose={() => setConfirmation(null)} title={t(confirmation === "clear" ? "companion.soul.clearTitle" : "companion.soul.removeTitle")} description={t(confirmation === "clear" ? "companion.soul.clearBody" : "companion.soul.removeBody")} footer={<div className="flex justify-end gap-2"><Button variant="ghost" disabled={pending !== null} onClick={() => setConfirmation(null)}>{t("companion.soul.keep")}</Button><Button variant="danger" loading={pending !== null} disabled={pending !== null} onClick={() => { if (confirmation !== null) void perform(confirmation, false); }}>{t(confirmation === "clear" ? "companion.soul.clearConfirm" : "companion.soul.removeConfirm")}</Button></div>} />
  </div>;
}
