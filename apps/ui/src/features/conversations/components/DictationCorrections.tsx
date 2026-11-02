import { Button, Text, motion as m } from "@lettuceai/crisp";
import { motion, useReducedMotion } from "framer-motion";
import { Trans, useTranslation } from "react-i18next";
import { correctionKey, type useDictationLearning } from "../data/use-dictation-learning";

export function DictationCorrections({ learning }: { learning: ReturnType<typeof useDictationLearning> }) {
  const { t } = useTranslation("conversations");
  const { t: common } = useTranslation("common");
  const reduced = useReducedMotion();
  if (!learning.suggestions.length && !learning.error) return null;
  return (
    <motion.div initial={{ opacity: 0, y: reduced ? 0 : 4 }} animate={{ opacity: 1, y: 0 }} transition={m.quick} className="space-y-2" role="status" aria-live="polite">
      {learning.suggestions.map((suggestion) => (
        <div key={correctionKey(suggestion)} className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1">
          <div className="min-w-0 flex-1 break-words">
            <Text as="span" variant="hint">{t("mic.learning.question")} </Text>
            <Trans t={t} i18nKey="mic.learning.pair" values={{ wrong: suggestion.wrong, correct: suggestion.correct }} components={{ wrong: <Text as="s" variant="hint" />, correct: <Text as="span" variant="label" /> }} />
          </div>
          <div className="flex shrink-0 gap-1">
            <Button size="sm" variant="ghost" disabled={learning.busy} onClick={() => void learning.ignore(suggestion)}>{t("mic.learning.ignore")}</Button>
            <Button size="sm" variant="tonal" disabled={learning.busy} onClick={() => void learning.approve(suggestion)}>{t("mic.learning.save")}</Button>
          </div>
        </div>
      ))}
      {learning.error && <div className="flex flex-wrap items-center justify-between gap-2">
        <Text variant="hint">{common(`errors.code.${learning.error.code}`)}</Text>
        {!learning.suggestions.length && <Button size="sm" variant="ghost" onClick={learning.retry}>{common("actions.retry")}</Button>}
      </div>}
    </motion.div>
  );
}
