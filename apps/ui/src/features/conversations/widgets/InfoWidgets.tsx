import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ChevronRight, Pin } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, Button, Chip, cn, icon, InlineMarkdown, Scale, Skeleton, Spinner } from "@lettuceai/crisp";
import type { MemoryCategory } from "@/api/generated/bindings";
import { companionStateQuery } from "../companion/queries";
import { axes, signed } from "../companion/values";
import { useWidgets, type WidgetPerson } from "./context";
import type { WidgetNode } from "./nodes";
import { WIDGET_ICON } from "./catalogue";
import { WidgetCard } from "./WidgetCard";

type Node<T extends WidgetNode["type"]> = Extract<WidgetNode, { type: T }>;

function PersonCard({ node, person, caption, empty }: { node: Node<"character_info" | "persona_info">; person: WidgetPerson | null; caption: string; empty: string }) {
  if (!person) return <WidgetCard design={node.design}><p className="text-sm text-fg-3">{empty}</p></WidgetCard>;
  return (
    <WidgetCard design={node.design}>
      <div className="flex items-center gap-3">
        <Avatar src={person.avatar} name={person.name} size="md" />
        <div className="min-w-0">
          <p className="truncate text-base font-semibold text-fg">{person.name}</p>
          <p className="text-2xs text-fg-3">{caption}</p>
        </div>
      </div>
      {person.description && (
        <p className="mt-2.5 line-clamp-5 text-sm leading-relaxed text-fg-2">
          <InlineMarkdown>{person.description}</InlineMarkdown>
        </p>
      )}
    </WidgetCard>
  );
}

export function CharacterInfoWidget({ node }: { node: Node<"character_info"> }) {
  const { t } = useTranslation("conversations");
  const { character, members } = useWidgets();
  const chosen = node.character_id ? members.find((member) => member.id === node.character_id) : undefined;
  return <PersonCard node={node} person={chosen ?? character ?? members[0] ?? null} caption={t("widgets.config.characterInfo.character")} empty={t("widgets.characterInfo.noCharacter")} />;
}

export function PersonaInfoWidget({ node }: { node: Node<"persona_info"> }) {
  const { t } = useTranslation("conversations");
  const { persona, personaLoading } = useWidgets();
  if (personaLoading) return <WidgetCard design={node.design}><Skeleton shape="block" height={56} /></WidgetCard>;
  return <PersonCard node={node} person={persona} caption={t("persona.title")} empty={t("widgets.personaInfo.noPersona")} />;
}

export function SessionInfoWidget({ node }: { node: Node<"session_info"> }) {
  const { t, i18n } = useTranslation("conversations");
  const { messageCount, sceneName, memoryOn, memories } = useWidgets();
  const number = new Intl.NumberFormat(i18n.language);
  const rows: { label: string; value: string | null }[] = [
    { label: t("widgets.sessionInfo.messages"), value: messageCount === undefined ? null : number.format(messageCount) },
    { label: t("widgets.sessionInfo.scene"), value: sceneName ?? t("widgets.sessionInfo.none") },
    { label: t("widgets.sessionInfo.memories"), value: memoryOn ? number.format(memories.length) : t("settings.memory.modes.disabled") },
  ];
  return (
    <WidgetCard design={node.design} title={node.title || t("widgets.types.session_info.label")} icon={WIDGET_ICON.session_info}>
      <dl className="space-y-1.5">
        {rows.map((row) => (
          <div key={row.label} className="flex items-baseline justify-between gap-3 text-sm">
            <dt className="text-fg-3">{row.label}</dt>
            <dd className="min-w-0 truncate text-right font-medium tabular-nums text-fg-2">{row.value ?? <Skeleton shape="text" width={32} />}</dd>
          </div>
        ))}
      </dl>
    </WidgetCard>
  );
}

