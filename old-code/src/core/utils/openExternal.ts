import { translateStatic } from "../i18n/context";
import { toast } from "../../ui/components/toast";

function describeUrl(url: string): string {
  try {
    return new URL(url).host || url;
  } catch {
    return url;
  }
}

export async function openExternalUrl(url: string, label?: string): Promise<void> {
  toast.info(translateStatic("common.toasts.openingBrowser"), label ?? describeUrl(url));
  try {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } catch (error) {
    console.error("Failed to open URL:", error);
    try {
      window.open(url, "_blank");
    } catch {
      toast.error(translateStatic("common.toasts.openLinkFailed"), describeUrl(url));
    }
  }
}
