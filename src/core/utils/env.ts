export const DEVELOPER_MODE_OVERRIDE_STORAGE_KEY = "lettuceai:developer-mode-enabled";

function readDeveloperModeOverride(): boolean {
  if (typeof window === "undefined") return false;
  return window.localStorage.getItem(DEVELOPER_MODE_OVERRIDE_STORAGE_KEY) === "true";
}

export function setDeveloperModeOverride(enabled: boolean) {
  if (typeof window === "undefined") return;
  if (enabled) {
    window.localStorage.setItem(DEVELOPER_MODE_OVERRIDE_STORAGE_KEY, "true");
  } else {
    window.localStorage.removeItem(DEVELOPER_MODE_OVERRIDE_STORAGE_KEY);
  }
}

/**
 * Detect if the app is running in development mode
 * In Tauri, we can use the Vite environment variable or check the build mode
 */
export function isDevelopmentMode(): boolean {
  return import.meta.env.DEV || readDeveloperModeOverride();
}

/**
 * Get the current environment (development, production, etc.)
 */
export function getEnvironment(): string {
  return import.meta.env.MODE;
}

/**
 * Detect if the app is running in production mode
 */
export function isProductionMode(): boolean {
  return import.meta.env.PROD;
}
