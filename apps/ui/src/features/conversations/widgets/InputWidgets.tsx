import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import { Minus, Plus } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, Chip, cn, CrispStringsProvider, DatePicker, icon, IconButton, motion as presets, Segmented, Skeleton, Textarea } from "@lettuceai/crisp";
import type { LibraryWidgetStat_Serialize, TimeChange, TimeMode } from "@/api/generated/bindings";
import { defaultNotation, diceLabel, quickDice, rollText } from "../data/dice";
import { datePickerStrings } from "../components/settings/date-picker-strings";
import { useWidgets } from "./context";
import type { WidgetNode } from "./nodes";
import { WIDGET_ICON } from "./catalogue";
import { useDiceRoll } from "./use-dice-roll";
import { WidgetCard } from "./WidgetCard";

type Node<T extends WidgetNode["type"]> = Extract<WidgetNode, { type: T }>;

/** Always a field, so the card never changes size; saves on blur when the text changed, Escape puts the saved text back. */
function NoteField({ value, label, placeholder, onSave }: { value: string; label: string; placeholder: string; onSave: (text: string) => Promise<boolean> }) {
  const [draft, setDraft] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const commit = async () => {
    if (saving || draft === null) return;
    if (draft === value) {
      setDraft(null);
      return;
    }
    setSaving(true);
    const saved = await onSave(draft);
    setSaving(false);
    if (saved) setDraft(null);
  };
  return (
    <Textarea
      rows={3}
      autoGrow
      maxRows={12}
      value={draft ?? value}
      aria-label={label}
      placeholder={placeholder}
      disabled={saving}
      className="text-sm"
      onChange={(event) => setDraft(event.target.value)}
      onBlur={() => void commit()}
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          setDraft(null);
          event.currentTarget.blur();
        }
      }}
    />
  );
}

export function ScratchPadWidget({ node }: { node: Node<"scratch_pad"> }) {
  const { t } = useTranslation("conversations");
  const { updateNode } = useWidgets();
  return (
    <WidgetCard design={node.design} title={node.title || t("widgets.types.scratch_pad.label")} description={node.description} icon={WIDGET_ICON.scratch_pad} trailing={<span className="text-2xs text-fg-3">{t("widgets.scratchPad.onlyYou")}</span>}>
      <NoteField
        value={node.content ?? ""}
        label={node.title || t("widgets.types.scratch_pad.label")}
        placeholder={t("widgets.scratchPad.placeholder")}
        onSave={(content) => updateNode(node.id, (current) => (current.type === "scratch_pad" ? { ...current, content } : current))}
      />
    </WidgetCard>
  );
}

export function AuthorNoteWidget({ node }: { node: Node<"author_note"> }) {
  const { t } = useTranslation("conversations");
  const { settings, saveAuthorNote } = useWidgets();
  return (
    <WidgetCard design={node.design} title={node.title || t("widgets.types.author_note.label")} description={node.description} icon={WIDGET_ICON.author_note} trailing={<span className="text-2xs text-fg-3">{t("widgets.authorNote.everyReply")}</span>}>
      {settings ? (
        <NoteField value={settings.author_note ?? ""} label={node.title || t("widgets.types.author_note.label")} placeholder={t("widgets.authorNote.placeholder")} onSave={saveAuthorNote} />
      ) : (
        <Skeleton shape="block" height={48} />
      )}
    </WidgetCard>
  );
}

