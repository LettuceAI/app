import { toast } from "../../ui/components/toast";
import { setTooltipSeen } from "../storage/appState";
import { openExternalUrl } from "../utils/openExternal";
import { getAppUpdateDismissKey, type AppUpdateInfo } from "./checkForAppUpdate";

export const NEXT_RELEASE_AVAILABLE_EVENT = "lettuce:next-release-available";

type UpdateToastLabels = {
  title: string;
  description: string;
  viewLabel: string;
  laterLabel: string;
};

export function presentAppUpdateToast(
  update: AppUpdateInfo,
  platformOs: string,
  labels: UpdateToastLabels,
) {
  if (update.isNextRelease) {
    window.dispatchEvent(new CustomEvent(NEXT_RELEASE_AVAILABLE_EVENT, { detail: update }));
    return;
  }

  const dismissKey = getAppUpdateDismissKey(update, platformOs);
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
