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
import { CreatePersonaPage, EditPersonaPage } from "@/features/personas";
import {
  ChatBranchesPage,
  ChatHistoryPage,
  ChatMemoryPage,
  ChatCompanionPage,
  ConversationPage,
} from "@/features/conversations";
import {
  LibraryPage,
  NewLorebookPage,
  LorebookProjectPage,
  LorebookEditorPage,
  LorebookPreviewPage,
  validateLibrarySearch,
} from "@/features/library";
import { HomePage } from "@/features/home";
import { OnboardingPage } from "@/features/onboarding";
import {
  builtSettingsSections,
  isSettingsSection,
  ModelEditorPage,
  ModelBrowserPage,
  type BrowserSearch,
  SettingsIndexPage,
  SettingsSectionPage,
} from "@/features/settings";
import { WelcomeBackPage } from "@/features/welcome-back";
import { RouteErrorState } from "../error-boundaries/RouteErrorState";
import { RouteNotFound } from "../error-boundaries/RouteNotFound";
import { AppShell } from "../shell/AppShell";
import { NavigationPage } from "../shell/NavigationPage";
import { PendingScreen } from "../shell/PendingScreen";
import {
  legacyMigrationOffered,
  onboardingCompleted,
  onboardingRedirect,
  welcomeBackPath,
  welcomePath,
} from "./onboarding-gate";

export interface RouterContext {
  api: ApiClient;
  queryClient: QueryClient;
}

