import { useRouteContext } from "@tanstack/react-router";
export function useApi() {
  return useRouteContext({ from: "__root__", select: (context) => context.api });
}
