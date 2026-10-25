import { type ReactNode } from "react";
import { type ChatLook } from "./chat";
import { ChatAppearanceContext } from "./context";

export function ChatAppearanceProvider({
  value,
  children,
}: {
  value: ChatLook;
  children: ReactNode;
}) {
  return <ChatAppearanceContext.Provider value={value}>{children}</ChatAppearanceContext.Provider>;
}
