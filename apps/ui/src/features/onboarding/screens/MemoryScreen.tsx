import { AnimatePresence, motion } from "framer-motion";
import { Brain, Check, PencilLine } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn, icon, motion as m } from "@lettuceai/crisp";
import { ActionError } from "../components/ActionError";
import { ActionButton, ScreenTitle } from "../components/ui";
import { useOnboarding } from "../flow/context";
import type { MemoryChoice } from "../flow/flow";

const options = [
  { id: "dynamic", copy: "automatic", Glyph: Brain, recommended: true },
  { id: "manual", copy: "manual", Glyph: PencilLine, recommended: false },
] as const satisfies readonly { id: MemoryChoice; copy: "automatic" | "manual"; Glyph: typeof Brain; recommended: boolean }[];

export function MemoryScreen() {
  const { t } = useTranslation("onboarding");
  const { state, dispatch, phone, finish } = useOnboarding();
  const chosen = options.find((option) => option.id === state.memory) ?? options[0];
  const select = (id: MemoryChoice) => dispatch({ type: "patch", patch: { memory: id } });
  const points = (["point1", "point2", "point3"] as const).map((point) => t(`memory.${chosen.copy}.${point}`));
  const title = <ScreenTitle title={t("memory.title")} lead={t("memory.lead")} />;

  if (phone) {
    return (
      <div>
        {title}
        <div className="grid grid-cols-2 gap-3" role="radiogroup" aria-label={t("memory.group")}>
          {options.map(({ id, copy, Glyph, recommended }) => (
            <button
              key={id}
              type="button"
              role="radio"
              aria-checked={state.memory === id}
              onClick={() => select(id)}
              className={cn(
                "flex flex-col items-start gap-3 rounded-2xl border p-4 text-left transition-colors",
                state.memory === id ? "border-accent bg-accent/10" : "border-line bg-fill hover:bg-fill-2",
              )}
            >
              <span className="flex w-full items-center justify-between">
                <span className={cn("flex h-9 w-9 items-center justify-center rounded-lg", state.memory === id ? "bg-accent/20 text-accent" : "bg-fill-2 text-fg-2")}>
                  <Glyph size={icon.lg} />
                </span>
                <span className={cn("flex h-5 w-5 items-center justify-center rounded-full border", state.memory === id ? "border-accent bg-accent text-on-accent" : "border-line-3")}>
                  {state.memory === id && <Check size={12} strokeWidth={3} />}
                </span>
              </span>
              <span>
                <span className="block text-lg font-semibold text-fg">{t(`memory.${copy}.title`)}</span>
                <span className={cn("block text-sm", recommended ? "text-accent" : "text-fg-3")}>
                  {recommended ? t("memory.recommended") : t("memory.classic")}
                </span>
              </span>
            </button>
          ))}
        </div>
        <AnimatePresence mode="wait" initial={false}>
          <motion.div key={chosen.id} initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -6 }} transition={m.settle} className="mt-5">
            <ul className="space-y-2.5">
              {points.map((text) => (
                <li key={text} className="flex gap-3 text-base leading-snug text-fg-2">
                  <Check size={icon.md} className="mt-0.5 shrink-0 text-accent" />
                  {text}
                </li>
              ))}
            </ul>
            <p className="mt-4 text-sm text-fg-3">{t(`memory.${chosen.copy}.note`)}</p>
          </motion.div>
        </AnimatePresence>
        <div className="mt-4">
          <ActionError error={finish.error} busy={finish.busy} onRetry={finish.run} />
        </div>
      </div>
    );
  }

  return (
    <div className="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)] items-start gap-12">
      <div>
        {title}
        <div className="space-y-2" role="radiogroup" aria-label={t("memory.group")}>
          {options.map(({ id, copy, Glyph, recommended }) => (
            <button
              key={id}
              type="button"
              role="radio"
              aria-checked={state.memory === id}
              onClick={() => select(id)}
              className={cn(
                "flex w-full items-center gap-4 rounded-2xl border p-4 text-left transition-colors",
                state.memory === id ? "border-accent bg-accent/10" : "border-line bg-fill hover:bg-fill-2",
              )}
            >
              <span className={cn("flex h-10 w-10 shrink-0 items-center justify-center rounded-xl", state.memory === id ? "bg-accent/20 text-accent" : "bg-fill-2 text-fg-2")}>
                <Glyph size={icon.lg} />
              </span>
              <span className="min-w-0 flex-1">
                <span className="block text-lg font-semibold text-fg">{t(`memory.${copy}.title`)}</span>
                <span className={cn("block text-sm", recommended ? "text-accent" : "text-fg-3")}>
                  {recommended ? t("memory.recommended") : t("memory.classic")}
                </span>
              </span>
              <span className={cn("flex h-5 w-5 shrink-0 items-center justify-center rounded-full border", state.memory === id ? "border-accent bg-accent text-on-accent" : "border-line-3")}>
                {state.memory === id && <Check size={12} strokeWidth={3} />}
              </span>
            </button>
          ))}
        </div>
        <div className="mt-8 space-y-3">
          <ActionError error={finish.error} busy={finish.busy} onRetry={finish.run} />
          <ActionButton
            label={finish.busy ? t("actions.finishing") : t("actions.finish")}
            disabled={finish.busy}
            loading={finish.busy}
            block
            onClick={finish.run}
          />
        </div>
      </div>
      <AnimatePresence mode="wait" initial={false}>
        <motion.div key={chosen.id} initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -6 }} transition={m.settle} className="rounded-2xl border border-line bg-fill p-6">
          <div className="flex items-center gap-3">
            <span className="flex h-10 w-10 items-center justify-center rounded-xl bg-accent/15 text-accent">
              <chosen.Glyph size={icon.lg} />
            </span>
            <h2 className="text-xl font-semibold text-fg">{t("memory.detailTitle", { title: t(`memory.${chosen.copy}.title`) })}</h2>
          </div>
          <ul className="mt-5 space-y-3">
            {points.map((text) => (
              <li key={text} className="flex gap-3 text-base leading-snug text-fg-2">
                <Check size={icon.md} className="mt-0.5 shrink-0 text-accent" />
                {text}
              </li>
            ))}
          </ul>
          <p className="mt-6 border-t border-line pt-4 text-sm text-fg-3">{t(`memory.${chosen.copy}.note`)}</p>
        </motion.div>
      </AnimatePresence>
    </div>
  );
}
