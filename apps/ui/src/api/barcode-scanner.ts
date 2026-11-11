import { ApiFailure, isInsideShell } from "./client";
import type { AppPlatform } from "./generated/bindings";

export type QrScan = { status: "scanned"; content: string } | { status: "denied" } | { status: "cancelled" };

/**
 * Opens the camera and reads one QR code. A scan the user backs out of, which the plugin reports as a rejection,
 * is `cancelled`; a refused camera permission is `denied`.
 */
export async function scanQrCode(platform: AppPlatform | undefined): Promise<QrScan> {
  if (!isInsideShell() || (platform !== "android" && platform !== "ios")) {
    throw new ApiFailure("unavailable", "QR scanning is only available on mobile", null);
  }
  let scanner: typeof import("@tauri-apps/plugin-barcode-scanner");
  try {
    scanner = await import("@tauri-apps/plugin-barcode-scanner");
    let permission = await scanner.checkPermissions();
    if (permission !== "granted") permission = await scanner.requestPermissions();
    if (permission !== "granted") return { status: "denied" };
  } catch (cause) {
    throw new ApiFailure("unavailable", "the camera could not be used", null, { cause });
  }
  try {
    const scanned = await scanner.scan({ formats: [scanner.Format.QRCode] });
    return { status: "scanned", content: scanned.content };
  } catch {
    return { status: "cancelled" };
  }
}
