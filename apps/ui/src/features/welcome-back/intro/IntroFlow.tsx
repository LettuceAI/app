import { useEffect, useState } from "react";
import { AnimatePresence } from "framer-motion";
import { ArrowRight } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, cn, icon } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import { AllChanges } from "./AllChanges";
import { FeatureStage, type FeatureId } from "./FeatureStage";
import type { Platform } from "./platform";
import { RedesignStage } from "./RedesignStage";
import { createShowcase } from "./showcase";

const pages = ["redesign", "groups", "companionGroups", "creator", "models", "lorebooks"] as const;

export type IntroPage = (typeof pages)[number];

/**
 * What 3.0 brings, right after Welcome back: the redesign first, with the app's own
 * pieces filling the screen, then one page per feature with its real screen.
 */
export function IntroFlow({ platform, api, onFinish, onPage }: { platform: Platform; api: ApiClient; onFinish: () => void; onPage?: (page: IntroPage) => void }) {
  const { t } = useTranslation("welcome-back");
  const phone = platform === "phone";
  const [showcase] = useState(() => createShowcase(Date.now()));
  const [at, setAt] = useState(0);
  const [everything, setEverything] = useState(false);
  const page = pages[at] ?? pages[0];
  const last = at === pages.length - 1;
  useEffect(() => onPage?.(page), [page, onPage]);
  return (
    <div className="relative min-h-0 flex-1">
      {everything && <AllChanges phone={phone} onBack={() => setEverything(false)} />}
      <Button
        variant="ghost"
        size="sm"
        className={cn("absolute z-20 rounded-full text-fg-2", phone ? "right-4 top-1" : "right-6 top-4")}
        onClick={() => setEverything(true)}
      >
        {t("intro.everyChange")}
      </Button>
      <div className="absolute inset-0">
        <AnimatePresence mode="wait" initial={true}>
          {page === "redesign" ? (
            <RedesignStage key="redesign" platform={platform} api={api} showcase={showcase} />
          ) : (
            <FeatureStage key={page} id={page satisfies FeatureId} platform={platform} api={api} showcase={showcase} />
          )}
        </AnimatePresence>
      </div>
      <div className={cn("pointer-events-none absolute inset-x-0 bottom-0 z-10 flex items-center gap-3 [&>*]:pointer-events-auto", phone ? "flex-col px-6 pb-[calc(var(--safe-bottom,0px)+1.5rem)] pt-3" : "justify-center pb-10 pt-4")}>
        <div className="flex justify-center gap-1.5" role="tablist" aria-label={t("intro.pages")}>
          {pages.map((entry, index) => (
            <button
              key={entry}
              type="button"
              role="tab"
              aria-selected={index === at}
              aria-label={t("intro.page", { number: index + 1 })}
              onClick={() => setAt(index)}
              className={cn("h-1.5 rounded-full transition-all", index === at ? "w-5 bg-white" : "w-1.5 bg-white/30")}
            />
          ))}
        </div>
        <div className={cn("flex gap-2", phone ? "w-full" : "ml-6")}>
          {last && !phone && (
            <Button variant="ghost" size="lg" className="rounded-full text-fg-2" onClick={() => setEverything(true)}>
              {t("intro.readEveryChange")}
            </Button>
          )}
          {at > 0 && (
            <Button variant="ghost" size="lg" className={cn("rounded-full text-fg-2", phone && "flex-1")} onClick={() => setAt(at - 1)}>
              {t("intro.back")}
            </Button>
          )}
          <Button
            variant="primary"
            size="lg"
            className={cn("min-w-48 rounded-full bg-white text-black hover:bg-white/90", phone && "flex-1")}
            trailing={<ArrowRight size={icon.md} />}
            onClick={() => (last ? onFinish() : setAt(at + 1))}
          >
            {last ? t("intro.open") : at === 0 ? t("intro.start") : t("intro.next")}
          </Button>
        </div>
      </div>
    </div>
  );
}
