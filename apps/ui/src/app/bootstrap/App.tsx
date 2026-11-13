import { useEffect } from "react";
import { RouterProvider } from "@tanstack/react-router";
import { openWelcomeBackAt } from "@/features/welcome-back";
import { AppProviders } from "../providers/AppProviders";
import type { AppRuntime } from "./runtime";

type WelcomeStep = Parameters<typeof openWelcomeBackAt>[0];

export function App({ runtime }: { runtime: AppRuntime }) {
  useEffect(() => {
    const target = window as Window & { lettuceWelcome?: (step?: WelcomeStep) => void };
    target.lettuceWelcome = (step = "done") => {
      if (!openWelcomeBackAt(step)) void runtime.router.navigate({ to: "/welcome-back" });
    };
    return () => {
      delete target.lettuceWelcome;
    };
  }, [runtime.router]);
  return (
    <AppProviders i18n={runtime.i18n} api={runtime.api} queryClient={runtime.queryClient}>
      <RouterProvider router={runtime.router} />
    </AppProviders>
  );
}
