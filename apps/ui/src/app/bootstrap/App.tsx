import { RouterProvider } from "@tanstack/react-router";
import { AppProviders } from "../providers/AppProviders";
import type { AppRuntime } from "./runtime";

export function App({ runtime }: { runtime: AppRuntime }) {
  return (
    <AppProviders i18n={runtime.i18n} queryClient={runtime.queryClient}>
      <RouterProvider router={runtime.router} />
    </AppProviders>
  );
}
