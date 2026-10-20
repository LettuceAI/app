import type { QueryClient } from "@tanstack/react-query";
import type { RouterHistory } from "@tanstack/react-router";
import type { i18n } from "i18next";
import { createApiClient, selectTransport, type ApiClient } from "@/api/client";
import type { Transport } from "@/api/transport";
import { createI18n } from "@/shared/i18n";
import { followSystemLocale, selectDeviceLocale } from "./device-locale";
import { createQueryClient } from "../providers/query-client";
import { createAppRouter, type AppRouter } from "../router/router";

export interface AppRuntime {
  api: ApiClient;
  queryClient: QueryClient;
  i18n: i18n;
  router: AppRouter;
}

export interface AppRuntimeOptions {
  transport?: Transport;
  history?: RouterHistory;
  i18n?: i18n;
}

export async function createAppRuntime(options: AppRuntimeOptions = {}): Promise<AppRuntime> {
  const api = createApiClient(options.transport ?? (await selectTransport()));
  let localization = options.i18n;
  if (!localization) {
    const status = await api.call("appStatus");
    const state = status.ui_state;
    const android = status.platform === "android";
    const locale = selectDeviceLocale(state, () => window.localStorage.getItem("app-locale"), android);
    if (state.locale !== locale) {
      await api.call("appUiStateUpdate", { patch: { locale } }).catch(() => undefined);
    }
    localization = await createI18n(locale);
    if (android) followSystemLocale(localization, (next) => api.call("appUiStateUpdate", { patch: { locale: next } }).then(() => undefined));
  }
  const queryClient = createQueryClient();
  const router = createAppRouter(
    { api, queryClient },
    options.history ? { history: options.history } : {},
  );
  return { api, queryClient, i18n: localization, router };
}
