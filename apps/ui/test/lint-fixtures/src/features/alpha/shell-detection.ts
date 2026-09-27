declare const __TAURI_INTERNALS__: unknown;

export const direct = __TAURI_INTERNALS__;
export const viaWindow = (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__;
