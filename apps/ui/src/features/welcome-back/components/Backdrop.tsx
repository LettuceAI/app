import { useEffect, useState } from "react";
import { animate, motion, useMotionValue, useMotionValueEvent } from "framer-motion";
import { mixHex, motion as m } from "@lettuceai/crisp";
import Grainient from "@/shared/ui/react-bits/Grainient";

const calm = ["#020403", "#06231a", "#1c6b4c", "#5f9a3a", "#030806"];
const bright = ["#020403", "#0a2e20", "#22744c", "#74aa48", "#030806"];

export interface BackdropProps {
  /** Turns the field half a turn and lifts it, for the finish. */
  finished: boolean;
  /** Darkens the field behind lists and progress. */
  dim: boolean;
}

export function Backdrop({ finished, dim }: BackdropProps) {
  const progress = useMotionValue(finished ? 1 : 0);
  const [t, setT] = useState(progress.get());
  useMotionValueEvent(progress, "change", setT);
  useEffect(() => {
    const controls = animate(progress, finished ? 1 : 0, { duration: 1.8, ease: [0.65, 0, 0.35, 1] });
    return () => controls.stop();
  }, [finished, progress]);

  return (
    <div aria-hidden className="pointer-events-none absolute inset-0 bg-[#030504]">
      <Grainient
        colors={calm.map((color, index) => mixHex(color, bright[index] ?? color, t))}
        angle={55 + 180 * t}
        flow={0.8 + 0.6 * Math.sin(Math.PI * t)}
        scale={1 - 0.3 * t}
        speed={0.45}
        contrast={1.1}
        saturation={0.9 + 0.2 * t}
        grainAmount={0.06}
        interactive={false}
        clickSwirl={false}
      />
      <motion.span className="absolute inset-0 bg-black" initial={false} animate={{ opacity: dim ? 0.5 : 0.1 }} transition={m.settle} />
    </div>
  );
}
