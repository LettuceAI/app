import { platform, arch, type } from "@tauri-apps/plugin-os";
import type { Platform, Arch, OsType } from "@tauri-apps/plugin-os";

export type AppPlatform = {
    type: "mobile" | "desktop";
    os: Platform;
    arch: Arch;
};

export function getPlatform(): AppPlatform {
    const platformType: OsType = type();
    const platformName: Platform = platform();
    const archName: Arch = arch();

    switch (platformType) {
        case "android":
        case "ios":
            return {
                type: "mobile",
                os: platformName,
                arch: archName
            };
        default:
            return {
                type: "desktop",
                os: platformName,
                arch: archName
            };
    };
};