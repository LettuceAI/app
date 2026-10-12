import { createLazyRoute } from "@tanstack/react-router";
import { WelcomeBackPage } from "./WelcomeBackPage";

export const Route = createLazyRoute("/welcome-back")({
  component: WelcomeBackPage,
});
