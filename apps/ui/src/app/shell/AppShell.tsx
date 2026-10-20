import { AppShell as CrispAppShell, cn, CrispStringsProvider, icon, TitleBar, useCompactLayout, type NavDestination } from "@lettuceai/crisp";
import { useQuery } from "@tanstack/react-query";
import { Outlet, useLocation, useNavigate, useRouteContext } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { ArrowLeft, Compass, House, Library, MessageSquare, Settings, Users } from "lucide-react";
import { useTranslation } from "react-i18next";
import { appStatusQuery } from "@/api/app-status-query";
import type { WindowEdge } from "@/api/window-controls";
import { useSidebarOnDetailPages } from "@/shared/hooks/useSidebarOnDetailPages";
import { useChatNavigation } from "@/features/conversations";
import { ChatMenu, SidebarRecents, type ChatMenuTarget } from "@/features/home";
import { isSettingsSection, SidebarSettings } from "@/features/settings";
import { LibraryCreateSheet } from "@/features/library";
import { OpenedFiles } from "@/features/opened-files";
import { CreateSheet } from "./CreateSheet";
import { useSidebarExpanded } from "./sidebarState";
import { destinations, destinationOf, type Destination } from "./navigation";
import { useAndroidBack } from "./useAndroidBack";
import { useWindowController } from "./useWindowController";

const resizeEdges: readonly [WindowEdge, string][] = [
  ["North", "inset-x-2 top-0 h-1 cursor-n-resize"],
  ["South", "inset-x-2 bottom-0 h-1 cursor-s-resize"],
  ["East", "inset-y-2 right-0 w-1 cursor-e-resize"],
  ["West", "inset-y-2 left-0 w-1 cursor-w-resize"],
  ["NorthWest", "left-0 top-0 h-2 w-2 cursor-nw-resize"],
  ["NorthEast", "right-0 top-0 h-2 w-2 cursor-ne-resize"],
  ["SouthWest", "left-0 bottom-0 h-2 w-2 cursor-sw-resize"],
  ["SouthEast", "right-0 bottom-0 h-2 w-2 cursor-se-resize"],
];

