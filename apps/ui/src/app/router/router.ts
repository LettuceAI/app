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
import { loadOnboardingRoute } from "@/features/onboarding";
import { loadWelcomeBackRoute } from "@/features/welcome-back";
import { RouteErrorState } from "../error-boundaries/RouteErrorState";
import { RouteNotFound } from "../error-boundaries/RouteNotFound";
import { AppShell } from "../shell/AppShell";
import { PendingScreen } from "../shell/PendingScreen";
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

const statusRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
}).lazy(() => import("../shell/status-route.lazy").then((module) => module.Route));

const welcomeRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: welcomePath,
}).lazy(loadOnboardingRoute);

const welcomeBackRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: welcomeBackPath,
}).lazy(loadWelcomeBackRoute);

const groupsRoute = createRoute({ getParentRoute: () => rootRoute, path: "/groups" })
  .lazy(() => import("../shell/navigation-route.lazy").then((module) => module.groupsRoute));
const discoverRoute = createRoute({ getParentRoute: () => rootRoute, path: "/discover" })
  .lazy(() => import("../shell/navigation-route.lazy").then((module) => module.discoverRoute));
const libraryRoute = createRoute({ getParentRoute: () => rootRoute, path: "/library" })
  .lazy(() => import("../shell/navigation-route.lazy").then((module) => module.libraryRoute));
const settingsRoute = createRoute({ getParentRoute: () => rootRoute, path: "/settings" })
  .lazy(() => import("../shell/navigation-route.lazy").then((module) => module.settingsRoute));

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
    defaultPreload: "intent",
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
