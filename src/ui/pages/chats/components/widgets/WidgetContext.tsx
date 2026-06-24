import { createContext, useContext, type ReactNode } from "react";
import type {
  Character,
  CompanionTimeOverride,
  Model,
  Persona,
  Session,
} from "../../../../../core/storage/schemas";

export interface WidgetActionContext {
  character: Character | null;
  /** Group members, when in a group chat — enables per-widget character targeting. */
  characters?: Character[];
  persona: Persona | null;
  session: Session | null;
  hasBackground: boolean;
  messageCount: number;
  sceneName: string | null;
  memories: string[];
  personas: Persona[];
  models: Model[];
  currentModelId: string | null;
  swapPlacesActive: boolean;
  voiceAutoplayActive: boolean;
  canRegenerate: boolean;
  canContinue: boolean;
  isGenerating: boolean;
  onSelectPersona: (personaId: string | null) => void | Promise<void>;
  onSelectModel: (modelId: string | null) => void | Promise<void>;
  onAuthorNoteSaved: (session: Session | null) => void;
  onRegenerate: () => void | Promise<void>;
  onToggleSwapPlaces: () => void;
  onNewSession: () => void | Promise<void>;
  onContinue: () => void | Promise<void>;
  onAbort: () => void | Promise<void>;
  onViewHistory: () => void;
  onOpenMemories: () => void;
  onOpenSearch: () => void;
  onToggleVoiceAutoplay: () => void | Promise<void>;
  onUpdateScratchPad: (nodeId: string, content: string) => void | Promise<void>;
  onUpdateAuthorNote: (content: string) => void | Promise<void>;
  onUpdateNode: (nodeId: string, patch: Record<string, unknown>) => void | Promise<void>;
  onUpdateCompanionTimeOverride: (
    override: CompanionTimeOverride | null,
  ) => void | Promise<void>;
  onInsertText: (text: string) => void;
}

const WidgetContext = createContext<WidgetActionContext | null>(null);

export function WidgetContextProvider({
  value,
  children,
}: {
  value: WidgetActionContext;
  children: ReactNode;
}) {
  return <WidgetContext.Provider value={value}>{children}</WidgetContext.Provider>;
}

export function useWidgetContext(): WidgetActionContext {
  const ctx = useContext(WidgetContext);
  if (!ctx) {
    throw new Error("useWidgetContext used outside WidgetContextProvider");
  }
  return ctx;
}
