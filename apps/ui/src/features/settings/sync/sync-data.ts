import { queryOptions, useMutation, useQueryClient } from "@tanstack/react-query";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import { isApiError, isApiFailure, type ApiClient } from "@/api/client";
import type { SyncConflictsList, SyncConflictValue, SyncStatus } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { useSettingsApi } from "../data";

export const syncStatusQuery = (api: ApiClient) =>
  queryOptions({ queryKey: queryKeys.sync.status(), queryFn: async (): Promise<SyncStatus> => api.call("syncStatus") });

export const syncConflictsQuery = (api: ApiClient) =>
  queryOptions({ queryKey: queryKeys.sync.conflicts(), queryFn: async (): Promise<SyncConflictsList> => api.call("syncConflictsList") });

export const openConflictCount = (list: SyncConflictsList | undefined) =>
  list ? list.conflicts.length + list.legacy_conflicts.length + list.forks.length : 0;

/**
 * Runs a session command. The status itself arrives as `sync_status_changed` events; the refetch afterwards only
 * guards against an event that was missed.
 */
export function useSyncAction() {
  const api = useSettingsApi();
  const client = useQueryClient();
  return useMutation({
    mutationFn: (run: (api: ApiClient) => Promise<unknown>) => run(api),
    onSettled: () => client.invalidateQueries({ queryKey: queryKeys.sync.status() }),
  });
}

/** The message for a failed session or command: the sync reason when the backend named one, else the error code. */
export function useSyncFailureText() {
  const { t } = useTranslation("sync");
  const { t: common } = useTranslation();
  return (error: unknown) => {
    const source = isApiFailure(error) || isApiError(error) ? error : null;
    if (source?.details?.type === "sync") return t(`failed.reason.${source.details.reason.type}`);
    return common(`errors.code.${source?.code ?? "internal"}`);
  };
}

/** Digits in a pairing PIN, as the host generates it. */
export const pinLength = 6;

export const isPin = (value: string) => value.length === pinLength && /^\d+$/.test(value);

export interface SyncEndpoint {
  address: string;
  port: number;
}

const isIpv4 = (value: string) => /^\d{1,3}(\.\d{1,3}){3}$/.test(value) && value.split(".").every((part) => Number(part) <= 255);

function portOf(value: unknown): number | null {
  const port = typeof value === "string" && /^\d{1,5}$/.test(value) ? Number(value) : value;
  return typeof port === "number" && Number.isInteger(port) && port >= 1 && port <= 65535 ? port : null;
}

/** `ip:port` as the host screen shows it. */
export function parseEndpoint(text: string): SyncEndpoint | null {
  const [address, port, ...rest] = text.trim().split(":");
  const parsed = portOf(port);
  return address && isIpv4(address) && parsed !== null && rest.length === 0 ? { address, port: parsed } : null;
}

/** The payload of a host QR code: `{ v, address, port, pin }`, or the previous app's `{ ip, port, pin }`. */
export function parseSyncCode(text: string): (SyncEndpoint & { pin: string }) | null {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    return null;
  }
  if (typeof value !== "object" || value === null) return null;
  const { address, ip, port, pin } = value as Record<string, unknown>;
  const host = typeof address === "string" ? address : ip;
  const parsed = portOf(port);
  if (typeof host !== "string" || !isIpv4(host) || parsed === null) return null;
  return typeof pin === "string" && isPin(pin) ? { address: host, port: parsed, pin } : null;
}

const firstLine = (text: string | null | undefined) => text?.split("\n").find((line) => line.trim() !== "")?.trim() ?? null;

/** What a candidate value is called, and a line that describes it. */
export function describeValue(value: SyncConflictValue, t: TFunction<"sync">): { name: string; detail: string | null } {
  switch (value.type) {
    case "character": {
      const { profile } = value.value.draft;
      return { name: profile.name, detail: firstLine(profile.description ?? profile.definition) };
    }
    case "persona":
      return { name: value.value.draft.title, detail: firstLine(value.value.draft.description) };
    case "persona_default":
      return { name: t("conflicts.defaultPersona"), detail: null };
    case "lorebook":
      return { name: value.value.lorebook.name, detail: null };
    case "prompt":
      return { name: value.value.prompt.name, detail: null };
    case "deleted":
      return { name: t("conflicts.deleted"), detail: null };
  }
}
