import { MessageCircle, Plus, Library, Users, Compass } from "lucide-react";
import { useLocation } from "react-router-dom";

import { TabItem } from "./NavItem";

export function BottomNav({ onCreateClick }: { onCreateClick: () => void }) {
  const { pathname } = useLocation();

  const handleCreateClick = () => {
    if (typeof window !== "undefined") {
      const globalWindow = window as any;
      if (pathname.startsWith("/settings/providers")) {
        if (typeof globalWindow.__openAddProvider === "function") {
          globalWindow.__openAddProvider();
        } else {
          window.dispatchEvent(new CustomEvent("providers:add"));
        }
        return;
      }

      if (pathname.startsWith("/settings/models")) {
        if (typeof globalWindow.__openAddModel === "function") {
          globalWindow.__openAddModel();
        } else {
          window.dispatchEvent(new CustomEvent("models:add"));
        }
        return;
      }

      if (pathname.startsWith("/settings/prompts")) {
        if (typeof globalWindow.__openAddPromptTemplate === "function") {
          globalWindow.__openAddPromptTemplate();
        } else {
          window.dispatchEvent(new CustomEvent("prompts:add"));
        }
        return;
      }
    }

    onCreateClick();
  };
  return (
    <div className="fixed bottom-0 left-0 right-0 z-30 border-t border-fg/8 bg-nav/95 px-2 pb-[calc(env(safe-area-inset-bottom)+8px)] pt-2 text-fg shadow-[0_-12px_32px_rgba(0,0,0,0.35)]">
      <div className="mx-auto flex w-full max-w-md lg:max-w-none items-stretch gap-1 lg:gap-2 lg:px-6">
        <TabItem
          to="/chat"
          icon={MessageCircle}
          label="Chats"
          active={pathname === "/" || pathname.startsWith("/chat")}
          className="flex-1 h-12 text-sm"
        />

        <TabItem
          to="/group-chats"
          icon={Users}
          label="Groups"
          active={pathname.startsWith("/group-chats")}
          className="flex-1 h-12 text-sm"
        />

        <button
          onClick={handleCreateClick}
          className="flex flex-1 h-12 items-center justify-center rounded-xl border border-fg/15 bg-fg/10 text-fg shadow-[0_8px_20px_rgba(0,0,0,0.25)] transition hover:border-fg/25 hover:bg-fg/20"
          aria-label="Create"
        >
          <Plus size={20} />
        </button>

        <TabItem
          to="/discover"
          icon={Compass}
          label="Discover"
          active={pathname.startsWith("/discover")}
          className="flex-1 h-12 text-sm"
        />

        <TabItem
          to="/library"
          icon={Library}
          label="Library"
          active={pathname.startsWith("/library")}
          className="flex-1 h-12 text-sm"
        />
      </div>
    </div>
  );
}
