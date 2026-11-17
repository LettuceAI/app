import { useLocation, useNavigate, useRouter, useSearch } from "@tanstack/react-router";

export interface AvatarPanelRoute {
  open: boolean;
  value: true | "banner" | undefined;
  routeKey: string | undefined;
  show: (value?: true | "banner") => void;
  hide: () => void;
}

/** Whether a navigation stays on the same page, as opening or closing the avatar panel does, so a page's leave guard can let it through. */
export function staysOnPage(current: { pathname: string }, next: { pathname: string }): boolean {
  return current.pathname === next.pathname;
}

/** The avatar panel as a search param on the owner's route: the form stays mounted underneath, and back closes the panel first. */
export function useAvatarPanelRoute(parameter: "avatarPanel" | "avatarCrop" | "avatarLibrary" = "avatarPanel"): AvatarPanelRoute {
  const navigate = useNavigate();
  const router = useRouter();
  const value = useSearch({ strict: false, select: (search) => search[parameter] });
  const open = value === true || (parameter === "avatarLibrary" && value === "banner");
  const pushedKey = `${parameter}Pushed` as const;
  const pushed = useLocation({ select: (location) => location.state[pushedKey] === true });
  const routeKey = useLocation({ select: (location) => location.state.__TSR_key });
  const show = (value: true | "banner" = true) => {
    if (open) return;
    void navigate({ to: ".", search: (previous: Record<string, unknown>) => ({ ...previous, [parameter]: value }), state: (previous: Record<string, unknown>) => ({ ...previous, [pushedKey]: true }), resetScroll: false } as never);
  };
  const hide = () => {
    if (!open || router.state.location.state.__TSR_key !== routeKey) return;
    if (pushed && router.history.canGoBack()) {
      router.history.back();
      return;
    }
    void navigate({ to: ".", search: (previous: Record<string, unknown>) => Object.fromEntries(Object.entries(previous).filter(([key]) => key !== parameter)), replace: true, resetScroll: false } as never);
  };
  return { open, value, routeKey, show, hide };
}
