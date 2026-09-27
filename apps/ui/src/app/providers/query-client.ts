import { QueryClient } from "@tanstack/react-query";

/**
 * Server state is refreshed by backend events and mutations, never by timers or focus changes, and a
 * typed failure is shown instead of being retried blindly.
 */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: {
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
