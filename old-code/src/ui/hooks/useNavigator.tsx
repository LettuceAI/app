import { createContext, ReactNode, useCallback, useContext, useMemo } from "react";
import { useLocation } from "react-router-dom";
import { Routes, useNavigationManager, resolveBackTarget } from "../navigation";

type NavigatorApi = {
  back: () => void;
  canExitApp: (path: string) => boolean;
};

const NavigatorContext = createContext<NavigatorApi | null>(null);

export function NavigatorProvider({ children }: { children: ReactNode }) {
  const { go, backOrReplace } = useNavigationManager();
  const location = useLocation();

  const back = useCallback(() => {
    const currentPath = location.pathname + location.search;
    const target = resolveBackTarget(currentPath);
    if (target) {
      go(target, { replace: true });
    } else {
      backOrReplace(Routes.chat);
    }
  }, [location.pathname, location.search, go, backOrReplace]);

  const canExitApp = useCallback((path: string) => {
    return path === "/" || path === "/chat" || path === "/library";
  }, []);

  const value = useMemo(() => ({ back, canExitApp }), [back, canExitApp]);

  return <NavigatorContext.Provider value={value}>{children}</NavigatorContext.Provider>;
}

export function useNavigator(): NavigatorApi {
  const ctx = useContext(NavigatorContext);
  if (!ctx) {
    throw new Error("useNavigator must be used within a NavigatorProvider");
  }
  return ctx;
}
