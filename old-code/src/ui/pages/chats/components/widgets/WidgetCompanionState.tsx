import { Sparkles } from "lucide-react";
import type { CompanionStateNode } from "../../../../../core/storage/chatWidgetSchemas";
import { cn } from "../../../../design-tokens";
import { useI18n, type TranslationKey } from "../../../../../core/i18n/context";
import { useWidgetAvailability, useWidgetContext } from "./WidgetContext";
import { isWidgetTypeAvailable } from "./editor/widgetFactories";
import { widgetCardClass } from "./widgetSurface";
import { RELATIONSHIP_AXIS_ANCHORS } from "../../../characters/utils/companionDefaults";

const RELATIONSHIP_METERS = [
  { key: "closeness", label: "chats.widgets.companionState.closeness", bipolar: true },
  { key: "trust", label: "chats.widgets.companionState.trust", bipolar: true },
  { key: "affection", label: "chats.widgets.companionState.affection", bipolar: true },
  { key: "tension", label: "chats.widgets.companionState.tension" },
  { key: "stability", label: "chats.widgets.companionState.stability" },
] satisfies {
  key: keyof typeof RELATIONSHIP_AXIS_ANCHORS;
  label: TranslationKey;
  bipolar?: boolean;
}[];

function Meter({
  label,
  value,
  low,
  mid,
  high,
  bipolar,
}: {
  label: string;
  value: number;
  low: string;
  mid?: string;
  high: string;
  bipolar?: boolean;
}) {
  if (bipolar) {
    const v = Math.max(-1, Math.min(1, value));
    const pct = Math.round(v * 1000) / 10;
    const mag = Math.abs(v) * 50;
    return (
      <div className="flex flex-col gap-1">
        <div className="flex items-center justify-between text-[11px]">
          <span className="text-fg/55">{label}</span>
          <span className="tabular-nums text-fg/45">
            {pct > 0 ? `+${pct.toFixed(1)}` : pct.toFixed(1)}%
          </span>
        </div>
        <div className="relative h-1.5 w-full overflow-hidden rounded-full bg-fg/10">
          <div className="absolute left-1/2 top-0 h-full w-px -translate-x-1/2 bg-fg/25" />
          <div
            className={cn(
              "absolute top-0 h-full",
              v >= 0 ? "rounded-r-full bg-accent/70" : "rounded-l-full bg-rose-400/70",
            )}
            style={v >= 0 ? { left: "50%", width: `${mag}%` } : { right: "50%", width: `${mag}%` }}
          />
        </div>
        <div className="flex items-center justify-between text-[9px] text-fg/35">
          <span>{low}</span>
          {mid ? <span>{mid}</span> : null}
          <span>{high}</span>
        </div>
      </div>
    );
  }
  const pct = Math.round(Math.max(0, Math.min(1, value)) * 1000) / 10;
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center justify-between text-[11px]">
        <span className="text-fg/55">{label}</span>
        <span className="tabular-nums text-fg/45">{pct.toFixed(1)}%</span>
      </div>
      <div className="h-1.5 w-full overflow-hidden rounded-full bg-fg/10">
        <div className="h-full rounded-full bg-accent/70" style={{ width: `${pct}%` }} />
      </div>
      <div className="flex items-center justify-between text-[9px] text-fg/35">
        <span>{low}</span>
        <span>{high}</span>
      </div>
    </div>
  );
}

export function WidgetCompanionState({ node }: { node: CompanionStateNode }) {
  const { t } = useI18n();
  const { session, hasBackground } = useWidgetContext();
  const supported = isWidgetTypeAvailable("companion_state", useWidgetAvailability());
  const relationship = session?.companionState?.relationshipState;

  return (
    <section
      className={cn(
        "flex flex-col gap-2.5 rounded-xl px-3 py-3",
        widgetCardClass(hasBackground, node.design),
      )}
    >
      <header className="flex items-center gap-2">
        <Sparkles size={14} className="text-fg/50" />
        <h3 className="text-sm font-semibold text-fg/75">
          {node.title || t("chats.widgets.companionState.defaultTitle")}
        </h3>
      </header>
      {!supported ? (
        <p className="text-[12px] italic text-fg/40">
          {t("chats.widgets.companionState.companionOnly")}
        </p>
      ) : !relationship ? (
        <p className="text-[12px] italic text-fg/40">{t("chats.widgets.companionState.noData")}</p>
      ) : (
        <div className="flex flex-col gap-2">
          {RELATIONSHIP_METERS.map((m) => (
            <Meter
              key={m.key}
              label={t(m.label)}
              value={(relationship as Record<string, number>)[m.key] ?? 0}
              low={RELATIONSHIP_AXIS_ANCHORS[m.key].low}
              mid={RELATIONSHIP_AXIS_ANCHORS[m.key].mid}
              high={RELATIONSHIP_AXIS_ANCHORS[m.key].high}
              bipolar={m.bipolar}
            />
          ))}
        </div>
      )}
    </section>
  );
}
