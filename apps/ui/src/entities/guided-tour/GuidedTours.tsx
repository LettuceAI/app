import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import { Button, Card, Text, toast, useCrispStrings, useDismissLayer } from "@lettuceai/crisp";
import { useBlocker } from "@tanstack/react-router";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { AppStatus } from "@/api/generated/bindings";
import { tooltipFlags, writeTourFlags } from "./state";
import { tours, type TourId, type TourStep } from "./tours";

interface TargetStep extends TourStep {
  element: Element;
}

function tourIds(pathname: string): readonly TourId[] {
  if (pathname === "/") return ["appShell"];
  if (pathname === "/settings/speech") return ["speechRecognition"];
  if (pathname === "/settings/memory") return ["dynamicMemory"];
  if (pathname === "/settings/imageGeneration") return ["imageGeneration", "sdEngine", "loraLibrary", "civitaiBrowse"];
  if (pathname === "/settings/model-browser") return ["hfBrowser"];
  if (pathname === "/settings/models/runtime-defaults") return ["runtimeDefaults"];
  if (pathname.startsWith("/settings/models/")) return ["editModelLlama"];
  if (pathname.startsWith("/chat/")) return ["groupChatDetail"];
  if (pathname === "/playground") return ["playground"];
  return [];
}

const control = "button, [role=tab], input, textarea";
function visible(element: Element): boolean {
  return element.getClientRects().length > 0 && getComputedStyle(element).visibility === "visible";
}

function findTarget(step: TourStep, navigation: HTMLElement | undefined, createLabel: string, searchLabels: readonly string[]) {
  const targets = Array.from(document.querySelectorAll(`[data-tour-id="${step.target}"]`));
  if (step.target === "nav-create" && navigation) {
    targets.push(...Array.from(navigation.querySelectorAll("button")).filter((button) =>
      (button.getAttribute("aria-label") ?? button.textContent?.trim()) === createLabel,
    ));
  }
  if (step.target === "top-search") {
    targets.push(...Array.from(document.querySelectorAll("button[aria-label], input[aria-label]")).filter(
      (element) => searchLabels.some((label) => element.getAttribute("aria-label") === label),
    ));
  }
  return targets.map((element) => element.closest(control) ?? element).find(visible);
}

interface ActiveTour {
  id: TourId;
  steps: TargetStep[];
}

