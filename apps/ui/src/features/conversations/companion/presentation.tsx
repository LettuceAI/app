import type { ReactNode } from "react";
import { TrendingDown, TrendingUp } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge, cn, icon, Panel, Scale, Text, Tooltip } from "@lettuceai/crisp";
import type { CompanionEmotionVector, CompanionRelationshipDefaults, CompanionStateView } from "@/api/generated/bindings";

import { axes, type Axis, emotionGroups, numericEntries, signed, referenceFor, dataLabel } from "./values";

export function SectionHead({ title, right }: { title: string; right?: ReactNode }) {
  return <div className="mb-2 flex items-baseline gap-2 px-1"><Text variant="label">{title}</Text>{right && <Text as="span" variant="caption" className="ml-auto text-xs text-fg-3">{right}</Text>}</div>;
}

export function BondTile({ axis, value, reference, compact = false, narrow = false }: { axis: Axis; value: number; reference: number | null; compact?: boolean; narrow?: boolean }) {
  const { t, i18n } = useTranslation("conversations");
  const bipolar = axis === "closeness" || axis === "trust" || axis === "affection";
  const delta = reference === null ? null : Math.round((value - reference) * 100);
  const good = delta !== null && (axis === "tension" ? delta < 0 : delta > 0);
  return (
    <Panel className="gap-0">
      <div className="flex items-center justify-between gap-2">
        <Text as="span" variant="caption" className="text-xs font-medium uppercase tracking-wide text-fg-3">{t(`companion.axes.${axis}.label`)}</Text>
        {delta !== null && delta !== 0 && <Tooltip content={t(delta > 0 ? "companion.aboveDefault" : "companion.belowDefault", { count: Math.abs(delta) })}>
          <Text as="span" variant="caption" className={cn("inline-flex items-center gap-0.5 text-xs tabular-nums", good ? "text-accent" : "text-danger")}>
            {delta > 0 ? <TrendingUp size={icon.xs} /> : <TrendingDown size={icon.xs} />}{signed(value - (reference ?? 0), i18n.language)}
          </Text>
        </Tooltip>}
      </div>
      <Text variant="title" className={cn("mt-0.5 font-semibold tabular-nums text-fg", compact ? "text-lg" : "text-xl")}>{bipolar ? signed(value, i18n.language) : new Intl.NumberFormat(i18n.language).format(Math.round(value * 100))}</Text>
      <Scale value={value} min={bipolar ? -1 : 0} max={1} {...(reference === null ? {} : { reference, referenceLabel: t("companion.defaultMark") })} tone={axis === "tension" ? "warning" : "accent"} negativeTone="danger" size="sm" label={t(`companion.axes.${axis}.label`)} className="mt-2" />
      {!compact && <div className="mt-1 flex justify-between gap-1"><Text as="span" variant="caption" className="text-2xs text-fg-4">{t(`companion.axes.${axis}.low`)}</Text>{bipolar && !narrow && <Text as="span" variant="caption" className="text-2xs text-fg-4">{t(`companion.axes.${axis}.mid`)}</Text>}<Text as="span" variant="caption" className="text-2xs text-fg-4">{t(`companion.axes.${axis}.high`)}</Text></div>}
    </Panel>
  );
}

export function BondTiles({ state, defaults, compact = false, narrow = false }: { state: CompanionStateView; defaults: CompanionRelationshipDefaults; compact?: boolean; narrow?: boolean }) {
  return axes.flatMap((axis) => {
    const value = state.relationship[axis];
    return value === null || (compact && axis === "stability") ? [] : [<BondTile key={axis} axis={axis} value={value} reference={referenceFor(axis, defaults)} compact={compact} narrow={narrow} />];
  });
}

export function EmotionGroup({ group, values, limit = 5 }: { group: (typeof emotionGroups)[number]; values: CompanionEmotionVector; limit?: number }) {
  const { t, i18n } = useTranslation("conversations");
  const entries = numericEntries(values).sort((a, b) => b[1] - a[1]).slice(0, limit);
  return <Panel className="gap-0 p-3.5">
    <Text variant="body" className="text-sm font-medium text-fg">{t(`companion.groups.${group}.title`)}</Text>
    <Text variant="caption" className="text-xs text-fg-3">{t(`companion.groups.${group}.hint`)}</Text>
    {entries.length > 0 ? <div className="mt-3 space-y-2">{entries.map(([emotion, value]) => <div key={emotion}>
      <div className="flex justify-between gap-2"><Text as="span" variant="secondary" className="text-sm text-fg-2">{t(`companion.emotions.${emotion}`)}</Text><Text as="span" variant="caption" className="text-sm tabular-nums text-fg-3">{new Intl.NumberFormat(i18n.language).format(Math.round(value * 100))}</Text></div>
      <Scale value={value} min={0} max={1} tone={group === "expressed" ? "warning" : group === "blocked" ? "danger" : "accent"} size="sm" label={t(`companion.emotions.${emotion}`)} className="mt-1" />
    </div>)}</div> : <Text variant="hint" className="mt-3 text-sm italic text-fg-4">{t("companion.nothingNow")}</Text>}
  </Panel>;
}

export function Drivers({ values }: { values: readonly string[] }) {
  const { t } = useTranslation("conversations");
  if (values.length === 0) return null;
  return <Panel className="mt-2 flex-row flex-wrap items-center gap-2 py-2.5"><Text as="span" variant="caption" className="text-sm text-fg-3">{t("companion.drivers")}</Text>{values.map((driver) => <Badge key={driver}>{dataLabel(driver)}</Badge>)}</Panel>;
}

