import { createLazyRoute } from "@tanstack/react-router";
import { OnboardingPage } from "./OnboardingPage";

export const Route = createLazyRoute("/welcome")({
  component: OnboardingPage,
});
