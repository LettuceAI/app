import type { QueryClient } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { AppStatus } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { tours, type TourId } from "./tours";

const writes = new WeakMap<QueryClient, Promise<void>>();

export function tooltipFlags(state: AppStatus["ui_state"] | undefined): Record<string, unknown> {
  const flags = state?.tooltips;
  if (flags && typeof flags === "object" && !Array.isArray(flags)) return { ...flags };
  return {};
}

export function writeTourFlags(api: ApiClient, client: QueryClient, completed: TourId | "reset") {
  const pending = writes.get(client) ?? Promise.resolve();
  const write = pending.catch(() => undefined).then(async () => {
    const key = queryKeys.app.status();
    const current = client.getQueryData<AppStatus>(key);
    const tooltips = tooltipFlags(current?.ui_state);
    if (completed === "reset") {
      for (const tour of Object.values(tours)) delete tooltips[tour.storageKey];
    } else {
      tooltips[tours[completed].storageKey] = true;
    }
    const result = await api.call("appUiStateUpdate", { patch: { tooltips } });
    client.setQueryData<AppStatus>(key, (status) =>
      status ? { ...status, ui_state: { ...status.ui_state, tooltips: result.state.tooltips } } : status,
    );
  });
  writes.set(client, write);
  return write;
}
