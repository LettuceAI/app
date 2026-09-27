import { QueryClient } from "@tanstack/react-query";

/**
 * Cached data stays fresh until a mutation or a backend event (see `AppEventBridge`) invalidates it;
 * nothing refetches on timers or focus changes, and a typed failure is shown instead of being retried
 * blindly.
 */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: Infinity,
        retry: false,
        refetchOnWindowFocus: false,
        refetchOnReconnect: false,
      },
      mutations: {
        retry: false,
      },
    },
  });
}