export function AppShell() {
  const { t } = useTranslation();
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const { data: status } = useQuery(appStatusQuery(api));
  const pathname = useLocation({ select: (location) => location.pathname });
  const navigate = useNavigate();
  const compact = useCompactLayout();
  const platform = status?.platform ?? "other";
  const mobile = platform === "android" || platform === "ios";
  const desktop = !mobile;
  const windowPlatform = platform === "macos" || platform === "windows" ? platform : "linux";
  const { controller, expanded, failed, perform } = useWindowController(platform);
  const welcome = pathname === "/welcome" || pathname === "/welcome-back";
  const tabs = compact || mobile;
  const { enabled: sidebarOnDetailPages } = useSidebarOnDetailPages(api);
  const mainScreen = Object.entries(destinations).some(([id, path]) => id !== "settings" && pathname === path);
  const navigationHidden = !mainScreen && (tabs || !sidebarOnDetailPages);
  useAndroidBack(platform, welcome || mainScreen);
  const [sidebarOpen, setSidebarOpen] = useSidebarExpanded(api);
  const [createOpen, setCreateOpen] = useState(false);
  const chats = useChatNavigation();
  const [chatMenu, setChatMenu] = useState<ChatMenuTarget | null>(null);
  const first: NavDestination<Destination> = tabs
    ? { id: "chats", label: t("navigation.chats"), icon: <MessageSquare size={icon.lg} /> }
    : { id: "chats", label: t("navigation.home"), icon: <House size={icon.lg} /> };
  const items: NavDestination<Destination>[] = [
    first,
    { id: "groups", label: t("navigation.groups"), icon: <Users size={icon.lg} /> },
    { id: "discover", label: t("navigation.discover"), icon: <Compass size={icon.lg} /> },
    { id: "library", label: t("navigation.library"), icon: <Library size={icon.lg} /> },
  ];
  const settings: NavDestination<Destination> = { id: "settings", label: t("navigation.settings"), icon: <Settings size={icon.lg} /> };
  const inSettings = pathname === "/settings" || pathname.startsWith("/settings/");
  const settingsMode = inSettings && !tabs && sidebarOnDetailPages;
  const backToApp: NavDestination<Destination> = { id: "chats", label: t("navigation.backToApp"), icon: <ArrowLeft size={icon.lg} /> };
  const section = pathname.startsWith("/settings/") ? pathname.slice("/settings/".length).split("/")[0] ?? "" : "";
  useEffect(() => {
    if (settingsMode && pathname === "/settings") void navigate({ to: "/settings/$section", params: { section: "about" }, replace: true });
  }, [settingsMode, pathname, navigate]);
  const titleBar = desktop ? (
    <TitleBar
      title={t("app.name")}
      platform={windowPlatform}
      className="relative [&>div:first-child]:pointer-events-none"
      {...(controller ? {
        onMinimize: () => perform(() => controller.minimize()),
        onMaximize: () => perform(() => controller.toggleMaximize()),
        onClose: () => perform(() => controller.close()),
      } : {})}
    />
  ) : null;
  return (
    <CrispStringsProvider strings={{ navPrimary: t("navigation.primary"), navCreate: t("navigation.create"), navCollapse: t("navigation.collapse"), navCollapseSidebar: t("navigation.collapseSidebar"), navExpandSidebar: t("navigation.expandSidebar"), titleBarMinimize: t("window.minimize"), titleBarMaximize: t("window.maximize"), titleBarClose: t("window.close") }}>
    <div className={cn("relative flex h-full min-h-0 flex-col overflow-clip bg-surface text-fg", desktop && !expanded && "rounded-2xl ring-1 ring-inset ring-line")}>
      {failed && <p role="alert" className="shrink-0 bg-danger/10 px-4 py-2 text-sm text-danger">{t("window.failed")}</p>}
      {welcome ? (
        <>
          {titleBar}
          <div className="@container relative min-h-0 flex-1 overflow-clip"><Outlet /></div>
        </>
      ) : (
        <CrispAppShell
          className={cn("overflow-clip", !tabs && !sidebarOpen && "[&_nav_.scrollbar-thin]:[scrollbar-width:none] [&_nav_.scrollbar-thin::-webkit-scrollbar]:hidden")}
          items={settingsMode ? [backToApp] : items}
          {...(tabs ? {} : { footerItems: [settings] })}
          value={destinationOf(pathname)}
          onChange={(id) => { void navigate({ to: id === "settings" && settingsMode ? "/" : destinations[id] }); }}
          {...(tabs && pathname === "/library" ? { onCreate: () => setCreateOpen(true), createLabel: t("navigation.create") } : {})}
          navStyle={navigationHidden ? "header" : tabs ? "tabs" : sidebarOpen ? "sidebar" : "rail"}
          {...(tabs ? {} : {
            onNavToggle: () => setSidebarOpen(!sidebarOpen),
            ...(settingsMode ? {
              navSection: <SidebarSettings active={isSettingsSection(section) ? section : null} onOpen={(id) => { void navigate({ to: "/settings/$section", params: { section: id } }); }} />,
            } : {
              onCreate: () => setCreateOpen(true),
              createLabel: t("navigation.create"),
              navSection: <SidebarRecents api={api} onOpen={(chat) => chats.openConversation(chat.id)} heldId={chatMenu?.chat.id ?? null} onMenu={setChatMenu} />,
            }),
          })}
          titleBar={titleBar}
        >
          <Outlet />
        </CrispAppShell>
      )}
      {pathname === "/library" ? <LibraryCreateSheet api={api} open={createOpen} onClose={() => setCreateOpen(false)} /> : <CreateSheet api={api} open={createOpen} onClose={() => setCreateOpen(false)} onCreate={(kind) => { void navigate({ to: kind === "persona" ? "/personas/new" : "/characters/new" }); }} onDiscover={() => { void navigate({ to: destinations.discover }); }} />}
      <ChatMenu api={api} target={chatMenu} onClose={() => setChatMenu(null)} />
      <OpenedFiles api={api} held={welcome} />
      {controller && desktop && windowPlatform !== "macos" && !expanded && resizeEdges.map(([edge, className]) => (
        <div key={edge} aria-hidden className={cn("absolute z-50 touch-none", className)} onPointerDown={(event) => {
          if (event.button !== 0) return;
          event.preventDefault();
          perform(() => controller.resize(edge));
        }} />
      ))}
    </div>
    </CrispStringsProvider>
  );
}
