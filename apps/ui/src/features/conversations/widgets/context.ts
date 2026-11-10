import { createContext, useContext, useMemo, useRef } from "react";
import type { ApiClient } from "@/api/client";
import type { AssetRef_Serialize, ConversationSettingsView, MemoryItemView, TimeChange } from "@/api/generated/bindings";
import type { WidgetNode } from "./nodes";

export interface WidgetPerson {
  id: string | null;
  name: string;
  avatar: string | null;
  description: string | null;
}

/** What the widgets read from the open chat and the actions they can take on it. Nothing that changes per keystroke lives here. */
export interface WidgetActions {
  api: ApiClient;
  conversationId: string;
  isGroup: boolean;
  companion: boolean;
  character: WidgetPerson | null;
  members: readonly WidgetPerson[];
  persona: WidgetPerson | null;
  personaLoading: boolean;
  settings: ConversationSettingsView | undefined;
  messageCount: number | undefined;
  sceneName: string | null;
  memoryOn: boolean;
  memories: readonly MemoryItemView[];
  memoryBusy: boolean;
  sending: boolean;
  canRegenerate: boolean;
  canContinue: boolean;
  swap: boolean;
  assets: ReadonlyMap<string, string>;
  regenerate: () => void;
  continueReply: () => void;
  stop: () => void;
  toggleSwap: () => void;
  newChat: () => void;
  openHistory: () => void;
  openMemories: () => void;
  openSearch: () => void;
  openPersona: () => void;
  openNote: () => void;
  openModel: () => void;
  saveAuthorNote: (text: string) => Promise<boolean>;
  insertText: (text: string) => void;
  updateNode: (id: string, change: (node: WidgetNode) => WidgetNode) => Promise<boolean>;
  pickImage: () => Promise<AssetRef_Serialize | null>;
  setTime: (change: TimeChange) => Promise<boolean>;
}

export const WidgetContext = createContext<WidgetActions | null>(null);

export function useWidgets(): WidgetActions {
  const actions = useContext(WidgetContext);
  if (!actions) throw new Error("WidgetContext is missing");
  return actions;
}

/** Functions that stay the same from render to render yet always run the latest ones given, so a context value built from them changes only with its data. */
export function useStableActions<T extends { [K in keyof T]: (...args: never[]) => unknown }>(latest: T): T {
  const current = useRef(latest);
  current.current = latest;
  return useMemo(
    () =>
      Object.fromEntries(
        (Object.keys(latest) as (keyof T)[]).map((key) => [key, (...args: never[]) => current.current[key](...args)]),
      ) as T,
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );
}
