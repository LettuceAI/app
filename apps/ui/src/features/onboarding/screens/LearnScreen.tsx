import { AnimatePresence, motion } from "framer-motion";
import { useTranslation } from "react-i18next";
import { motion as m } from "@lettuceai/crisp";
import { learnSlides, type LearnSlideId } from "../content/slides";
import { useOnboarding } from "../flow/context";
import { ScreenTitle } from "../components/ui";

function plainOf(id: LearnSlideId): id is Exclude<LearnSlideId, "welcome"> {
  return id !== "welcome";
}

export function LearnScreen() {
  const { t } = useTranslation("onboarding");
  const { state } = useOnboarding();
  const id = learnSlides[state.slide] ?? learnSlides[0];
  return (
    <AnimatePresence mode="wait" initial={false}>
      <motion.div
        key={state.slide}
        initial={{ opacity: 0, x: 24 }}
        animate={{ opacity: 1, x: 0 }}
        exit={{ opacity: 0, x: -24 }}
        transition={m.settle}
      >
        <ScreenTitle title={t(`learn.${id}.title`)} lead={t(`learn.${id}.body`)} kicker={t(`learn.${id}.tag`)} />
        {plainOf(id) && (
          <blockquote className="max-w-[58ch] border-l-2 border-accent pl-5">
            <p className="text-sm font-medium text-fg-3">{t("learn.inOtherWords")}</p>
            <p className="mt-1 text-lg leading-relaxed text-fg">{t(`learn.${id}.plain`)}</p>
          </blockquote>
        )}
      </motion.div>
    </AnimatePresence>
  );
}
