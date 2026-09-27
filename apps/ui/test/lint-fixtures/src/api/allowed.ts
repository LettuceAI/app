import { invoke } from "@tauri-apps/api/core";
import { createMockTransport } from "@/api/mock-transport";
import { commands } from "@/api/generated/bindings";

export const allowed = [invoke, createMockTransport, commands, (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__];
