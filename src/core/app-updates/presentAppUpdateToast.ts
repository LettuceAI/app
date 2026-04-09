import { toast } from "../../ui/components/toast";
import { setTooltipSeen } from "../storage/appState";
import type { AppUpdateInfo } from "./checkForAppUpdate";

type UpdateToastLabels = {
  title: string;
  description: string;
  viewLabel: string;
  laterLabel: string;
};

async function openExternalUrl(url: string) {
  try {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } catch {
    window.open(url, "_blank");
  }
}

export function presentAppUpdateToast(
  update: AppUpdateInfo,
  platformOs: string,
  labels: UpdateToastLabels,
) {
  const dismissKey = `app-update-dismissed:${platformOs}:${update.releaseTag}`;
  const targetUrl = update.channel === "dev" ? update.releaseUrl : update.downloadUrl;

  toast.success(labels.title, labels.description, {
    id: "app-update-available",
    duration: 12000,
    actionLabel: labels.viewLabel,
    actionTone: "accent",
    onAction: async () => {
      await setTooltipSeen(dismissKey, true);
      await openExternalUrl(targetUrl);
    },
    secondaryLabel: labels.laterLabel,
    secondaryTone: "neutral",
    onSecondary: () => {
      void setTooltipSeen(dismissKey, true);
    },
  });
}
