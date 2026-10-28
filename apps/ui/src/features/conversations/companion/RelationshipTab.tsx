import { Bot, Sprout, User } from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Button, EmptyState, icon, relativeTime, StatTile, Text } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { CompanionSoulView, CompanionStateView, MemoryItemView, ThymosStatus } from "@/api/generated/bindings";
import { BondTiles, Drivers, EmotionGroup, SectionHead } from "./presentation";
import { emotionGroups } from "./values";
import { EmotionInstallNotice } from "./EmotionInstallNotice";

export function RelationshipTab({ api, state, soul, emotion, items, name, phone, firstMessageAt, onOpen, timelineFallback }: {
  api: ApiClient; state: CompanionStateView | null; soul: CompanionSoulView; emotion: ThymosStatus; items: readonly MemoryItemView[] | null; name: string; phone: boolean; firstMessageAt: number | null; onOpen: (item: MemoryItemView) => void; timelineFallback?: ReactNode;
}) {
  const { t, i18n } = useTranslation("conversations");
  if (state === null) return <EmptyState icon={<Sprout size={icon.xl} />} title={t("companion.nothingTitle")} description={t("companion.nothingBody", { name })} />;
  const moments = (items ?? []).filter((item) => item.category === "relationship" || item.category === "milestone" || item.category === "emotional_snapshot").toSorted((a, b) => (b.observed_at ?? -Infinity) - (a.observed_at ?? -Infinity));
  return <div className="space-y-7">
    <section>
      <SectionHead title={t("companion.bond")} right={state.relationship.last_interaction_at > 0 ? t("companion.lastTalked", { when: relativeTime(state.relationship.last_interaction_at, undefined, i18n.language).toLowerCase() }) : undefined} />
      <div className="grid grid-cols-2 gap-2 @2xl:grid-cols-3 @6xl:grid-cols-6">
        <BondTiles state={state} defaults={soul.config.relationship_defaults} narrow={phone} />
        <StatTile label={t("companion.messages")} value={new Intl.NumberFormat(i18n.language).format(state.relationship.interaction_count)} {...(firstMessageAt === null ? {} : { sub: t("companion.togetherSince", { date: new Intl.DateTimeFormat(i18n.language, { month: "short", day: "numeric" }).format(firstMessageAt) }) })} />
      </div>
    </section>
    <section>
      <SectionHead title={t("companion.engine")} right={emotion.type === "installed" ? t("companion.updated", { when: relativeTime(state.emotions_updated_at, undefined, i18n.language).toLowerCase() }) : undefined} />
      {emotion.type !== "installed" ? <EmotionInstallNotice api={api} /> : <><div className="grid gap-2 @2xl:grid-cols-2 @6xl:grid-cols-4">{emotionGroups.map((group) => <EmotionGroup key={group} group={group} values={state[group]} />)}</div><Drivers values={state.active_drivers} /></>}
    </section>
    {(items !== null || timelineFallback) && <section>
      <SectionHead title={t("companion.timeline")} right={items === null ? undefined : t("companion.events", { count: moments.length })} />
      {timelineFallback}
      <ol className="grid gap-2 @2xl:grid-cols-2 @6xl:grid-cols-3">{moments.map((item) => <li key={item.id}>
        <Button variant="ghost" onClick={() => onOpen(item)} className="block h-full w-full whitespace-normal rounded-xl border border-line bg-surface-1 px-3.5 py-3 text-left font-normal hover:bg-fill">
          <span className="block min-w-0 w-full">
          <span className="flex flex-wrap items-center gap-x-2 text-xs text-fg-3">
            <Text as="span" variant="caption" className="font-medium uppercase tracking-wide text-fg-2">{item.category ? t(`memory.categories.${item.category}`) : null}</Text>
            {item.observed_at !== null && <Text as="span" variant="caption">{relativeTime(item.observed_at, undefined, i18n.language)}</Text>}
            <Text as="span" variant="caption" className="inline-flex items-center gap-1">{item.origin === "model" ? <Bot size={icon.xs} /> : <User size={icon.xs} />}{t(`memory.origins.${item.origin}`)}</Text>
          </span>
          <Text as="span" variant="body" className="mt-1.5 block text-sm leading-relaxed text-fg">{item.text}</Text>
          </span>
        </Button>
      </li>)}</ol>
    </section>}
  </div>;
}
