import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export const NANOGPT_QUOTA_EVENT = "nanogpt://quota";

export type NanoGptQuotaWindow = {
  used?: number | null;
  remaining?: number | null;
  limit?: number | null;
  percentUsed?: number | null;
  resetAt?: string | null;
  unit?: string | null;
};

export type NanoGptSubscriptionUsage = {
  credentialId: string;
  credentialLabel: string;
  active?: boolean | null;
  state?: string | null;
  weekly?: NanoGptQuotaWindow | null;
  daily?: NanoGptQuotaWindow | null;
  monthly?: NanoGptQuotaWindow | null;
  currentPeriodEnd?: string | null;
  graceUntil?: string | null;
  fetchedAt: number;
};

export type NanoGptQuotaWarning = {
  threshold: number;
  percent: number;
  kind: "weekly" | "daily" | "monthly";
};

export type NanoGptQuotaEvent = {
  usage: NanoGptSubscriptionUsage;
  warning?: NanoGptQuotaWarning | null;
};

export function onNanoGptQuota(
  handler: (event: NanoGptQuotaEvent) => void,
): Promise<UnlistenFn> {
  return listen<NanoGptQuotaEvent>(NANOGPT_QUOTA_EVENT, (event) => handler(event.payload));
}

export async function fetchNanoGptSubscriptionUsage(
  credentialId: string,
): Promise<NanoGptSubscriptionUsage> {
  return invoke<NanoGptSubscriptionUsage>("nanogpt_subscription_usage", {
    credentialId,
  });
}

export function primaryNanoGptQuota(
  usage: NanoGptSubscriptionUsage,
): { kind: "weekly" | "daily" | "monthly"; window: NanoGptQuotaWindow } | null {
  if (usage.weekly) return { kind: "weekly", window: usage.weekly };
  if (usage.daily) return { kind: "daily", window: usage.daily };
  if (usage.monthly) return { kind: "monthly", window: usage.monthly };
  return null;
}

export function quotaPercent(window: NanoGptQuotaWindow): number | null {
  if (
    typeof window.used === "number" &&
    typeof window.limit === "number" &&
    window.limit > 0
  ) {
    return Math.min(1, Math.max(0, window.used / window.limit));
  }
  if (typeof window.percentUsed === "number" && Number.isFinite(window.percentUsed)) {
    return Math.min(1, Math.max(0, window.percentUsed));
  }
  return null;
}

export function parseNanoGptTimestamp(value?: string | null): Date | null {
  if (!value) return null;
  if (/^\d+(?:\.\d+)?$/.test(value)) {
    const numeric = Number(value);
    const millis = numeric < 1_000_000_000_000 ? numeric * 1000 : numeric;
    const parsed = new Date(millis);
    return Number.isNaN(parsed.getTime()) ? null : parsed;
  }
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
}
