import type { ErrorComponentProps } from "@tanstack/react-router";
import { FailureState } from "@/shared/ui/FailureState";

export function RouteErrorState({ error, reset }: ErrorComponentProps) {
  return <FailureState error={error} onRetry={reset} />;
}