export function GuidedTours({ api, pathname, mobile, status }: {
  api: ApiClient;
  pathname: string;
  mobile: boolean;
  status: AppStatus | undefined;
}) {
  const { t } = useTranslation();
  const { t: homeText } = useTranslation("home");
  const { strings } = useCrispStrings();
  const client = useQueryClient();
  const [active, setActive] = useState<ActiveTour | null>(null);
  const [dismissed, setDismissed] = useState<ReadonlySet<TourId>>(() => new Set());
  const startReady = status !== undefined;
  const flags = status?.ui_state.tooltips;
  const navigationLabel = t("navigation.primary");
  const createLabel = t("navigation.create");
  const searchChats = homeText("search");
  const searchCharacters = homeText("searchCharacters");

  useEffect(() => {
    if (!startReady || active || typeof HTMLDialogElement.prototype.showModal !== "function") return;
    const stored = tooltipFlags({ tooltips: flags });
    const unseen = tourIds(pathname).filter((id) => !stored[tours[id].storageKey] && !dismissed.has(id));
    if (unseen.length === 0) return;

    let timer = 0;
    const discover = () => {
      timer = 0;
      // Let an existing sheet or dialog finish before opening a tour.
      if (Array.from(document.querySelectorAll('[role="dialog"], dialog[open]')).some(visible)) return;
      const navigation = Array.from(document.querySelectorAll("nav")).find(
        (nav) => nav.getAttribute("aria-label") === navigationLabel,
      );
      for (const id of unseen) {
        const definition: readonly TourStep[] = tours[id].steps;
        const readyTarget = id === "imageGeneration" ? "imagegen-avatar" : definition[0]?.target;
        const steps = definition.flatMap((step) => {
          if (mobile && step.desktopOnly) return [];
          const element = findTarget(step, navigation, createLabel, [strings.appBarSearch, searchChats, searchCharacters]);
          return element ? [{ ...step, element }] : [];
        });
        if (!steps.some((step) => step.target === readyTarget)) continue;
        observer.disconnect();
        setActive({ id, steps });
        return;
      }
    };
    const schedule = () => {
      if (!timer) timer = window.setTimeout(discover, 300);
    };
    const observer = new MutationObserver(schedule);
    observer.observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ["data-tour-id", "open", "hidden"] });
    schedule();
    return () => {
      observer.disconnect();
      window.clearTimeout(timer);
    };
  }, [active, createLabel, dismissed, flags, mobile, navigationLabel, pathname, searchChats, searchCharacters, startReady, strings.appBarSearch]);

  if (!active) return null;
  const finish = () => {
    setActive(null);
    setDismissed((previous) => new Set(previous).add(active.id));
    void writeTourFlags(api, client, active.id).catch((error: unknown) => {
      toast.error(t(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    });
  };
  return <TourSpotlight steps={active.steps} onFinish={finish} />;
}

const edge = 12;
const gap = 12;
const padding = 6;
const clamp = (value: number, min: number, max: number) => Math.max(min, Math.min(value, max));

function TourSpotlight({ steps, onFinish }: { steps: TargetStep[]; onFinish: () => void }) {
  const { t } = useTranslation("tour");
  const [index, setIndex] = useState(0);
  const dialog = useRef<HTMLDialogElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const nextButton = useRef<HTMLButtonElement>(null);
  const [geometry, setGeometry] = useState<{ hole: CSSProperties; panel: CSSProperties } | null>(null);
  const step = steps[index]!;
  const last = index === steps.length - 1;
  const next = () => {
    if (last) onFinish();
    else setIndex(index + 1);
  };

  useDismissLayer(true, onFinish);
  useBlocker({
    shouldBlockFn: ({ action }) => {
      if (action !== "BACK") return false;
      onFinish();
      return true;
    },
    enableBeforeUnload: false,
  });

  useLayoutEffect(() => {
    const element = dialog.current;
    if (!element) return;
    element.showModal();
    return () => element.close();
  }, []);

  useLayoutEffect(() => {
    if (!step.element.isConnected) return;
    const anchor = step.element;
    const update = () => {
      if (!panel.current) return;
      const target = anchor.getBoundingClientRect();
      const { width, height } = panel.current.getBoundingClientRect();
      const vw = window.innerWidth;
      const vh = window.innerHeight;
      const left = clamp(target.left - padding, 0, vw);
      const top = clamp(target.top - padding, 0, vh);
      const right = clamp(target.right + padding, left, vw);
      const bottom = clamp(target.bottom + padding, top, vh);
      const centered = clamp((left + right - width) / 2, edge, vw - width - edge);
      const fitsBelow = bottom + gap + height + edge <= vh;
      const fitsAbove = top - gap - height >= edge;
      const beside = !fitsBelow && !fitsAbove;
      const fitsRight = right + gap + width + edge <= vw;
      const fitsLeft = left - gap - width >= edge;
      const panelLeft = beside && fitsRight ? right + gap : beside && fitsLeft ? left - gap - width : centered;
      const panelTop = fitsBelow ? bottom + gap : fitsAbove ? top - gap - height : vh - height - edge;
      setGeometry({
        hole: { left, top, width: right - left, height: bottom - top },
        panel: { left: panelLeft, top: clamp(panelTop, edge, vh - height - edge) },
      });
    };

    anchor.scrollIntoView({ block: "nearest", inline: "nearest" });
    update();
    nextButton.current?.focus({ preventScroll: true });
    const resize = new ResizeObserver(update);
    resize.observe(anchor);
    if (panel.current) resize.observe(panel.current);
    document.addEventListener("scroll", update, true);
    window.addEventListener("resize", update);
    return () => {
      resize.disconnect();
      document.removeEventListener("scroll", update, true);
      window.removeEventListener("resize", update);
    };
  }, [step]);

  return createPortal(
    <dialog
      ref={dialog}
      aria-labelledby="guided-tour-title"
      aria-describedby="guided-tour-body"
      className="fixed inset-0 m-0 h-full max-h-none w-full max-w-none overflow-hidden bg-transparent p-0 backdrop:bg-transparent"
      onCancel={(event) => {
        event.preventDefault();
        onFinish();
      }}
    >
      {geometry && <div aria-hidden className="pointer-events-none absolute rounded-xl outline-2 outline-accent" style={{ ...geometry.hole, boxShadow: "0 0 0 200vmax rgb(0 0 0 / 0.65)" }} />}
      <Card ref={panel} level="raised" className="absolute max-h-[calc(100dvh-1.5rem)] w-[min(22rem,calc(100vw-1.5rem))] overflow-y-auto border border-line" style={geometry?.panel}>
        <div aria-live="polite" className="space-y-2">
          <Text variant="hint">{t("stepCounter", { current: index + 1, total: steps.length })}</Text>
          <h2 id="guided-tour-title" className="text-base font-semibold text-fg">{t(`${step.key}.title`)}</h2>
          <Text id="guided-tour-body" className="text-fg-2">{t(`${step.key}.body`)}</Text>
        </div>
        <div className="mt-4 flex flex-wrap items-center justify-between gap-2">
          <Button variant="ghost" onClick={onFinish}>{t("skipTour")}</Button>
          <Button ref={nextButton} variant="primary" onClick={next}>{t(last ? "gotIt" : "next")}</Button>
        </div>
      </Card>
    </dialog>,
    document.body,
  );
}