export function StatTrackerWidget({ node }: { node: Node<"stat_tracker"> }) {
  const { t } = useTranslation("conversations");
  const { updateNode } = useWidgets();
  const [local, setLocal] = useState<LibraryWidgetStat_Serialize[] | null>(null);
  const pending = useRef(0);
  const stats = local ?? node.stats;
  const set = (id: string, wanted: number) => {
    const next = stats.map((stat) => {
      if (stat.id !== id) return stat;
      const floor = stat.min === null ? wanted : Math.max(stat.min, wanted);
      return { ...stat, value: stat.max === null ? floor : Math.min(stat.max, floor) };
    });
    setLocal(next);
    pending.current += 1;
    void updateNode(node.id, (current) => (current.type === "stat_tracker" ? { ...current, stats: next } : current)).then(() => {
      pending.current -= 1;
      if (pending.current === 0) setLocal(null);
    });
  };
  return (
    <WidgetCard design={node.design} title={node.title || t("widgets.types.stat_tracker.label")} description={node.description} icon={WIDGET_ICON.stat_tracker}>
      {stats.length === 0 ? (
        <p className="text-sm text-fg-3">{t("widgets.statTracker.empty")}</p>
      ) : (
        <div className="space-y-3">
          {stats.map((stat) => {
            const value = stat.value ?? 0;
            const cap = stat.max === null ? null : `/${stat.max}`;
            return (
              <div key={stat.id}>
                <div className="flex flex-wrap items-center justify-between gap-x-2 gap-y-1.5">
                  <span className="min-w-24 flex-1 truncate text-sm text-fg-2">{stat.label}</span>
                  <div className="flex items-center gap-1">
                    <IconButton label={t("widgets.statTracker.decrease", { label: stat.label })} size="sm" disabled={stat.min !== null && value <= stat.min} onClick={() => set(stat.id, value - 1)}>
                      <Minus size={icon.sm} />
                    </IconButton>
                    <span className="min-w-10 text-center text-sm font-semibold tabular-nums text-fg">
                      {value}
                      {cap !== null && <span className="text-2xs font-normal text-fg-3">{cap}</span>}
                    </span>
                    <IconButton label={t("widgets.statTracker.increase", { label: stat.label })} size="sm" disabled={stat.max !== null && value >= stat.max} onClick={() => set(stat.id, value + 1)}>
                      <Plus size={icon.sm} />
                    </IconButton>
                  </div>
                </div>
                {stat.min !== null && stat.max !== null && stat.max > stat.min && (
                  <div className="mt-1.5 h-1 overflow-hidden rounded-full bg-fill-2">
                    <motion.div
                      data-testid="stat-bar"
                      className="h-full rounded-full bg-fg-3"
                      initial={false}
                      animate={{ width: `${((Math.min(stat.max, Math.max(stat.min, value)) - stat.min) / (stat.max - stat.min)) * 100}%` }}
                      transition={presets.settle}
                    />
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </WidgetCard>
  );
}

export function QuickSnippetsWidget({ node }: { node: Node<"quick_snippets"> }) {
  const { t } = useTranslation("conversations");
  const { insertText } = useWidgets();
  return (
    <WidgetCard design={node.design} title={node.title || t("widgets.types.quick_snippets.label")} description={node.description} icon={WIDGET_ICON.quick_snippets}>
      {node.snippets.length === 0 ? (
        <p className="text-sm text-fg-3">{t("widgets.quickSnippets.empty")}</p>
      ) : (
        <div className="flex flex-wrap gap-1.5">
          {node.snippets.map((snippet) => (
            <Chip key={snippet.id} title={snippet.text} className="max-w-full" onClick={() => insertText(snippet.text)}>
              <span className="truncate">{snippet.label || snippet.text}</span>
            </Chip>
          ))}
        </div>
      )}
    </WidgetCard>
  );
}

export function DiceWidget({ node }: { node: Node<"dice"> }) {
  const { t } = useTranslation("conversations");
  const { insertText } = useWidgets();
  const reduced = useReducedMotion();
  const dice = useDiceRoll();
  const [invalid, setInvalid] = useState(false);
  const notation = node.notation?.trim() || defaultNotation;
  const rolling = dice.tumble !== null;
  const roll = (text: string) => setInvalid(!dice.roll(text));
  const single = dice.result !== null && dice.result.rolls.length === 1 && dice.result.modifier === 0;
  const crit = !rolling && single && dice.result?.total === dice.result?.sides;
  const fumble = !rolling && single && dice.result?.total === 1;
  const natural = crit || fumble ? t("widgets.dice.natural", { n: dice.result?.total }) : null;
  const recent = `${t("dice.recent")}: ${dice.history.slice(1).map((entry) => entry.total).join(" · ")}`;
  const breakdown = dice.result && (dice.result.rolls.length > 1 || dice.result.modifier !== 0)
    ? `${dice.result.rolls.join(" + ")}${dice.result.modifier !== 0 ? ` ${dice.result.modifier > 0 ? "+" : "-"} ${Math.abs(dice.result.modifier)}` : ""}`
    : null;
  return (
    <WidgetCard
      design={node.design}
      title={node.title || t("widgets.types.dice.label")}
      description={node.description}
      icon={WIDGET_ICON.dice}
      trailing={<span className="font-mono text-2xs text-fg-3">{notation}</span>}
    >
      <div className="space-y-2.5">
        <button
          type="button"
          aria-label={t("dice.roll")}
          onClick={() => roll(notation)}
          className="flex w-full flex-col items-center gap-0.5 rounded-lg border border-dashed border-line-2 py-3 transition-colors hover:border-line-3 hover:bg-fill"
        >
          <AnimatePresence mode="popLayout" initial={false}>
            <motion.span
              key={rolling ? "tumble" : (dice.result?.id ?? "idle")}
              initial={reduced || rolling ? false : { opacity: 0, y: 8, scale: 0.6 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              transition={presets.settle}
              className={cn("text-3xl font-semibold leading-none tabular-nums", rolling ? "text-fg-3" : crit ? "text-accent" : fumble ? "text-danger" : "text-fg")}
              aria-live="polite"
            >
              {rolling ? dice.tumble : (dice.result?.total ?? "?")}
            </motion.span>
          </AnimatePresence>
          <span className="text-2xs tabular-nums text-fg-3">{natural ?? breakdown ?? (dice.result === null ? t("dice.tapToRoll") : " ")}</span>
        </button>
        {invalid && <p className="text-sm text-danger">{t("dice.invalid")}</p>}
        <div className="flex flex-wrap gap-1.5">
          {quickDice.map((sides) => (
            <Chip key={sides} onClick={() => roll(`1d${sides}`)}>{diceLabel(sides)}</Chip>
          ))}
        </div>
        {dice.result && !rolling && (
          <Button variant="secondary" size="sm" onClick={() => insertText(rollText(dice.result!))}>{t("dice.insert")}</Button>
        )}
        {dice.history.length > 1 && (
          <div className="flex items-center gap-2 border-t border-line pt-2.5">
            <p className="min-w-0 flex-1 truncate text-2xs tabular-nums text-fg-3" title={dice.history.map((entry) => `${entry.notation}: ${entry.rolls.join(", ")}`).join("\n")}>
              {recent}
            </p>
            <Button variant="ghost" size="sm" onClick={dice.clearHistory}>{t("dice.clear")}</Button>
          </div>
        )}
      </div>
    </WidgetCard>
  );
}

const timeModes: readonly TimeMode[] = ["live", "frozen", "ticking"];

export function TimeWidget({ node }: { node: Node<"time"> }) {
  const { t, i18n } = useTranslation("conversations");
  const { settings, setTime } = useWidgets();
  const [draft, setDraft] = useState<{ mode: TimeMode; anchor: Date } | null>(null);
  const [saving, setSaving] = useState(false);
  const time = settings?.time;
  const [now, setNow] = useState(() => Date.now());
  const moving = time !== undefined && time.mode !== "frozen";
  useEffect(() => {
    if (!moving) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [moving]);
  const shown = !time
    ? null
    : time.mode === "frozen"
      ? (time.anchor_at ?? time.effective_at)
      : time.mode === "ticking" && time.anchor_at !== null && time.set_at !== null
        ? time.anchor_at + (now - time.set_at)
        : time.mode === "live"
          ? now
          : time.effective_at;
  const clock = new Intl.DateTimeFormat(i18n.language, {
    hour: "2-digit",
    minute: "2-digit",
    second: node.show_seconds ? "2-digit" : undefined,
    hour12: node.hour_format === "h12",
  });
  const day = new Intl.DateTimeFormat(i18n.language, { weekday: "long", year: "numeric", month: "long", day: "numeric" });
  const run = async (change: TimeChange) => {
    setSaving(true);
    const saved = await setTime(change);
    setSaving(false);
    if (saved) setDraft(null);
  };
  const choose = (mode: TimeMode) => {
    if (!time) return;
    if (mode === "live") void run({ type: "set", mode, anchor_at: null });
    else setDraft({ mode, anchor: new Date(time.mode === "ticking" ? time.effective_at : (time.anchor_at ?? time.effective_at)) });
  };
  return (
    <WidgetCard
      design={node.design}
      title={node.title || t("widgets.types.time.label")}
      icon={WIDGET_ICON.time}
      trailing={time && <span className="text-2xs text-fg-3">{t(`settings.time.${time.mode}`)}</span>}
    >
      <div className="text-center">
        {shown === null ? (
          <Skeleton shape="block" height={36} />
        ) : (
          <>
            <p className="text-3xl font-semibold leading-none tabular-nums text-fg">{clock.format(shown)}</p>
            {node.show_date !== false && <p className="mt-1 text-2xs text-fg-3">{day.format(shown)}</p>}
          </>
        )}
      </div>
      {time && !time.enabled && <p className="mt-3 text-2xs text-fg-3">{t("widgets.time.awarenessOff")}</p>}
      {time?.enabled && (
        <CrispStringsProvider locale={i18n.language} strings={datePickerStrings(t)}>
          <div className="mt-3 space-y-2.5">
            <Segmented
              size="sm"
              block
              aria-label={t("settings.time.mode")}
              value={draft?.mode ?? time.mode}
              onChange={choose}
              options={timeModes.map((mode) => ({ value: mode, label: t(`settings.time.${mode}`) }))}
            />
            {draft && (
              <>
                <DatePicker withTime value={draft.anchor} onChange={(anchor) => setDraft({ ...draft, anchor })} locale={i18n.language} disabled={saving} placeholder={t("settings.time.chooseAnchor")} />
                <div className="flex items-center justify-end gap-2">
                  <Button variant="ghost" size="sm" disabled={saving} onClick={() => setDraft(null)}>{t("settings.time.cancel")}</Button>
                  <Button size="sm" loading={saving} disabled={saving || !Number.isFinite(draft.anchor.getTime())} onClick={() => void run({ type: "set", mode: draft.mode, anchor_at: draft.anchor.getTime() })}>
                    {t("settings.time.apply")}
                  </Button>
                </div>
              </>
            )}
          </div>
        </CrispStringsProvider>
      )}
    </WidgetCard>
  );
}
