import { useEffect } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";

import { useI18n } from "../../core/i18n/context";
import {
  onNanoGptQuota,
  parseNanoGptTimestamp,
  primaryNanoGptQuota,
} from "../../core/usage/nanogpt";
import { toast } from "./toast";

function formatRemaining(value?: number | null): string {
  if (typeof value !== "number" || !Number.isFinite(value)) return "—";
  return new Intl.NumberFormat(undefined, {
    notation: value >= 100_000 ? "compact" : "standard",
    maximumFractionDigits: value >= 100_000 ? 1 : 0,
  }).format(value);
}

export function NanoGptQuotaMonitor() {
  const { t } = useI18n();

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let cancelled = false;

    void onNanoGptQuota(({ usage, warning }) => {
      if (!warning) return;
      const primary = primaryNanoGptQuota(usage);
      const reset = parseNanoGptTimestamp(
        primary?.window.resetAt ?? usage.currentPeriodEnd,
      );
      const remaining = formatRemaining(primary?.window.remaining);
      const description = reset
        ? t("usageAnalytics.page.nanoGpt.warningDescription", {
            account: usage.credentialLabel,
            remaining,
            reset: reset.toLocaleString(undefined, {
              dateStyle: "medium",
              timeStyle: "short",
            }),
          })
        : t("usageAnalytics.page.nanoGpt.warningDescriptionNoReset", {
            account: usage.credentialLabel,
            remaining,
          });
      const title = t("usageAnalytics.page.nanoGpt.warningTitle", {
        percent: Math.round(warning.percent * 100),
      });
      const options = { id: `nanogpt-quota-${usage.credentialId}` };

      if (warning.threshold >= 100) {
        toast.error(title, description, { ...options, duration: 12_000 });
      } else {
        toast.warning(title, description, { ...options, duration: 10_000 });
      }
    }).then((dispose) => {
      if (cancelled) dispose();
      else unlisten = dispose;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [t]);

  return null;
}
