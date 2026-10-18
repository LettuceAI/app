import {
  createRootRouteWithContext,
  createRoute,
  createRouter,
  notFound,
  redirect,
  type RouterHistory,
} from "@tanstack/react-router";
import type { QueryClient } from "@tanstack/react-query";
import { appStatusQuery } from "@/api/app-status-query";
import type { AppStatus } from "@/api/generated/bindings";
import type { ApiClient } from "@/api/client";
import { CreateCharacterPage, EditCharacterPage } from "@/features/characters";
import { CreatePersonaPage } from "@/features/personas";
import { ChatBranchesPage, ChatMemoryPage, ConversationPage } from "@/features/conversations";
import { LibraryPage, validateLibrarySearch } from "@/features/library";
import { HomePage } from "@/features/home";
import { OnboardingPage } from "@/features/onboarding";
import { builtSettingsSections, isSettingsSection, ModelEditorPage, SettingsIndexPage, SettingsSectionPage } from "@/features/settings";
import { WelcomeBackPage } from "@/features/welcome-back";
import { RouteErrorState } from "../error-boundaries/RouteErrorState";
import { RouteNotFound } from "../error-boundaries/RouteNotFound";
import { AppShell } from "../shell/AppShell";
import { NavigationPage } from "../shell/NavigationPage";
import { PendingScreen } from "../shell/PendingScreen";
import { legacyMigrationOffered, onboardingCompleted, onboardingRedirect, welcomeBackPath, welcomePath } from "./onboarding-gate";

export interface RouterContext {
  api: ApiClient;
  queryClient: QueryClient;
}

const rootRoute = createRootRouteWithContext<RouterContext>()({
  beforeLoad: ({ context, location }) => {
    const query = appStatusQuery(context.api);
    const checkGate = (status: AppStatus) => {
      const target = onboardingRedirect({ completed: onboardingCompleted(status), legacy: legacyMigrationOffered(status) }, location.pathname);
      if (target) throw redirect({ to: target, replace: true });
    };
    const cached = context.queryClient.getQueryData(query.queryKey);
    if (cached) {
      // Keep navigation synchronous once connected. Refresh invalidated status in
      // the background, just as ensureQueryData's revalidateIfStale did before.
      void context.queryClient.prefetchQuery(query);
      return checkGate(cached);
    }
    return context.queryClient.ensureQueryData(query).then(checkGate);
  },
  component: AppShell,
});

const homeRoute = createRoute({ getParentRoute: () => rootRoute, path: "/", component: HomePage });
const welcomeRoute = createRoute({ getParentRoute: () => rootRoute, path: welcomePath, component: OnboardingPage });
const welcomeBackRoute = createRoute({ getParentRoute: () => rootRoute, path: welcomeBackPath, component: WelcomeBackPage });
const groupsRoute = createRoute({ getParentRoute: () => rootRoute, path: "/groups", component: NavigationPage });
const discoverRoute = createRoute({ getParentRoute: () => rootRoute, path: "/discover", component: NavigationPage });
const libraryRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/library",
  component: LibraryPage,
  validateSearch: validateLibrarySearch,
  beforeLoad: ({ search }) => {
    if (search.create === "character") throw redirect({ to: "/characters/new", replace: true });
    if (search.create === "persona") throw redirect({ to: "/personas/new", replace: true });
  },
});
const newCharacterRoute = createRoute({ getParentRoute: () => rootRoute, path: "/characters/new", component: CreateCharacterPage });
const editCharacterRoute = createRoute({ getParentRoute: () => rootRoute, path: "/characters/$characterId/edit", component: EditCharacterPage });
const newPersonaRoute = createRoute({ getParentRoute: () => rootRoute, path: "/personas/new", component: CreatePersonaPage });
const settingsRoute = createRoute({ getParentRoute: () => rootRoute, path: "/settings", component: SettingsIndexPage });
const settingsSectionRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings/$section",
  beforeLoad: ({ params }) => {
    if (!isSettingsSection(params.section) || !builtSettingsSections.has(params.section)) throw notFound();
  },
  component: SettingsSectionPage,
});

const chatRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/chat/$conversationId",
  validateSearch: (search: Record<string, unknown>): { panel?: "settings" } => (search.panel === "settings" ? { panel: "settings" } : {}),
  component: ConversationPage,
});
const chatSettingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/chat/$conversationId/settings",
  beforeLoad: ({ params }) => {
    throw redirect({ to: "/chat/$conversationId", params: { conversationId: params.conversationId }, search: { panel: "settings" }, replace: true });
  },
});
const chatMemoryRoute = createRoute({ getParentRoute: () => rootRoute, path: "/chat/$conversationId/memory", component: ChatMemoryPage });
const chatBranchesRoute = createRoute({ getParentRoute: () => rootRoute, path: "/chat/$conversationId/branches", component: ChatBranchesPage });

const newModelRoute = createRoute({ getParentRoute: () => rootRoute, path: "/settings/models/new", component: ModelEditorPage });
const editModelRoute = createRoute({ getParentRoute: () => rootRoute, path: "/settings/models/$modelId", component: ModelEditorPage });

const routeTree = rootRoute.addChildren([homeRoute, welcomeRoute, welcomeBackRoute, groupsRoute, discoverRoute, libraryRoute, newCharacterRoute, editCharacterRoute, newPersonaRoute, settingsRoute, settingsSectionRoute, newModelRoute, editModelRoute, chatRoute, chatSettingsRoute, chatMemoryRoute, chatBranchesRoute]);

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
    defaultPendingMs: 150,
    defaultPendingMinMs: 0,
    ...(options.history ? { history: options.history } : {}),
  });
}

export type AppRouter = ReturnType<typeof createAppRouter>;

declare module "@tanstack/history" {
  interface HistoryState {
    panelPushed?: boolean;
    libraryOverlayPushed?: boolean;
    libraryKind?: "character" | "persona" | "lorebook";
  }
}

declare module "@tanstack/react-router" {
  interface Register {
    router: AppRouter;
  }
}
