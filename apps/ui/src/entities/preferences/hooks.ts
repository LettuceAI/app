import { useContext, useMemo, useSyncExternalStore } from "react";
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

const DARK_QUERY = "(prefers-color-scheme: dark)";
const subscribeSystemDark = (notify: () => void) => {
  const query = window.matchMedia(DARK_QUERY);
  query.addEventListener("change", notify);
  return () => query.removeEventListener("change", notify);
};
const readSystemDark = () => window.matchMedia(DARK_QUERY).matches;
export function useSystemDark(): boolean {
  return useSyncExternalStore(subscribeSystemDark, readSystemDark);
}
