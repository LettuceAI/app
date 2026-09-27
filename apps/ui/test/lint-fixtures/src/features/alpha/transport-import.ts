import { createMockTransport } from "@/api/mock-transport";
import { createTauriTransport } from "@/api/tauri-transport";

export const leaked = [createMockTransport, createTauriTransport];