export function CompanionStateWidget({ node }: { node: Node<"companion_state"> }) {
  const { t, i18n } = useTranslation("conversations");
  const { api, conversationId, companion, isGroup } = useWidgets();
  const supported = companion && !isGroup;
  const state = useQuery({ ...companionStateQuery(api, conversationId), enabled: supported });
  const number = new Intl.NumberFormat(i18n.language);
  let body;
  if (!supported) body = <p className="text-sm text-fg-3">{t("widgets.companionState.companionOnly")}</p>;
  else if (state.isPending) body = <Skeleton shape="block" height={96} />;
  else if (state.isError) body = <p className="text-sm text-fg-3">{t("settings.loadFailed")}</p>;
  else if (!state.data) body = <p className="text-sm text-fg-3">{t("widgets.companionState.noData")}</p>;
  else {
    const relationship = state.data.relationship;
    body = (
      <div className="space-y-3">
        {axes.map((axis) => {
          const value = relationship[axis];
          if (value === null) return null;
          const bipolar = axis === "closeness" || axis === "trust" || axis === "affection";
          return (
            <div key={axis}>
              <div className="flex items-baseline justify-between gap-2 text-sm">
                <span className="text-fg-3">{t(`companion.axes.${axis}.label`)}</span>
                <span className="tabular-nums text-fg-2">{bipolar ? signed(value, i18n.language) : number.format(Math.round(value * 100))}</span>
              </div>
              <Scale value={value} min={bipolar ? -1 : 0} max={1} tone={axis === "tension" ? "warning" : "accent"} label={t(`companion.axes.${axis}.label`)} className="mt-1.5" />
              <div className="mt-1 flex justify-between gap-1 text-2xs text-fg-4">
                <span>{t(`companion.axes.${axis}.low`)}</span>
                <span>{t(`companion.axes.${axis}.high`)}</span>
              </div>
            </div>
          );
        })}
      </div>
    );
  }
  return (
    <WidgetCard design={node.design} title={node.title || t("widgets.types.companion_state.label")} icon={WIDGET_ICON.companion_state}>
      {body}
    </WidgetCard>
  );
}

export function MemoryWidget({ node }: { node: Node<"memory"> }) {
  const { t } = useTranslation("conversations");
  const { memoryOn, memories, memoryBusy, openMemories } = useWidgets();
  const [category, setCategory] = useState<MemoryCategory | null>(null);
  const limit = node.limit ?? 10;
  const items = useMemo(() => [...memories].sort((a, b) => Number(b.pinned) - Number(a.pinned)), [memories]);
  const categories = useMemo(() => [...new Set(items.flatMap((item) => (item.category ? [item.category] : [])))], [items]);
  const filtered = category ? items.filter((item) => item.category === category) : items;
  const shown = filtered.slice(0, limit);
  return (
    <WidgetCard
      design={node.design}
      title={node.title || t("widgets.types.memory.label")}
      icon={WIDGET_ICON.memory}
      trailing={
        <>
          {memoryBusy && <Spinner size="sm" label={t("memory.status.runningTitle")} />}
          {memoryOn && (
            <Button variant="ghost" size="sm" trailing={<ChevronRight size={icon.xs} />} onClick={openMemories}>
              {t("widgets.memory.manage")}
            </Button>
          )}
        </>
      }
    >
      {!memoryOn ? (
        <p className="text-sm text-fg-3">{t("settings.memory.off")}</p>
      ) : (
        <div className="space-y-2.5">
          {categories.length > 1 && (
            <div className="flex flex-wrap gap-1.5">
              <Chip selected={category === null} onClick={() => setCategory(null)}>{t("widgets.memory.all")}</Chip>
              {categories.map((entry) => (
                <Chip key={entry} selected={category === entry} onClick={() => setCategory(entry)}>{t(`memory.categories.${entry}`)}</Chip>
              ))}
            </div>
          )}
          {shown.length === 0 ? (
            <p className="text-sm text-fg-3">{t("widgets.memory.empty")}</p>
          ) : (
            <ul className="space-y-1.5">
              {shown.map((item) => (
                <li key={item.id}>
                  <button type="button" onClick={openMemories} title={item.text} className="flex w-full items-start gap-2 rounded-md text-left text-sm leading-snug text-fg-2 transition-colors hover:text-fg">
                    {item.pinned ? <Pin size={icon.xs} className="mt-1 shrink-0 text-accent" aria-hidden /> : <span aria-hidden className="mt-[7px] h-1 w-1 shrink-0 rounded-full bg-fg-3" />}
                    <span className={cn("line-clamp-2 min-w-0", item.temperature === "cold" && "text-fg-3")}>{item.text}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
          {filtered.length > limit && (
            <Button variant="ghost" size="sm" onClick={openMemories}>{t("widgets.memory.more", { count: filtered.length - limit })}</Button>
          )}
        </div>
      )}
    </WidgetCard>
  );
}
