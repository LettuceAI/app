import { useState } from "react";
import { ArrowLeft, ArrowRight } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn, Dialog, IconButton, icon } from "@lettuceai/crisp";
import { shotUrl } from "../assets";
import type { GuidedRoute } from "../content/slides";
import { useOnboarding } from "../flow/context";
import { guideContent, type GuideName } from "../content/guide-content";

function guideNameOf(route: GuidedRoute, phone: boolean): GuideName {
  return `${route}${phone ? "Mobile" : "Desktop"}`;
}

export function GuideScreen({ route }: { route: GuidedRoute }) {
  const { t } = useTranslation("onboarding");
  const { state, dispatch, phone } = useOnboarding();
  const [zoom, setZoom] = useState<string | null>(null);
  const steps = guideContent[guideNameOf(route, phone)];
  const step = steps[Math.min(state.guide, steps.length - 1)];
  if (!step) return null;
  const slide = step.slides[Math.min(state.guideSlide, step.slides.length - 1)];
  const many = step.slides.length > 1;
  const setSlide = (index: number) => dispatch({ type: "patch", patch: { guideSlide: index } });

  const caption = (index: number, text: string) => (
    <p className="mt-3 flex gap-2.5 text-base text-fg-2">
      <span className="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-accent text-2xs font-bold text-on-accent">
        {index + 1}
      </span>
      {text}
    </p>
  );

  const text = (
    <div>
      <h1 className={cn("font-bold tracking-[-0.02em] text-fg", phone ? "text-3xl" : "text-4xl")}>{t(step.title)}</h1>
      <p className="mt-3 max-w-[52ch] text-base leading-relaxed text-fg-2">{t(step.body)}</p>
    </div>
  );

  const picture = phone ? (
    <div className="-mx-5 flex snap-x snap-mandatory gap-4 overflow-x-auto px-5 [scrollbar-width:none]">
      {step.slides.map((item, index) => (
        <div key={item.shot} className="w-[240px] shrink-0 snap-center">
          <button
            type="button"
            onClick={() => setZoom(shotUrl(item.shot))}
            className="block w-full overflow-hidden rounded-2xl ring-1 ring-line-2"
          >
            <img src={shotUrl(item.shot)} alt="" className="block w-full" />
          </button>
          {caption(index, t(item.caption))}
        </div>
      ))}
    </div>
  ) : slide ? (
    <div>
      <button
        type="button"
        onClick={() => setZoom(shotUrl(slide.shot))}
        className="block w-full overflow-hidden rounded-xl ring-1 ring-line-2"
      >
        <img src={shotUrl(slide.shot)} alt="" className="block w-full" />
      </button>
      {caption(state.guideSlide, t(slide.caption))}
      {many && (
        <div className="mt-3 flex items-center gap-2">
          <IconButton
            label={t("guide.previousPicture")}
            size="sm"
            onClick={() => setSlide((state.guideSlide - 1 + step.slides.length) % step.slides.length)}
          >
            <ArrowLeft size={icon.sm} />
          </IconButton>
          <span className="text-sm tabular-nums text-fg-3">
            {t("guide.pictureCount", { current: state.guideSlide + 1, total: step.slides.length })}
          </span>
          <IconButton
            label={t("guide.nextPicture")}
            size="sm"
            onClick={() => setSlide((state.guideSlide + 1) % step.slides.length)}
          >
            <ArrowRight size={icon.sm} />
          </IconButton>
        </div>
      )}
    </div>
  ) : null;

  return (
    <>
      {phone ? (
        <div className="space-y-7">
          {text}
          {picture}
        </div>
      ) : (
        <div className="grid grid-cols-[minmax(0,5fr)_minmax(0,6fr)] items-start gap-12">
          {text}
          {picture}
        </div>
      )}
      <Dialog open={zoom !== null} onClose={() => setZoom(null)} size="lg" title={t("guide.screenshot")}>
        {zoom && <img src={zoom} alt="" className="w-full rounded-lg" />}
      </Dialog>
    </>
  );
}