const rootRoute = createRootRouteWithContext<RouterContext>()({
  validateSearch: (search: Record<string, unknown>): { settingsPanel?: "whatsNew" } => ({
    ...(search.settingsPanel === "whatsNew" ? { settingsPanel: "whatsNew" as const } : {}),
  }),
  beforeLoad: ({ context, location }) => {
    const query = appStatusQuery(context.api);
    const checkGate = (status: AppStatus) => {
      const setupReplay =
        onboardingCompleted(status) &&
        new URLSearchParams(location.searchStr).get("setup") === "true";
      const target = onboardingRedirect(
        { completed: onboardingCompleted(status), legacy: legacyMigrationOffered(status) },
        location.pathname,
        setupReplay,
      );
      if (target) throw redirect({ to: target, replace: true });
      return { setupReplay: setupReplay && location.pathname === welcomePath };
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
const welcomeRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: welcomePath,
  validateSearch: (search: Record<string, unknown>): { setup?: true } => ({
    ...(search.setup === true || search.setup === "true" ? { setup: true as const } : {}),
  }),
  component: OnboardingPage,
});
const welcomeBackRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: welcomeBackPath,
  component: WelcomeBackPage,
});
const groupsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/groups",
  component: NavigationPage,
});
const discoverRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/discover",
  component: NavigationPage,
});
const libraryRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/library",
  component: LibraryPage,
  validateSearch: validateLibrarySearch,
  beforeLoad: ({ search }) => {
    if (search.create === "character") throw redirect({ to: "/characters/new", replace: true });
    if (search.create === "persona") throw redirect({ to: "/personas/new", replace: true });
    if (search.create === "lorebook")
      throw redirect({ to: "/library/lorebooks/new", replace: true });
  },
});
const newLorebookRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/library/lorebooks/new",
  component: NewLorebookPage,
  validateSearch: (
    search: Record<string, unknown>,
  ): { panel?: "character" | "persona" | "paste"; source?: string } => ({
    ...(["character", "persona", "paste"].includes(String(search.panel))
      ? { panel: search.panel as "character" | "persona" | "paste" }
      : {}),
    ...(typeof search.source === "string" ? { source: search.source } : {}),
  }),
});
const lorebookEditorRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/library/lorebooks/$lorebookId",
  component: LorebookEditorPage,
});
const lorebookPreviewRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/library/lorebooks/$lorebookId/preview",
  component: LorebookPreviewPage,
  validateSearch: (
    search: Record<string, unknown>,
  ): { mode?: "conversation" | "editor"; chat?: string } => ({
    ...(search.mode === "editor" || search.mode === "conversation" ? { mode: search.mode } : {}),
    ...(typeof search.chat === "string" ? { chat: search.chat } : {}),
  }),
});
const lorebookProjectRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/library/lorebooks/projects/$projectId",
  component: LorebookProjectPage,
  validateSearch: (
    search: Record<string, unknown>,
  ): {
    step?: "outline" | "entries" | "save";
    panel?: "plan" | "draft" | "refine";
    entry?: string;
  } => ({
    ...(["outline", "entries", "save"].includes(String(search.step))
      ? { step: search.step as "outline" | "entries" | "save" }
      : {}),
    ...(["plan", "draft", "refine"].includes(String(search.panel))
      ? { panel: search.panel as "plan" | "draft" | "refine" }
      : {}),
    ...(typeof search.entry === "string" ? { entry: search.entry } : {}),
  }),
});
const newCharacterRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/characters/new",
  component: CreateCharacterPage,
});
const editCharacterRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/characters/$characterId/edit",
  component: EditCharacterPage,
  validateSearch: (search: Record<string, unknown>): { tab?: "soul" } => search.tab === "soul" ? { tab: "soul" } : {},
});
const characterHistoryRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/characters/$characterId/history",
  component: ChatHistoryPage,
});
const newPersonaRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/personas/new",
  component: CreatePersonaPage,
});
const editPersonaRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/personas/$personaId/edit",
  component: EditPersonaPage,
});
const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings",
  component: SettingsIndexPage,
});
const settingsSectionRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings/$section",
  validateSearch: (
    search: Record<string, unknown>,
  ): {
    imagePicker?: "avatar" | "scene" | "writer";
    localImageView?: "engine" | "models" | "bundle" | "loras";
    engineInstall?: boolean;
    civitaiModel?: number;
    prompt?: string;
    promptSource?: string;
    replyPicker?: "model" | "roleplay" | "conversational";
    speechTab?: "model" | "vocabulary" | "corrections" | "examples";
    speechPanel?: "models" | "microphone" | "word" | "correction" | "example";
    speechItem?: string;
    logFile?: string;
    logPanel?:
      | "line"
      | "filters"
      | "parts"
      | "time"
      | "more"
      | "report"
      | "delete-day"
      | "delete-all";
    logLine?: number;
    loreTab?: "scanning" | "lorebook" | "entry";
    lorePicker?: string;
    usageTab?: "overview" | "requests" | "time" | "nanogpt";
    usageRange?: "today" | "7d" | "30d" | "all";
    usageProvider?: string;
    usageStatus?: string;
    usageOperation?: string;
    usageModel?: string;
    usagePanel?: "request" | "clear";
    usageRequest?: string;
    usageAccount?: string;
    compTab?: "writer" | "emotions";
    compPicker?: "primary" | "fallback" | "prompt";
    custTab?: "general" | "theme" | "chat";
    custPanel?: "preview" | "theme";
    custTheme?: string;
  } => ({
    ...(typeof search.logFile === "string" ? { logFile: search.logFile } : {}),
    ...(["line", "filters", "parts", "time", "more", "report", "delete-day", "delete-all"].includes(
      String(search.logPanel),
    )
      ? {
          logPanel: search.logPanel as
            | "line"
            | "filters"
            | "parts"
            | "time"
            | "more"
            | "report"
            | "delete-day"
            | "delete-all",
        }
      : {}),
    ...(typeof search.logLine === "number" &&
    Number.isSafeInteger(search.logLine) &&
    search.logLine >= 0
      ? { logLine: search.logLine }
      : {}),
    ...(["scanning", "lorebook", "entry"].includes(String(search.loreTab))
      ? { loreTab: search.loreTab as "scanning" | "lorebook" | "entry" }
      : {}),
    ...(typeof search.lorePicker === "string" ? { lorePicker: search.lorePicker } : {}),
    ...(["overview", "requests", "time", "nanogpt"].includes(String(search.usageTab))
      ? { usageTab: search.usageTab as "overview" | "requests" | "time" | "nanogpt" }
      : {}),
    ...(["today", "7d", "30d", "all"].includes(String(search.usageRange))
      ? { usageRange: search.usageRange as "today" | "7d" | "30d" | "all" }
      : {}),
    ...(["all", "pending", "succeeded", "failed", "cancelled", "interrupted"].includes(
      String(search.usageStatus),
    )
      ? { usageStatus: search.usageStatus as string }
      : {}),
    ...(typeof search.usageProvider === "string" ? { usageProvider: search.usageProvider } : {}),
    ...(typeof search.usageOperation === "string" ? { usageOperation: search.usageOperation } : {}),
    ...(typeof search.usageModel === "string" ? { usageModel: search.usageModel } : {}),
    ...(typeof search.usageAccount === "string" ? { usageAccount: search.usageAccount } : {}),
    ...(search.usagePanel === "clear" ||
    (search.usagePanel === "request" &&
      typeof search.usageRequest === "string" &&
      search.usageRequest)
      ? { usagePanel: search.usagePanel }
      : {}),
    ...(search.usagePanel === "request" &&
    typeof search.usageRequest === "string" &&
    search.usageRequest
      ? { usageRequest: search.usageRequest }
      : {}),
    ...(search.compTab === "writer" || search.compTab === "emotions"
      ? { compTab: search.compTab }
      : {}),
    ...(["primary", "fallback", "prompt"].includes(String(search.compPicker))
      ? { compPicker: search.compPicker as "primary" | "fallback" | "prompt" }
      : {}),
    ...(["general", "theme", "chat"].includes(String(search.custTab))
      ? { custTab: search.custTab as "general" | "theme" | "chat" }
      : {}),
    ...(search.custPanel === "preview" || search.custPanel === "theme"
      ? { custPanel: search.custPanel }
      : {}),
    ...(typeof search.custTheme === "string" ? { custTheme: search.custTheme } : {}),
    ...(["model", "vocabulary", "corrections", "examples"].includes(String(search.speechTab))
      ? { speechTab: search.speechTab as "model" | "vocabulary" | "corrections" | "examples" }
      : {}),
    ...(["models", "microphone", "word", "correction", "example"].includes(
      String(search.speechPanel),
    )
      ? {
          speechPanel: search.speechPanel as
            | "models"
            | "microphone"
            | "word"
            | "correction"
            | "example",
        }
      : {}),
    ...(typeof search.speechItem === "string" && search.speechItem
      ? { speechItem: search.speechItem }
      : {}),
    ...(search.replyPicker === "model" ||
    search.replyPicker === "roleplay" ||
    search.replyPicker === "conversational"
      ? { replyPicker: search.replyPicker }
      : {}),
    ...(typeof search.prompt === "string" && search.prompt ? { prompt: search.prompt } : {}),
    ...(search.prompt === "new" && typeof search.promptSource === "string" && search.promptSource
      ? { promptSource: search.promptSource }
      : {}),
    ...(search.imagePicker === "avatar" ||
    search.imagePicker === "scene" ||
    search.imagePicker === "writer"
      ? { imagePicker: search.imagePicker }
      : {}),
    ...(search.localImageView === "engine" ||
    search.localImageView === "models" ||
    search.localImageView === "bundle" ||
    search.localImageView === "loras"
      ? { localImageView: search.localImageView }
      : {}),
    ...(search.localImageView === "engine" && search.engineInstall === true
      ? { engineInstall: true }
      : {}),
    ...(search.localImageView === "loras" &&
    typeof search.civitaiModel === "number" &&
    Number.isSafeInteger(search.civitaiModel) &&
    search.civitaiModel > 0
      ? { civitaiModel: search.civitaiModel }
      : {}),
  }),
  beforeLoad: ({ params }) => {
    if (!isSettingsSection(params.section) || !builtSettingsSections.has(params.section))
      throw notFound();
  },
  component: SettingsSectionPage,
});

const chatRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/chat/$conversationId",
  validateSearch: (search: Record<string, unknown>): { panel?: "settings"; message?: string } => ({
    ...(search.panel === "settings" ? { panel: "settings" as const } : {}),
    ...(typeof search.message === "string" && search.message !== ""
      ? { message: search.message }
      : {}),
  }),
  component: ConversationPage,
});
const chatSettingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/chat/$conversationId/settings",
  beforeLoad: ({ params }) => {
    throw redirect({
      to: "/chat/$conversationId",
      params: { conversationId: params.conversationId },
      search: { panel: "settings" },
      replace: true,
    });
  },
});
const chatMemoryRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/chat/$conversationId/memory",
  component: ChatMemoryPage,
});
const chatCompanionRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/chat/$conversationId/companion",
  validateSearch: (search: Record<string, unknown>): { tab?: "relationship" | "memories" | "soul" | "activity" } =>
    search.tab === "relationship" || search.tab === "memories" || search.tab === "soul" || search.tab === "activity" ? { tab: search.tab } : { tab: "relationship" },
  component: ChatCompanionPage,
});
const chatBranchesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/chat/$conversationId/branches",
  component: ChatBranchesPage,
});

const modelBrowserRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings/model-browser",
  component: ModelBrowserPage,
  validateSearch: (input: Record<string, unknown>): BrowserSearch => ({
    ...(input.token === true ? { token: true } : {}),
    ...(input.kind === "image" || input.kind === "text" ? { kind: input.kind } : {}),
    ...(input.view === "downloaded" ? { view: input.view } : {}),
    ...(typeof input.repo === "string" ? { repo: input.repo } : {}),
    ...(typeof input.author === "string" ? { author: input.author } : {}),
    ...(typeof input.destination === "string" ? { destination: input.destination } : {}),
    ...(input.tab === "info" || input.tab === "download" ? { tab: input.tab } : {}),
    ...(typeof input.profile === "string" ? { profile: input.profile } : {}),
    ...(typeof input.role === "string" ? { role: input.role } : {}),
    ...(input.imageStep === "search" || input.imageStep === "repo" || input.imageStep === "review"
      ? { imageStep: input.imageStep }
      : {}),
  }),
});

const newModelRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings/models/new",
  component: ModelEditorPage,
});
const editModelRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings/models/$modelId",
  component: ModelEditorPage,
});

const routeTree = rootRoute.addChildren([
  homeRoute,
  welcomeRoute,
  welcomeBackRoute,
  groupsRoute,
  discoverRoute,
  libraryRoute,
  newLorebookRoute,
  lorebookProjectRoute,
  lorebookEditorRoute,
  lorebookPreviewRoute,
  newCharacterRoute,
  editCharacterRoute,
  characterHistoryRoute,
  newPersonaRoute,
  editPersonaRoute,
  settingsRoute,
  settingsSectionRoute,
  modelBrowserRoute,
  newModelRoute,
  editModelRoute,
  chatRoute,
  chatSettingsRoute,
  chatMemoryRoute,
  chatCompanionRoute,
  chatBranchesRoute,
]);

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
    hfBrowserPushed?: boolean | undefined;
    settingsPanelPushed?: boolean;
    setupReplayPushed?: boolean;
    speechPanelPushed?: boolean;
    replyPickerPushed?: boolean;
    customizationPushed?: boolean;
    usagePushed?: boolean;
    logPanelPushed?: boolean;
    lorebookPanelPushed?: boolean;
    lorebookFlowPushed?: boolean;
    companionPickerPushed?: boolean;
    promptPushed?: boolean;
    panelPushed?: boolean;
    libraryOverlayPushed?: boolean;
    imagePickerPushed?: boolean;
    localImagesPushed?: boolean;
    engineInstallPushed?: boolean;
    loraDetailPushed?: boolean;
    libraryKind?: "character" | "persona" | "lorebook";
  }
}

declare module "@tanstack/react-router" {
  interface Register {
    router: AppRouter;
  }
}
