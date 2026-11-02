import type { ReactNode } from "react";
import { AvatarOwnerContext, type AvatarOwner } from "./avatar-owner-context";

export function AvatarOwnerProvider({ owner, children }: { owner: AvatarOwner; children: ReactNode }) {
  return <AvatarOwnerContext.Provider value={owner}>{children}</AvatarOwnerContext.Provider>;
}
