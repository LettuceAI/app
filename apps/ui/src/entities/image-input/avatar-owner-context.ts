import { createContext, useContext } from "react";

export interface AvatarOwner {
  /** Keeps the owner's form so it survives a trip to settings, and lets its leave guard through once. */
  keepForSettings: (routeKey: string | undefined) => void;
}

export const AvatarOwnerContext = createContext<AvatarOwner | null>(null);

export function useAvatarOwner(): AvatarOwner | null {
  return useContext(AvatarOwnerContext);
}
