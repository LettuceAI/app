import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { useTranslation } from "react-i18next";
import { Button, Chip, Input, Sheet } from "@lettuceai/crisp";
import { defaultNotation, diceLabel, quickDice, rollNotation, rollText, type Roll } from "../data/dice";

export interface DiceSheetProps {
  open: boolean;
  onInsert: (text: string) => void;
  onClose: () => void;
}

export function DiceSheet({ open, onInsert, onClose }: DiceSheetProps) {
  const { t } = useTranslation("conversations");
  const [notation, setNotation] = useState(defaultNotation);
  const [result, setResult] = useState<Roll | null>(null);
  const [history, setHistory] = useState<Roll[]>([]);
  const [invalid, setInvalid] = useState(false);
  const [rolling, setRolling] = useState(false);
  const [tumble, setTumble] = useState(0);
  const seq = useRef(0);
  const tumbling = useRef<number | null>(null);
  const settling = useRef<number | null>(null);
  useEffect(
    () => () => {
      if (tumbling.current) window.clearInterval(tumbling.current);
      if (settling.current) window.clearTimeout(settling.current);
    },
    [],
  );
  const roll = (text: string) => {
    seq.current += 1;
    const rolled = rollNotation(text, seq.current);
    setInvalid(rolled === null);
    if (!rolled) return;
    setRolling(true);
    if (tumbling.current) window.clearInterval(tumbling.current);
    const maxFace = rolled.sides * rolled.rolls.length;
    tumbling.current = window.setInterval(() => setTumble(Math.floor(Math.random() * maxFace) + 1), 50);
    if (settling.current) window.clearTimeout(settling.current);
    settling.current = window.setTimeout(() => {
      if (tumbling.current) window.clearInterval(tumbling.current);
      tumbling.current = null;
      setResult(rolled);
      setHistory((previous) => [rolled, ...previous].slice(0, 8));
      setRolling(false);
    }, 380);
  };
  return (
    <Sheet open={open} onClose={onClose} size="sm" title={t("dice.title")} description={t("dice.hint")}>
      <div className="space-y-4 pb-1">
        <div className="flex gap-2">
          <Input
            value={notation}
            onChange={(event) => setNotation(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") roll(notation);
            }}
            placeholder={defaultNotation}
            spellCheck={false}
            autoCapitalize="none"
            aria-label={t("dice.notation")}
            aria-invalid={invalid}
          />
          <Button variant="primary" onClick={() => roll(notation)}>{t("dice.roll")}</Button>
        </div>
        {invalid && <p className="text-sm text-danger">{t("dice.invalid")}</p>}
        <div className="flex flex-wrap gap-1.5">
          {quickDice.map((sides) => (
            <Chip key={sides} onClick={() => roll(`1d${sides}`)}>{diceLabel(sides)}</Chip>
          ))}
        </div>
        <div className="flex min-h-24 flex-col items-center justify-center gap-1 rounded-xl border border-line px-3 py-4" aria-live="polite">
          {rolling ? (
            <span className="text-4xl font-bold tabular-nums text-fg-muted">{tumble}</span>
          ) : (
            <AnimatePresence mode="popLayout">
              {result ? (
                <motion.div
                  key={result.id}
                  initial={{ opacity: 0, y: 12, scale: 0.6 }}
                  animate={{ opacity: 1, y: 0, scale: 1 }}
                  transition={{ type: "spring", stiffness: 340, damping: 16 }}
                  className="flex flex-col items-center gap-1"
                >
                  <span className="text-4xl font-bold tabular-nums text-fg">{result.total}</span>
                  {(result.rolls.length > 1 || result.modifier !== 0) && (
                    <span className="text-xs tabular-nums text-fg-muted">
                      {result.rolls.join(" + ")}
                      {result.modifier !== 0 && ` ${result.modifier > 0 ? "+" : "-"} ${Math.abs(result.modifier)}`}
                    </span>
                  )}
                </motion.div>
              ) : (
                <span className="text-sm text-fg-muted">{t("dice.tapToRoll")}</span>
              )}
            </AnimatePresence>
          )}
        </div>
        {result && !rolling && <Button variant="secondary" onClick={() => onInsert(rollText(result))}>{t("dice.insert")}</Button>}
        {history.length > 0 && (
          <div className="space-y-2 border-t border-line pt-3">
            <div className="flex items-center justify-between">
              <span className="text-xs font-semibold text-fg-muted">{t("dice.recent")}</span>
              <Button variant="ghost" size="sm" onClick={() => setHistory([])}>{t("dice.clear")}</Button>
            </div>
            <div className="flex flex-wrap gap-1.5">
              {history.map((entry) => (
                <span key={entry.id} title={`${entry.notation}: ${entry.rolls.join(", ")}`} className="rounded-md border border-line px-1.5 py-0.5 text-xs tabular-nums text-fg-muted">{entry.total}</span>
              ))}
            </div>
          </div>
        )}
      </div>
    </Sheet>
  );
}
