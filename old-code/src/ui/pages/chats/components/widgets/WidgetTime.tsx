import { useMemo } from "react";
import { Clock } from "lucide-react";
import type { TimeNode } from "../../../../../core/storage/chatWidgetSchemas";
import { cn, interactive } from "../../../../design-tokens";
import { DateTimePicker } from "../../../../components/DateTimePicker";
import { useI18n, type TranslationKey } from "../../../../../core/i18n/context";
import { useWidgetAvailability, useWidgetContext } from "./WidgetContext";
import { useWidgetEdit } from "./WidgetEditContext";
import { widgetCardClass } from "./widgetSurface";
import { useCompanionTimeOverrideEditor } from "../../utils/companionTimeOverride";

const MODE_OPTIONS = [
  { mode: "off", label: "chats.widgets.time.live" },
  { mode: "frozen", label: "chats.widgets.time.frozen" },
  { mode: "ticking", label: "chats.widgets.time.ticking" },
] satisfies { mode: "off" | "frozen" | "ticking"; label: TranslationKey }[];

export function WidgetTime({ node }: { node: TimeNode }) {
  const { t } = useI18n();
  const { hasBackground, session, onUpdateCompanionTimeOverride } = useWidgetContext();
  const { isCompanion, isGroup } = useWidgetAvailability();
  const { editing: areaEditing } = useWidgetEdit();

  const override = session?.companionState?.preferences?.timeOverride;
  const canEdit = !areaEditing && !!session && isCompanion && !isGroup;
  const {
    activeMode,
    selectedMode,
    editing,
    beginEdit,
    selectLive,
    apply,
    cancel,
    draftMs,
    setDraftMs,
    shownMs,
    nowMs,
    isOverridden,
  } = useCompanionTimeOverrideEditor(override, onUpdateCompanionTimeOverride, canEdit);

  const timeFormatter = useMemo(
    () =>
      new Intl.DateTimeFormat(undefined, {
        hour: "2-digit",
        minute: "2-digit",
        second: node.showSeconds ? "2-digit" : undefined,
        hour12: node.hourFormat === "12h",
      }),
    [node.showSeconds, node.hourFormat],
  );
  const dateFormatter = useMemo(
    () =>
      new Intl.DateTimeFormat(undefined, {
        weekday: "long",
        year: "numeric",
        month: "long",
        day: "numeric",
      }),
    [],
  );

  const awarenessOff = !session?.companionState?.preferences?.timeAwarenessEnabled;

  return (
    <section
      className={cn(
        "flex flex-col gap-2.5 rounded-xl px-3 py-3",
        widgetCardClass(hasBackground, node.design),
      )}
    >
      <header className="flex items-center gap-2">
        <Clock size={14} className="text-fg/50" />
        <h3 className="text-sm font-semibold text-fg/75">{node.title || t("chats.widgets.time.defaultTitle")}</h3>
        <span
          className={cn(
            "ml-auto rounded-full px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
            isOverridden ? "bg-accent/15 text-accent/80" : "text-fg/35",
          )}
        >
          {isOverridden
            ? activeMode === "frozen"
              ? t("chats.widgets.time.frozen")
              : t("chats.widgets.time.custom")
            : t("chats.widgets.time.live")}
        </span>
      </header>

      <div className="flex flex-col items-center gap-0.5 py-1">
        <span className="text-3xl font-semibold leading-none tabular-nums text-fg/90">
          {timeFormatter.format(shownMs)}
        </span>
        {node.showDate !== false && (
          <span className="text-[12px] text-fg/50">{dateFormatter.format(shownMs)}</span>
        )}
        {isOverridden && (
          <span className="mt-1 text-[10px] text-fg/35">
            {t("chats.widgets.time.realTime", { time: timeFormatter.format(nowMs) })}
          </span>
        )}
      </div>

      {canEdit && (
        <div className="flex gap-1">
          {MODE_OPTIONS.map((opt) => (
            <button
              key={opt.mode}
              type="button"
              onClick={() =>
                opt.mode === "off" ? selectLive() : beginEdit(opt.mode)
              }
              className={cn(
                "flex-1 rounded-md border px-2 py-1 text-[11px] font-medium",
                interactive.transition.fast,
                (editing ? selectedMode : activeMode) === opt.mode
                  ? "border-accent/40 bg-accent/15 text-accent/90"
                  : "border-fg/12 bg-fg/5 text-fg/60 hover:border-fg/25 hover:bg-fg/10 hover:text-fg/80",
              )}
            >
              {t(opt.label)}
            </button>
          ))}
        </div>
      )}

      {canEdit && !editing && isOverridden && (
        <button
          type="button"
          onClick={() => beginEdit(activeMode === "frozen" ? "frozen" : "ticking")}
          className={cn(
            "self-center text-[11px] text-accent/80 hover:text-accent",
            interactive.transition.fast,
          )}
        >
          {t("chats.widgets.time.changeTime")}
        </button>
      )}

      {editing && selectedMode !== "off" && (
        <div className="flex flex-col gap-2">
          <DateTimePicker valueMs={draftMs} onChange={setDraftMs} />
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => setDraftMs(Date.now())}
              className={cn(
                "rounded-md border border-fg/12 bg-fg/5 px-2 py-1.5 text-[11px] font-medium text-fg/60 hover:border-fg/25 hover:text-fg/80",
                interactive.transition.fast,
              )}
            >
              {t("chats.widgets.time.now")}
            </button>
            <button
              type="button"
              onClick={cancel}
              className={cn(
                "ml-auto rounded-md border border-fg/12 bg-fg/5 px-2.5 py-1.5 text-[11px] font-medium text-fg/60 hover:border-fg/25 hover:text-fg/80",
                interactive.transition.fast,
              )}
            >
              {t("common.buttons.cancel")}
            </button>
            <button
              type="button"
              onClick={apply}
              className={cn(
                "rounded-md bg-accent px-2.5 py-1.5 text-[11px] font-semibold text-black shadow-sm",
                interactive.transition.fast,
                interactive.active.scale,
                "hover:brightness-110",
              )}
            >
              {selectedMode === "frozen" ? t("chats.widgets.time.freeze") : t("chats.widgets.time.set")}
            </button>
          </div>
        </div>
      )}

      {canEdit && awarenessOff && (
        <p className="text-[10px] italic leading-snug text-fg/35">
          {t("chats.widgets.time.awarenessHint")}
        </p>
      )}
    </section>
  );
}
