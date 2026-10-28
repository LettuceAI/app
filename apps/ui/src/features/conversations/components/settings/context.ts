import type { QueryClient } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { ConversationSettingsPatch_Deserialize, ConversationSettingsView, ConversationView } from "@/api/generated/bindings";
import type { ConversationServices } from "../../data/actions";

export type PanelView =
  | "root" | "history" | "notes" | "starters" | "advanced";

export type PanelSheet = "persona" | "model" | "background" | "note" | "lorebooks" | "history_actions" | "companion_time";

export interface PanelCtx {
  api: ApiClient;
  queryClient: QueryClient;
  services: ConversationServices;
  conversationId: string;
  view: ConversationView;
  settings: ConversationSettingsView | undefined;
  characterId: string | null;
  busy: boolean;
  activeSheet: PanelSheet | null;
  footerTarget: HTMLDivElement | null;
  save: (change: Partial<ConversationSettingsPatch_Deserialize>) => Promise<boolean>;
  run: <T>(task: () => Promise<T>) => Promise<T | undefined>;
  open: (view: PanelView) => void;
  openSheet: (sheet: PanelSheet) => void;
  closeSheet: () => void;
  back: () => void;
  close: () => void;
  home: () => void;
  goTo: (conversationId: string, replace: boolean) => void;
}
