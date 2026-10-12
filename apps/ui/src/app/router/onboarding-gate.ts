import type { AppStatus } from "@/api/generated/bindings";

export function onboardingCompleted(status: Pick<AppStatus, "ui_state">): boolean {
  const onboarding = status.ui_state.onboarding;
  return (
    typeof onboarding === "object" &&
    onboarding !== null &&
    (onboarding as { completed?: unknown }).completed === true
  );
}

export const legacyMigrationKey = "legacyMigration";

/** Whether this device holds data from the previous app that has not been moved or turned down yet. */
export function legacyMigrationOffered(status: Pick<AppStatus, "ui_state" | "legacy_state">): boolean {
  const legacy = status.legacy_state.type === "ready" || status.legacy_state.type === "requires_legacy_upgrade";
  const mark = status.ui_state[legacyMigrationKey];
  return legacy && mark !== "declined" && mark !== "migrated";
}

export const welcomePath = "/welcome";
export const welcomeBackPath = "/welcome-back";

export interface EntryState {
  completed: boolean;
  legacy: boolean;
}

/**
 * Where a visit to `pathname` must go, or null when it may stay. The welcome-back flow keeps its route once the move
 * has brought the previous app's completed onboarding along, so it can finish its last steps.
 */
export function onboardingRedirect({ completed, legacy }: EntryState, pathname: string): "/" | typeof welcomePath | typeof welcomeBackPath | null {
  if (pathname === welcomeBackPath) return completed || legacy ? null : welcomePath;
  if (!completed) {
    const target = legacy ? welcomeBackPath : welcomePath;
    return pathname === target ? null : target;
  }
  return pathname === welcomePath ? "/" : null;
}
