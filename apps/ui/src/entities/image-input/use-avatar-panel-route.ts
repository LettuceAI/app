import { useLocation, useNavigate, useRouter, useSearch } from "@tanstack/react-router";

export interface AvatarPanelRoute {
  open: boolean;
  routeKey: string | undefined;
  show: () => void;
  hide: () => void;
}

/** Whether a navigation stays on the same page, as opening or closing the avatar panel does, so a page's leave guard can let it through. */
export function staysOnPage(current: { pathname: string }, next: { pathname: string }): boolean {
  return current.pathname === next.pathname;
}

/** The avatar panel as a search param on the owner's route: the form stays mounted underneath, and back closes the panel first. */
export function useAvatarPanelRoute(): AvatarPanelRoute {
  const navigate = useNavigate();
  const router = useRouter();
  const open = useSearch({ strict: false, select: (search) => (search as { avatarPanel?: unknown }).avatarPanel === true });
  const pushed = useLocation({ select: (location) => location.state.avatarPanelPushed === true });
  const routeKey = useLocation({ select: (location) => location.state.__TSR_key });
  const show = () => {
    if (open) return;
    void navigate({ to: ".", search: (previous: Record<string, unknown>) => ({ ...previous, avatarPanel: true }), state: (previous: Record<string, unknown>) => ({ ...previous, avatarPanelPushed: true }), resetScroll: false } as never);
  };
  const hide = () => {
    if (!open || router.state.location.state.__TSR_key !== routeKey) return;
    if (pushed && router.history.canGoBack()) {
      router.history.back();
      return;
    }
    void navigate({ to: ".", search: (previous: Record<string, unknown>) => Object.fromEntries(Object.entries(previous).filter(([key]) => key !== "avatarPanel")), replace: true, resetScroll: false } as never);
  };
  return { open, routeKey, show, hide };
}
