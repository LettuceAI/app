import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Dices } from "lucide-react";
import type { DiceNode } from "../../../../../core/storage/chatWidgetSchemas";
import { cn, interactive } from "../../../../design-tokens";
import { useI18n } from "../../../../../core/i18n/context";
import { useWidgetContext } from "./WidgetContext";
import { widgetCardClass } from "./widgetSurface";

interface RollResult {
  id: number;
  total: number;
  rolls: number[];
  modifier: number;
  sides: number;
  notation: string;
}

const QUICK_DICE = [4, 6, 8, 10, 12, 20, 100];

function rollNotation(notation: string, id: number): RollResult | null {
  const match = notation
    .trim()
    .toLowerCase()
    .match(/^(\d*)d(\d+)\s*([+-]\s*\d+)?$/);
  if (!match) return null;
  const count = Math.min(Math.max(parseInt(match[1] || "1", 10), 1), 100);
  const sides = Math.min(Math.max(parseInt(match[2], 10), 2), 1000);
  const modifier = match[3] ? parseInt(match[3].replace(/\s/g, ""), 10) : 0;
  const rolls = Array.from({ length: count }, () => Math.floor(Math.random() * sides) + 1);
  const total = rolls.reduce((a, b) => a + b, 0) + modifier;
  return { id, total, rolls, modifier, sides, notation: `${count}d${sides}${modifier ? (modifier > 0 ? `+${modifier}` : modifier) : ""}` };
}

export function WidgetDice({ node }: { node: DiceNode }) {
  const { t } = useI18n();
  const { hasBackground, onInsertText } = useWidgetContext();
  const [result, setResult] = useState<RollResult | null>(null);
  const [history, setHistory] = useState<RollResult[]>([]);
  const [rolling, setRolling] = useState(false);
  const [tumble, setTumble] = useState(0);
  const seqRef = useRef(0);
  const tumbleRef = useRef<number | null>(null);
  const settleRef = useRef<number | null>(null);
  const defaultNotation = node.notation?.trim() || "1d20";

  useEffect(() => {
    return () => {
      if (tumbleRef.current) window.clearInterval(tumbleRef.current);
      if (settleRef.current) window.clearTimeout(settleRef.current);
    };
  }, []);

  const roll = (notation: string) => {
    const id = seqRef.current + 1;
    seqRef.current = id;
    const rolled = rollNotation(notation, id);
    if (!rolled) return;

    setRolling(true);
    if (tumbleRef.current) window.clearInterval(tumbleRef.current);
    const maxFace = rolled.sides * rolled.rolls.length;
    tumbleRef.current = window.setInterval(() => {
      setTumble(Math.floor(Math.random() * maxFace) + 1);
    }, 50);

    if (settleRef.current) window.clearTimeout(settleRef.current);
    settleRef.current = window.setTimeout(() => {
      if (tumbleRef.current) window.clearInterval(tumbleRef.current);
      tumbleRef.current = null;
      setResult(rolled);
      setHistory((prev) => [rolled, ...prev].slice(0, 8));
      setRolling(false);
    }, 380);
  };

  return (
    <section
      className={cn(
        "flex flex-col gap-2.5 rounded-xl px-3 py-3",
        widgetCardClass(hasBackground, node.design),
      )}
    >
      <header className="flex items-center gap-2">
        <Dices size={14} className="text-fg/50" />
        <h3 className="text-sm font-semibold text-fg/75">{node.title || t("chats.widgets.dice.defaultTitle")}</h3>
        <span className="ml-auto font-mono text-[11px] text-fg/40">{defaultNotation}</span>
      </header>

      {/* Result display */}
      <button
        type="button"
        onClick={() => roll(defaultNotation)}
        className={cn(
          "group relative flex h-24 w-full items-center justify-center overflow-hidden rounded-xl border border-fg/12 bg-fg/5",
          interactive.transition.fast,
          interactive.active.scale,
          "hover:bg-fg/8",
        )}
        aria-label={t("chats.widgets.dice.roll")}
      >
        {rolling ? (
          <span className="text-4xl font-bold tabular-nums text-fg/50">{tumble}</span>
        ) : (
          <AnimatePresence mode="popLayout">
            {result ? (
              <motion.div
                key={`r-${result.id}`}
                initial={{ opacity: 0, y: 12, scale: 0.6 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                transition={{ type: "spring", stiffness: 340, damping: 16 }}
                className="flex flex-col items-center gap-1"
              >
                <span className="text-4xl font-bold leading-none tabular-nums text-fg/90">
                  {result.total}
                </span>
                {(result.rolls.length > 1 || result.modifier !== 0) && (
                  <div className="flex flex-wrap items-center justify-center gap-1">
                    {result.rolls.map((r, i) => (
                      <span
                        key={i}
                        className="rounded border border-fg/12 bg-fg/8 px-1 text-[9px] font-semibold tabular-nums text-fg/55"
                      >
                        {r}
                      </span>
                    ))}
                    {result.modifier !== 0 && (
                      <span className="text-[9px] font-medium text-fg/45">
                        {result.modifier > 0 ? `+${result.modifier}` : result.modifier}
                      </span>
                    )}
                  </div>
                )}
              </motion.div>
            ) : (
              <span className="text-[12px] font-medium text-fg/45">{t("chats.widgets.dice.tapToRoll")}</span>
            )}
          </AnimatePresence>
        )}
      </button>

      {/* Quick dice */}
      <div className="flex flex-wrap gap-1">
        {QUICK_DICE.map((sides) => (
          <button
            key={sides}
            type="button"
            onClick={() => roll(`1d${sides}`)}
            className={cn(
              "rounded-md border border-fg/12 bg-fg/5 px-2 py-1 text-[11px] font-medium text-fg/60",
              interactive.transition.fast,
              "hover:border-fg/25 hover:bg-fg/10 hover:text-fg/80",
            )}
          >
            d{sides}
          </button>
        ))}
      </div>

      {result && !rolling && (
        <button
          type="button"
          onClick={() => onInsertText(`🎲 ${result.notation}: ${result.total}`)}
          className="self-start text-[11px] text-accent/80 transition hover:text-accent"
        >
          {t("chats.widgets.dice.insertIntoMessage")}
        </button>
      )}

      {history.length > 0 && (
        <div className="flex flex-col gap-1 border-t border-fg/8 pt-2">
          <div className="flex items-center justify-between">
            <span className="text-[10px] font-semibold uppercase tracking-wide text-fg/35">
              {t("chats.widgets.dice.recent")}
            </span>
            <button
              type="button"
              onClick={() => setHistory([])}
              className="text-[10px] text-fg/35 transition hover:text-fg/60"
            >
              {t("chats.widgets.dice.clear")}
            </button>
          </div>
          <div className="flex flex-wrap gap-1">
            {history.map((h) => (
              <span
                key={h.id}
                title={`${h.notation}: ${h.rolls.join(", ")}`}
                className="rounded-md border border-fg/10 bg-fg/5 px-1.5 py-0.5 font-mono text-[10px] tabular-nums text-fg/55"
              >
                {h.total}
              </span>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}
