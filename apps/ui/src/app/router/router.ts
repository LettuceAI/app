import {
  createRootRouteWithContext,
  createRoute,
  createRouter,
  redirect,
  type RouterHistory,
} from "@tanstack/react-router";
import type { QueryClient } from "@tanstack/react-query";
import { appStatusQuery } from "@/api/app-status-query";
import type { ApiClient } from "@/api/client";
import { OnboardingPage } from "@/features/onboarding";
import { WelcomeBackPage } from "@/features/welcome-back";
import { RouteErrorState } from "../error-boundaries/RouteErrorState";
import { RouteNotFound } from "../error-boundaries/RouteNotFound";
import { AppShell } from "../shell/AppShell";
import { NavigationPage } from "../shell/NavigationPage";
import { PendingScreen } from "../shell/PendingScreen";
import { StatusPage } from "../shell/StatusPage";
import { legacyMigrationOffered, onboardingCompleted, onboardingRedirect, welcomeBackPath, welcomePath } from "./onboarding-gate";

export interface RouterContext {
  api: ApiClient;
  queryClient: QueryClient;
}

const rootRoute = createRootRouteWithContext<RouterContext>()({
  beforeLoad: async ({ context, location }) => {
    const status = await context.queryClient.ensureQueryData({
      ...appStatusQuery(context.api),
      revalidateIfStale: true,
    });
    const target = onboardingRedirect({ completed: onboardingCompleted(status), legacy: legacyMigrationOffered(status) }, location.pathname);
    if (target) throw redirect({ to: target, replace: true });
  },
  component: AppShell,
});

const statusRoute = createRoute({ getParentRoute: () => rootRoute, path: "/", component: StatusPage });
const welcomeRoute = createRoute({ getParentRoute: () => rootRoute, path: welcomePath, component: OnboardingPage });
const welcomeBackRoute = createRoute({ getParentRoute: () => rootRoute, path: welcomeBackPath, component: WelcomeBackPage });
const groupsRoute = createRoute({ getParentRoute: () => rootRoute, path: "/groups", component: NavigationPage });
const discoverRoute = createRoute({ getParentRoute: () => rootRoute, path: "/discover", component: NavigationPage });
const libraryRoute = createRoute({ getParentRoute: () => rootRoute, path: "/library", component: NavigationPage });
const settingsRoute = createRoute({ getParentRoute: () => rootRoute, path: "/settings", component: NavigationPage });

const routeTree = rootRoute.addChildren([statusRoute, welcomeRoute, welcomeBackRoute, groupsRoute, discoverRoute, libraryRoute, settingsRoute]);

export interface AppRouterOptions {
  history?: RouterHistory;
}

export function createAppRouter(context: RouterContext, options: AppRouterOptions = {}) {
  return createRouter({
    routeTree,
    context,
    defaultErrorComponent: RouteErrorState,
    defaultNotFoundComponent: RouteNotFound,
    defaultPendingComponent: PendingScreen,
    defaultPendingMs: 0,
    ...(options.history ? { history: options.history } : {}),
  });
}

export type AppRouter = ReturnType<typeof createAppRouter>;

declare module "@tanstack/react-router" {
  interface Register {
    router: AppRouter;
  }
}
