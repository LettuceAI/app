import { useCallback, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import beetrootSrc from "../../../../assets/beetroot.png";

interface BeetDrop {
  id: number;
  startX: number;
  size: number;
  rotateStart: number;
  rotateEnd: number;
  duration: number;
  drift: number;
  travelY: number;
}

let nextId = 1;

function spawnDrop(): BeetDrop {
  const id = nextId++;
  const startX = 10 + Math.random() * 80; // vw percentage
  const size = 130 + Math.random() * 80;
  const rotateStart = -45 + Math.random() * 90;
  const direction = Math.random() < 0.5 ? -1 : 1;
  const rotateEnd = rotateStart + direction * (360 + Math.random() * 360);
  const duration = 3.2 + Math.random() * 1.0;
  const drift = (Math.random() - 0.5) * 110;
  const viewportHeight = typeof window !== "undefined" ? window.innerHeight : 800;
  const travelY = viewportHeight + size + 80;
  return { id, startX, size, rotateStart, rotateEnd, duration, drift, travelY };
}

export interface UseBeetrootRainResult {
  fire: () => void;
  overlay: React.ReactNode;
}

export function useBeetrootRain(): UseBeetrootRainResult {
  const [drops, setDrops] = useState<BeetDrop[]>([]);

  const fire = useCallback(() => {
    const count = 4 + Math.floor(Math.random() * 3); // 4–6 per trigger
    for (let i = 0; i < count; i++) {
      const stagger = i * (120 + Math.random() * 140); 
      window.setTimeout(() => {
        const drop = spawnDrop();
        setDrops((prev) => [...prev, drop]);
        const totalMs = drop.duration * 1000 + 250;
        window.setTimeout(() => {
          setDrops((prev) => prev.filter((d) => d.id !== drop.id));
        }, totalMs);
      }, stagger);
    }
  }, []);

  const overlay = (
    <div
      aria-hidden
      className="pointer-events-none fixed inset-0 z-60 overflow-hidden"
    >
      <AnimatePresence>
        {drops.map((drop) => (
          <motion.img
            key={drop.id}
            src={beetrootSrc}
            alt=""
            initial={{ y: 0, x: 0, opacity: 0, rotate: drop.rotateStart, scale: 0.85 }}
            animate={{
              y: [0, drop.travelY * 0.18, drop.travelY],
              x: drop.drift,
              opacity: [0, 1, 1, 1, 0.9],
              rotate: drop.rotateEnd,
              scale: [0.85, 1, 1, 1.06],
            }}
            exit={{ opacity: 0 }}
            transition={{
              y: {
                duration: drop.duration,
                times: [0, 0.25, 1],
                ease: [0.42, 0, 0.65, 1],
              },
              x: { duration: drop.duration, ease: "easeInOut" },
              rotate: { duration: drop.duration, ease: "linear" },
              opacity: { duration: drop.duration, times: [0, 0.05, 0.4, 0.85, 1] },
              scale: { duration: drop.duration, times: [0, 0.18, 0.85, 1] },
            }}
            style={{
              position: "absolute",
              top: -drop.size - 40,
              left: `${drop.startX}vw`,
              width: drop.size,
              height: drop.size,
              marginLeft: -drop.size / 2,
              filter: "drop-shadow(0 8px 16px rgba(0,0,0,0.35))",
              willChange: "transform, opacity",
            }}
          />
        ))}
      </AnimatePresence>
    </div>
  );

  return { fire, overlay };
}
