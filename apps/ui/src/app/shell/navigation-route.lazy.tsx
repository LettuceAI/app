import { createLazyRoute } from "@tanstack/react-router";
import { NavigationPage } from "./NavigationPage";

export const groupsRoute = createLazyRoute("/groups")({ component: NavigationPage });
export const discoverRoute = createLazyRoute("/discover")({ component: NavigationPage });
export const libraryRoute = createLazyRoute("/library")({ component: NavigationPage });
export const settingsRoute = createLazyRoute("/settings")({ component: NavigationPage });
