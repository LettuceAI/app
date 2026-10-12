import type { RemoteModelContract } from "@/api/generated/bindings";
import { learnSlides, type GuidedRoute } from "../content/slides";

export type Step = "welcome" | "connect" | "learn" | "path" | "gemini" | "openrouter" | "model" | "memory";
export type Route = "provider" | GuidedRoute;
export type MemoryChoice = "dynamic" | "manual";

export interface SavedAccount {
  id: string;
  revision: number;
  kind: string;
}

export interface SavedModel {
  id: string;
  revision: number;
}

export interface FlowState {
  step: Step;
  route: Route;
  slide: number;
  guide: number;
  guideSlide: number;
  providerKind: string | null;
  search: string;
  apiKey: string;
  label: string;
  baseUrl: string;
  account: SavedAccount | null;
  model: RemoteModelContract | null;
  manualId: string;
  displayName: string;
  savedModel: SavedModel | null;
  memory: MemoryChoice;
}

export const initialFlow: FlowState = {
  step: "welcome",
  route: "provider",
  slide: 0,
  guide: 0,
  guideSlide: 0,
  providerKind: null,
  search: "",
  apiKey: "",
  label: "",
  baseUrl: "",
  account: null,
  model: null,
  manualId: "",
  displayName: "",
  savedModel: null,
  memory: "dynamic",
};

export interface GuideLengths {
  gemini: number;
  openrouter: number;
}

export type FlowAction =
  | { type: "patch"; patch: Partial<FlowState> }
  | { type: "go"; step: Step }
  | { type: "chooseProvider"; kind: string | null }
  | { type: "startGuided" }
  | { type: "chooseRoute"; route: GuidedRoute }
  | { type: "back"; phone: boolean; guides: GuideLengths }
  | { type: "pickModel"; model: RemoteModelContract; displayName: string }
  | { type: "pickManual"; id: string };

export function flowReducer(state: FlowState, action: FlowAction): FlowState {
  switch (action.type) {
    case "patch":
      return { ...state, ...action.patch };
    case "go":
      return { ...state, step: action.step, guide: 0, guideSlide: 0 };
    case "chooseProvider":
      return {
        ...state,
        providerKind: action.kind,
        route: "provider",
        apiKey: "",
        label: "",
        baseUrl: "",
      };
    case "startGuided":
      return { ...state, slide: 0, route: "gemini", step: "learn", guide: 0, guideSlide: 0 };
    case "chooseRoute":
      return {
        ...state,
        route: action.route,
        providerKind: action.route,
        apiKey: "",
        step: action.route,
        guide: 0,
        guideSlide: 0,
      };
    case "pickModel":
      return { ...state, model: action.model, manualId: "", displayName: action.displayName };
    case "pickManual":
      return { ...state, model: null, manualId: action.id, displayName: action.id };
    case "back":
      return goBack(state, action.phone, action.guides);
  }
}

function goBack(state: FlowState, phone: boolean, guides: GuideLengths): FlowState {
  switch (state.step) {
    case "connect":
      return phone && state.providerKind
        ? { ...state, providerKind: null }
        : { ...state, step: "welcome" };
    case "learn":
      return state.slide > 0
        ? { ...state, slide: state.slide - 1 }
        : { ...state, step: "connect", route: "provider" };
    case "path":
      return { ...state, slide: learnSlides.length - 1, step: "learn" };
    case "gemini":
    case "openrouter":
      return state.guide > 0
        ? { ...state, guide: state.guide - 1, guideSlide: 0 }
        : { ...state, step: "path" };
    case "model":
      return state.route === "provider"
        ? { ...state, step: "connect" }
        : { ...state, step: state.route, guide: guides[state.route] };
    case "memory":
      return { ...state, step: "model" };
    case "welcome":
      return state;
  }
}

export type Where =
  | { kind: "learn"; current: number; total: number }
  | { kind: "path" }
  | { kind: "connect" }
  | { kind: "guided"; route: GuidedRoute | "provider"; current: number; total: number }
  | { kind: "model" }
  | { kind: "memory" }
  | null;

export interface Progress {
  segments: number;
  filled: number;
  fraction: number;
  where: Where;
}

export function progressOf(state: FlowState, guides: GuideLengths): Progress {
  const { step, route, slide, guide } = state;
  const routeSteps = route === "gemini" ? guides.gemini + 1 : route === "openrouter" ? guides.openrouter + 1 : 1;
  const total = routeSteps + 2;
  const guiding = step === "gemini" || step === "openrouter";
  const position = guiding ? guide + 1 : step === "connect" ? 1 : step === "model" ? routeSteps + 1 : step === "memory" ? total : 0;
  const tutorial = route !== "provider";
  const learnTotal = learnSlides.length;
  const segments = step === "learn" || step === "path" ? learnTotal + 1 : tutorial ? total : 3;
  const filled =
    step === "learn" ? slide + 1
    : step === "path" ? learnTotal + 1
    : tutorial ? position
    : step === "connect" ? 1
    : step === "model" ? 2
    : 3;
  const fraction =
    step === "learn" ? (slide + 1) / (learnTotal + 1)
    : step === "path" ? 1
    : tutorial ? position / total
    : step === "connect" ? 1 / 3
    : step === "model" ? 2 / 3
    : step === "memory" ? 1
    : 0;
  const where: Where =
    step === "learn" ? { kind: "learn", current: slide + 1, total: learnTotal }
    : step === "path" ? { kind: "path" }
    : step === "connect" ? { kind: "connect" }
    : tutorial && position ? { kind: "guided", route, current: position, total }
    : step === "model" ? { kind: "model" }
    : step === "memory" ? { kind: "memory" }
    : null;
  return { segments, filled, fraction, where };
}
