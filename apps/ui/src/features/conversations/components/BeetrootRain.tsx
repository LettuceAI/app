import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import { usePreferences } from "@/entities/preferences";
import beetroot from "@/shared/assets/beetroot.png";
import type { GenerationStore } from "../data/generation-store";

const matches = /\b(beet|beets|beetroot|beetroots)\b/i;
type Drop = {
  id: string;
  left: number;
  size: number;
  rotation: number;
  drift: number;
  duration: number;
  delay: number;
};

export function BeetrootRain({ store }: { store: GenerationStore }) {
  const preferences = usePreferences();
  const reduced = useReducedMotion();
  const active = useRef(false);
  const streamed = useRef(false);
  const [drops, setDrops] = useState<Drop[]>([]);
  useEffect(() => {
    const fire = () => {
      if (preferences.value.beetrootRain === false || reduced) return;
      const count = 4 + Math.floor(Math.random() * 3);
      setDrops((current) => [
        ...current,
        ...Array.from({ length: count }, (_, index) => ({
          id: crypto.randomUUID(),
          left: 10 + Math.random() * 80,
          size: 130 + Math.random() * 80,
          rotation: -45 + Math.random() * 90,
          drift: (Math.random() - 0.5) * 110,
          duration: 3.2 + Math.random(),
          delay: index * 0.18,
        })),
      ]);
    };
    const meta = store.subscribeMeta(() => {
      const next = store.getMeta();
      if (next.active && !active.current) {
        streamed.current = false;
        if (next.userText && matches.test(next.userText)) fire();
      }
      active.current = next.active;
    });
    const text = store.subscribeText(() => {
      if (!store.getMeta().active || streamed.current || !matches.test(store.getText().text))
        return;
      streamed.current = true;
      fire();
    });
    return () => {
      meta();
      text();
    };
  }, [store, preferences.value.beetrootRain, reduced]);
  return (
    <div aria-hidden="true" className="pointer-events-none absolute inset-0 z-30 overflow-clip">
      <AnimatePresence>
        {preferences.value.beetrootRain !== false &&
          !reduced &&
          drops.map((drop) => (
            <motion.img
              key={drop.id}
              src={beetroot}
              alt=""
              className="absolute -top-40"
              style={{ left: `${drop.left}%`, width: drop.size }}
              initial={{ y: 0, opacity: 0, rotate: drop.rotation }}
              animate={{
                y: "120vh",
                x: drop.drift,
                opacity: [0, 1, 1, 0],
                rotate: drop.rotation + 450,
              }}
              exit={{ opacity: 0 }}
              transition={{ duration: drop.duration, delay: drop.delay, ease: "easeIn" }}
              onAnimationComplete={() =>
                setDrops((current) => current.filter((value) => value.id !== drop.id))
              }
            />
          ))}
      </AnimatePresence>
    </div>
  );
}
