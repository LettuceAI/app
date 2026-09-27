import type { ReactNode } from "react";
import { ThemeProvider } from "@lettuceai/crisp";
import { QueryClientProvider, type QueryClient } from "@tanstack/react-query";
import { MotionConfig } from "framer-motion";
import type { i18n } from "i18next";
import { I18nextProvider } from "react-i18next";
import type { ApiClient } from "@/api/client";
import { FatalErrorBoundary } from "../error-boundaries/FatalErrorBoundary";
import { AppEventBridge } from "./AppEventBridge";

export interface ShellProvidersProps {
  i18n: i18n;
  children: ReactNode;
}

/** Localization, theme and motion: everything a screen needs, including the fatal error screen. */
export function ShellProviders({ i18n, children }: ShellProvidersProps) {
  return (
    <I18nextProvider i18n={i18n}>
      <ThemeProvider>
        <MotionConfig reducedMotion="user">
          <FatalErrorBoundary>{children}</FatalErrorBoundary>
        </MotionConfig>
      </ThemeProvider>
    </I18nextProvider>
  );
}

export interface AppProvidersProps extends ShellProvidersProps {
  api: ApiClient;
  queryClient: QueryClient;
}

export function AppProviders({ i18n, api, queryClient, children }: AppProvidersProps) {
  return (
    <ShellProviders i18n={i18n}>
      <QueryClientProvider client={queryClient}>
        <AppEventBridge api={api} queryClient={queryClient} />
        {children}
      </QueryClientProvider>
    </ShellProviders>
  );
}
