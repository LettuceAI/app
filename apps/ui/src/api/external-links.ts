import { ApiFailure, isInsideShell } from "./client";

export async function openExternalUrl(url: string): Promise<void> {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    throw new ApiFailure("invalid_input", "external URL is malformed", null);
  }
  if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
    throw new ApiFailure("invalid_input", "external URL must use HTTP or HTTPS", null);
  }
  try {
    if (isInsideShell()) {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(parsed.href);
    } else {
      window.open(parsed.href, "_blank", "noopener,noreferrer");
    }
  } catch (cause) {
    throw new ApiFailure("unavailable", "external URL could not be opened", null, { cause });
  }
}
