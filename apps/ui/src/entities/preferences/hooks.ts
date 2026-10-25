import { useContext, useMemo } from "react";
import { PreferencesContext, ChatAppearanceContext } from "./context";
import { chatAppearance } from "./appearance";
export function usePreferences() {
  const context = useContext(PreferencesContext);
  if (!context) throw new Error("PreferencesProvider is missing");
  return context;
}
export function useChatLook() {
  return useContext(ChatAppearanceContext);
}
export function useMessageAppearance() {
  const look = useChatLook();
  return useMemo(() => chatAppearance(look), [look]);
}
