export type ReachableStep = "welcome" | "migrate" | "models" | "done" | "intro";

let requestedStep: ReachableStep | null = null;
let showStep: ((step: ReachableStep) => void) | null = null;

/** Temporary developer hook: opens Welcome back at a given step, for checking the 3.0 welcome. */
export function openWelcomeBackAt(step: ReachableStep): boolean {
  if (showStep) {
    showStep(step);
    return true;
  }
  requestedStep = step;
  return false;
}

export function takeRequestedStep(): ReachableStep | null {
  const step = requestedStep;
  requestedStep = null;
  return step;
}

export function listenForStepRequests(show: (step: ReachableStep) => void): () => void {
  showStep = show;
  return () => {
    if (showStep === show) showStep = null;
  };
}
