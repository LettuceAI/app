import type { AppEvent } from "./generated/bindings";

export const queryKeys = {
  app: {
    all: ["app"] as const,
    status: () => [...queryKeys.app.all, "status"] as const,
  },
  conversations: {
    all: ["conversations"] as const,
    lists: () => [...queryKeys.conversations.all, "list"] as const,
    detail: (conversationId: string) => [...queryKeys.conversations.all, "detail", conversationId] as const,
    messages: (conversationId: string) => [...queryKeys.conversations.detail(conversationId), "messages"] as const,
  },
  settings: { all: ["settings"] as const, filterLog: () => ["settings", "content-filter"] as const },
  jobs: {
    all: ["jobs"] as const,
    lists: () => [...queryKeys.jobs.all, "list"] as const,
    detail: (jobId: string) => [...queryKeys.jobs.all, "detail", jobId] as const,
  },
};

export type QueryKeyPrefix = readonly unknown[];

type AppEventOf<T extends AppEvent["type"]> = Extract<AppEvent, { type: T }>;

type AppEventInvalidations = {
  [T in AppEvent["type"]]?: (event: AppEventOf<T>) => readonly QueryKeyPrefix[];
};

/** Which cached queries an application event makes stale; each key invalidates everything under it. */
export const appEventInvalidations: AppEventInvalidations = {
  generation_settled: (event) => [queryKeys.conversations.lists(), queryKeys.conversations.detail(event.conversation_id)],
  settings_changed: (event) => event.section === "ui_state"
    ? [queryKeys.settings.all, queryKeys.app.status()]
    : [queryKeys.settings.all],
  content_filter_hit: () => [queryKeys.settings.filterLog()],
  job_updated: (event) => [queryKeys.jobs.lists(), queryKeys.jobs.detail(event.job.id)],
};

/** The keys to invalidate for `event`; an event type the table does not know invalidates nothing. */
export function invalidationsFor(event: AppEvent): readonly QueryKeyPrefix[] {
  const table = appEventInvalidations as Record<string, ((event: AppEvent) => readonly QueryKeyPrefix[]) | undefined>;
  const rule = Object.hasOwn(table, event.type) ? table[event.type] : undefined;
  return rule ? rule(event) : [];
}
