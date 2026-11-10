import { useCallback, useEffect, useRef, useState } from "react";
import { rollNotation, type Roll } from "../data/dice";

const tumbleMs = 50;
const settleMs = 380;
const historySize = 8;

/** Rolls a notation with a short tumble of random faces before the result lands. */
export function useDiceRoll() {
  const [result, setResult] = useState<Roll | null>(null);
  const [history, setHistory] = useState<Roll[]>([]);
  const [tumble, setTumble] = useState<number | null>(null);
  const sequence = useRef(0);
  const spinning = useRef<number | null>(null);
  const settling = useRef<number | null>(null);

  const stop = useCallback(() => {
    if (spinning.current !== null) window.clearInterval(spinning.current);
    if (settling.current !== null) window.clearTimeout(settling.current);
    spinning.current = null;
    settling.current = null;
  }, []);
  useEffect(() => stop, [stop]);

  /** Resolves false when the notation cannot be rolled. */
  const roll = useCallback(
    (notation: string): boolean => {
      sequence.current += 1;
      const rolled = rollNotation(notation, sequence.current);
      if (!rolled) return false;
      stop();
      const faces = rolled.sides * rolled.rolls.length;
      const face = () => Math.floor(Math.random() * faces) + 1;
      setTumble(face());
      spinning.current = window.setInterval(() => setTumble(face()), tumbleMs);
      settling.current = window.setTimeout(() => {
        stop();
        setResult(rolled);
        setHistory((previous) => [rolled, ...previous].slice(0, historySize));
        setTumble(null);
      }, settleMs);
      return true;
    },
    [stop],
  );

  return { result, history, tumble, roll, clearHistory: () => setHistory([]) };
}
