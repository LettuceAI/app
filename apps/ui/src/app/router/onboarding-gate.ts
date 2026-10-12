import type { AppStatus } from "@/api/generated/bindings";

export function onboardingCompleted(status: Pick<AppStatus, "ui_state">): boolean {
  const onboarding = status.ui_state.onboarding;
  return (
    typeof onboarding === "object" &&
    onboarding !== null &&
    (onboarding as { completed?: unknown }).completed === true
  );
}

export const welcomePath = "/welcome";

/** Where a visit to `pathname` must go, or null when it may stay. */
export function onboardingRedirect(completed: boolean, pathname: string): "/" | typeof welcomePath | null {
  const onWelcome = pathname === welcomePath;
  if (!completed && !onWelcome) return welcomePath;
  if (completed && onWelcome) return "/";
  return null;
}
