import type { ReactNode } from "react";
import { AnimatePresence, motion, useIsPresent, useReducedMotion } from "framer-motion";
import { motion as presets } from "@lettuceai/crisp";

export function ModelEditorReveal({ open, children }: { open: boolean; children: ReactNode }) {
  return <AnimatePresence initial={false}>{open && <RevealBody>{children}</RevealBody>}</AnimatePresence>;
}

function RevealBody({ children }: { children: ReactNode }) {
  const present = useIsPresent();
  const reduced = useReducedMotion();
  return <motion.div
    inert={!present}
    initial={reduced ? false : { height: 0, opacity: 0 }}
    animate={{ height: "auto", opacity: 1 }}
    exit={{ height: 0, opacity: 0 }}
    transition={reduced ? { duration: 0 } : presets.collapse}
    className="overflow-clip"
  ><div className="divide-y divide-line">{children}</div></motion.div>;
}
