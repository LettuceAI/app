import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { appStatusQuery } from "@/api/app-status-query";
import type { ApiClient } from "@/api/client";
import { queryKeys } from "@/api/query-keys";

export const desktopViews = ["home", "list", "hero"] as const;
export type DesktopView = (typeof desktopViews)[number];

export const phoneLayouts = ["list", "gallery", "hero", "recent"] as const;
export type PhoneLayout = (typeof phoneLayouts)[number];

const desktopKey = "homeDesktopView";
const phoneKey = "homePhoneLayout";

function pick<T extends string>(value: unknown, options: readonly T[], fallback: T): T {
  return options.find((option) => option === value) ?? fallback;
}

/** A layout choice kept in this device's UI state, shown at once and saved behind it. */
function useStoredChoice<T extends string>(api: ApiClient, key: string, options: readonly T[], fallback: T) {
  const queryClient = useQueryClient();
  const status = useQuery(appStatusQuery(api));
  const [chosen, setChosen] = useState<T | null>(null);
  const value = chosen ?? pick(status.data?.ui_state[key], options, fallback);
  const choose = (next: T) => {
    setChosen(next);
    void api
      .call("appUiStateUpdate", { patch: { [key]: next } })
      .then(() => queryClient.invalidateQueries({ queryKey: queryKeys.app.status() }));
  };
  return [value, choose] as const;
}

export function useDesktopView(api: ApiClient) {
  return useStoredChoice<DesktopView>(api, desktopKey, desktopViews, "home");
}

export function usePhoneLayout(api: ApiClient) {
  return useStoredChoice<PhoneLayout>(api, phoneKey, phoneLayouts, "list");
}
